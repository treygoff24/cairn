//! `cairn-protocol` — daemon wire protocol event envelopes and decisions.
//!
//! # Scope: Wave 1.2 daemon contract
//!
//! Adapters translate harness-specific tool calls into these lowest-common-
//! denominator events. The daemon replies with [`DaemonDecision`], computing the
//! strongest behavior the registered [`AdapterCapabilities`] support. This crate
//! intentionally stays synchronous and runtime-free so it can sit on the hook hot
//! path.

use std::path::PathBuf;

use cairn_types::{
    AdapterCapabilities, DaemonEventKind, FileId, FileVersion, ProtocolVersion, RepoEpoch,
    RepoEpochId, SessionId, Timestamp, WorktreeId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The wire-protocol version this build speaks.
pub const CURRENT_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion(1);

/// Versioned daemon event message for adapter/client wire transport.
///
/// The inner [`DaemonEvent`] enum remains the stable event contract consumed by
/// storage and in-process clients; this envelope is the explicit wire boundary
/// that lets future versions negotiate or reject incompatible messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonEventEnvelope {
    pub protocol_version: ProtocolVersion,
    pub event: DaemonEvent,
}

impl DaemonEventEnvelope {
    /// Wraps an event in the current Cairn protocol version.
    #[must_use]
    pub fn current(event: DaemonEvent) -> Self {
        Self {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            event,
        }
    }
}

/// Versioned daemon decision message for adapter/client wire transport.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonDecisionEnvelope {
    pub protocol_version: ProtocolVersion,
    pub decision: DaemonDecision,
}

impl DaemonDecisionEnvelope {
    /// Wraps a decision in the current Cairn protocol version.
    #[must_use]
    pub fn current(decision: DaemonDecision) -> Self {
        Self {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            decision,
        }
    }
}

/// Event emitted by an adapter or daemon on the daemon-internal hook protocol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum DaemonEvent {
    /// A new agent session attached to a worktree.
    SessionStart(SessionStart),
    /// An agent session ended or detached.
    SessionEnd(SessionEnd),
    /// A harness tool call is about to run.
    ToolIntent(ToolIntent),
    /// A harness tool call finished.
    ToolResult(ToolResult),
    /// The agent observed a file version.
    ReadObserved(ReadObserved),
    /// The agent intends to edit a file.
    EditIntent(EditIntent),
    /// An edit was applied and a new version may exist.
    EditApplied(EditApplied),
    /// A command is about to run.
    CommandIntent(CommandIntent),
    /// A command finished.
    CommandResult(CommandResult),
    /// The harness is about to compact context.
    CompactIntent(CompactIntent),
    /// The repository epoch or VCS operation state changed.
    VcsStateChanged(VcsStateChanged),
    /// Adapter liveness and capability report.
    AdapterHeartbeat(AdapterHeartbeat),
}

impl DaemonEvent {
    /// Stable discriminant used by storage and downstream materialized views.
    #[must_use]
    pub const fn kind(&self) -> DaemonEventKind {
        match self {
            Self::SessionStart(_) => DaemonEventKind::SessionStart,
            Self::SessionEnd(_) => DaemonEventKind::SessionEnd,
            Self::ToolIntent(_) => DaemonEventKind::ToolIntent,
            Self::ToolResult(_) => DaemonEventKind::ToolResult,
            Self::ReadObserved(_) => DaemonEventKind::ReadObserved,
            Self::EditIntent(_) => DaemonEventKind::EditIntent,
            Self::EditApplied(_) => DaemonEventKind::EditApplied,
            Self::CommandIntent(_) => DaemonEventKind::CommandIntent,
            Self::CommandResult(_) => DaemonEventKind::CommandResult,
            Self::CompactIntent(_) => DaemonEventKind::CompactIntent,
            Self::VcsStateChanged(_) => DaemonEventKind::VcsStateChanged,
            Self::AdapterHeartbeat(_) => DaemonEventKind::AdapterHeartbeat,
        }
    }

    /// Wraps this event in a current-version wire envelope.
    #[must_use]
    pub fn into_current_envelope(self) -> DaemonEventEnvelope {
        DaemonEventEnvelope::current(self)
    }
}

/// Shared identity fields for event payloads sent by adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterRef {
    pub adapter_id: String,
    pub adapter_kind: AdapterKind,
}

/// Harness family that produced an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    ClaudeCode,
    Codex,
    Cursor,
    Mcp,
    HarnessSim,
    Other,
}

/// A session started and registered its daemon-facing capability floor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionStart {
    pub agent_session_id: SessionId,
    pub root_session_id: SessionId,
    pub parent_session_id: Option<SessionId>,
    pub spawn_event_id: Option<String>,
    pub lineage_depth: u32,
    pub worktree_id: WorktreeId,
    pub harness: AdapterRef,
    pub capabilities: AdapterCapabilities,
    pub repo_epoch: Option<RepoEpoch>,
    pub started_at: Timestamp,
    pub task_id: Option<String>,
    pub task_summary: Option<String>,
    pub inherited_context_frame_ids: Vec<String>,
}

/// A session stopped producing events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionEnd {
    pub agent_session_id: SessionId,
    pub worktree_id: WorktreeId,
    pub harness: AdapterRef,
    pub ended_at: Timestamp,
    pub reason: SessionEndReason,
    pub token_usage: Option<TokenUsage>,
}

/// Why a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEndReason {
    Completed,
    Abandoned,
    Compacted,
    Crashed,
    AdapterDisconnected,
    Unknown,
}

/// A harness tool call is about to execute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolIntent {
    pub agent_session_id: SessionId,
    pub worktree_id: WorktreeId,
    pub harness: AdapterRef,
    pub tool_call_id: String,
    pub tool_name: String,
    pub input: Value,
    pub repo_epoch_id: Option<RepoEpochId>,
    pub occurred_at: Timestamp,
    pub token_usage: Option<TokenUsage>,
}

/// A harness tool call finished.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub agent_session_id: SessionId,
    pub worktree_id: WorktreeId,
    pub harness: AdapterRef,
    pub tool_call_id: String,
    pub tool_name: String,
    pub result: ToolResultPayload,
    pub repo_epoch_id: Option<RepoEpochId>,
    pub occurred_at: Timestamp,
    pub token_usage: Option<TokenUsage>,
}

/// Open-ended result body plus portable execution metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResultPayload {
    pub status: ToolStatus,
    pub output: Value,
    pub error: Option<String>,
    pub latency_ms: Option<u64>,
}

/// Portable tool or command outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
    Unknown,
}

