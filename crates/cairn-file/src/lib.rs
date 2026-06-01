//! `cairn-file` — content-addressed `FileVersion` and source-class base.
//!
//! # Design invariants
//!
//! - **Content hash is the source of truth for freshness.** `mtime_observed` is
//!   recorded for diagnostics only and must never drive a freshness decision (see
//!   the Phase 1 foundation-risk premortem, risk 4).
//! - **Streaming hash.** File content is hashed via `blake3`'s streaming
//!   `Hasher::update` API — large files never load entirely into memory.
//! - **`file_id` is path-derived.** A `FileId` is deterministically derived from
//!   the worktree-relative path, making it stable across rehashes for the same
//!   logical file.

use std::borrow::Cow;
use std::ffi::OsStr;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

pub use cairn_types::{ContentHash, FileId, FileVersion, RepoEpochId, SourceClass, Timestamp};
use thiserror::Error;

/// Errors produced by file-version construction and source-class classification.
#[derive(Error, Debug)]
pub enum FileError {
    /// An I/O operation on the file (read, metadata, symlink resolution) failed.
    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// The file's mtime is before the Unix epoch — a system-clock anomaly.
    #[error("mtime before the Unix epoch on {path}")]
    InvalidMtime { path: PathBuf },

    /// A path that must be worktree-relative escaped or named a root.
    #[error("invalid worktree-relative path {path}: {reason}")]
    InvalidRelativePath { path: PathBuf, reason: &'static str },
}

// ---------------------------------------------------------------------------
// content hash (streaming blake3)
// ---------------------------------------------------------------------------

/// Stream the file at `path` through a `blake3::Hasher` and return the
/// lowercase-hex digest as a [`ContentHash`].
///
/// Symlinks are hashed as the bytes of the link target path returned by
/// `read_link`, not by opening the target. This keeps dangling symlinks
/// hashable and makes the hash describe the directory entry that was observed.
///
/// Reads in 64 KiB buffers — large files never load entirely into memory.
pub fn content_hash_path(path: &Path) -> Result<ContentHash, FileError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| FileError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    if metadata.file_type().is_symlink() {
        let target = read_symlink_target(path)?;
        return Ok(content_hash_path_bytes(&target));
    }

    hash_regular_file_contents(path)
}

