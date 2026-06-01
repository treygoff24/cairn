//! Read `HEAD` and resolve symbolic refs via loose refs and `packed-refs`.

use std::path::Path;

use crate::error::VcsError;
use crate::gitdir::GitDirs;

/// Parsed `HEAD`: either a symbolic branch ref or a detached object id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HeadState {
    Symbolic {
        branch_ref: String,
        head_oid: Option<String>,
    },
    Detached {
        head_oid: String,
    },
    Missing,
}

pub(crate) fn read_head(dirs: &GitDirs) -> Result<HeadState, VcsError> {
    let head_path = dirs.git_dir.join("HEAD");
    if !head_path.is_file() {
        return Ok(HeadState::Missing);
    }

    let raw = match std::fs::read_to_string(&head_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HeadState::Missing),
        Err(e) => return Err(VcsError::io(&head_path, e)),
    };

    let trimmed = raw.trim();
    if let Some(ref_name) = trimmed.strip_prefix("ref: ") {
        let branch_ref = ref_name.trim().to_string();
        let head_oid = resolve_ref(dirs, &branch_ref)?;
        return Ok(HeadState::Symbolic {
            branch_ref,
            head_oid,
        });
    }

    if is_git_oid(trimmed) {
        return Ok(HeadState::Detached {
            head_oid: trimmed.to_ascii_lowercase(),
        });
    }

    Ok(HeadState::Missing)
}

pub(crate) fn is_git_oid(s: &str) -> bool {
    let len = s.len();
    if len != 40 && len != 64 {
        return false;
    }
    s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn resolve_ref(dirs: &GitDirs, ref_name: &str) -> Result<Option<String>, VcsError> {
    let ref_path = ref_name.strip_prefix("refs/").unwrap_or(ref_name);
    for base in [&dirs.git_dir, &dirs.common_dir] {
        let loose = base.join("refs").join(ref_path);
        if let Some(oid) = read_loose_ref(&loose)? {
            return Ok(Some(oid));
        }
    }

    for base in [&dirs.common_dir, &dirs.git_dir] {
        if let Some(oid) = lookup_packed_ref(base, ref_name)? {
            return Ok(Some(oid));
        }
    }

    Ok(None)
}

fn read_loose_ref(path: &Path) -> Result<Option<String>, VcsError> {
    if !path.is_file() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(path).map_err(|e| VcsError::io(path, e))?;
    let oid = raw.trim();
    if is_git_oid(oid) {
        Ok(Some(oid.to_ascii_lowercase()))
    } else {
        Ok(None)
    }
}

fn lookup_packed_ref(base: &Path, ref_name: &str) -> Result<Option<String>, VcsError> {
    let packed = base.join("packed-refs");
    if !packed.is_file() {
        return Ok(None);
    }

    let contents = std::fs::read_to_string(&packed).map_err(|e| VcsError::io(&packed, e))?;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('^') {
            continue;
        }
        let Some((oid, name)) = line.split_once(' ') else {
            continue;
        };
        if name.trim() == ref_name && is_git_oid(oid) {
            return Ok(Some(oid.to_ascii_lowercase()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gitdir::resolve_git_dirs;
    use crate::test_fixtures::build_fixture;
    use tempfile::TempDir;

    /// Builds a fixture and resolves its dirs; the `TempDir` must be kept alive
    /// by the caller for the duration of the reads.
    fn dirs_fixture(name: &str) -> (TempDir, GitDirs) {
        let tmp = build_fixture(name);
        let dirs = resolve_git_dirs(tmp.path()).unwrap().expect("repo");
        (tmp, dirs)
    }

    #[test]
    fn normal_symbolic_head_resolves_oid() {
        let (_tmp, dirs) = dirs_fixture("normal");
        let head = read_head(&dirs).unwrap();
        assert_eq!(
            head,
            HeadState::Symbolic {
                branch_ref: "refs/heads/main".to_string(),
                head_oid: Some("1111111111111111111111111111111111111111".to_string()),
            }
        );
    }

    #[test]
    fn detached_head_parses_oid() {
        let (_tmp, dirs) = dirs_fixture("detached");
        let head = read_head(&dirs).unwrap();
        assert_eq!(
            head,
            HeadState::Detached {
                head_oid: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            }
        );
    }

    #[test]
    fn packed_refs_used_when_loose_missing() {
        let (_tmp, dirs) = dirs_fixture("packed-refs-only");
        let head = read_head(&dirs).unwrap();
        assert_eq!(
            head,
            HeadState::Symbolic {
                branch_ref: "refs/heads/main".to_string(),
                head_oid: Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
            }
        );
    }
}
