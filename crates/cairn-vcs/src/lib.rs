//! `cairn-vcs` — `RepoEpoch`: git operation-state capture.
//!
//! Reads `.git` metadata directly (no `git` subprocess, no libgit2). Suitable for the
//! daemon hot path. Git object/index parsing is intentionally out of scope for Phase 1.

mod digest;
mod error;
mod gitdir;
mod head;
mod operation;

#[cfg(test)]
mod test_fixtures;

pub use error::VcsError;

use std::path::Path;

use cairn_types::{GitCommonDirId, OperationState, RepoEpoch, Timestamp, WorktreeId};

use digest::{repo_epoch_id, working_tree_digest};
use gitdir::{present_markers, resolve_git_dirs};
use head::{HeadState, read_head};
use operation::detect_operation_state;

/// Captures the git operation-state snapshot for a worktree.
///
/// `worktree_id` and `git_common_dir_id` are supplied by `cairn-identity` (not computed
/// here). When `.git` is missing or not a repository, returns `Ok` with
/// [`OperationState::UnknownVcs`] and `head_oid` / `branch_ref` set to `None`.
///
/// `index_tree_oid` is always `None` in Phase 1 (binary index parsing requires a git library).
pub fn capture(
    worktree_root: &Path,
    worktree_id: WorktreeId,
    git_common_dir_id: Option<GitCommonDirId>,
    started_at: Timestamp,
) -> Result<RepoEpoch, VcsError> {
    let Some(dirs) = resolve_git_dirs(worktree_root)? else {
        return Ok(unknown_vcs_epoch(
            worktree_id,
            git_common_dir_id,
            started_at,
        ));
    };

    let head = read_head(&dirs)?;
    let operation_state = detect_operation_state(&dirs, &head);
    let markers: Vec<&str> = present_markers(&dirs.git_dir, &dirs.common_dir);

    let (head_oid, branch_ref) = head_fields(&head);
    let working_tree_digest = working_tree_digest(head_oid.as_deref(), operation_state, &markers);
    let repo_epoch_id = repo_epoch_id(head_oid.as_deref(), operation_state, &worktree_id);

    Ok(RepoEpoch {
        repo_epoch_id,
        worktree_id,
        git_common_dir_id,
        head_oid,
        branch_ref,
        index_tree_oid: None,
        working_tree_digest,
        operation_state,
        started_at,
    })
}

fn head_fields(head: &HeadState) -> (Option<String>, Option<String>) {
    match head {
        HeadState::Symbolic {
            branch_ref,
            head_oid,
        } => (head_oid.clone(), Some(branch_ref.clone())),
        HeadState::Detached { head_oid } => (Some(head_oid.clone()), None),
        HeadState::Missing => (None, None),
    }
}

fn unknown_vcs_epoch(
    worktree_id: WorktreeId,
    _git_common_dir_id: Option<GitCommonDirId>,
    started_at: Timestamp,
) -> RepoEpoch {
    let operation_state = OperationState::UnknownVcs;
    let markers: &[&str] = &[];
    let working_tree_digest = working_tree_digest(None, operation_state, markers);
    let repo_epoch_id = repo_epoch_id(None, operation_state, &worktree_id);

    RepoEpoch {
        repo_epoch_id,
        worktree_id,
        git_common_dir_id: None,
        head_oid: None,
        branch_ref: None,
        index_tree_oid: None,
        working_tree_digest,
        operation_state,
        started_at,
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    use cairn_types::OperationState;

    use crate::test_fixtures::build_fixture;

    fn capture_fixture(name: &str) -> RepoEpoch {
        // The tempdir stays alive until after `capture` finishes reading it.
        let tmp = build_fixture(name);
        capture(
            tmp.path(),
            WorktreeId::new(format!("wt-{name}")),
            Some(GitCommonDirId::new("gcd-test")),
            Timestamp(1),
        )
        .expect("capture must not error")
    }

    #[test]
    fn not_a_repo_is_unknown_vcs_without_panic() {
        let epoch = capture_fixture("not-a-repo");
        assert_eq!(epoch.operation_state, OperationState::UnknownVcs);
        assert!(epoch.head_oid.is_none());
        assert!(epoch.branch_ref.is_none());
        assert!(epoch.git_common_dir_id.is_none());
    }

    #[test]
    fn normal_repo() {
        let epoch = capture_fixture("normal");
        assert_eq!(epoch.operation_state, OperationState::Normal);
        assert_eq!(
            epoch.head_oid.as_deref(),
            Some("1111111111111111111111111111111111111111")
        );
        assert_eq!(epoch.branch_ref.as_deref(), Some("refs/heads/main"));
        assert!(epoch.index_tree_oid.is_none());
    }

    #[test]
    fn detached_head() {
        let epoch = capture_fixture("detached");
        assert_eq!(epoch.operation_state, OperationState::DetachedHead);
        assert!(epoch.branch_ref.is_none());
        assert!(epoch.head_oid.is_some());
    }

    #[test]
    fn merge_in_progress() {
        let epoch = capture_fixture("merge");
        assert_eq!(epoch.operation_state, OperationState::MergeInProgress);
    }

    #[test]
    fn rebase_in_progress() {
        let epoch = capture_fixture("rebase-merge");
        assert_eq!(epoch.operation_state, OperationState::RebaseInProgress);
    }

    #[test]
    fn rebase_apply_in_progress() {
        let epoch = capture_fixture("rebase-apply");
        assert_eq!(epoch.operation_state, OperationState::RebaseInProgress);
    }

    #[test]
    fn cherry_pick_in_progress() {
        let epoch = capture_fixture("cherry-pick");
        assert_eq!(epoch.operation_state, OperationState::CherryPickInProgress);
    }

    #[test]
    fn bisect_in_progress() {
        let epoch = capture_fixture("bisect");
        assert_eq!(epoch.operation_state, OperationState::BisectInProgress);
    }

    #[test]
    fn merge_beats_detached_head_marker() {
        let epoch = capture_fixture("merge-over-detached-head");
        assert_eq!(epoch.operation_state, OperationState::MergeInProgress);
    }

    #[test]
    fn linked_worktree_resolves_head() {
        let epoch = capture_fixture("linked");
        assert_eq!(epoch.operation_state, OperationState::Normal);
        assert_eq!(
            epoch.head_oid.as_deref(),
            Some("cccccccccccccccccccccccccccccccccccccccc")
        );
    }

    #[test]
    fn partial_git_dir_does_not_panic() {
        let epoch = capture_fixture("partial");
        assert_eq!(epoch.operation_state, OperationState::Normal);
        assert!(epoch.head_oid.is_none());
        assert!(epoch.branch_ref.is_none());
    }

    #[test]
    fn digest_changes_when_markers_change() {
        let normal = capture_fixture("normal");
        let merge = capture_fixture("merge");
        assert_ne!(normal.working_tree_digest, merge.working_tree_digest);
        assert_ne!(normal.repo_epoch_id, merge.repo_epoch_id);
    }

    #[test]
    fn unknown_vcs_still_has_content_hash() {
        let epoch = capture_fixture("not-a-repo");
        assert!(!epoch.working_tree_digest.as_hex().is_empty());
    }
}
