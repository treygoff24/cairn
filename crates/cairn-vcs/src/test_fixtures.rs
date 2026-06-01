//! Runtime git-worktree fixtures for tests.
//!
//! Fixtures are built in a fresh `TempDir` rather than committed on disk: git
//! refuses to track any path containing a `.git` component, so on-disk `.git`
//! fixtures would not survive a checkout. Each builder writes exactly the marker
//! files a test needs; keep the returned `TempDir` alive for the test's reads.

use std::fs;
use std::path::Path;

use tempfile::TempDir;

const OID_MAIN: &str = "1111111111111111111111111111111111111111";
const OID_DETACHED: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OID_PACKED: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const OID_FEATURE: &str = "cccccccccccccccccccccccccccccccccccccccc";
const OID_OTHER: &str = "3333333333333333333333333333333333333333";

/// Builds the named worktree fixture under a fresh tempdir and returns it. The
/// tempdir's `path()` is the worktree root; the value must outlive the reads.
pub(crate) fn build_fixture(name: &str) -> TempDir {
    let tmp = TempDir::new().expect("create tempdir");
    let root = tmp.path();
    match name {
        // No `.git` at all — not a git worktree.
        "not-a-repo" => {}

        "normal" => {
            write(root, ".git/HEAD", "ref: refs/heads/main\n");
            write(root, ".git/refs/heads/main", &line(OID_MAIN));
        }
        "detached" => {
            write(root, ".git/HEAD", &line(OID_DETACHED));
        }
        "merge" => {
            write(root, ".git/HEAD", "ref: refs/heads/main\n");
            write(root, ".git/refs/heads/main", &line(OID_MAIN));
            write(root, ".git/MERGE_HEAD", &line(OID_OTHER));
        }
        "rebase-merge" => {
            write(root, ".git/HEAD", "ref: refs/heads/main\n");
            mkdir(root, ".git/rebase-merge");
        }
        "rebase-apply" => {
            write(root, ".git/HEAD", "ref: refs/heads/main\n");
            mkdir(root, ".git/rebase-apply");
        }
        "cherry-pick" => {
            write(root, ".git/HEAD", "ref: refs/heads/main\n");
            write(root, ".git/CHERRY_PICK_HEAD", &line(OID_OTHER));
        }
        "bisect" => {
            write(root, ".git/HEAD", "ref: refs/heads/main\n");
            write(root, ".git/BISECT_LOG", "bisect start\n");
        }
        // Detached HEAD with a merge in progress: the in-progress op wins.
        "merge-over-detached-head" => {
            write(root, ".git/HEAD", &line(OID_DETACHED));
            write(root, ".git/MERGE_HEAD", &line(OID_OTHER));
        }
        // `.git` dir exists but has no HEAD — must not panic.
        "partial" => {
            mkdir(root, ".git");
        }
        // Symbolic HEAD whose ref is resolved from `packed-refs` (no loose ref).
        "packed-refs-only" => {
            write(root, ".git/HEAD", "ref: refs/heads/main\n");
            write(
                root,
                ".git/packed-refs",
                &format!(
                    "# pack-refs with: peeled fully-peeled sorted\n{OID_PACKED} refs/heads/main\n"
                ),
            );
        }
        // Linked worktree: `.git` is a gitfile; commondir points at the shared dir.
        "linked" => {
            write(root, ".git", "gitdir: main.git/worktrees/feature\n");
            write(
                root,
                "main.git/worktrees/feature/HEAD",
                "ref: refs/heads/feature\n",
            );
            write(root, "main.git/worktrees/feature/commondir", "../..\n");
            write(root, "main.git/refs/heads/feature", &line(OID_FEATURE));
        }
        other => panic!("unknown vcs fixture: {other}"),
    }
    tmp
}

fn line(oid: &str) -> String {
    format!("{oid}\n")
}

fn mkdir(root: &Path, rel: &str) {
    fs::create_dir_all(root.join(rel)).expect("create fixture dir");
}

fn write(root: &Path, rel: &str, contents: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create fixture parent dir");
    }
    fs::write(path, contents).expect("write fixture file");
}