/// The agent observed a concrete file version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadObserved {
    pub agent_session_id: SessionId,
    pub worktree_id: WorktreeId,
    pub harness: AdapterRef,
    pub tool_call_id: Option<String>,
    pub file_version: FileVersion,
    pub observed_at: Timestamp,
    pub context_frame_id: Option<String>,
    pub task_id: Option<String>,
}

/// The agent intends to edit a target file and may provide an expected version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditIntent {
    pub agent_session_id: SessionId,
    pub worktree_id: WorktreeId,
    pub harness: AdapterRef,
    pub tool_call_id: String,
    pub file_id: Option<FileId>,
    pub path: PathBuf,
    pub expected_target_version: Option<FileVersion>,
    pub related_observed_versions: Vec<FileVersion>,
    pub edit: EditPayload,
    pub occurred_at: Timestamp,
}

/// Harness-normalized edit body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditPayload {
    pub edit_kind: EditKind,
    pub input: Value,
    pub exact_diff: Option<String>,
}

/// Coarse edit operation family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditKind {
    Create,
    Modify,
    Delete,
    Rename,
    Unknown,
}

/// An edit completed and records the before/after versions the adapter can prove.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditApplied {
    pub agent_session_id: SessionId,
    pub worktree_id: WorktreeId,
    pub harness: AdapterRef,
    pub tool_call_id: String,
    pub file_id: Option<FileId>,
    pub path: PathBuf,
    pub before_version: Option<FileVersion>,
    pub after_version: Option<FileVersion>,
    pub exact_diff: Option<String>,
    pub applied_at: Timestamp,
    pub race_detected: bool,
}

/// A shell/process command is about to execute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandIntent {
    pub agent_session_id: SessionId,
    pub worktree_id: WorktreeId,
    pub harness: AdapterRef,
    pub tool_call_id: String,
    pub command: CommandSpec,
    pub repo_epoch_id: Option<RepoEpochId>,
    pub occurred_at: Timestamp,
}

/// Portable command invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub shell: Option<String>,
    pub cwd: PathBuf,
    pub env_keys: Vec<String>,
    pub timeout_ms: Option<u64>,
}

/// A shell/process command finished.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandResult {
    pub agent_session_id: SessionId,
    pub worktree_id: WorktreeId,
    pub harness: AdapterRef,
    pub tool_call_id: String,
    pub command: CommandSpec,
    pub result: CommandResultPayload,
    pub repo_epoch_before: Option<RepoEpochId>,
    pub repo_epoch_after: Option<RepoEpoch>,
    pub occurred_at: Timestamp,
}

/// Portable command result with bounded text fields chosen by the adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResultPayload {
    pub status: ToolStatus,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

/// The harness is about to compact context and can receive a survival packet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactIntent {
    pub agent_session_id: SessionId,
    pub worktree_id: WorktreeId,
    pub harness: AdapterRef,
    pub compact_id: String,
    pub current_token_usage: Option<TokenUsage>,
    pub working_set_file_versions: Vec<FileVersion>,
    pub context_frame_ids: Vec<String>,
    pub occurred_at: Timestamp,
}

/// A repo epoch changed because VCS state or working-tree content changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VcsStateChanged {
    pub agent_session_id: Option<SessionId>,
    pub worktree_id: WorktreeId,
    pub harness: Option<AdapterRef>,
    pub previous_epoch: Option<RepoEpoch>,
    pub new_epoch: RepoEpoch,
    pub changed_at: Timestamp,
    pub cause: VcsChangeCause,
}

/// Coarse reason a new repo epoch was captured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VcsChangeCause {
    FileContentChanged,
    BranchChanged,
    IndexChanged,
    OperationStateChanged,
    WorktreeInitialized,
    Unknown,
}

/// Adapter liveness, capability, and metrics heartbeat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterHeartbeat {
    pub agent_session_id: Option<SessionId>,
    pub worktree_id: WorktreeId,
    pub harness: AdapterRef,
    pub capabilities: AdapterCapabilities,
    pub sent_at: Timestamp,
    pub daemon_generation_id: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub queued_event_count: u32,
    pub degraded: Option<DegradedState>,
}

/// Token accounting reported by adapters that can provide it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub tool_result_tokens: u64,
}

/// Degraded/fail-open state visible to adapters and CLI clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradedState {
    pub reason: String,
    pub fail_open: bool,
    pub since: Timestamp,
}

impl DegradedState {
    /// Construct a healthy state for clients that report optional degradation.
    #[must_use]
    pub fn healthy() -> Self {
        Self {
            reason: String::new(),
            fail_open: false,
            since: Timestamp(0),
        }
    }

    /// Construct a fail-open degraded state without taking a wall-clock dependency.
    #[must_use]
    pub fn degraded(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            fail_open: true,
            since: Timestamp(0),
        }
    }

    /// Attach the timestamp at which degradation started.
    #[must_use]
    pub fn with_since(mut self, since: Timestamp) -> Self {
        self.since = since;
        self
    }
}

/// Daemon response to one event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonDecision {
    pub decision_kind: DaemonDecisionKind,
    pub context_frames: Vec<ContextFrameRef>,
    pub replacement_result: Option<ToolResultPayload>,
    pub modified_input: Option<Value>,
    pub deny_reason: Option<DenyReason>,
    pub revalidation_instructions: Vec<RevalidationInstruction>,
    pub confidence: Confidence,
    pub expires_at: Option<Timestamp>,
    pub degraded: Option<DegradedDecision>,
}

impl DaemonDecision {
    /// No intervention; the adapter should proceed normally.
    #[must_use]
    pub fn allow(confidence: Confidence) -> Self {
        Self::new(DaemonDecisionKind::Allow, confidence)
    }

    /// Block the event where the adapter supports blocking.
    #[must_use]
    pub fn deny(
        deny_reason: DenyReason,
        revalidation_instructions: Vec<RevalidationInstruction>,
        confidence: Confidence,
    ) -> Self {
        Self::new(DaemonDecisionKind::Deny, confidence)
            .with_deny_reason(deny_reason)
            .with_revalidation_instructions(revalidation_instructions)
    }

    /// Warn or guide the adapter without requiring a hard block.
    #[must_use]
    pub fn advisory(
        context_frames: Vec<ContextFrameRef>,
        revalidation_instructions: Vec<RevalidationInstruction>,
        confidence: Confidence,
    ) -> Self {
        Self::new(DaemonDecisionKind::Advisory, confidence)
            .with_context_frames(context_frames)
            .with_revalidation_instructions(revalidation_instructions)
    }

    /// Attach contextual frames to an adapter operation.
    #[must_use]
    pub fn decorate(context_frames: Vec<ContextFrameRef>, confidence: Confidence) -> Self {
        Self::new(DaemonDecisionKind::Decorate, confidence).with_context_frames(context_frames)
    }

