use cairn_identity::{
    IdentityError, PathCaseSemantics, WorktreeIdentity, detect_path_case_semantics,
};
use cairn_types::{ConfigHash, ProtocolVersion};
use std::fs;
use std::path::Path;

struct TempTree(tempfile::TempDir);

impl TempTree {
    fn new(label: &str) -> Self {
        Self(
            tempfile::Builder::new()
                .prefix(&format!("cairn-identity-adversarial-{label}"))
                .tempdir()
                .expect("create temporary identity fixture"),
        )
    }

    fn path(&self) -> &Path {
        self.0.path()
    }
}

fn identity(root: impl AsRef<Path>) -> Result<WorktreeIdentity, IdentityError> {
    WorktreeIdentity::compute(
        root,
        ConfigHash::from_hex("1111111111111111111111111111111111111111111111111111111111111111"),
        ProtocolVersion(1),
    )
}

#[test]
fn malformed_git_file_is_rejected() {
    let fixture = TempTree::new("malformed-git");
    fs::write(fixture.path().join(".git"), "not a gitdir pointer\n").unwrap();

    assert!(matches!(
        identity(fixture.path()),
        Err(IdentityError::MalformedGitFile { .. })
    ));
}

#[test]
fn empty_commondir_is_rejected() {
    let fixture = TempTree::new("empty-commondir");
    let git_dir = fixture.path().join("gitdirs/worktree");
    let worktree = fixture.path().join("worktree");
    fs::create_dir_all(&git_dir).unwrap();
    fs::create_dir_all(&worktree).unwrap();
    fs::write(worktree.join(".git"), "gitdir: ../gitdirs/worktree\n").unwrap();
    fs::write(git_dir.join("commondir"), " \n").unwrap();

    assert!(matches!(
        identity(&worktree),
        Err(IdentityError::EmptyGitMetadataPath { .. })
    ));
}

#[cfg(unix)]
#[test]
fn non_utf8_git_metadata_is_reported_as_a_read_error() {
    let fixture = TempTree::new("non-utf8-metadata");
    fs::write(fixture.path().join(".git"), b"gitdir: \xff\n").unwrap();

    let error = identity(fixture.path()).expect_err("non-UTF-8 metadata must not be lossy");
    match error {
        IdentityError::ReadGitMetadataFile { source, .. } => {
            assert_eq!(source.kind(), std::io::ErrorKind::InvalidData);
        }
        other => panic!("expected metadata read error, got {other:?}"),
    }
}

#[test]
fn lexical_root_aliases_share_identity() {
    let fixture = TempTree::new("lexical-alias");
    let root = fixture.path().join("worktree");
    fs::create_dir_all(root.join("child")).unwrap();
    let alias = root.join("child/..");

    assert_eq!(identity(&root).unwrap(), identity(alias).unwrap());
}

#[cfg(unix)]
#[test]
fn symlinked_root_alias_shares_identity() {
    let fixture = TempTree::new("symlink-alias");
    let root = fixture.path().join("worktree");
    let alias = fixture.path().join("alias");
    fs::create_dir(&root).unwrap();
    std::os::unix::fs::symlink(&root, &alias).unwrap();

    assert_eq!(identity(&root).unwrap(), identity(alias).unwrap());
}

#[test]
fn case_variants_follow_detected_filesystem_semantics() {
    let fixture = TempTree::new("case");
    let mixed_case = fixture.path().join("CaseRoot");
    let lower_case = fixture.path().join("caseroot");
    fs::create_dir(&mixed_case).unwrap();

    match detect_path_case_semantics(&mixed_case).unwrap() {
        PathCaseSemantics::CaseInsensitive => {
            assert_eq!(
                identity(&mixed_case).unwrap(),
                identity(&lower_case).unwrap()
            );
        }
        PathCaseSemantics::CaseSensitive | PathCaseSemantics::Unknown => {
            fs::create_dir(&lower_case).unwrap();
            assert_ne!(
                identity(&mixed_case).unwrap().worktree_id,
                identity(&lower_case).unwrap().worktree_id
            );
        }
    }
}

#[test]
fn sibling_worktrees_share_common_repo_identity_but_not_worktree_identity() {
    let fixture = TempTree::new("siblings");
    let common = fixture.path().join("common.git");
    let git_dir_a = fixture.path().join("gitdirs/a");
    let git_dir_b = fixture.path().join("gitdirs/b");
    let worktree_a = fixture.path().join("worktrees/a");
    let worktree_b = fixture.path().join("worktrees/b");
    for dir in [&common, &git_dir_a, &git_dir_b, &worktree_a, &worktree_b] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(git_dir_a.join("commondir"), "../../common.git\n").unwrap();
    fs::write(git_dir_b.join("commondir"), "../../common.git\n").unwrap();
    fs::write(worktree_a.join(".git"), "gitdir: ../../gitdirs/a\n").unwrap();
    fs::write(worktree_b.join(".git"), "gitdir: ../../gitdirs/b\n").unwrap();

    let a = identity(&worktree_a).unwrap();
    let b = identity(&worktree_b).unwrap();

    assert_eq!(a.git_common_dir, b.git_common_dir);
    assert_eq!(a.git_common_dir_id, b.git_common_dir_id);
    assert_ne!(a.worktree_id, b.worktree_id);
}
