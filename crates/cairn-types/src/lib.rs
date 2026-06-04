//! `cairn-types` — shared domain types and typed IDs for the Cairn daemon.
//!
//! # Scope: Phase 1 identity substrate ONLY
//!
//! This crate is the workspace's type floor. Wave 1.1 Tasks 3–6 (`cairn-config`,
//! `cairn-identity`, `cairn-file`, `cairn-vcs`) **consume** the types defined here
//! rather than redefining their own. The names and shapes below are the *frozen
//! contract*: the orchestrator commits them before the Wave 1.1 fan-out so six
//! parallel workers cannot diverge on what (for example) `SourceClass` or
//! `RepoEpochId` mean. Wave 1.1 Task 2 (the `cairn-types` owner) fleshes out
//! serde derives, `Display`, validation, and round-trip tests, and may refine an
//! inner representation — but does not rename or reshape these public items.
//!
//! # Deferred — do NOT author these in Phase 1
//!
//! The following spec Appendix B entities belong to later phases. Introducing
//! them here would pull downstream concepts into the identity floor:
//!
//! - `Observation` / `InheritedObservation` (Phase 2 ledgers). Note these carry a
//!   `graph_version` field — `GraphVersion` is a **Phase 4 / `cairn-graph`**
//!   concept and must not be defined in `cairn-types` during Phase 1.
//! - `ContextFrame`, `Task`, `CompactionCheckpoint`, `PreCompactSurvivalPacket`
//!   (Phase 3 context).
//! - `SurfaceItem` (Phase 4 graph).
//! - `DaemonDecision`, `DenyDecision`, `OverrideDeny` (Phase 5 decisions).
//! - The full `DaemonEvent` envelope structs (Wave 1.2 `cairn-protocol`). Only the
//!   event-*kind* discriminant ([`DaemonEventKind`]) lives here.

use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Defines a string-newtype ID with a uniform constructor and accessor.
///
/// The inner representation is an implementation detail Task 2 may refine (for
/// example to a fixed-size byte array); the public type *name* is the frozen
/// contract that consumer crates build against.
macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            /// Wraps a raw identifier value.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Borrows the raw identifier value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

id_type! {
    /// Stable identity of a file within a worktree.
    FileId
}
id_type! {
    /// Canonical worktree identity (derived from canonical root + git common dir).
    WorktreeId
}
id_type! {
    /// Identity of a repo epoch: a captured git operation-state snapshot.
    RepoEpochId
}
id_type! {
    /// Identity of the git common directory shared across linked worktrees.
    GitCommonDirId
}
id_type! {
    /// Identity of one agent session.
    SessionId
}

/// A content digest. Cairn content-addresses file contents with **BLAKE3** —
/// fast on the read/edit hot path, cryptographic, and the algorithm the wider
/// agent-tooling substrate already standardizes on. Stored here as the lowercase
/// hex digest; Task 2 may switch the inner representation to a `[u8; 32]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct ContentHash(String);

impl ContentHash {
    /// Wraps a precomputed lowercase-hex BLAKE3 digest. For trusted internal
    /// construction from a freshly computed digest; deserialization from untrusted
    /// input is validated via [`TryFrom<String>`].
    pub fn from_hex(hex: impl Into<String>) -> Self {
        Self(hex.into())
    }

    /// Borrows the lowercase-hex digest.
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ContentHash {
    type Error = TypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if is_blake3_hex(&value) {
            Ok(Self(value))
        } else {
            Err(TypeError::InvalidContentHash(value))
        }
    }
}

/// Deterministic hash of the effective Cairn configuration. Computed by
/// `cairn-config`, embedded in worktree identity by `cairn-identity`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String")]
pub struct ConfigHash(String);

impl ConfigHash {
    /// Wraps a precomputed lowercase-hex config digest. For trusted internal
    /// construction; deserialization from untrusted input is validated via
    /// [`TryFrom<String>`].
    pub fn from_hex(hex: impl Into<String>) -> Self {
        Self(hex.into())
    }

    /// Borrows the lowercase-hex digest.
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ConfigHash {
    type Error = TypeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if is_blake3_hex(&value) {
            Ok(Self(value))
        } else {
            Err(TypeError::InvalidConfigHash(value))
        }
    }
}

