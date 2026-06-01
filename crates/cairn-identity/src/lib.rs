//! `cairn-identity` — canonical project/worktree identity.
//!
//! Cairn runs one daemon per worktree. This crate turns a user-supplied worktree
//! root into the stable identity tuple every downstream ledger uses: canonical
//! root, optional git-common-dir identity, worktree identity, config hash, and
//! protocol version.
//!
//! The implementation deliberately reads `.git` metadata directly. Normal repos
//! have a `.git` directory. Linked worktrees have a `.git` file containing
//! `gitdir: <path>`, and that gitdir may contain a `commondir` file pointing back
//! to the shared repository `.git` directory.
//!
//! Path identity is path-based after `std::fs::canonicalize`. On filesystems that
//! prove case-insensitive by resolving an ASCII case-variant alias to the same
//! filesystem entry, identity paths are ASCII-folded before hashing. If the probe
//! is inconclusive, paths remain case-sensitive. Bind-mount aliases are therefore
//! not collapsed by inode alone; they keep distinct canonical paths and cannot
//! false-match into the same worktree identity.

use cairn_types::{ConfigHash, GitCommonDirId, ProtocolVersion, WorktreeId};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

const WORKTREE_HASH_DOMAIN: &[u8] = b"cairn.identity.worktree.v1";
const GIT_COMMON_DIR_HASH_DOMAIN: &[u8] = b"cairn.identity.git-common-dir.v1";
const HASH_SEGMENT_NONE: &[u8] = b"<none>";

/// Canonical identity of the Cairn daemon's worktree scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeIdentity {
    /// Canonicalized root path used by the identity hash.
    pub canonical_root: PathBuf,
    /// Canonicalized shared git directory for this worktree, when the root is a
    /// git repository.
    pub git_common_dir: Option<PathBuf>,
    /// Stable ID derived from [`git_common_dir`](Self::git_common_dir).
    pub git_common_dir_id: Option<GitCommonDirId>,
    /// Stable ID derived from the canonical root plus optional common git dir.
    pub worktree_id: WorktreeId,
    /// Effective Cairn config hash supplied by `cairn-config`.
    pub config_hash: ConfigHash,
    /// Adapter/daemon protocol version supplied by `cairn-config`.
    pub protocol_version: ProtocolVersion,
}

impl WorktreeIdentity {
    /// Computes identity for one daemon/worktree scope.
    pub fn compute(
        root: impl AsRef<Path>,
        config_hash: ConfigHash,
        protocol_version: ProtocolVersion,
    ) -> Result<Self, IdentityError> {
        compute_worktree_identity(root, config_hash, protocol_version)
    }
}

/// Observed case semantics for the filesystem containing a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathCaseSemantics {
    /// ASCII case variants resolve to the same filesystem entry.
    CaseInsensitive,
    /// ASCII case variants do not resolve to the same filesystem entry.
    CaseSensitive,
    /// The path contained no ASCII component suitable for a read-only probe.
    Unknown,
}

/// Errors returned while computing canonical worktree identity.
#[derive(Debug, Error)]
pub enum IdentityError {
    /// The supplied root could not be resolved to a real path.
    #[error("failed to canonicalize worktree root `{path}`: {source}")]
    CanonicalizeRoot { path: PathBuf, source: io::Error },

    /// The resolved root is not a directory.
    #[error("worktree root `{path}` is not a directory")]
    RootNotDirectory { path: PathBuf },

    /// The root's `.git` entry could not be inspected.
    #[error("failed to inspect git metadata entry `{path}`: {source}")]
    InspectGitEntry { path: PathBuf, source: io::Error },

    /// A git metadata file could not be read as text.
    #[error("failed to read git metadata file `{path}`: {source}")]
    ReadGitMetadataFile { path: PathBuf, source: io::Error },

    /// A `.git` file did not use Git's `gitdir: <path>` format.
    #[error("malformed .git file `{path}`: expected `gitdir: <path>`")]
    MalformedGitFile { path: PathBuf },

    /// A `.git` or `commondir` file pointed to an empty path.
    #[error("git metadata file `{path}` points to an empty path")]
    EmptyGitMetadataPath { path: PathBuf },

    /// The resolved `.git` path is neither a file nor a directory.
    #[error("unsupported git metadata entry `{path}`: expected file or directory")]
    UnsupportedGitEntry { path: PathBuf },

