//! Resolve `.git` (directory, gitfile, or linked worktree) without invoking git.

use std::path::{Path, PathBuf};

use crate::error::VcsError;

/// Resolved git metadata directories for one worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitDirs {
    /// Per-worktree git directory (`HEAD`, rebase state, …).
    pub git_dir: PathBuf,
    /// Shared common directory (refs, `packed-refs`, objects, …).
    pub common_dir: PathBuf,
}

/// Locates git metadata for `worktree_root`, or `None` when this is not a git worktree.
pub(crate) fn resolve_git_dirs(worktree_root: &Path) -> Result<Option<GitDirs>, VcsError> {
    let dot_git = worktree_root.join(".git");
    if !dot_git.exists() {
        return Ok(None);
    }

    let git_dir = resolve_git_dir_path(worktree_root, &dot_git)?;
    let common_dir = resolve_common_dir(&git_dir)?;

    Ok(Some(GitDirs {
        git_dir,
        common_dir,
    }))
}

fn resolve_git_dir_path(worktree_root: &Path, dot_git: &Path) -> Result<PathBuf, VcsError> {
    let meta = dot_git.metadata().map_err(|e| VcsError::io(dot_git, e))?;

    if meta.is_dir() {
        return Ok(dot_git.to_path_buf());
    }

    if !meta.is_file() {
        return Ok(dot_git.to_path_buf());
    }

    let contents = std::fs::read_to_string(dot_git).map_err(|e| VcsError::io(dot_git, e))?;
    let gitdir_line = contents
        .lines()
        .find_map(|line| line.strip_prefix("gitdir:"))
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let Some(relative_or_absolute) = gitdir_line else {
        return Ok(dot_git.to_path_buf());
    };

    let path = Path::new(relative_or_absolute);
    let git_dir = if path.is_absolute() {
        path.to_path_buf()
    } else {
        worktree_root.join(path)
    };

    Ok(git_dir)
}

fn resolve_common_dir(git_dir: &Path) -> Result<PathBuf, VcsError> {
    let commondir_file = git_dir.join("commondir");
    if !commondir_file.is_file() {
        return Ok(git_dir.to_path_buf());
    }

    let relative = std::fs::read_to_string(&commondir_file)
        .map_err(|e| VcsError::io(&commondir_file, e))?
        .trim()
        .to_string();

    if relative.is_empty() {
        return Ok(git_dir.to_path_buf());
    }

    // Git's `commondir` path is relative to the per-worktree git dir itself (its
    // standard content is `../..`), not to the parent. Resolve against `git_dir`
    // and lexically fold the `..` components so the result is a clean path.
    let rel = Path::new(&relative);
    let common = if rel.is_absolute() {
        rel.to_path_buf()
    } else {
        normalize_lexical(&git_dir.join(rel))
    };
    Ok(common)
}

/// Lexically resolves `.` and `..` components without touching the filesystem.
fn normalize_lexical(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Returns `true` when `name` exists as a file or directory under `dir`.
pub(crate) fn marker_present(dir: &Path, name: &str) -> bool {
    let path = dir.join(name);
    path.is_file() || path.is_dir()
}

/// Marker names present under `git_dir`, then under `common_dir` if distinct (sorted, deduped).
pub(crate) fn present_markers(git_dir: &Path, common_dir: &Path) -> Vec<&'static str> {
    const MARKERS: &[&str] = &[
        "BISECT_LOG",
        "CHERRY_PICK_HEAD",
        "MERGE_HEAD",
        "rebase-apply",
        "rebase-merge",
    ];

    let mut found = Vec::new();
    for marker in MARKERS {
        if marker_present(git_dir, marker)
            || (common_dir != git_dir && marker_present(common_dir, marker))
        {
            found.push(*marker);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::build_fixture;

    #[test]
    fn normal_repo_git_dir_is_directory() {
        let tmp = build_fixture("normal");
        let dirs = resolve_git_dirs(tmp.path())
            .unwrap()
            .expect("fixture is a repo");
        assert!(dirs.git_dir.ends_with(".git"));
        assert_eq!(dirs.git_dir, dirs.common_dir);
    }

    #[test]
    fn linked_worktree_resolves_gitfile_and_commondir() {
        let tmp = build_fixture("linked");
        let dirs = resolve_git_dirs(tmp.path())
            .unwrap()
            .expect("fixture is a repo");
        assert!(dirs.git_dir.ends_with("worktrees/feature"));
        assert!(dirs.common_dir.ends_with("main.git"));
        assert_ne!(dirs.git_dir, dirs.common_dir);
    }

    #[test]
    fn missing_dot_git_returns_none() {
        let tmp = build_fixture("not-a-repo");
        assert!(resolve_git_dirs(tmp.path()).unwrap().is_none());
    }
}