    /// Replace a tool or command result where the adapter can accept replacements.
    #[must_use]
    pub fn replace_result(replacement_result: ToolResultPayload, confidence: Confidence) -> Self {
        Self::new(DaemonDecisionKind::ReplaceResult, confidence)
            .with_replacement_result(replacement_result)
    }

    /// Modify an adapter tool input before execution.
    #[must_use]
    pub fn modify_input(modified_input: Value, confidence: Confidence) -> Self {
        Self::new(DaemonDecisionKind::ModifyInput, confidence).with_modified_input(modified_input)
    }

    /// Observe the event but do not change adapter behavior.
    #[must_use]
    pub fn observe_only(confidence: Confidence) -> Self {
        Self::new(DaemonDecisionKind::ObserveOnly, confidence)
    }

    /// Fail-open decision for unavailable or uncertain daemon state.
    #[must_use]
    pub fn degraded_allow(reason: impl Into<String>, since: Timestamp) -> Self {
        Self {
            degraded: Some(DegradedDecision {
                state: DegradedState {
                    reason: reason.into(),
                    fail_open: true,
                    since,
                },
                fallback_kind: DaemonDecisionKind::Allow,
            }),
            ..Self::allow(Confidence::Heuristic)
        }
    }

    /// Attach context frames to any decision kind.
    #[must_use]
    pub fn with_context_frames(mut self, context_frames: Vec<ContextFrameRef>) -> Self {
        self.context_frames = context_frames;
        self
    }

    /// Attach a replacement result payload to a replace-result decision.
    #[must_use]
    pub fn with_replacement_result(mut self, replacement_result: ToolResultPayload) -> Self {
        self.replacement_result = Some(replacement_result);
        self
    }

    /// Attach a modified input payload to a modify-input decision.
    #[must_use]
    pub fn with_modified_input(mut self, modified_input: Value) -> Self {
        self.modified_input = Some(modified_input);
        self
    }

    /// Attach a structured deny/advisory reason.
    #[must_use]
    pub fn with_deny_reason(mut self, mut deny_reason: DenyReason) -> Self {
        if deny_reason.minimum_revalidation_set.is_empty() {
            deny_reason.minimum_revalidation_set = self.revalidation_instructions.clone();
        }
        self.deny_reason = Some(deny_reason);
        self
    }

    /// Attach concrete revalidation steps the agent should take.
    #[must_use]
    pub fn with_revalidation_instructions(
        mut self,
        revalidation_instructions: Vec<RevalidationInstruction>,
    ) -> Self {
        if let Some(reason) = &mut self.deny_reason
            && reason.minimum_revalidation_set.is_empty()
        {
            reason.minimum_revalidation_set = revalidation_instructions.clone();
        }
        self.revalidation_instructions = revalidation_instructions;
        self
    }

    /// Set the advisory/decoration expiry timestamp.
    #[must_use]
    pub fn with_expires_at(mut self, expires_at: Timestamp) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    /// Attach fail-open metadata to a decision.
    #[must_use]
    pub fn with_degraded(mut self, degraded: DegradedDecision) -> Self {
        self.degraded = Some(degraded);
        self
    }

    fn new(decision_kind: DaemonDecisionKind, confidence: Confidence) -> Self {
        Self {
            decision_kind,
            context_frames: Vec::new(),
            replacement_result: None,
            modified_input: None,
            deny_reason: None,
            revalidation_instructions: Vec::new(),
            confidence,
            expires_at: None,
            degraded: None,
        }
    }

    /// Wraps this decision in a current-version wire envelope.
    #[must_use]
    pub fn into_current_envelope(self) -> DaemonDecisionEnvelope {
        DaemonDecisionEnvelope::current(self)
    }
}

/// Supported daemon decision kinds from the hook protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonDecisionKind {
    Allow,
    Deny,
    Advisory,
    Decorate,
    ReplaceResult,
    ModifyInput,
    ObserveOnly,
}

/// A compact context frame reference plus optional inline text for hot-path pushes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFrameRef {
    pub context_frame_id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub provenance: Vec<ProvenanceMarker>,
    pub confidence: Confidence,
}

/// Structured confidence tier from the spec's provenance model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Verified,
    Resolved,
    Inferred,
    Heuristic,
    Speculative,
}

/// Provenance markers that explain why a pushed fact is trustworthy enough.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceMarker {
    CompilerBacked,
    LspBacked,
    ResolvedImportGraph,
    TreeSitterStructural,
    FrameworkConvention,
    RegexHeuristic,
    RepoLocalPattern,
    Stale,
    MergeStateDegraded,
    PartialGraph,
    Other(String),
}

/// Structured deny/advisory explanation for stale or unsafe operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenyReason {
    pub deny_id: Option<String>,
    pub severity: DenySeverity,
    pub message: String,
    pub target_file: Option<PathBuf>,
    pub current_target_version: Option<FileVersion>,
    pub stale_observation_versions: Vec<FileVersion>,
    pub dependency_causes: Vec<String>,
    #[serde(default)]
    pub minimum_revalidation_set: Vec<RevalidationInstruction>,
    pub proof_available: bool,
    pub override_policy: OverridePolicy,
}

/// Spec-named deny payload retained alongside [`DenyReason`].
///
/// `DenyReason` is the frozen Wave 1.2 public name. This alias exposes the
/// §6/Appendix-B schema name without breaking existing downstream references.
pub type DenyDecision = DenyReason;

/// Hard denies should block where the adapter can block; soft denies become advisory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenySeverity {
    Hard,
    Soft,
}

/// Whether and how a human or agent can override a deny.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverridePolicy {
    NotAllowed,
    RationaleRequired,
    Allowed,
}

/// Agent request to override a prior deny after observing required versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverrideDeny {
    pub deny_id: String,
    pub agent_session_id: SessionId,
    pub observed_dependency_versions: Vec<FileVersion>,
    pub rationale: String,
    pub requested_scope: OverrideScope,
}

/// Scope requested by an override attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideScope {
    SameTargetAndDependencyVersions,
    TargetFile,
    Session,
    Worktree,
}

/// Concrete action the agent should take to make a stale belief fresh again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevalidationInstruction {
    pub instruction_kind: RevalidationKind,
    pub path: Option<PathBuf>,
    pub file_id: Option<FileId>,
    pub reason: String,
}

/// Revalidation operation family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevalidationKind {
    RereadFile,
    RefreshRepoEpoch,
    RerunCommand,
    QueryMcp,
    WaitForIndex,
    Other,
}