    /// A gitdir path could not be resolved to a real path.
    #[error("failed to canonicalize git directory `{path}`: {source}")]
    CanonicalizeGitDir { path: PathBuf, source: io::Error },

    /// The resolved gitdir is not a directory.
    #[error("resolved git directory `{path}` is not a directory")]
    GitDirNotDirectory { path: PathBuf },

    /// A common git dir path could not be resolved to a real path.
    #[error("failed to canonicalize git common directory `{path}`: {source}")]
    CanonicalizeGitCommonDir { path: PathBuf, source: io::Error },

    /// The resolved common git dir is not a directory.
    #[error("resolved git common directory `{path}` is not a directory")]
    GitCommonDirNotDirectory { path: PathBuf },
}

/// Computes identity for one daemon/worktree scope.
pub fn compute_worktree_identity(
    root: impl AsRef<Path>,
    config_hash: ConfigHash,
    protocol_version: ProtocolVersion,
) -> Result<WorktreeIdentity, IdentityError> {
    let canonical_root = canonicalize_worktree_root(root)?;
    let git_common_dir = discover_git_common_dir(&canonical_root)?;

    let identity_root = path_for_identity(&canonical_root);
    let identity_common_dir = git_common_dir.as_deref().map(path_for_identity);

    let git_common_dir_id = identity_common_dir
        .as_deref()
        .map(git_common_dir_id_for_path);
    let worktree_id = worktree_id_for_paths(&identity_root, identity_common_dir.as_deref());

    Ok(WorktreeIdentity {
        canonical_root: identity_root,
        git_common_dir: identity_common_dir,
        git_common_dir_id,
        worktree_id,
        config_hash,
        protocol_version,
    })
}

/// Resolves a worktree root to the canonical directory path returned by the OS.
pub fn canonicalize_worktree_root(root: impl AsRef<Path>) -> Result<PathBuf, IdentityError> {
    let root = root.as_ref();
    let canonical_root =
        fs::canonicalize(root).map_err(|source| IdentityError::CanonicalizeRoot {
            path: root.to_path_buf(),
            source,
        })?;

    if !canonical_root.is_dir() {
        return Err(IdentityError::RootNotDirectory {
            path: canonical_root,
        });
    }

    Ok(canonical_root)
}

/// Resolves the shared git common directory for a worktree root.
pub fn resolve_git_common_dir(root: impl AsRef<Path>) -> Result<Option<PathBuf>, IdentityError> {
    let canonical_root = canonicalize_worktree_root(root)?;
    discover_git_common_dir(&canonical_root)
}

/// Detects case semantics for an existing directory without writing probe files.
pub fn detect_path_case_semantics(
    path: impl AsRef<Path>,
) -> Result<PathCaseSemantics, IdentityError> {
    let canonical_path = canonicalize_worktree_root(path)?;
    Ok(detect_existing_path_case_semantics(&canonical_path))
}

fn discover_git_common_dir(canonical_root: &Path) -> Result<Option<PathBuf>, IdentityError> {
    let git_entry = canonical_root.join(".git");
    let metadata = match fs::metadata(&git_entry) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(IdentityError::InspectGitEntry {
                path: git_entry,
                source,
            });
        }
    };

    if metadata.is_dir() {
        return canonicalize_git_dir(&git_entry).map(Some);
    }

    if metadata.is_file() {
        let git_dir = read_gitdir_pointer(&git_entry)?;
        let git_dir = canonicalize_git_dir(&git_dir)?;
        return resolve_common_dir_from_git_dir(&git_dir);
    }

    Err(IdentityError::UnsupportedGitEntry { path: git_entry })
}

fn read_gitdir_pointer(git_file: &Path) -> Result<PathBuf, IdentityError> {
    let contents = read_git_metadata_file(git_file)?;
    let Some(git_dir) = contents.trim().strip_prefix("gitdir:") else {
        return Err(IdentityError::MalformedGitFile {
            path: git_file.to_path_buf(),
        });
    };

    metadata_path_from_file(git_file, git_dir)
}