/// A BLAKE3 digest is exactly 64 lowercase-hex characters (32 bytes). Used to
/// validate [`ContentHash`] / [`ConfigHash`] deserialized from untrusted input.
fn is_blake3_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Daemon ↔ adapter wire-protocol version. Pinned by `cairn-config`, part of
/// worktree identity, and checked at adapter registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProtocolVersion(pub u32);

/// A wall-clock observation timestamp. The inner unit (Task 2 finalizes) is
/// nanoseconds since the Unix epoch, UTC. This is recorded for diagnostics and
/// ordering — it MUST NOT drive freshness decisions, which are content-hash
/// based (see `cairn-file`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(pub i64);

impl Timestamp {
    /// Returns the current wall-clock timestamp as nanoseconds since the Unix epoch.
    #[must_use]
    pub fn now() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let clamped = i64::try_from(nanos).unwrap_or(i64::MAX);
        Self(clamped)
    }
}

// Serialized as a decimal STRING, not a JSON number: nanosecond timestamps exceed
// JavaScript's safe-integer range (2^53), so a numeric wire form would silently
// lose precision in the TypeScript/JS adapter. A string is exact across languages.
impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse::<i64>()
            .map(Timestamp)
            .map_err(serde::de::Error::custom)
    }
}

/// Git operation state at the moment a [`RepoEpoch`] is captured (spec Appendix B).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Normal,
    DetachedHead,
    MergeInProgress,
    RebaseInProgress,
    CherryPickInProgress,
    BisectInProgress,
    UnknownVcs,
}

/// Coarse classification of a file's role (spec Appendix B). Drives later
/// extraction and decoration policy; the initial heuristics live in `cairn-file`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceClass {
    Source,
    Test,
    Generated,
    Vendored,
    BuildArtifact,
    Config,
    Lockfile,
    Migration,
    Fixture,
    Unknown,
}

/// Content-addressed snapshot of a file at an observed version (spec Appendix B).
///
/// `content_hash` is the source of truth for freshness. `mtime_observed` is
/// recorded for diagnostics only and must not drive freshness decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVersion {
    pub file_id: FileId,
    pub path: PathBuf,
    pub content_hash: ContentHash,
    pub size: u64,
    pub mtime_observed: Timestamp,
    pub executable_bit: bool,
    pub symlink_target: Option<PathBuf>,
    pub repo_epoch_id: RepoEpochId,
    pub source_class: SourceClass,
}

// Equality deliberately EXCLUDES `mtime_observed`. It is recorded for diagnostics
// only; freshness and version identity are content-hash based and must never be
// time based (the product's core discipline). Two observations of the same file
// that differ only in mtime are equal.
impl PartialEq for FileVersion {
    fn eq(&self, other: &Self) -> bool {
        self.file_id == other.file_id
            && self.path == other.path
            && self.content_hash == other.content_hash
            && self.size == other.size
            && self.executable_bit == other.executable_bit
            && self.symlink_target == other.symlink_target
            && self.repo_epoch_id == other.repo_epoch_id
            && self.source_class == other.source_class
    }
}

impl Eq for FileVersion {}

/// Captured git operation-state snapshot for a worktree (spec Appendix B).
///
/// `head_oid`, `branch_ref`, and `index_tree_oid` are raw git object identifiers
/// (hex), present only when the working tree is in a git repo and the value is
/// available. `working_tree_digest` is a Cairn [`ContentHash`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoEpoch {
    pub repo_epoch_id: RepoEpochId,
    pub worktree_id: WorktreeId,
    pub git_common_dir_id: Option<GitCommonDirId>,
    pub head_oid: Option<String>,
    pub branch_ref: Option<String>,
    pub index_tree_oid: Option<String>,
    pub working_tree_digest: ContentHash,
    pub operation_state: OperationState,
    pub started_at: Timestamp,
}