fn hash_regular_file_contents(path: &Path) -> Result<ContentHash, FileError> {
    let file = std::fs::File::open(path).map_err(|e| FileError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut hasher = blake3::Hasher::new();
    let mut reader = io::BufReader::with_capacity(65_536, file);
    let mut buf = [0u8; 65_536];
    loop {
        let n = reader.read(&mut buf).map_err(|e| FileError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(ContentHash::from_hex(
        hasher.finalize().to_hex().to_string(),
    ))
}

/// Hash an in-memory byte slice with `blake3` and return the lowercase-hex
/// digest as a [`ContentHash`].
///
/// Convenience for small inputs (tests, synthetic data) where streaming adds no
/// benefit.
pub fn content_hash_bytes(data: &[u8]) -> ContentHash {
    ContentHash::from_hex(blake3::hash(data).to_hex().to_string())
}

fn content_hash_path_bytes(path: &Path) -> ContentHash {
    content_hash_bytes(os_str_bytes(path.as_os_str()).as_ref())
}

fn read_symlink_target(path: &Path) -> Result<PathBuf, FileError> {
    std::fs::read_link(path).map_err(|e| FileError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

#[cfg(unix)]
fn os_str_bytes(value: &OsStr) -> Cow<'_, [u8]> {
    use std::os::unix::ffi::OsStrExt;

    Cow::Borrowed(value.as_bytes())
}

#[cfg(windows)]
fn os_str_bytes(value: &OsStr) -> Cow<'_, [u8]> {
    use std::os::windows::ffi::OsStrExt;

    Cow::Owned(
        value
            .encode_wide()
            .flat_map(|unit| unit.to_le_bytes())
            .collect(),
    )
}

#[cfg(not(any(unix, windows)))]
fn os_str_bytes(value: &OsStr) -> Cow<'_, [u8]> {
    Cow::Borrowed(value.as_encoded_bytes())
}

// ---------------------------------------------------------------------------
// file_id derivation
// ---------------------------------------------------------------------------

/// Derive a [`FileId`] from the worktree-relative path.
///
/// # Rule
///
/// `FileId` is a domain-separated BLAKE3 digest of the validated
/// worktree-relative path's path-component bytes. This makes it:
/// - **Deterministic** — same relative path always produces the same `FileId`.
/// - **Stable** — survives rehashing as long as the file hasn't moved.
/// - **Byte-safe** — no lossy UTF-8 conversion and no separator substitution.
///
/// Absolute paths, root/prefix components, parent-directory traversal, current
/// directory components, and empty paths are rejected instead of being joined
/// against a worktree root.
pub fn file_id_from_path(relative_path: &Path) -> Result<FileId, FileError> {
    let identity_bytes = validated_relative_path_identity(relative_path)?;
    let digest = blake3::hash(&identity_bytes);

    Ok(FileId::new(format!("path-blake3:{}", digest.to_hex())))
}

fn validated_relative_path_identity(relative_path: &Path) -> Result<Vec<u8>, FileError> {
    let mut identity = b"cairn-file-id-v1\0".to_vec();
    let mut saw_normal_component = false;

    for component in relative_path.components() {
        match component {
            Component::Normal(part) => {
                saw_normal_component = true;
                append_path_component(&mut identity, part);
            }
            Component::CurDir => {
                return Err(invalid_relative_path(
                    relative_path,
                    "current-directory components are not allowed",
                ));
            }
            Component::ParentDir => {
                return Err(invalid_relative_path(
                    relative_path,
                    "parent-directory components are not allowed",
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(invalid_relative_path(
                    relative_path,
                    "absolute/rooted paths are not allowed",
                ));
            }
        }
    }

    if !saw_normal_component {
        return Err(invalid_relative_path(
            relative_path,
            "path must contain a file component",
        ));
    }

    Ok(identity)
}

fn append_path_component(identity: &mut Vec<u8>, part: &OsStr) {
    let component = os_str_bytes(part);

    identity.extend_from_slice(&(component.len() as u64).to_le_bytes());
    identity.extend_from_slice(component.as_ref());
}

fn invalid_relative_path(relative_path: &Path, reason: &'static str) -> FileError {
    FileError::InvalidRelativePath {
        path: relative_path.to_path_buf(),
        reason,
    }
}

// ---------------------------------------------------------------------------
// mtime observation (diagnostics only — no freshness decisions)
// ---------------------------------------------------------------------------

fn mtime_for(path: &Path) -> Result<Timestamp, FileError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| FileError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let system_time = metadata.modified().map_err(|e| FileError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let nanos = system_time
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| FileError::InvalidMtime {
            path: path.to_path_buf(),
        })?;

    Ok(Timestamp(nanos.as_nanos() as i64))
}

// ---------------------------------------------------------------------------
// source-class classification
// ---------------------------------------------------------------------------

/// Classify a file by its worktree-relative path.
///
/// # Heuristic rules (applied in priority order)
///
/// | Priority | Match                                         | Class           |
/// |----------|-----------------------------------------------|-----------------|
/// | 1        | Path component `node_modules` or `vendor`     | `Vendored`      |
/// | 2        | Path component `target`, `dist`, `build`      | `BuildArtifact` |
/// | 3        | Path component is `tests` or `test`           | `Test`          |
/// | 4        | Filename `*_test.*` or starts with `test_`    | `Test`          |
/// | 5        | Known lockfile basename or filename `*.lock`  | `Lockfile`      |
/// | 6        | Path component `migrations`                   | `Migration`     |
/// | 7        | Path component `fixtures`                     | `Fixture`       |
/// | 8        | Extension `.toml`, `.yaml`, `.yml`, `.json`   | `Config`        |
/// | 9        | Extension in known generated set              | `Generated`     |
/// | 10       | Extension in known source set                 | `Source`        |
/// | 11       | Default                                       | `Unknown`       |
///
/// `Generated` extensions: `.generated`, `.gen`, `.min.js`, `.min.css`,
/// `.map`, `.pyc`, `.pyo`, `.class`, `.o`, `.obj`, `.rlib`, `.d`.
///
/// `Source` extensions: Rust/TS/Python/JS/Go/C/C++/Java/Kotlin/Swift/Ruby/Zig
/// and related (`.rs`, `.ts`, `.tsx`, `.py`, `.js`, `.jsx`, `.go`, `.c`,
/// `.h`, `.cpp`, `.hpp`, `.cc`, `.java`, `.kt`, `.swift`, `.rb`, `.zig`,
/// `.html`, `.css`, `.scss`, `.less`, `.md`, `.txt`).
///
/// Unknown paths (no extension match) default to `Unknown`. These defaults are
/// designed to be overridden later by explicit user config (`cairn-config`).
pub fn classify(relative_path: &Path) -> SourceClass {
    // Collect path components for directory-matching rules.
    let components: Vec<&str> = relative_path
        .components()
        .filter_map(|c| {
            let s = c.as_os_str().to_str()?;
            if s == "/" { None } else { Some(s) }
        })
        .collect();

    let lower_components: Vec<String> = components.iter().map(|s| s.to_lowercase()).collect();

    // Priority 1: vendored directories
    if lower_components
        .iter()
        .any(|c| c == "node_modules" || c == "vendor")
    {
        return SourceClass::Vendored;
    }

    // Priority 2: build artifact directories
    if lower_components
        .iter()
        .any(|c| c == "target" || c == "dist" || c == "build")
    {
        return SourceClass::BuildArtifact;
    }

    // Priority 3: test directories
    if lower_components.iter().any(|c| c == "tests" || c == "test") {
        return SourceClass::Test;
    }

    // Priority 4: test file naming patterns
    if let Some(filename) = components.last()
        && is_test_filename(filename)
    {
        return SourceClass::Test;
    }

    // Priority 5: lockfiles
    if let Some(filename) = components.last()
        && is_lockfile_basename(filename)
    {
        return SourceClass::Lockfile;
    }

    // Priority 6: migrations directory
    if lower_components.iter().any(|c| c == "migrations") {
        return SourceClass::Migration;
    }

    // Priority 7: fixtures directory
    if lower_components.iter().any(|c| c == "fixtures") {
        return SourceClass::Fixture;
    }

    // Priority 8: config extensions
    if let Some(filename) = components.last() {
        let lower_name = filename.to_lowercase();
        if lower_name.ends_with(".toml")
            || lower_name.ends_with(".yaml")
            || lower_name.ends_with(".yml")
            || lower_name.ends_with(".json")
        {
            return SourceClass::Config;
        }
    }

    // Priority 9: generated extensions
    if let Some(filename) = components.last()
        && is_generated_extension(filename)
    {
        return SourceClass::Generated;
    }

    // Priority 10: source extensions
    if let Some(filename) = components.last()
        && is_source_extension(filename)
    {
        return SourceClass::Source;
    }

    // Priority 11: unknown
    SourceClass::Unknown
}

fn is_test_filename(filename: &str) -> bool {
    let lower_name = filename.to_lowercase();

    if let Some(stem) = filename.split('.').next() {
        let lower_stem = stem.to_lowercase();
        if lower_stem.ends_with("_test") || lower_stem.starts_with("test_") {
            return true;
        }
    }

    // Also match bare `*_test` without extension.
    lower_name.ends_with("_test") || lower_name.starts_with("test_")
}

fn is_lockfile_basename(filename: &str) -> bool {
    let lower_name = filename.to_lowercase();

    lower_name.ends_with(".lock")
        || matches!(
            lower_name.as_str(),
            "package-lock.json"
                | "npm-shrinkwrap.json"
                | "pnpm-lock.yaml"
                | "pnpm-lock.yml"
                | "yarn.lock"
                | "bun.lock"
                | "bun.lockb"
                | "cargo.lock"
                | "poetry.lock"
                | "pipfile.lock"
                | "uv.lock"
                | "gemfile.lock"
                | "composer.lock"
                | "go.sum"
                | "mix.lock"
                | "flake.lock"
                | "pubspec.lock"
        )
}

fn is_generated_extension(filename: &str) -> bool {
    let generated_exts = [
        ".generated",
        ".gen",
        ".min.js",
        ".min.css",
        ".map",
        ".pyc",
        ".pyo",
        ".class",
        ".o",
        ".obj",
        ".rlib",
        ".d",
    ];
    let lower = filename.to_lowercase();
    generated_exts.iter().any(|ext| lower.ends_with(ext))
}

fn is_source_extension(filename: &str) -> bool {
    let source_exts = [
        ".rs", ".ts", ".tsx", ".py", ".js", ".jsx", ".go", ".c", ".h", ".cpp", ".hpp", ".cc",
        ".java", ".kt", ".kts", ".swift", ".rb", ".zig", ".html", ".css", ".scss", ".less", ".md",
        ".txt", ".sql", ".sh", ".bash", ".zsh", ".fish", ".proto", ".graphql", ".gql", ".vue",
        ".svelte",
    ];
    let lower = filename.to_lowercase();
    source_exts.iter().any(|ext| lower.ends_with(ext))
}

// ---------------------------------------------------------------------------
// FileVersion construction
// ---------------------------------------------------------------------------

/// Build a [`FileVersion`] from a worktree-relative path.
///
/// `worktree_root` is the absolute root of the worktree — used to construct the
/// full filesystem path for reading. `repo_epoch_id` identifies the VCS epoch
/// this version was observed during.
///
/// # Fields captured
///
/// | Field             | Source                                              |
/// |-------------------|-----------------------------------------------------|
/// | `file_id`         | [`file_id_from_path`] of the relative path           |
/// | `path`            | The relative path (as given)                         |
/// | `content_hash`    | Streaming BLAKE3 of file content                     |
/// | `size`            | `metadata.len()`                                     |
/// | `mtime_observed`  | `metadata.modified()` → nanoseconds since epoch       |
/// | `executable_bit`  | Unix permission bit (mode & 0o111 != 0)              |
/// | `symlink_target`  | `Some(target)` if symlink, else `None`               |
/// | `repo_epoch_id`   | As passed in                                         |
/// | `source_class`    | [`classify`] of the relative path                    |
pub fn build_file_version(
    relative_path: &Path,
    worktree_root: &Path,
    repo_epoch_id: RepoEpochId,
) -> Result<FileVersion, FileError> {
    let file_id = file_id_from_path(relative_path)?;
    let absolute_path = worktree_root.join(relative_path);

    let symlink_metadata =
        std::fs::symlink_metadata(&absolute_path).map_err(|e| FileError::Io {
            path: absolute_path.clone(),
            source: e,
        })?;

    let is_symlink = symlink_metadata.file_type().is_symlink();
    let symlink_target = if is_symlink {
        Some(read_symlink_target(&absolute_path)?)
    } else {
        None
    };

    // Hash content. For symlinks, hash the symlink's own path bytes (its
    // "content" is the target path), not the resolved target's content.
    let content_hash = content_hash_path(&absolute_path)?;

    // For file size, use the symlink metadata (len of the symlink entry), same
    // as what we hashed.
    let size = symlink_metadata.len();

    let mtime_observed = mtime_for(&absolute_path)?;

    // Unix executable bit: any execute permission on the file.
    #[cfg(unix)]
    let executable_bit = {
        use std::os::unix::fs::PermissionsExt;
        symlink_metadata.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let executable_bit = false;

    let source_class = classify(relative_path);

    Ok(FileVersion {
        file_id,
        path: relative_path.to_path_buf(),
        content_hash,
        size,
        mtime_observed,
        executable_bit,
        symlink_target,
        repo_epoch_id,
        source_class,
    })
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn td() -> tempfile::TempDir {
        tempfile::tempdir().expect("failed to create temp dir")
    }

    // ---------------------------------------------------------------
    // content hash
    // ---------------------------------------------------------------

    #[test]
    fn identical_content_produces_identical_hash() {
        let a = content_hash_bytes(b"hello cairn");
        let b = content_hash_bytes(b"hello cairn");
        assert_eq!(a, b);
    }

    #[test]
    fn different_content_produces_different_hash() {
        let a = content_hash_bytes(b"hello cairn");
        let b = content_hash_bytes(b"goodbye cairn");
        assert_ne!(a, b);
    }

    #[test]
    fn empty_file_produces_valid_hash() {
        let h = content_hash_bytes(b"");
        assert!(!h.as_hex().is_empty());
        assert_eq!(h.as_hex().len(), 64);
    }

    #[test]
    fn streaming_and_bytes_yield_same_hash() {
        let dir = td();
        let path = dir.path().join("stream_test.txt");
        let data = b"some content for streaming comparison";
        std::fs::write(&path, data).unwrap();

        let stream_hash = content_hash_path(&path).unwrap();
        let bytes_hash = content_hash_bytes(data);
        assert_eq!(stream_hash, bytes_hash);
    }

    #[test]
    fn streaming_works_for_larger_file() {
        let dir = td();
        let path = dir.path().join("large.bin");
        // 2 MiB — exercises streaming buffer loop
        let data = {
            let mut v = Vec::with_capacity(2 * 1024 * 1024);
            let mut rng: u64 = 0x_dead_beef_cafe_babe;
            for _ in 0..(2 * 1024 * 1024 / 8) {
                v.extend_from_slice(&rng.to_le_bytes());
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            }
            v.resize(2 * 1024 * 1024, 0u8);
            v
        };
        std::fs::write(&path, &data).unwrap();

        let stream_hash = content_hash_path(&path).unwrap();
        let bytes_hash = content_hash_bytes(&data);
        assert_eq!(stream_hash, bytes_hash);
    }

    #[test]
    fn hash_of_nonexistent_file_is_io_error() {
        let result = content_hash_path(Path::new("/nonexistent/cairn_test_file"));
        assert!(result.is_err());
        match result {
            Err(FileError::Io { .. }) => {} // expected
            _ => panic!("expected Io error"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlink_hash_uses_target_path_bytes_not_target_content() {
        let dir = td();
        let target_path = dir.path().join("real_file.txt");
        let link_path = dir.path().join("link.txt");
        let link_target = Path::new("real_file.txt");

        std::fs::write(&target_path, b"original target content").unwrap();
        std::os::unix::fs::symlink(link_target, &link_path).unwrap();

        let original_hash = content_hash_path(&link_path).unwrap();
        std::fs::write(&target_path, b"changed target content").unwrap();
        let changed_target_hash = content_hash_path(&link_path).unwrap();

        assert_eq!(original_hash, content_hash_bytes(b"real_file.txt"));
        assert_eq!(original_hash, changed_target_hash);
        assert_ne!(
            original_hash,
            content_hash_bytes(b"original target content")
        );
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_hashes_without_following_target() {
        let dir = td();
        let link_path = dir.path().join("dangling.txt");
        let missing_target = Path::new("missing-target.txt");

        std::os::unix::fs::symlink(missing_target, &link_path).unwrap();

        assert_eq!(
            content_hash_path(&link_path).unwrap(),
            content_hash_bytes(b"missing-target.txt")
        );
    }

    // ---------------------------------------------------------------
    // file_id derivation
    // ---------------------------------------------------------------

    #[test]
    fn file_id_is_deterministic() {
        let a = file_id_from_path(Path::new("src/main.rs")).unwrap();
        let b = file_id_from_path(Path::new("src/main.rs")).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn different_paths_produce_different_ids() {
        let a = file_id_from_path(Path::new("src/lib.rs")).unwrap();
        let b = file_id_from_path(Path::new("src/main.rs")).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn file_id_is_a_domain_separated_digest() {
        let id = file_id_from_path(Path::new("some/deep/path/file.rs")).unwrap();

        assert!(id.as_str().starts_with("path-blake3:"));
        assert_eq!(id.as_str().len(), "path-blake3:".len() + 64);
    }

    #[cfg(unix)]
    #[test]
    fn file_id_distinguishes_backslash_from_path_separator() {
        let backslash_path = file_id_from_path(Path::new(r"a\b")).unwrap();
        let separated_path = file_id_from_path(Path::new("a/b")).unwrap();

        assert_ne!(backslash_path, separated_path);
    }

    #[test]
    fn file_id_rejects_absolute_paths() {
        let result = file_id_from_path(Path::new("/tmp/cairn-file.rs"));

        assert!(matches!(result, Err(FileError::InvalidRelativePath { .. })));
    }

    #[test]
    fn file_id_rejects_parent_dir_paths() {
        let result = file_id_from_path(Path::new("src/../outside.rs"));

        assert!(matches!(result, Err(FileError::InvalidRelativePath { .. })));
    }

    // ---------------------------------------------------------------
    // source-class classification
    // ---------------------------------------------------------------

    #[test]
    fn test_dir_is_test() {
        assert_eq!(classify(Path::new("tests/foo_test.rs")), SourceClass::Test);
        assert_eq!(classify(Path::new("test/helper.py")), SourceClass::Test);
    }

    #[test]
    fn test_suffix_is_test() {
        assert_eq!(classify(Path::new("src/foo_test.rs")), SourceClass::Test);
        assert_eq!(classify(Path::new("src/foo_test.py")), SourceClass::Test);
    }

    #[test]
    fn test_prefix_is_test() {
        assert_eq!(classify(Path::new("src/test_foo.rs")), SourceClass::Test);
        assert_eq!(classify(Path::new("src/test_utils.py")), SourceClass::Test);
    }

    #[test]
    fn lockfile_is_lockfile() {
        assert_eq!(classify(Path::new("Cargo.lock")), SourceClass::Lockfile);
        assert_eq!(
            classify(Path::new("subdir/package-lock.json.lock")),
            SourceClass::Lockfile
        );
        assert_eq!(
            classify(Path::new("package-lock.json")),
            SourceClass::Lockfile
        );
        assert_eq!(classify(Path::new("pnpm-lock.yaml")), SourceClass::Lockfile);
        assert_eq!(classify(Path::new("yarn.lock")), SourceClass::Lockfile);
        assert_eq!(classify(Path::new("poetry.lock")), SourceClass::Lockfile);
        assert_eq!(classify(Path::new("Gemfile.lock")), SourceClass::Lockfile);
        assert_eq!(classify(Path::new("composer.lock")), SourceClass::Lockfile);
    }

    #[test]
    fn node_modules_is_vendored() {
        assert_eq!(
            classify(Path::new("node_modules/foo/index.js")),
            SourceClass::Vendored
        );
    }

    #[test]
    fn vendor_dir_is_vendored() {
        assert_eq!(
            classify(Path::new("vendor/libfoo/src/lib.rs")),
            SourceClass::Vendored
        );
    }

    #[test]
    fn target_dir_is_build_artifact() {
        assert_eq!(
            classify(Path::new("target/debug/cairn")),
            SourceClass::BuildArtifact
        );
    }

    #[test]
    fn dist_dir_is_build_artifact() {
        assert_eq!(
            classify(Path::new("dist/bundle.js")),
            SourceClass::BuildArtifact
        );
    }

    #[test]
    fn build_dir_is_build_artifact() {
        assert_eq!(
            classify(Path::new("build/output.o")),
            SourceClass::BuildArtifact
        );
    }

    #[test]
    fn migrations_dir_is_migration() {
        assert_eq!(
            classify(Path::new("migrations/001_init.sql")),
            SourceClass::Migration
        );
    }

    #[test]
    fn fixtures_dir_is_fixture() {
        assert_eq!(
            classify(Path::new("fixtures/data.json")),
            SourceClass::Fixture
        );
    }

    #[test]
    fn config_extensions_are_config() {
        assert_eq!(classify(Path::new("Cargo.toml")), SourceClass::Config);
        assert_eq!(classify(Path::new("config.yaml")), SourceClass::Config);
        assert_eq!(classify(Path::new("config.yml")), SourceClass::Config);
        assert_eq!(classify(Path::new("settings.json")), SourceClass::Config);
    }

    #[test]
    fn source_extensions_are_source() {
        assert_eq!(classify(Path::new("src/main.rs")), SourceClass::Source);
        assert_eq!(classify(Path::new("src/index.ts")), SourceClass::Source);
        assert_eq!(classify(Path::new("app.py")), SourceClass::Source);
        assert_eq!(classify(Path::new("index.html")), SourceClass::Source);
        assert_eq!(classify(Path::new("style.css")), SourceClass::Source);
        assert_eq!(classify(Path::new("README.md")), SourceClass::Source);
    }

    #[test]
    fn generated_extensions_are_generated() {
        assert_eq!(classify(Path::new("output.min.js")), SourceClass::Generated);
        assert_eq!(
            classify(Path::new("data/thing.generated")),
            SourceClass::Generated
        );
        assert_eq!(
            classify(Path::new("bundle.min.css")),
            SourceClass::Generated
        );
        assert_eq!(classify(Path::new("source.map")), SourceClass::Generated);
    }

    #[test]
    fn unknown_extension_is_unknown() {
        assert_eq!(classify(Path::new("mystery.xyz")), SourceClass::Unknown);
        assert_eq!(classify(Path::new("no_extension")), SourceClass::Unknown);
    }

    #[test]
    fn test_dir_takes_priority_over_extension() {
        assert_eq!(classify(Path::new("tests/helper.rs")), SourceClass::Test);
    }

    #[test]
    fn vendored_takes_priority_over_extension() {
        assert_eq!(
            classify(Path::new("node_modules/react/index.js")),
            SourceClass::Vendored
        );
    }

    #[test]
    fn build_artifact_takes_priority_over_extension() {
        assert_eq!(
            classify(Path::new("target/debug/build/output.rs")),
            SourceClass::BuildArtifact
        );
    }

    #[test]
    fn vendored_and_build_artifacts_beat_test_rules() {
        assert_eq!(
            classify(Path::new("node_modules/dep/foo_test.js")),
            SourceClass::Vendored
        );
        assert_eq!(
            classify(Path::new("vendor/tests/helper.rs")),
            SourceClass::Vendored
        );
        assert_eq!(
            classify(Path::new("target/debug/foo_test.js")),
            SourceClass::BuildArtifact
        );
    }

    // ---------------------------------------------------------------
    // FileVersion construction
    // ---------------------------------------------------------------

    fn make_epoch() -> RepoEpochId {
        RepoEpochId::new("test-epoch-001")
    }

    #[test]
    fn build_normal_file() {
        let dir = td();
        let file_path = dir.path().join("hello.rs");
        std::fs::write(&file_path, b"fn main() {}").unwrap();

        let fv = build_file_version(Path::new("hello.rs"), dir.path(), make_epoch()).unwrap();

        assert_eq!(
            fv.file_id,
            file_id_from_path(Path::new("hello.rs")).unwrap()
        );
        assert_eq!(fv.path, Path::new("hello.rs"));
        assert_eq!(fv.content_hash, content_hash_bytes(b"fn main() {}"));
        assert!(fv.size > 0);
        assert!(!fv.executable_bit);
        assert!(fv.symlink_target.is_none());
        assert_eq!(fv.source_class, SourceClass::Source);
    }

    #[cfg(unix)]
    #[test]
    fn executable_bit_is_captured() {
        use std::os::unix::fs::PermissionsExt;

        let dir = td();
        let file_path = dir.path().join("script.sh");
        std::fs::write(&file_path, b"#!/bin/sh\necho hi").unwrap();

        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let fv = build_file_version(Path::new("script.sh"), dir.path(), make_epoch()).unwrap();
        assert!(fv.executable_bit, "executable bit should be set");
    }

    #[cfg(unix)]
    #[test]
    fn non_executable_file_has_no_executable_bit() {
        use std::os::unix::fs::PermissionsExt;

        let dir = td();
        let file_path = dir.path().join("data.txt");
        std::fs::write(&file_path, b"plain data").unwrap();

        let mut perms = std::fs::metadata(&file_path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&file_path, perms).unwrap();

        let fv = build_file_version(Path::new("data.txt"), dir.path(), make_epoch()).unwrap();
        assert!(!fv.executable_bit);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_target_is_captured() {
        let dir = td();
        let target_path = dir.path().join("real_file.txt");
        let link_path = dir.path().join("link.txt");

        std::fs::write(&target_path, b"target content").unwrap();
        std::os::unix::fs::symlink(&target_path, &link_path).unwrap();

        let fv = build_file_version(Path::new("link.txt"), dir.path(), make_epoch()).unwrap();

        assert!(fv.symlink_target.is_some());
        assert_eq!(fv.symlink_target.as_ref().unwrap(), &target_path);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_file_version_hashes_link_target_path() {
        let dir = td();
        let target_path = dir.path().join("real_file.txt");
        let link_path = dir.path().join("link.txt");
        let link_target = Path::new("real_file.txt");

        std::fs::write(&target_path, b"target content").unwrap();
        std::os::unix::fs::symlink(link_target, &link_path).unwrap();

        let fv = build_file_version(Path::new("link.txt"), dir.path(), make_epoch()).unwrap();

        assert_eq!(fv.symlink_target, Some(link_target.to_path_buf()));
        assert_eq!(fv.content_hash, content_hash_bytes(b"real_file.txt"));
        assert_eq!(fv.size, b"real_file.txt".len() as u64);
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_file_version_does_not_error() {
        let dir = td();
        let link_path = dir.path().join("dangling.txt");
        let missing_target = Path::new("missing-target.txt");

        std::os::unix::fs::symlink(missing_target, &link_path).unwrap();

        let fv = build_file_version(Path::new("dangling.txt"), dir.path(), make_epoch()).unwrap();

        assert_eq!(fv.symlink_target, Some(missing_target.to_path_buf()));
        assert_eq!(fv.content_hash, content_hash_bytes(b"missing-target.txt"));
        assert_eq!(fv.size, b"missing-target.txt".len() as u64);
    }

    #[test]
    fn mtime_is_recorded() {
        let dir = td();
        let file_path = dir.path().join("mtime_test.txt");
        std::fs::write(&file_path, b"test").unwrap();

        let fv = build_file_version(Path::new("mtime_test.txt"), dir.path(), make_epoch()).unwrap();

        // mtime should be recent (positive nanoseconds since epoch)
        assert!(fv.mtime_observed.0 > 0);
    }

    #[test]
    fn mtime_is_not_used_for_freshness() {
        // Verifies the design invariant: we never compare mtime to decide
        // staleness. Identical content has the same hash regardless of
        // different mtimes.
        let dir = td();

        let path_a = dir.path().join("version_a.txt");
        let path_b = dir.path().join("version_b.txt");

        std::fs::write(&path_a, b"same content").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        std::fs::write(&path_b, b"same content").unwrap();

        let fv_a =
            build_file_version(Path::new("version_a.txt"), dir.path(), make_epoch()).unwrap();
        let fv_b =
            build_file_version(Path::new("version_b.txt"), dir.path(), make_epoch()).unwrap();

        // Same content → same hash. This is the freshness mechanism.
        assert_eq!(fv_a.content_hash, fv_b.content_hash);
        // mtimes may differ — irrelevant for correctness.
    }

    #[test]
    fn missing_file_returns_error() {
        let dir = td();
        let result = build_file_version(Path::new("does_not_exist.txt"), dir.path(), make_epoch());
        assert!(result.is_err());
    }

    #[test]
    fn build_file_version_rejects_absolute_relative_path() {
        let dir = td();
        let file_path = dir.path().join("absolute-input.rs");
        std::fs::write(&file_path, b"fn main() {}").unwrap();

        let result = build_file_version(&file_path, dir.path(), make_epoch());

        assert!(matches!(result, Err(FileError::InvalidRelativePath { .. })));
    }

    #[test]
    fn all_source_classes_have_test_coverage() {
        // Enumerates every SourceClass variant to confirm the classify()
        // function covers all variants at a path-to-class level.
        let test_cases: Vec<(SourceClass, &str)> = vec![
            (SourceClass::Source, "src/main.rs"),
            (SourceClass::Test, "tests/foo_test.rs"),
            (SourceClass::Generated, "output.min.js"),
            (SourceClass::Vendored, "vendor/lib.rs"),
            (SourceClass::BuildArtifact, "target/debug/out.o"),
            (SourceClass::Config, "Cargo.toml"),
            (SourceClass::Lockfile, "Cargo.lock"),
            (SourceClass::Migration, "migrations/001.sql"),
            (SourceClass::Fixture, "fixtures/data.json"),
            (SourceClass::Unknown, "mystery.xyz"),
        ];

        for (expected, path) in test_cases {
            let actual = classify(Path::new(path));
            assert_eq!(
                actual, expected,
                "path '{}' should classify as {:?} but got {:?}",
                path, expected, actual
            );
        }
    }
}