fn resolve_common_dir_from_git_dir(git_dir: &Path) -> Result<Option<PathBuf>, IdentityError> {
    let common_dir_file = git_dir.join("commondir");
    let contents = match read_git_metadata_file(&common_dir_file) {
        Ok(contents) => contents,
        Err(IdentityError::ReadGitMetadataFile { source, .. })
            if source.kind() == io::ErrorKind::NotFound =>
        {
            return Ok(Some(git_dir.to_path_buf()));
        }
        Err(error) => return Err(error),
    };

    let common_dir = metadata_path_from_file(&common_dir_file, &contents)?;
    canonicalize_git_common_dir(&common_dir).map(Some)
}

fn read_git_metadata_file(path: &Path) -> Result<String, IdentityError> {
    fs::read_to_string(path).map_err(|source| IdentityError::ReadGitMetadataFile {
        path: path.to_path_buf(),
        source,
    })
}

fn metadata_path_from_file(file: &Path, raw_path: &str) -> Result<PathBuf, IdentityError> {
    let trimmed_path = raw_path.trim();
    if trimmed_path.is_empty() {
        return Err(IdentityError::EmptyGitMetadataPath {
            path: file.to_path_buf(),
        });
    }

    let path = Path::new(trimmed_path);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    let base_dir = file.parent().unwrap_or_else(|| Path::new("."));
    Ok(base_dir.join(path))
}

fn canonicalize_git_dir(path: &Path) -> Result<PathBuf, IdentityError> {
    let canonical_path =
        fs::canonicalize(path).map_err(|source| IdentityError::CanonicalizeGitDir {
            path: path.to_path_buf(),
            source,
        })?;

    if !canonical_path.is_dir() {
        return Err(IdentityError::GitDirNotDirectory {
            path: canonical_path,
        });
    }

    Ok(canonical_path)
}

fn canonicalize_git_common_dir(path: &Path) -> Result<PathBuf, IdentityError> {
    let canonical_path =
        fs::canonicalize(path).map_err(|source| IdentityError::CanonicalizeGitCommonDir {
            path: path.to_path_buf(),
            source,
        })?;

    if !canonical_path.is_dir() {
        return Err(IdentityError::GitCommonDirNotDirectory {
            path: canonical_path,
        });
    }

    Ok(canonical_path)
}

fn path_for_identity(path: &Path) -> PathBuf {
    match detect_existing_path_case_semantics(path) {
        PathCaseSemantics::CaseInsensitive => ascii_fold_path(path),
        PathCaseSemantics::CaseSensitive | PathCaseSemantics::Unknown => path.to_path_buf(),
    }
}

fn detect_existing_path_case_semantics(path: &Path) -> PathCaseSemantics {
    let Some(case_variant) = case_variant_path(path) else {
        return PathCaseSemantics::Unknown;
    };

    let Ok(original_metadata) = fs::metadata(path) else {
        return PathCaseSemantics::Unknown;
    };

    let variant_metadata = match fs::metadata(&case_variant) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return PathCaseSemantics::CaseSensitive;
        }
        Err(_) => return PathCaseSemantics::Unknown,
    };

    if metadata_refer_to_same_entry(&original_metadata, &variant_metadata) {
        PathCaseSemantics::CaseInsensitive
    } else {
        PathCaseSemantics::CaseSensitive
    }
}

fn case_variant_path(path: &Path) -> Option<PathBuf> {
    let mut suffixes = Vec::new();
    let mut cursor = path;

    loop {
        let component_name = cursor.file_name()?;
        if let Some(toggled_name) = toggle_ascii_case(component_name) {
            let parent = cursor.parent()?;
            let mut candidate = parent.join(toggled_name);
            for suffix in suffixes.iter().rev() {
                candidate.push(suffix);
            }
            return Some(candidate);
        }

        suffixes.push(component_name.to_os_string());
        cursor = cursor.parent()?;
    }
}

#[cfg(unix)]
fn toggle_ascii_case(name: &OsStr) -> Option<OsString> {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let mut bytes = name.as_bytes().to_vec();
    let mut changed = false;
    for byte in &mut bytes {
        if byte.is_ascii_lowercase() {
            *byte = byte.to_ascii_uppercase();
            changed = true;
        } else if byte.is_ascii_uppercase() {
            *byte = byte.to_ascii_lowercase();
            changed = true;
        }
    }

    changed.then(|| OsString::from_vec(bytes))
}

