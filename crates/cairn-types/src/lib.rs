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

use std::path::PathBuf;

/// Defines a string-newtype ID with a uniform constructor and accessor.
///
/// The inner representation is an implementation detail Task 2 may refine (for
/// example to a fixed-size byte array); the public type *name* is the frozen
/// contract that consumer crates build against.
macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentHash(String);

impl ContentHash {
    /// Wraps a precomputed lowercase-hex BLAKE3 digest.
    pub fn from_hex(hex: impl Into<String>) -> Self {
        Self(hex.into())
    }

    /// Borrows the lowercase-hex digest.
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

/// Deterministic hash of the effective Cairn configuration. Computed by
/// `cairn-config`, embedded in worktree identity by `cairn-identity`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConfigHash(String);

impl ConfigHash {
    /// Wraps a precomputed lowercase-hex config digest.
    pub fn from_hex(hex: impl Into<String>) -> Self {
        Self(hex.into())
    }

    /// Borrows the lowercase-hex digest.
    pub fn as_hex(&self) -> &str {
        &self.0
    }
}

/// Daemon ↔ adapter wire-protocol version. Pinned by `cairn-config`, part of
/// worktree identity, and checked at adapter registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProtocolVersion(pub u32);

/// A wall-clock observation timestamp. The inner unit (Task 2 finalizes) is
/// nanoseconds since the Unix epoch, UTC. This is recorded for diagnostics and
/// ordering — it MUST NOT drive freshness decisions, which are content-hash
/// based (see `cairn-file`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(pub i64);

/// Git operation state at the moment a [`RepoEpoch`] is captured (spec Appendix B).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Captured git operation-state snapshot for a worktree (spec Appendix B).
///
/// `head_oid`, `branch_ref`, and `index_tree_oid` are raw git object identifiers
/// (hex), present only when the working tree is in a git repo and the value is
/// available. `working_tree_digest` is a Cairn [`ContentHash`].
#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
}
