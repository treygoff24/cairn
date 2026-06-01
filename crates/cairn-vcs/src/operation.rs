//! Map marker files to [`OperationState`](cairn_types::OperationState).

use cairn_types::OperationState;

use crate::gitdir::{GitDirs, marker_present};
use crate::head::HeadState;

/// Operation precedence: in-progress states beat detached/normal.
pub(crate) fn detect_operation_state(dirs: &GitDirs, head: &HeadState) -> OperationState {
    if marker_present(&dirs.git_dir, "MERGE_HEAD") || marker_present(&dirs.common_dir, "MERGE_HEAD")
    {
        return OperationState::MergeInProgress;
    }

    for name in ["rebase-merge", "rebase-apply"] {
        if marker_present(&dirs.git_dir, name) || marker_present(&dirs.common_dir, name) {
            return OperationState::RebaseInProgress;
        }
    }

    if marker_present(&dirs.git_dir, "CHERRY_PICK_HEAD")
        || marker_present(&dirs.common_dir, "CHERRY_PICK_HEAD")
    {
        return OperationState::CherryPickInProgress;
    }

    if marker_present(&dirs.git_dir, "BISECT_LOG") || marker_present(&dirs.common_dir, "BISECT_LOG")
    {
        return OperationState::BisectInProgress;
    }

    match head {
        HeadState::Detached { .. } => OperationState::DetachedHead,
        HeadState::Symbolic { .. } | HeadState::Missing => OperationState::Normal,
    }
}
