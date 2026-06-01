//! Deterministic BLAKE3 digests for epoch identity and change detection.

use cairn_types::{ContentHash, OperationState, RepoEpochId, WorktreeId};

/// Stable tag for hashing — must stay stable across releases.
pub(crate) fn operation_state_tag(state: OperationState) -> &'static str {
    match state {
        OperationState::Normal => "normal",
        OperationState::DetachedHead => "detached_head",
        OperationState::MergeInProgress => "merge_in_progress",
        OperationState::RebaseInProgress => "rebase_in_progress",
        OperationState::CherryPickInProgress => "cherry_pick_in_progress",
        OperationState::BisectInProgress => "bisect_in_progress",
        OperationState::UnknownVcs => "unknown_vcs",
    }
}

/// `working_tree_digest` inputs (documented for downstream freshness logic):
///
/// - `head_oid` (empty string when absent)
/// - `operation_state` tag
/// - sorted marker names present under git metadata dirs
pub(crate) fn working_tree_digest(
    head_oid: Option<&str>,
    operation_state: OperationState,
    markers: &[&str],
) -> ContentHash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"working_tree_digest/v1\0");
    hasher.update(head_oid.unwrap_or("").as_bytes());
    hasher.update(b"\0");
    hasher.update(operation_state_tag(operation_state).as_bytes());
    hasher.update(b"\0");
    for marker in markers {
        hasher.update(marker.as_bytes());
        hasher.update(b"\0");
    }
    ContentHash::from_hex(hasher.finalize().to_hex().to_string())
}

/// `repo_epoch_id` = BLAKE3(`head_oid` + `operation_state` + `worktree_id`).
pub(crate) fn repo_epoch_id(
    head_oid: Option<&str>,
    operation_state: OperationState,
    worktree_id: &WorktreeId,
) -> RepoEpochId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"repo_epoch_id/v1\0");
    hasher.update(head_oid.unwrap_or("").as_bytes());
    hasher.update(b"\0");
    hasher.update(operation_state_tag(operation_state).as_bytes());
    hasher.update(b"\0");
    hasher.update(worktree_id.as_str().as_bytes());
    RepoEpochId::new(hasher.finalize().to_hex().to_string())
}
