use cairn_types::{OperationState, Timestamp, WorktreeId};
use cairn_vcs::capture;
use std::path::Path;
use std::process::{Command, Output};

fn git(root: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap_or_else(|error| panic!("run git {args:?}: {error}"))
}

fn git_ok(root: &Path, args: &[&str]) {
    let output = git(root, args);
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn repo_epoch_tracks_normal_detached_merge_and_recovery_in_one_worktree() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    git_ok(root, &["init", "-b", "main"]);
    git_ok(root, &["config", "user.name", "Cairn Test"]);
    git_ok(root, &["config", "user.email", "cairn@example.invalid"]);
    std::fs::write(root.join("tracked.txt"), "base\n").unwrap();
    git_ok(root, &["add", "tracked.txt"]);
    git_ok(root, &["commit", "-m", "base"]);

    let worktree_id = WorktreeId::new("actual-worktree");
    let normal = capture(root, worktree_id.clone(), None, Timestamp(1)).unwrap();
    assert_eq!(normal.operation_state, OperationState::Normal);
    assert_eq!(normal.branch_ref.as_deref(), Some("refs/heads/main"));

    git_ok(root, &["checkout", "--detach", "HEAD"]);
    let detached = capture(root, worktree_id.clone(), None, Timestamp(2)).unwrap();
    assert_eq!(detached.operation_state, OperationState::DetachedHead);
    assert_eq!(detached.head_oid, normal.head_oid);
    assert!(detached.branch_ref.is_none());
    assert_ne!(detached.repo_epoch_id, normal.repo_epoch_id);

    git_ok(root, &["checkout", "main"]);
    git_ok(root, &["checkout", "-b", "conflict"]);
    std::fs::write(root.join("tracked.txt"), "conflict branch\n").unwrap();
    git_ok(root, &["commit", "-am", "conflict branch"]);
    git_ok(root, &["checkout", "main"]);
    std::fs::write(root.join("tracked.txt"), "main branch\n").unwrap();
    git_ok(root, &["commit", "-am", "main branch"]);

    let before_merge = capture(root, worktree_id.clone(), None, Timestamp(3)).unwrap();
    let merge = git(root, &["merge", "conflict"]);
    assert!(!merge.status.success(), "merge fixture must conflict");
    let merging = capture(root, worktree_id.clone(), None, Timestamp(4)).unwrap();
    assert_eq!(merging.operation_state, OperationState::MergeInProgress);
    assert_eq!(merging.head_oid, before_merge.head_oid);
    assert_ne!(merging.repo_epoch_id, before_merge.repo_epoch_id);
    assert_ne!(
        merging.working_tree_digest,
        before_merge.working_tree_digest
    );

    git_ok(root, &["merge", "--abort"]);
    let recovered = capture(root, worktree_id, None, Timestamp(5)).unwrap();
    assert_eq!(recovered.operation_state, OperationState::Normal);
    assert_eq!(recovered.head_oid, before_merge.head_oid);
    assert_eq!(recovered.repo_epoch_id, before_merge.repo_epoch_id);
    assert_eq!(
        recovered.working_tree_digest,
        before_merge.working_tree_digest
    );
}