/// What an adapter can do, registered with the daemon (spec Appendix B bitset).
///
/// Modeled as named booleans (serde-trivial, self-documenting); the compact wire
/// encoding is Task 2's call. Phase 1 only records these — the enforcement paths
/// that read them arrive in Phase 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AdapterCapabilities {
    pub can_pre_edit_block: bool,
    pub can_pre_read_decorate: bool,
    pub can_post_read_decorate: bool,
    pub can_command_replace: bool,
    pub can_modify_tool_input: bool,
    pub can_async_notify: bool,
    pub can_precompact: bool,
    pub can_report_token_usage: bool,
    pub can_report_exact_edit_diff: bool,
    pub can_attach_file_precondition: bool,
}

/// Discriminant of a daemon event kind (spec Appendix B). The full envelope
/// structs are authored in Wave 1.2 `cairn-protocol`; only this kind tag is the
/// shared `cairn-types` contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonEventKind {
    SessionStart,
    SessionEnd,
    ToolIntent,
    ToolResult,
    ReadObserved,
    EditIntent,
    EditApplied,
    CommandIntent,
    CommandResult,
    CompactIntent,
    VcsStateChanged,
    AdapterHeartbeat,
    CapabilityRegistration,
}

/// Error type for `cairn-types` validation failures.
#[derive(Debug, Error)]
pub enum TypeError {
    /// Input is not 64 lowercase-hex characters (a 32-byte BLAKE3 digest).
    #[error("invalid content hash: {0}")]
    InvalidContentHash(String),