/// Fail-open metadata attached to a decision when the daemon is degraded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradedDecision {
    pub state: DegradedState,
    pub fallback_kind: DaemonDecisionKind,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_types::{
        AdapterCapabilities, ContentHash, GitCommonDirId, OperationState, SourceClass,
    };

    const JS_SAFE_TS: i64 = 1_700_000_000_000_000_123;

    #[test]
    fn all_event_variants_roundtrip() {
        for event in sample_events() {
            let json = serde_json::to_string(&event).expect("event should serialize");
            let back: DaemonEvent = serde_json::from_str(&json).expect("event should deserialize");
            assert_eq!(back, event);
            assert_eq!(back.kind(), event.kind());
        }
    }

    #[test]
    fn event_kind_is_stable_for_each_variant() {
        let expected = [
            DaemonEventKind::SessionStart,
            DaemonEventKind::SessionEnd,
            DaemonEventKind::ToolIntent,
            DaemonEventKind::ToolResult,
            DaemonEventKind::ReadObserved,
            DaemonEventKind::EditIntent,
            DaemonEventKind::EditApplied,
            DaemonEventKind::CommandIntent,
            DaemonEventKind::CommandResult,
            DaemonEventKind::CompactIntent,
            DaemonEventKind::VcsStateChanged,
            DaemonEventKind::AdapterHeartbeat,
        ];

        for (event, kind) in sample_events().into_iter().zip(expected) {
            assert_eq!(event.kind(), kind);
        }
    }

    #[test]
    fn event_payloads_include_required_wire_details() {
        let cases = vec![
            (
                DaemonEvent::SessionStart(sample_session_start()),
                vec![
                    ("/kind", serde_json::json!("session_start")),
                    ("/payload/agent_session_id", serde_json::json!("session-1")),
                    (
                        "/payload/root_session_id",
                        serde_json::json!("root-session"),
                    ),
                    (
                        "/payload/parent_session_id",
                        serde_json::json!("parent-session"),
                    ),
                    ("/payload/spawn_event_id", serde_json::json!("event-42")),
                    ("/payload/lineage_depth", serde_json::json!(1)),
                    ("/payload/worktree_id", serde_json::json!("worktree-1")),
                    ("/payload/task_id", serde_json::json!("task-1")),
                    (
                        "/payload/capabilities/can_pre_edit_block",
                        serde_json::json!(true),
                    ),
                    (
                        "/payload/repo_epoch/repo_epoch_id",
                        serde_json::json!("epoch-1"),
                    ),
                ],
            ),
            (
                DaemonEvent::SessionEnd(sample_session_end()),
                vec![
                    ("/kind", serde_json::json!("session_end")),
                    ("/payload/agent_session_id", serde_json::json!("session-1")),
                    ("/payload/worktree_id", serde_json::json!("worktree-1")),
                    ("/payload/token_usage/input_tokens", serde_json::json!(100)),
                ],
            ),
            (
                DaemonEvent::ToolIntent(sample_tool_intent()),
                vec![
                    ("/kind", serde_json::json!("tool_intent")),
                    ("/payload/tool_call_id", serde_json::json!("tool-1")),
                    ("/payload/input/path", serde_json::json!("src/lib.rs")),
                    ("/payload/repo_epoch_id", serde_json::json!("epoch-1")),
                ],
            ),
            (
                DaemonEvent::ToolResult(sample_tool_result()),
                vec![
                    ("/kind", serde_json::json!("tool_result")),
                    ("/payload/tool_name", serde_json::json!("Read")),
                    ("/payload/result/status", serde_json::json!("succeeded")),
                    ("/payload/repo_epoch_id", serde_json::json!("epoch-1")),
                ],
            ),
            (
                DaemonEvent::ReadObserved(sample_read_observed()),
                vec![
                    ("/kind", serde_json::json!("read_observed")),
                    ("/payload/file_version/file_id", serde_json::json!("file-1")),
                    (
                        "/payload/file_version/repo_epoch_id",
                        serde_json::json!("epoch-1"),
                    ),
                    ("/payload/context_frame_id", serde_json::json!("ctx-read")),
                ],
            ),
            (
                DaemonEvent::EditIntent(sample_edit_intent()),
                vec![
                    ("/kind", serde_json::json!("edit_intent")),
                    ("/payload/file_id", serde_json::json!("file-1")),
                    (
                        "/payload/expected_target_version/repo_epoch_id",
                        serde_json::json!("epoch-1"),
                    ),
                    ("/payload/edit/input/old", serde_json::json!("a")),
                ],
            ),
            (
                DaemonEvent::EditApplied(sample_edit_applied()),
                vec![
                    ("/kind", serde_json::json!("edit_applied")),
                    (
                        "/payload/before_version/repo_epoch_id",
                        serde_json::json!("epoch-1"),
                    ),
                    (
                        "/payload/after_version/repo_epoch_id",
                        serde_json::json!("epoch-2"),
                    ),
                    ("/payload/race_detected", serde_json::json!(false)),
                ],
            ),
            (
                DaemonEvent::CommandIntent(sample_command_intent()),
                vec![
                    ("/kind", serde_json::json!("command_intent")),
                    ("/payload/command/program", serde_json::json!("cargo")),
                    ("/payload/repo_epoch_id", serde_json::json!("epoch-1")),
                ],
            ),
            (
                DaemonEvent::CommandResult(sample_command_result()),
                vec![
                    ("/kind", serde_json::json!("command_result")),
                    ("/payload/result/exit_code", serde_json::json!(0)),
                    ("/payload/repo_epoch_before", serde_json::json!("epoch-1")),
                    (
                        "/payload/repo_epoch_after/repo_epoch_id",
                        serde_json::json!("epoch-2"),
                    ),
                ],
            ),
            (
                DaemonEvent::CompactIntent(sample_compact_intent()),
                vec![
                    ("/kind", serde_json::json!("compact_intent")),
                    ("/payload/compact_id", serde_json::json!("compact-1")),
                    (
                        "/payload/working_set_file_versions/0/repo_epoch_id",
                        serde_json::json!("epoch-1"),
                    ),
                    (
                        "/payload/context_frame_ids/0",
                        serde_json::json!("ctx-read"),
                    ),
                ],
            ),
            (
                DaemonEvent::VcsStateChanged(sample_vcs_state_changed()),
                vec![
                    ("/kind", serde_json::json!("vcs_state_changed")),
                    (
                        "/payload/previous_epoch/repo_epoch_id",
                        serde_json::json!("epoch-1"),
                    ),
                    (
                        "/payload/new_epoch/repo_epoch_id",
                        serde_json::json!("epoch-2"),
                    ),
                    ("/payload/cause", serde_json::json!("file_content_changed")),
                ],
            ),
            (
                DaemonEvent::AdapterHeartbeat(sample_adapter_heartbeat()),
                vec![
                    ("/kind", serde_json::json!("adapter_heartbeat")),
                    (
                        "/payload/capabilities/can_modify_tool_input",
                        serde_json::json!(true),
                    ),
                    (
                        "/payload/daemon_generation_id",
                        serde_json::json!("daemon-gen-1"),
                    ),
                    ("/payload/queued_event_count", serde_json::json!(3)),
                ],
            ),
        ];

        for (event, expected_pointers) in cases {
            let json = serde_json::to_value(event).expect("event should become json");

            for (pointer, expected) in expected_pointers {
                assert_eq!(
                    json.pointer(pointer),
                    Some(&expected),
                    "missing or mismatched pointer {pointer}"
                );
            }
        }
    }

    #[test]
    fn decision_payloads_include_required_wire_fields() {
        let cases = vec![
            (
                DaemonDecision::allow(Confidence::Verified),
                vec![
                    ("/decision_kind", serde_json::json!("allow")),
                    ("/context_frames", serde_json::json!([])),
                    ("/replacement_result", serde_json::Value::Null),
                    ("/modified_input", serde_json::Value::Null),
                    ("/deny_reason", serde_json::Value::Null),
                    ("/revalidation_instructions", serde_json::json!([])),
                    ("/confidence", serde_json::json!("verified")),
                    ("/expires_at", serde_json::Value::Null),
                ],
            ),
            (
                sample_deny_decision(),
                vec![
                    ("/decision_kind", serde_json::json!("deny")),
                    (
                        "/deny_reason/message",
                        serde_json::json!("target file changed since last read"),
                    ),
                    (
                        "/deny_reason/current_target_version/repo_epoch_id",
                        serde_json::json!("epoch-2"),
                    ),
                    (
                        "/revalidation_instructions/0/instruction_kind",
                        serde_json::json!("reread_file"),
                    ),
                    ("/confidence", serde_json::json!("verified")),
                    ("/expires_at", serde_json::json!(JS_SAFE_TS.to_string())),
                ],
            ),
            (
                sample_advisory_decision(),
                vec![
                    ("/decision_kind", serde_json::json!("advisory")),
                    (
                        "/context_frames/0/context_frame_id",
                        serde_json::json!("ctx-1"),
                    ),
                    ("/deny_reason/severity", serde_json::json!("soft")),
                    (
                        "/revalidation_instructions/0/instruction_kind",
                        serde_json::json!("wait_for_index"),
                    ),
                ],
            ),
            (
                sample_decorate_decision(),
                vec![
                    ("/decision_kind", serde_json::json!("decorate")),
                    (
                        "/context_frames/0/confidence",
                        serde_json::json!("resolved"),
                    ),
                    (
                        "/context_frames/0/provenance/0",
                        serde_json::json!("resolved_import_graph"),
                    ),
                ],
            ),
            (
                sample_replace_result_decision(),
                vec![
                    ("/decision_kind", serde_json::json!("replace_result")),
                    ("/replacement_result/status", serde_json::json!("succeeded")),
                    (
                        "/replacement_result/output/replacement",
                        serde_json::json!(true),
                    ),
                ],
            ),
            (
                sample_modify_input_decision(),
                vec![
                    ("/decision_kind", serde_json::json!("modify_input")),
                    ("/modified_input/command", serde_json::json!("rg")),
                    ("/modified_input/args/0", serde_json::json!("needle")),
                    ("/degraded/state/fail_open", serde_json::json!(true)),
                    ("/degraded/fallback_kind", serde_json::json!("allow")),
                ],
            ),
            (
                DaemonDecision::observe_only(Confidence::Heuristic),
                vec![
                    ("/decision_kind", serde_json::json!("observe_only")),
                    ("/confidence", serde_json::json!("heuristic")),
                ],
            ),
        ];

        for (decision, expected_pointers) in cases {
            let json = serde_json::to_value(decision).expect("decision should become json");

            for (pointer, expected) in expected_pointers {
                assert_eq!(
                    json.pointer(pointer),
                    Some(&expected),
                    "missing or mismatched pointer {pointer}"
                );
            }
        }
    }

    #[test]
    fn all_decision_kinds_roundtrip() {
        for decision in sample_decisions() {
            let json = serde_json::to_string(&decision).expect("decision should serialize");
            let back: DaemonDecision =
                serde_json::from_str(&json).expect("decision should deserialize");
            assert_eq!(back, decision);
        }
    }

    #[test]
    fn decision_payloads_include_required_wire_details() {
        let cases = vec![
            (
                DaemonDecision::allow(Confidence::Verified),
                vec![
                    ("/decision_kind", serde_json::json!("allow")),
                    ("/context_frames", serde_json::json!([])),
                    ("/confidence", serde_json::json!("verified")),
                ],
            ),
            (
                sample_deny_decision(),
                vec![
                    ("/decision_kind", serde_json::json!("deny")),
                    (
                        "/deny_reason/message",
                        serde_json::json!("target file changed since last read"),
                    ),
                    (
                        "/deny_reason/current_target_version/repo_epoch_id",
                        serde_json::json!("epoch-2"),
                    ),
                    (
                        "/revalidation_instructions/0/instruction_kind",
                        serde_json::json!("reread_file"),
                    ),
                    ("/confidence", serde_json::json!("verified")),
                    ("/expires_at", serde_json::json!(JS_SAFE_TS.to_string())),
                ],
            ),
            (
                sample_advisory_decision(),
                vec![
                    ("/decision_kind", serde_json::json!("advisory")),
                    (
                        "/context_frames/0/context_frame_id",
                        serde_json::json!("ctx-1"),
                    ),
                    ("/deny_reason/severity", serde_json::json!("soft")),
                    (
                        "/revalidation_instructions/0/instruction_kind",
                        serde_json::json!("wait_for_index"),
                    ),
                ],
            ),
            (
                sample_decorate_decision(),
                vec![
                    ("/decision_kind", serde_json::json!("decorate")),
                    (
                        "/context_frames/0/provenance/0",
                        serde_json::json!("resolved_import_graph"),
                    ),
                    ("/confidence", serde_json::json!("resolved")),
                ],
            ),
            (
                sample_replace_result_decision(),
                vec![
                    ("/decision_kind", serde_json::json!("replace_result")),
                    ("/replacement_result/status", serde_json::json!("succeeded")),
                    (
                        "/replacement_result/output/replacement",
                        serde_json::json!(true),
                    ),
                ],
            ),
            (
                sample_modify_input_decision(),
                vec![
                    ("/decision_kind", serde_json::json!("modify_input")),
                    ("/modified_input/command", serde_json::json!("rg")),
                    ("/degraded/state/fail_open", serde_json::json!(true)),
                    ("/degraded/fallback_kind", serde_json::json!("allow")),
                ],
            ),
            (
                DaemonDecision::observe_only(Confidence::Heuristic),
                vec![
                    ("/decision_kind", serde_json::json!("observe_only")),
                    ("/confidence", serde_json::json!("heuristic")),
                ],
            ),
        ];

        for (decision, expected_pointers) in cases {
            let json = serde_json::to_value(decision).expect("decision should become json");

            for (pointer, expected) in expected_pointers {
                assert_eq!(
                    json.pointer(pointer),
                    Some(&expected),
                    "missing or mismatched pointer {pointer}"
                );
            }
        }
    }

    #[test]
    fn degraded_allow_is_explicit_fail_open_wire_shape() {
        let decision = DaemonDecision::degraded_allow("daemon unavailable", timestamp());
        let json = serde_json::to_value(&decision).expect("decision should become json");

        assert_eq!(
            json.pointer("/decision_kind"),
            Some(&Value::String("allow".into()))
        );
        assert_eq!(
            json.pointer("/confidence"),
            Some(&Value::String("heuristic".into()))
        );
        assert_eq!(
            json.pointer("/degraded/state/reason"),
            Some(&Value::String("daemon unavailable".into()))
        );
        assert_eq!(
            json.pointer("/degraded/state/fail_open"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            json.pointer("/degraded/state/since"),
            Some(&Value::String(JS_SAFE_TS.to_string()))
        );
        assert_eq!(
            json.pointer("/degraded/fallback_kind"),
            Some(&Value::String("allow".into()))
        );

        let back: DaemonDecision =
            serde_json::from_value(json).expect("degraded decision should deserialize");
        assert_eq!(back, decision);
    }

    #[test]
    fn deny_and_override_payloads_roundtrip() {
        let deny: DenyDecision = DenyReason {
            deny_id: Some("deny-1".into()),
            severity: DenySeverity::Hard,
            message: "target file changed since last read".into(),
            target_file: Some(PathBuf::from("src/lib.rs")),
            current_target_version: Some(file_version("file-1", "src/lib.rs", "epoch-2")),
            stale_observation_versions: vec![file_version("file-1", "src/lib.rs", "epoch-1")],
            dependency_causes: vec!["src/main.rs changed".into()],
            minimum_revalidation_set: vec![reread_instruction()],
            proof_available: true,
            override_policy: OverridePolicy::RationaleRequired,
        };
        let json = serde_json::to_string(&deny).expect("deny decision should serialize");
        let back: DenyDecision =
            serde_json::from_str(&json).expect("deny decision should deserialize");
        assert_eq!(back, deny);

        let override_deny = OverrideDeny {
            deny_id: "deny-1".into(),
            agent_session_id: agent_session_id(),
            observed_dependency_versions: vec![file_version("file-2", "src/main.rs", "epoch-2")],
            rationale: "I reread the changed dependency and this edit is still scoped.".into(),
            requested_scope: OverrideScope::SameTargetAndDependencyVersions,
        };
        let json = serde_json::to_value(&override_deny).expect("override should become json");
        assert_eq!(
            json.pointer("/requested_scope"),
            Some(&Value::String("same_target_and_dependency_versions".into()))
        );

        let back: OverrideDeny =
            serde_json::from_value(json).expect("override deny should deserialize");
        assert_eq!(back, override_deny);
    }

    #[test]
    fn versioned_event_envelope_roundtrips() {
        let envelope = DaemonEvent::ToolIntent(sample_tool_intent()).into_current_envelope();
        let json = serde_json::to_value(&envelope).expect("event envelope should become json");

        assert_eq!(
            json.pointer("/protocol_version").cloned(),
            Some(serde_json::json!(CURRENT_PROTOCOL_VERSION.0))
        );
        assert_eq!(
            json.pointer("/event/kind"),
            Some(&Value::String("tool_intent".into()))
        );

        let back: DaemonEventEnvelope =
            serde_json::from_value(json).expect("event envelope should deserialize");
        assert_eq!(back, envelope);
    }

    #[test]
    fn versioned_decision_envelope_roundtrips() {
        let envelope = sample_replace_result_decision().into_current_envelope();
        let json = serde_json::to_value(&envelope).expect("decision envelope should become json");

        assert_eq!(
            json.pointer("/protocol_version").cloned(),
            Some(serde_json::json!(CURRENT_PROTOCOL_VERSION.0))
        );
        assert_eq!(
            json.pointer("/decision/decision_kind"),
            Some(&Value::String("replace_result".into()))
        );

        let back: DaemonDecisionEnvelope =
            serde_json::from_value(json).expect("decision envelope should deserialize");
        assert_eq!(back, envelope);
    }

    #[test]
    fn timestamp_remains_js_safe_string_inside_event_payloads() {
        let event = DaemonEvent::ReadObserved(sample_read_observed());
        let json: Value = serde_json::to_value(event).expect("event should become json");

        assert_eq!(
            json.pointer("/payload/observed_at"),
            Some(&Value::String(JS_SAFE_TS.to_string()))
        );
        assert_eq!(
            json.pointer("/payload/file_version/mtime_observed"),
            Some(&Value::String(JS_SAFE_TS.to_string()))
        );
    }

    #[test]
    fn timestamp_remains_js_safe_string_inside_decision_payloads() {
        let decision = sample_decorate_decision();
        let json: Value = serde_json::to_value(decision).expect("decision should become json");

        assert_eq!(
            json.pointer("/expires_at"),
            Some(&Value::String(JS_SAFE_TS.to_string()))
        );
    }

    #[test]
    fn timestamp_remains_js_safe_string_inside_wire_envelopes() {
        let event_envelope =
            DaemonEvent::ReadObserved(sample_read_observed()).into_current_envelope();
        let event_json =
            serde_json::to_value(event_envelope).expect("event envelope should become json");
        assert_eq!(
            event_json.pointer("/event/payload/observed_at"),
            Some(&Value::String(JS_SAFE_TS.to_string()))
        );
        assert_eq!(
            event_json.pointer("/event/payload/file_version/mtime_observed"),
            Some(&Value::String(JS_SAFE_TS.to_string()))
        );

        let decision_envelope = sample_decorate_decision().into_current_envelope();
        let decision_json =
            serde_json::to_value(decision_envelope).expect("decision envelope should become json");
        assert_eq!(
            decision_json.pointer("/decision/expires_at"),
            Some(&Value::String(JS_SAFE_TS.to_string()))
        );
    }

    fn sample_events() -> Vec<DaemonEvent> {
        vec![
            DaemonEvent::SessionStart(sample_session_start()),
            DaemonEvent::SessionEnd(sample_session_end()),
            DaemonEvent::ToolIntent(sample_tool_intent()),
            DaemonEvent::ToolResult(sample_tool_result()),
            DaemonEvent::ReadObserved(sample_read_observed()),
            DaemonEvent::EditIntent(sample_edit_intent()),
            DaemonEvent::EditApplied(sample_edit_applied()),
            DaemonEvent::CommandIntent(sample_command_intent()),
            DaemonEvent::CommandResult(sample_command_result()),
            DaemonEvent::CompactIntent(sample_compact_intent()),
            DaemonEvent::VcsStateChanged(sample_vcs_state_changed()),
            DaemonEvent::AdapterHeartbeat(sample_adapter_heartbeat()),
        ]
    }

    fn sample_decisions() -> Vec<DaemonDecision> {
        vec![
            DaemonDecision::allow(Confidence::Verified),
            sample_deny_decision(),
            sample_advisory_decision(),
            sample_decorate_decision(),
            sample_replace_result_decision(),
            sample_modify_input_decision(),
            DaemonDecision::observe_only(Confidence::Heuristic),
        ]
    }

    fn sample_session_start() -> SessionStart {
        SessionStart {
            agent_session_id: agent_session_id(),
            root_session_id: SessionId::new("root-session"),
            parent_session_id: Some(SessionId::new("parent-session")),
            spawn_event_id: Some("event-42".into()),
            lineage_depth: 1,
            worktree_id: worktree_id(),
            harness: adapter_ref(),
            capabilities: capabilities(),
            repo_epoch: Some(repo_epoch("epoch-1")),
            started_at: timestamp(),
            task_id: Some("task-1".into()),
            task_summary: Some("port protocol tests".into()),
            inherited_context_frame_ids: vec!["ctx-parent".into()],
        }
    }

    fn sample_session_end() -> SessionEnd {
        SessionEnd {
            agent_session_id: agent_session_id(),
            worktree_id: worktree_id(),
            harness: adapter_ref(),
            ended_at: timestamp(),
            reason: SessionEndReason::Completed,
            token_usage: Some(token_usage()),
        }
    }

    fn sample_tool_intent() -> ToolIntent {
        ToolIntent {
            agent_session_id: agent_session_id(),
            worktree_id: worktree_id(),
            harness: adapter_ref(),
            tool_call_id: "tool-1".into(),
            tool_name: "Read".into(),
            input: serde_json::json!({ "path": "src/lib.rs" }),
            repo_epoch_id: Some(RepoEpochId::new("epoch-1")),
            occurred_at: timestamp(),
            token_usage: Some(token_usage()),
        }
    }

    fn sample_tool_result() -> ToolResult {
        ToolResult {
            agent_session_id: agent_session_id(),
            worktree_id: worktree_id(),
            harness: adapter_ref(),
            tool_call_id: "tool-1".into(),
            tool_name: "Read".into(),
            result: tool_result_payload(),
            repo_epoch_id: Some(RepoEpochId::new("epoch-1")),
            occurred_at: timestamp(),
            token_usage: Some(token_usage()),
        }
    }

    fn sample_read_observed() -> ReadObserved {
        ReadObserved {
            agent_session_id: agent_session_id(),
            worktree_id: worktree_id(),
            harness: adapter_ref(),
            tool_call_id: Some("tool-1".into()),
            file_version: file_version("file-1", "src/lib.rs", "epoch-1"),
            observed_at: timestamp(),
            context_frame_id: Some("ctx-read".into()),
            task_id: Some("task-1".into()),
        }
    }

    fn sample_edit_intent() -> EditIntent {
        EditIntent {
            agent_session_id: agent_session_id(),
            worktree_id: worktree_id(),
            harness: adapter_ref(),
            tool_call_id: "tool-2".into(),
            file_id: Some(FileId::new("file-1")),
            path: PathBuf::from("src/lib.rs"),
            expected_target_version: Some(file_version("file-1", "src/lib.rs", "epoch-1")),
            related_observed_versions: vec![file_version("file-2", "src/main.rs", "epoch-1")],
            edit: EditPayload {
                edit_kind: EditKind::Modify,
                input: serde_json::json!({ "old": "a", "new": "b" }),
                exact_diff: Some("-a\n+b\n".into()),
            },
            occurred_at: timestamp(),
        }
    }

    fn sample_edit_applied() -> EditApplied {
        EditApplied {
            agent_session_id: agent_session_id(),
            worktree_id: worktree_id(),
            harness: adapter_ref(),
            tool_call_id: "tool-2".into(),
            file_id: Some(FileId::new("file-1")),
            path: PathBuf::from("src/lib.rs"),
            before_version: Some(file_version("file-1", "src/lib.rs", "epoch-1")),
            after_version: Some(file_version("file-1", "src/lib.rs", "epoch-2")),
            exact_diff: Some("-a\n+b\n".into()),
            applied_at: timestamp(),
            race_detected: false,
        }
    }

    fn sample_command_intent() -> CommandIntent {
        CommandIntent {
            agent_session_id: agent_session_id(),
            worktree_id: worktree_id(),
            harness: adapter_ref(),
            tool_call_id: "tool-3".into(),
            command: command_spec(),
            repo_epoch_id: Some(RepoEpochId::new("epoch-1")),
            occurred_at: timestamp(),
        }
    }

    fn sample_command_result() -> CommandResult {
        CommandResult {
            agent_session_id: agent_session_id(),
            worktree_id: worktree_id(),
            harness: adapter_ref(),
            tool_call_id: "tool-3".into(),
            command: command_spec(),
            result: CommandResultPayload {
                status: ToolStatus::Succeeded,
                exit_code: Some(0),
                stdout: "ok".into(),
                stderr: String::new(),
                duration_ms: 42,
            },
            repo_epoch_before: Some(RepoEpochId::new("epoch-1")),
            repo_epoch_after: Some(repo_epoch("epoch-2")),
            occurred_at: timestamp(),
        }
    }

    fn sample_compact_intent() -> CompactIntent {
        CompactIntent {
            agent_session_id: agent_session_id(),
            worktree_id: worktree_id(),
            harness: adapter_ref(),
            compact_id: "compact-1".into(),
            current_token_usage: Some(token_usage()),
            working_set_file_versions: vec![file_version("file-1", "src/lib.rs", "epoch-1")],
            context_frame_ids: vec!["ctx-read".into(), "ctx-edit".into()],
            occurred_at: timestamp(),
        }
    }

    fn sample_vcs_state_changed() -> VcsStateChanged {
        VcsStateChanged {
            agent_session_id: Some(agent_session_id()),
            worktree_id: worktree_id(),
            harness: Some(adapter_ref()),
            previous_epoch: Some(repo_epoch("epoch-1")),
            new_epoch: repo_epoch("epoch-2"),
            changed_at: timestamp(),
            cause: VcsChangeCause::FileContentChanged,
        }
    }

    fn sample_adapter_heartbeat() -> AdapterHeartbeat {
        AdapterHeartbeat {
            agent_session_id: Some(agent_session_id()),
            worktree_id: worktree_id(),
            harness: adapter_ref(),
            capabilities: capabilities(),
            sent_at: timestamp(),
            daemon_generation_id: Some("daemon-gen-1".into()),
            token_usage: Some(token_usage()),
            queued_event_count: 3,
            degraded: Some(DegradedState {
                reason: "daemon restarting".into(),
                fail_open: true,
                since: timestamp(),
            }),
        }
    }

    fn sample_deny_decision() -> DaemonDecision {
        DaemonDecision::deny(
            DenyReason {
                deny_id: Some("deny-1".into()),
                severity: DenySeverity::Hard,
                message: "target file changed since last read".into(),
                target_file: Some(PathBuf::from("src/lib.rs")),
                current_target_version: Some(file_version("file-1", "src/lib.rs", "epoch-2")),
                stale_observation_versions: vec![file_version("file-1", "src/lib.rs", "epoch-1")],
                dependency_causes: vec!["src/main.rs changed".into()],
                minimum_revalidation_set: vec![reread_instruction()],
                proof_available: true,
                override_policy: OverridePolicy::RationaleRequired,
            },
            vec![reread_instruction()],
            Confidence::Verified,
        )
        .with_expires_at(timestamp())
    }

    fn sample_advisory_decision() -> DaemonDecision {
        let revalidation = RevalidationInstruction {
            instruction_kind: RevalidationKind::WaitForIndex,
            path: None,
            file_id: None,
            reason: "wait for import graph coverage".into(),
        };

        DaemonDecision::advisory(
            vec![context_frame()],
            vec![revalidation.clone()],
            Confidence::Heuristic,
        )
        .with_deny_reason(DenyReason {
            deny_id: None,
            severity: DenySeverity::Soft,
            message: "dependency graph is still warming".into(),
            target_file: None,
            current_target_version: None,
            stale_observation_versions: Vec::new(),
            dependency_causes: vec!["partial graph".into()],
            minimum_revalidation_set: vec![revalidation],
            proof_available: false,
            override_policy: OverridePolicy::Allowed,
        })
        .with_expires_at(timestamp())
    }

    fn sample_decorate_decision() -> DaemonDecision {
        DaemonDecision::decorate(vec![context_frame()], Confidence::Resolved)
            .with_expires_at(timestamp())
    }

    fn sample_replace_result_decision() -> DaemonDecision {
        DaemonDecision::replace_result(
            ToolResultPayload {
                status: ToolStatus::Succeeded,
                output: serde_json::json!({ "replacement": true }),
                error: None,
                latency_ms: Some(7),
            },
            Confidence::Verified,
        )
        .with_expires_at(timestamp())
    }

    fn sample_modify_input_decision() -> DaemonDecision {
        DaemonDecision::modify_input(
            serde_json::json!({ "command": "rg", "args": ["needle"] }),
            Confidence::Resolved,
        )
        .with_expires_at(timestamp())
        .with_degraded(DegradedDecision {
            state: DegradedState {
                reason: "command replacement unavailable".into(),
                fail_open: true,
                since: timestamp(),
            },
            fallback_kind: DaemonDecisionKind::Allow,
        })
    }

    fn agent_session_id() -> SessionId {
        SessionId::new("session-1")
    }

    fn worktree_id() -> WorktreeId {
        WorktreeId::new("worktree-1")
    }

    fn adapter_ref() -> AdapterRef {
        AdapterRef {
            adapter_id: "adapter-1".into(),
            adapter_kind: AdapterKind::HarnessSim,
        }
    }

    fn capabilities() -> AdapterCapabilities {
        AdapterCapabilities {
            can_pre_edit_block: true,
            can_pre_read_decorate: true,
            can_post_read_decorate: true,
            can_command_replace: true,
            can_modify_tool_input: true,
            can_async_notify: false,
            can_precompact: true,
            can_report_token_usage: true,
            can_report_exact_edit_diff: true,
            can_attach_file_precondition: true,
        }
    }

    fn timestamp() -> Timestamp {
        Timestamp(JS_SAFE_TS)
    }

    fn hash() -> ContentHash {
        ContentHash::from_hex("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
    }

    fn repo_epoch(id: &str) -> RepoEpoch {
        RepoEpoch {
            repo_epoch_id: RepoEpochId::new(id),
            worktree_id: worktree_id(),
            git_common_dir_id: Some(GitCommonDirId::new("git-common-1")),
            head_oid: Some("deadbeef".into()),
            branch_ref: Some("refs/heads/main".into()),
            index_tree_oid: Some("feedface".into()),
            working_tree_digest: hash(),
            operation_state: OperationState::Normal,
            started_at: timestamp(),
        }
    }

    fn file_version(file_id: &str, path: &str, epoch_id: &str) -> FileVersion {
        FileVersion {
            file_id: FileId::new(file_id),
            path: PathBuf::from(path),
            content_hash: hash(),
            size: 123,
            mtime_observed: timestamp(),
            executable_bit: false,
            symlink_target: None,
            repo_epoch_id: RepoEpochId::new(epoch_id),
            source_class: SourceClass::Source,
        }
    }

    fn token_usage() -> TokenUsage {
        TokenUsage {
            input_tokens: 100,
            output_tokens: 50,
            cached_input_tokens: 10,
            tool_result_tokens: 25,
        }
    }

    fn tool_result_payload() -> ToolResultPayload {
        ToolResultPayload {
            status: ToolStatus::Succeeded,
            output: serde_json::json!({ "content": "hello" }),
            error: None,
            latency_ms: Some(5),
        }
    }

    fn command_spec() -> CommandSpec {
        CommandSpec {
            program: "cargo".into(),
            args: vec!["fmt".into()],
            shell: None,
            cwd: PathBuf::from("/repo"),
            env_keys: vec!["RUSTFLAGS".into()],
            timeout_ms: Some(30_000),
        }
    }

    fn context_frame() -> ContextFrameRef {
        ContextFrameRef {
            context_frame_id: "ctx-1".into(),
            title: Some("staleness warning".into()),
            body: Some("src/lib.rs changed since your last read".into()),
            provenance: vec![ProvenanceMarker::ResolvedImportGraph],
            confidence: Confidence::Resolved,
        }
    }

    fn reread_instruction() -> RevalidationInstruction {
        RevalidationInstruction {
            instruction_kind: RevalidationKind::RereadFile,
            path: Some(PathBuf::from("src/lib.rs")),
            file_id: Some(FileId::new("file-1")),
            reason: "target version is stale".into(),
        }
    }
}