#[cfg(windows)]
fn toggle_ascii_case(name: &OsStr) -> Option<OsString> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    let mut units: Vec<u16> = name.encode_wide().collect();
    let mut changed = false;
    for unit in &mut units {
        if (b'a' as u16..=b'z' as u16).contains(unit) {
            *unit -= u16::from(b'a' - b'A');
            changed = true;
        } else if (b'A' as u16..=b'Z' as u16).contains(unit) {
            *unit += u16::from(b'a' - b'A');
            changed = true;
        }
    }

    changed.then(|| OsString::from_wide(&units))
}

#[cfg(not(any(unix, windows)))]
fn toggle_ascii_case(name: &OsStr) -> Option<OsString> {
    let mut text = name.to_string_lossy().into_owned();
    let mut changed = false;
    let folded: String = text
        .drain(..)
        .map(|character| {
            if character.is_ascii_lowercase() {
                changed = true;
                character.to_ascii_uppercase()
            } else if character.is_ascii_uppercase() {
                changed = true;
                character.to_ascii_lowercase()
            } else {
                character
            }
        })
        .collect();

    changed.then(|| OsString::from(folded))
}

#[cfg(unix)]
fn ascii_fold_path(path: &Path) -> PathBuf {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let bytes = path
        .as_os_str()
        .as_bytes()
        .iter()
        .map(u8::to_ascii_lowercase)
        .collect();
    PathBuf::from(OsString::from_vec(bytes))
}

#[cfg(windows)]
fn ascii_fold_path(path: &Path) -> PathBuf {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    let units = path
        .as_os_str()
        .encode_wide()
        .map(|unit| {
            if (b'A' as u16..=b'Z' as u16).contains(&unit) {
                unit + u16::from(b'a' - b'A')
            } else {
                unit
            }
        })
        .collect::<Vec<_>>();
    PathBuf::from(OsString::from_wide(&units))
}

#[cfg(not(any(unix, windows)))]
fn ascii_fold_path(path: &Path) -> PathBuf {
    let folded = path
        .to_string_lossy()
        .chars()
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();
    PathBuf::from(folded)
}

#[cfg(unix)]
fn metadata_refer_to_same_entry(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(windows)]
fn metadata_refer_to_same_entry(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    match (
        left.volume_serial_number(),
        left.file_index(),
        right.volume_serial_number(),
        right.file_index(),
    ) {
        (Some(left_volume), Some(left_index), Some(right_volume), Some(right_index)) => {
            left_volume == right_volume && left_index == right_index
        }
        _ => false,
    }
}

#[cfg(not(any(unix, windows)))]
fn metadata_refer_to_same_entry(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

fn worktree_id_for_paths(canonical_root: &Path, git_common_dir: Option<&Path>) -> WorktreeId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(WORKTREE_HASH_DOMAIN);
    update_hash_segment(&mut hasher, b"root");
    update_hash_segment(&mut hasher, &path_bytes(canonical_root));
    update_hash_segment(&mut hasher, b"git-common-dir");
    match git_common_dir {
        Some(path) => update_hash_segment(&mut hasher, &path_bytes(path)),
        None => update_hash_segment(&mut hasher, HASH_SEGMENT_NONE),
    }

    WorktreeId::new(format!("wt_{}", hasher.finalize().to_hex()))
}

fn git_common_dir_id_for_path(git_common_dir: &Path) -> GitCommonDirId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(GIT_COMMON_DIR_HASH_DOMAIN);
    update_hash_segment(&mut hasher, &path_bytes(git_common_dir));

    GitCommonDirId::new(format!("gcd_{}", hasher.finalize().to_hex()))
}