    /// Input is not 64 lowercase-hex characters (a 32-byte BLAKE3 digest).
    #[error("invalid config hash: {0}")]
    InvalidConfigHash(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_roundtrip() {
        let h = ContentHash::from_hex(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );
        let json = serde_json::to_string(&h).unwrap();
        let back: ContentHash = serde_json::from_str(&json).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn config_hash_roundtrip() {
        let h = ConfigHash::from_hex(
            "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
        );
        let json = serde_json::to_string(&h).unwrap();
        let back: ConfigHash = serde_json::from_str(&json).unwrap();
        assert_eq!(h, back);
    }

    #[test]
    fn protocol_version_roundtrip() {
        let v = ProtocolVersion(7);
        let json = serde_json::to_string(&v).unwrap();
        let back: ProtocolVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn timestamp_roundtrip() {
        let ts = Timestamp(1_700_000_000_000_000_000);
        let json = serde_json::to_string(&ts).unwrap();
        let back: Timestamp = serde_json::from_str(&json).unwrap();
        assert_eq!(ts, back);
    }

    #[test]
    fn file_id_roundtrip() {
        let id = FileId::new("file-42");
        let json = serde_json::to_string(&id).unwrap();
        let back: FileId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn worktree_id_roundtrip() {
        let id = WorktreeId::new("wt-1");
        let json = serde_json::to_string(&id).unwrap();
        let back: WorktreeId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn repo_epoch_id_roundtrip() {
        let id = RepoEpochId::new("epoch-9");
        let json = serde_json::to_string(&id).unwrap();
        let back: RepoEpochId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn git_common_dir_id_roundtrip() {
        let id = GitCommonDirId::new("git-dir");
        let json = serde_json::to_string(&id).unwrap();
        let back: GitCommonDirId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn session_id_roundtrip() {
        let id = SessionId::new("sess-xyz");
        let json = serde_json::to_string(&id).unwrap();
        let back: SessionId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn operation_state_snake_case() {
        assert_eq!(
            serde_json::to_string(&OperationState::DetachedHead).unwrap(),
            "\"detached_head\""
        );
        assert_eq!(
            serde_json::to_string(&OperationState::UnknownVcs).unwrap(),
            "\"unknown_vcs\""
        );
    }

    #[test]
    fn source_class_snake_case() {
        assert_eq!(
            serde_json::to_string(&SourceClass::BuildArtifact).unwrap(),
            "\"build_artifact\""
        );
        assert_eq!(
            serde_json::to_string(&SourceClass::Lockfile).unwrap(),
            "\"lockfile\""
        );
    }

    #[test]
    fn daemon_event_kind_snake_case() {
        assert_eq!(
            serde_json::to_string(&DaemonEventKind::SessionStart).unwrap(),
            "\"session_start\""
        );
        assert_eq!(
            serde_json::to_string(&DaemonEventKind::VcsStateChanged).unwrap(),
            "\"vcs_state_changed\""
        );
        assert_eq!(
            serde_json::to_string(&DaemonEventKind::AdapterHeartbeat).unwrap(),
            "\"adapter_heartbeat\""
        );
    }

    #[test]
    fn file_version_roundtrip() {
        let fv = FileVersion {
            file_id: FileId::new("f1"),
            path: PathBuf::from("src/lib.rs"),
            content_hash: ContentHash::from_hex(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ),
            size: 1234,
            mtime_observed: Timestamp(1_700_000_000_000_000_000),
            executable_bit: false,
            symlink_target: None,
            repo_epoch_id: RepoEpochId::new("e1"),
            source_class: SourceClass::Source,
        };
        let json = serde_json::to_string(&fv).unwrap();
        let back: FileVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(fv, back);
    }

    #[test]
    fn repo_epoch_roundtrip() {
        let re = RepoEpoch {
            repo_epoch_id: RepoEpochId::new("re1"),
            worktree_id: WorktreeId::new("wt1"),
            git_common_dir_id: Some(GitCommonDirId::new("g1")),
            head_oid: Some("deadbeef".into()),
            branch_ref: Some("refs/heads/main".into()),
            index_tree_oid: None,
            working_tree_digest: ContentHash::from_hex(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ),
            operation_state: OperationState::Normal,
            started_at: Timestamp(1_700_000_000_000_000_000),
        };
        let json = serde_json::to_string(&re).unwrap();
        let back: RepoEpoch = serde_json::from_str(&json).unwrap();
        assert_eq!(re, back);
    }

    #[test]
    fn adapter_capabilities_roundtrip() {
        let caps = AdapterCapabilities {
            can_pre_edit_block: true,
            can_pre_read_decorate: false,
            can_post_read_decorate: true,
            can_command_replace: false,
            can_modify_tool_input: true,
            can_async_notify: false,
            can_precompact: true,
            can_report_token_usage: false,
            can_report_exact_edit_diff: true,
            can_attach_file_precondition: false,
        };
        let json = serde_json::to_string(&caps).unwrap();
        let back: AdapterCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, back);
    }

    #[test]
    fn timestamp_serializes_as_string_for_js_safety() {
        // Must be a JSON string, not a number: nanosecond values exceed JS's
        // safe-integer range and would silently lose precision as a number.
        let ts = Timestamp(1_700_000_000_000_000_000);
        assert_eq!(
            serde_json::to_string(&ts).unwrap(),
            "\"1700000000000000000\""
        );
        let back: Timestamp = serde_json::from_str("\"1700000000000000000\"").unwrap();
        assert_eq!(back, ts);
    }

    #[test]
    fn content_and_config_hash_deserialization_validates() {
        let ok = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert!(serde_json::from_str::<ContentHash>(&format!("\"{ok}\"")).is_ok());
        // Wrong length, uppercase, and non-hex are rejected.
        assert!(serde_json::from_str::<ContentHash>("\"abc\"").is_err());
        assert!(
            serde_json::from_str::<ContentHash>(&format!("\"{}\"", ok.to_uppercase())).is_err()
        );
        assert!(serde_json::from_str::<ConfigHash>("\"not-a-hash\"").is_err());
    }

    #[test]
    fn file_version_equality_ignores_mtime() {
        let base = FileVersion {
            file_id: FileId::new("f1"),
            path: "src/lib.rs".into(),
            content_hash: ContentHash::from_hex(
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ),
            size: 42,
            mtime_observed: Timestamp(1),
            executable_bit: false,
            symlink_target: None,
            repo_epoch_id: RepoEpochId::new("re1"),
            source_class: SourceClass::Source,
        };
        let mut later = base.clone();
        later.mtime_observed = Timestamp(999_999);
        assert_eq!(base, later, "mtime must not affect FileVersion equality");

        let mut changed = base.clone();
        changed.content_hash = ContentHash::from_hex(
            "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
        );
        assert_ne!(
            base, changed,
            "a content-hash change must make versions unequal"
        );
    }
}