fn update_hash_segment(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/identity")
            .join(relative)
    }

    fn config_hash(hex: &str) -> ConfigHash {
        ConfigHash::from_hex(hex)
    }

    fn protocol_version() -> ProtocolVersion {
        ProtocolVersion(1)
    }

    fn identity_for(root: impl AsRef<Path>) -> Result<WorktreeIdentity, IdentityError> {
        WorktreeIdentity::compute(
            root,
            config_hash("1111111111111111111111111111111111111111111111111111111111111111"),
            protocol_version(),
        )
    }

    fn unique_temp_path(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "cairn-identity-{label}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn copy_fixture_file(relative: &str, destination: &Path) -> io::Result<()> {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(fixture_path(relative), destination)?;
        Ok(())
    }

    fn materialize_normal_repo(label: &str) -> io::Result<PathBuf> {
        let root = unique_temp_path(label);
        copy_fixture_file("normal/repo/dot-git/HEAD", &root.join(".git/HEAD"))?;
        Ok(root)
    }

    fn materialize_linked_worktrees(label: &str) -> io::Result<PathBuf> {
        let root = unique_temp_path(label);
        copy_fixture_file("linked/common.git/HEAD", &root.join("common.git/HEAD"))?;
        copy_fixture_file(
            "linked/gitdirs/worktree-a/commondir",
            &root.join("gitdirs/worktree-a/commondir"),
        )?;
        copy_fixture_file(
            "linked/gitdirs/worktree-b/commondir",
            &root.join("gitdirs/worktree-b/commondir"),
        )?;
        copy_fixture_file("linked/worktree-a/git-file", &root.join("worktree-a/.git"))?;
        copy_fixture_file("linked/worktree-b/git-file", &root.join("worktree-b/.git"))?;
        Ok(root)
    }

    fn materialize_nested_worktrees(label: &str) -> io::Result<PathBuf> {
        let parent = unique_temp_path(label);
        copy_fixture_file("nested/parent/dot-git/HEAD", &parent.join(".git/HEAD"))?;
        copy_fixture_file(
            "nested/parent/child/dot-git/HEAD",
            &parent.join("child/.git/HEAD"),
        )?;
        Ok(parent)
    }

    #[test]
    fn normal_repo_uses_dot_git_as_common_dir() -> Result<(), Box<dyn Error>> {
        let root = materialize_normal_repo("normal-repo")?;
        let identity = identity_for(&root)?;
        let common_dir = resolve_git_common_dir(&root)?.expect("normal repo has .git dir");

        assert_eq!(
            identity.git_common_dir,
            Some(path_for_identity(&common_dir))
        );
        assert!(identity.git_common_dir_id.is_some());
        assert_eq!(common_dir, fs::canonicalize(root.join(".git"))?);
        fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn symlinked_root_and_real_path_share_identity() -> Result<(), Box<dyn Error>> {
        let linked_root = materialize_linked_worktrees("symlink-linked")?;
        let real_root = linked_root.join("worktree-a");
        let temp_dir = unique_temp_path("symlink");
        fs::create_dir_all(&temp_dir)?;
        let symlink_root = temp_dir.join("worktree-a-link");

        if let Err(error) = create_dir_symlink(&real_root, &symlink_root) {
            fs::remove_dir_all(&temp_dir).ok();
            fs::remove_dir_all(&linked_root).ok();
            if symlink_creation_can_be_skipped(&error) {
                return Ok(());
            }
            return Err(Box::new(error));
        }

        let real_identity = identity_for(&real_root)?;
        let symlink_identity = identity_for(&symlink_root)?;

        assert_eq!(
            real_identity.canonical_root,
            symlink_identity.canonical_root
        );
        assert_eq!(real_identity.worktree_id, symlink_identity.worktree_id);
        assert_eq!(
            real_identity.git_common_dir_id,
            symlink_identity.git_common_dir_id
        );

        fs::remove_dir_all(&temp_dir)?;
        fs::remove_dir_all(&linked_root)?;
        Ok(())
    }

    #[test]
    fn sibling_linked_worktrees_share_common_dir_but_not_worktree_id() -> Result<(), Box<dyn Error>>
    {
        let linked_root = materialize_linked_worktrees("linked-siblings")?;
        let worktree_a = identity_for(linked_root.join("worktree-a"))?;
        let worktree_b = identity_for(linked_root.join("worktree-b"))?;

        assert_ne!(worktree_a.canonical_root, worktree_b.canonical_root);
        assert_ne!(worktree_a.worktree_id, worktree_b.worktree_id);
        assert_eq!(worktree_a.git_common_dir, worktree_b.git_common_dir);
        assert_eq!(worktree_a.git_common_dir_id, worktree_b.git_common_dir_id);
        fs::remove_dir_all(&linked_root)?;
        Ok(())
    }

    #[test]
    fn nested_worktrees_do_not_collide() -> Result<(), Box<dyn Error>> {
        let parent_root = materialize_nested_worktrees("nested-worktrees")?;
        let parent = identity_for(&parent_root)?;
        let child = identity_for(parent_root.join("child"))?;

        assert_ne!(parent.canonical_root, child.canonical_root);
        assert_ne!(parent.worktree_id, child.worktree_id);
        fs::remove_dir_all(&parent_root)?;
        Ok(())
    }

    #[test]
    fn detached_head_does_not_affect_identity() -> Result<(), Box<dyn Error>> {
        let root = unique_temp_path("detached-head");
        let git_dir = root.join(".git");
        fs::create_dir_all(&git_dir)?;
        fs::write(git_dir.join("HEAD"), "ref: refs/heads/main\n")?;

        let branch_identity = identity_for(&root)?;
        fs::write(
            git_dir.join("HEAD"),
            "0123456789abcdef0123456789abcdef01234567\n",
        )?;
        let detached_identity = identity_for(&root)?;

        assert_eq!(branch_identity.worktree_id, detached_identity.worktree_id);
        assert_eq!(
            branch_identity.git_common_dir_id,
            detached_identity.git_common_dir_id
        );

        fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn config_hash_is_part_of_full_identity() -> Result<(), Box<dyn Error>> {
        let linked_root = materialize_linked_worktrees("config-hash")?;
        let root = linked_root.join("worktree-a");
        let first = WorktreeIdentity::compute(
            &root,
            config_hash("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            protocol_version(),
        )?;
        let second = WorktreeIdentity::compute(
            &root,
            config_hash("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            protocol_version(),
        )?;

        assert_eq!(first.worktree_id, second.worktree_id);
        assert_ne!(first, second);
        fs::remove_dir_all(&linked_root)?;
        Ok(())
    }

    #[test]
    fn case_variants_follow_filesystem_semantics() -> Result<(), Box<dyn Error>> {
        let temp_dir = unique_temp_path("case");
        let mixed_case_root = temp_dir.join("CaseRoot");
        let lower_case_root = temp_dir.join("caseroot");
        fs::create_dir_all(&mixed_case_root)?;

        match detect_path_case_semantics(&mixed_case_root)? {
            PathCaseSemantics::CaseInsensitive => {
                let mixed = identity_for(&mixed_case_root)?;
                let lower = identity_for(&lower_case_root)?;
                assert_eq!(mixed.canonical_root, lower.canonical_root);
                assert_eq!(mixed.worktree_id, lower.worktree_id);
            }
            PathCaseSemantics::CaseSensitive | PathCaseSemantics::Unknown => {
                fs::create_dir_all(&lower_case_root)?;
                let mixed = identity_for(&mixed_case_root)?;
                let lower = identity_for(&lower_case_root)?;
                assert_ne!(mixed.canonical_root, lower.canonical_root);
                assert_ne!(mixed.worktree_id, lower.worktree_id);
            }
        }

        fs::remove_dir_all(&temp_dir)?;
        Ok(())
    }

    #[test]
    fn symlink_loop_fails_cleanly() -> Result<(), Box<dyn Error>> {
        let temp_dir = unique_temp_path("symlink-loop");
        fs::create_dir_all(&temp_dir)?;
        let loop_path = temp_dir.join("loop");

        if let Err(error) = create_dir_symlink(&loop_path, &loop_path) {
            fs::remove_dir_all(&temp_dir).ok();
            if symlink_creation_can_be_skipped(&error) {
                return Ok(());
            }
            return Err(Box::new(error));
        }

        let error = identity_for(&loop_path).expect_err("symlink loop should not compute identity");
        assert!(matches!(error, IdentityError::CanonicalizeRoot { .. }));

        fs::remove_dir_all(&temp_dir)?;
        Ok(())
    }

    #[test]
    fn root_without_git_repo_has_no_common_dir() -> Result<(), Box<dyn Error>> {
        let root = fixture_path("case/CaseRoot");
        let identity = identity_for(&root)?;

        assert_eq!(identity.git_common_dir, None);
        assert_eq!(identity.git_common_dir_id, None);
        Ok(())
    }

    #[cfg(unix)]
    fn create_dir_symlink(target: &Path, link: &Path) -> io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn create_dir_symlink(target: &Path, link: &Path) -> io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[cfg(not(any(unix, windows)))]
    fn create_dir_symlink(_target: &Path, _link: &Path) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "directory symlink tests are unsupported on this platform",
        ))
    }

    fn symlink_creation_can_be_skipped(error: &io::Error) -> bool {
        matches!(
            error.kind(),
            io::ErrorKind::PermissionDenied | io::ErrorKind::Unsupported
        )
    }
}
