I’m treating the uploaded brief as binding ground truth for the domain constraints, especially Rust, local-first, hooks plus MCP, multi-agent as steady state, and broad eventual language/framework coverage.

## 1. Build-order implication of the non-obvious choice

**Confirm, with one correction:** the graph should not come first, but a tiny truth substrate must exist before the ledger is useful. The right build order is ledger-first, graph-assisted, not graph-first.

Your phasing is directionally right. I would patch it this way:

1. **Project identity, daemon lifecycle, storage, event log, file-version model.**
   Before “observation ledger,” we need stable project/worktree identity, content-addressed file versions, monotonic event IDs, session IDs, repo epochs, and adapter capability records. Otherwise the ledger is writing poetry on fog.
2. **Session registry, ObservationLedger, EditLedger, direct file freshness.**
   First useful coordination feature: “you read version V of file F, another session advanced F, do not edit blindly.” This does not require a semantic graph.
3. **ContextFrame ledger and scheduler skeleton.**
   The scheduler can start as a no-op or template emitter, but the data model must exist early: what was shown, to whom, from which source versions, with what expiration semantics.
4. **Minimal graph, not full graph.**
   The ledger depends on a truth layer, but not the deluxe cathedral. The first graph should produce:
   - file inventory
   - file hashes
   - imports/exports for P0-alpha languages
   - static dependency sets
   - exported-surface fingerprints
   - simple symbol locations
   - provenance/confidence on every fact
5. **Hooks and MCP as clients of the same daemon contracts.**
   Do not wait until the graph is mature to design hooks. Build adapter contracts and a harness simulator early, but let them call thin ledger and graph APIs.
6. **Diagnostics, framework extractors, broadcasts, leases.**
   These sit on top once the event log, file versioning, sessions, observations, graph facts, and ContextFrames are stable.

The part of the graph that must come earlier than your sketch is **file identity plus dependency enough for stale checks**. Full framework extraction does not need to come early. The core rule: **graph facts are inputs to belief management; they must not become the primary API surface.**

Axes changed: process model, storage/event model, graph model, context scheduling, validation.

------

## 2. Subagent inheritance

Recommendation: **lineage-aware copy-on-spawn plus a subagent orientation packet.** Do not use live shared view. Do not start blank. The child must get enough inherited belief to avoid blind grep, but its belief state must remain auditable and temporally frozen at spawn.

The model:

### Session identity

Every session has its own `agent_session_id`.

```text
Session {
  agent_session_id
  root_session_id
  parent_session_id nullable
  spawn_event_id nullable
  lineage_depth
  harness
  task_label nullable
  capabilities
  created_at
}
```

If α spawns α′:

```text
α.agent_session_id  = s_100
α′.agent_session_id = s_143
α′.parent_session_id = s_100
α′.root_session_id = s_100
```

The child is a real session, not a thread inside the parent.

### Observation inheritance

At spawn, the daemon creates an inherited observation snapshot:

```text
InheritedObservation {
  child_session_id
  inherited_from_session_id
  inherited_from_observation_id
  file_path
  file_version
  graph_version
  observed_at_original
  inherited_at
  observation_kind
  observation_fidelity
  source_context_frame_id nullable
}
```

The child’s materialized “effective observation set” is:

```text
direct_observations(child)
UNION inherited_observations(child)
```

But each row preserves origin. That origin matters for staleness, provenance, and benchmark analysis.

### Subagent orientation packet

The child should receive a compact `subagent_orientation_packet`, not the parent’s whole transcript. It should include:

- current task slice
- parent’s relevant working set
- files and symbols already explored
- stale risks already known
- key ContextFrame IDs
- “changed since parent saw this” warnings
- suggested first reads or MCP calls

This is a delta-briefing variant, not a full SessionStart packet.

### How staleness works

If α read `services/auth.ts` at version V1, then α spawns α′ at 14:00 with inherited observation V1. If β edits `services/auth.ts` at 14:03 to version V2, then α′ trying to edit a dependent file at 14:04 is stale.

The stale check uses:

```text
last_relevant_observation_time =
  max(direct observation time, inherited_at for inherited observations)
```

But the observed file version remains V1. So the deny says:

```text
Dependency services/auth.ts advanced from V1 to V2
after this child session inherited its parent’s observation.
Re-read services/auth.ts before editing.
```

If β edited before α′ was spawned, the daemon should detect that at spawn time and either exclude the stale inherited fact or mark it stale inside the orientation packet. A child should not inherit already-rotten fruit without a label.

### Parent learning from child

Child observations do **not** automatically become parent observations. The parent did not see the child’s reads. When the child returns, the parent gets a `subagent_report_context_frame` with summary facts and pointers. Those become parent observations of the report, not of the underlying full file contents.

This is important. Otherwise the parent can act as if it read files it never saw.

------

## 3. Git/VCS integration depth

The canonical observation key should be **file-content-hash plus repo epoch**, not commit SHA alone and not global dirty hash alone.

A commit SHA is insufficient because the system spends most of its life in dirty working trees. A dirty-files hash is too coarse because one unrelated dirty file should not invalidate every observation. The right model has three layers.

### Canonical file version

```text
FileVersion {
  file_id
  path
  content_hash
  size
  mtime_observed
  executable_bit
  symlink_target nullable
  repo_epoch_id
}
```

The `content_hash` is the truth. If the same file content returns later, it can be recognized.

### Repo epoch

```text
RepoEpoch {
  repo_epoch_id
  worktree_id
  git_common_dir_id nullable
  head_oid nullable
  branch_ref nullable
  index_tree_oid nullable
  working_tree_digest
  operation_state
  started_at
}
```

`operation_state` includes:

```text
normal
detached_head
merge_in_progress
rebase_in_progress
cherry_pick_in_progress
bisect_in_progress
unknown_vcs
```

### Observation

```text
Observation {
  agent_session_id
  file_id
  path
  file_version
  repo_epoch_id
  graph_version
  observed_at
  context_frame_id nullable
}
```

### Branch checkout mid-session

A branch checkout creates a new `repo_epoch_id`.

Existing observations are not deleted. They become **epoch-stale unless revalidated by content hash**. If `src/foo.ts` has the same content hash before and after checkout, the file observation can be treated as content-current, but the dependency graph may still need revalidation because imports, config, generated files, and package versions may have changed.

So the rule is:

- Same content hash, same relevant graph inputs: observation remains valid.
- Same content hash, different graph/config inputs: observation is content-valid but graph-stale.
- Different content hash: observation is stale.

### Stash and restore

Stash/restore is just working-tree mutation. Observations invalidate when content hash changes. If a file returns to an already-observed content hash, the daemon can revalidate it, but should annotate the epoch transition.

Stash operations should not fork the ledger. They create events:

```text
VcsStateChanged
FileVersionAdvanced
GraphInputsChanged
```

### Worktree-per-agent setups

Recommendation: **support them, but do not make them the coordination domain in V1.**

The coherence domain for P0 is **one daemon per worktree**. Multiple sessions inside the same worktree coordinate deeply. Separate git worktrees get separate daemons and separate ledgers.

This means:

- P0 does not promise cross-worktree stale-dependency arbitration.
- P0 can still report sibling worktrees as advisory if discovered.
- P1/P2 can add a “worktree federation” that detects likely merge conflicts and semantic divergence across worktrees.

So we work alongside worktrees, but the flagship product experience should be shared-working-tree multi-agent coordination. Worktree-per-agent remains compatible, not central.

### Mid-rebase and mid-merge

During merge/rebase/cherry-pick states, the daemon enters `VCS_UNSTABLE`.

Contract:

- Read decorations continue, marked with degraded provenance.
- Target-file freshness checks still run from content hash.
- Dependency-staleness denies are downgraded unless the dependency graph is known fresh.
- Files with conflict markers or unresolved index stages are high-risk and can be denied in strict mode.
- Diagnostics are labeled as merge-state diagnostics, not normal project diagnostics.

No semantic freshness guarantee should be claimed during unresolved VCS operations.

Axes changed: durable storage, multi-agent coordination, failure behavior, operator debugging.

------

## 4. PreCompact hook design

This was underdesigned in the spec. Your instinct is right: rich PreCompact is P1, but the **checkpoint primitive is P0**.

### What PreCompact emits

PreCompact should emit a **survival packet**, not a SessionStart-redux.

It should tell the agent what must survive compression:

```text
PreCompactSurvivalPacket {
  current_task_summary
  active_working_set
  files_observed_with_versions
  files_edited_by_this_session
  unresolved_diagnostics
  unresolved_stale_warnings
  other_agent_changes_since_last_read
  key_symbols_and_routes
  must_keep_context_frame_ids
  suggested_resume_mcp_calls
}
```

The packet should be pointer-heavy. It should preserve enough symbolic anchors for post-compact recovery without dumping the whole ledger into the model.

A good PreCompact message says:

> Keep these four facts and these six frame IDs. After compaction, call `briefcase_observed_state` if you need the full ledger slice.

### What the daemon does

The daemon should record a context epoch transition:

```text
CompactionCheckpoint {
  checkpoint_id
  agent_session_id
  pre_compact_context_frame_ids
  promoted_fact_ids
  omitted_fact_ids
  working_set_snapshot
  created_at
}
```

It should not compact the event log. The event log is the black box recorder. It can compact materialized views later, but PreCompact is not storage GC.

After PreCompact, the scheduler should treat the agent as having **reduced working memory**. Facts included in the survival packet remain “seen.” Facts omitted from the survival packet become eligible for lightweight re-emission when relevant.

### Priority

- **P0:** record PreCompact events, create checkpoints, expose resume state through MCP.
- **P1:** emit high-quality survival packets.
- **P2:** learn which facts should survive compaction from benchmark utility receipts.

If long-session benchmarks are part of V1 acceptance, then P0 must include the skeleton. Full survival-packet quality can wait until P1.

------

## 5. P0 language-tier split

The 20-language requirement is load-bearing for the product promise, but not for P0-alpha enforcement quality. Ship tiers.

The dangerous version is “20 languages all pretend to be equally supported.” That creates false precision and bad denies. The right split is **coverage broad, enforcement narrow**.

### P0-alpha: enforcement-grade languages

P0-alpha should support dep-staleness enforcement, exported-surface fingerprints, read decorations, symbol lookup, and diagnostic deltas for:

```text
TypeScript / JavaScript
Python
Go
Rust
```

I would consider Java and C# only if you need Spring and ASP.NET in the first benchmark corpus. Otherwise they belong in P0-beta.

The criterion is not popularity alone. P0-alpha languages need:

- high expected agent workload
- feasible incremental graph extraction
- meaningful diagnostics integration
- representative static and dynamic behavior
- enough framework value to validate the product thesis

### P0-alpha fallback for all 20

All target languages should at least have:

- file inventory
- syntax outline where grammar exists
- comment/string-aware text search
- basic symbol-ish spans
- no hard semantic denies from weak facts

So the public story is not “only four languages exist.” It is:

```text
Tier 1: enforcement-grade
Tier 2: navigation-grade
Tier 3: syntax-outline fallback
```

### P0-beta

P0-beta expands enforcement or navigation-grade support to:

```text
Java
C#
PHP
Ruby
C
C++
Objective-C
Swift
Kotlin
Dart
Lua
Luau
Scala
Pascal/Delphi
Elixir or Clojure
```

Framework support should be staged by benchmark value, not language vanity.

### Gating signal for P0-beta

Do not begin serious P0-beta extractor expansion until:

- Observation/EditLedger APIs have stabilized across at least two benchmark cycles.
- P0-alpha false-positive stale denies are below 5 percent.
- ContextFrame duplicate-token ratio is below 10 percent.
- Adding a new extractor does not require core daemon changes.
- The extractor fixture harness can measure precision/recall automatically.

The trap is letting language breadth consume the substrate. The substrate is the product spine. Extractors are organs.

------

## 6. MCP tool surface

Recommendation: **few high-leverage tools with rich parameters.** Six or seven tools, not twenty tiny ones.

Tool descriptions should be optimized for **agent selection accuracy**, especially smaller models and subagents. Operator readability is secondary. Descriptions should be short, imperative, and include “use this when…” language.

### Minimal V1 MCP tools

#### `briefcase_orient`

Use when starting, resuming, or after compaction.

Returns task-aware project orientation, current repo state, recent deltas, active diagnostics, and suggested next reads.

#### `briefcase_find`

Use when locating a symbol, file, route, test, config, command, or framework object.

Inputs:

```text
query
kind optional: symbol | route | file | test | config | command
scope optional
confidence_min optional
```

This replaces a family of `find_symbol`, `find_route`, `find_tests` tools.

#### `briefcase_explain`

Use when the agent needs to understand how something works.

Inputs:

```text
target
target_kind
depth
include_callers
include_callees
include_tests
include_routes
token_budget
```

This is the “how does X work?” tool.

#### `briefcase_impact`

Use before editing or after another agent changes something.

Returns impact radius, dependency changes, affected symbols, affected sessions, and stale observations.

This tool is crucial because it exposes the coordination substrate intentionally.

#### `briefcase_diagnostics`

Use for diagnostic deltas, full diagnostic context, or “what did my edit break?”

Inputs:

```text
scope
since_event_id optional
mode: introduced | resolved | changed | full
```

#### `briefcase_observed_state`

Use when the agent asks “what have I seen?” or “what changed since I last saw X?”

This should expose bounded ledger state. The ledger should not be purely internal. If the product is belief management, the agent needs a mirror.

#### `briefcase_prove`

Use to challenge or inspect a fact, context frame, stale deny, diagnostic attribution, or graph edge.

Inputs:

```text
fact_id optional
context_frame_id optional
deny_id optional
diagnostic_id optional
graph_edge_id optional
```

Outputs source spans, versions, extractor provenance, confidence, and invalidation status.

### Challenge traces

Expose them via MCP. Keeping challenge traces operator-only makes the agent unable to recover in-band. That would be self-defeating.

### Ledger introspection

Expose bounded ledger-state tools. Raw event-log access is operator-only, but “what have I observed?” belongs in MCP.

Axes changed: MCP surface, provenance, false-positive recovery, context scheduling.

------

## 7. Exported-surface fingerprint, precisely defined

Patch the spec here: we need two fingerprints, not one.

```text
contract_fingerprint
implementation_fingerprint
```

The **contract fingerprint** decides hard stale-deny eligibility. The **implementation fingerprint** decides advisory warnings, impact analysis, and strict-mode behavior.

This avoids two bad extremes: denying every dependency body edit, or ignoring meaningful behavior changes.

### Common representation

Each file exports a set of surface items:

```text
SurfaceItem {
  language
  module_id
  export_name
  qualified_name
  kind
  visibility
  signature_repr
  type_repr nullable
  decorators_or_attributes
  route_contract nullable
  source_span
  provenance
  confidence
}
```

Then:

```text
symbol_contract_hash = hash(canonical SurfaceItem contract fields)
file_contract_hash = merkle_hash(symbol_contract_hashes)

symbol_implementation_hash = hash(normalized exported body where available)
file_implementation_hash = merkle_hash(symbol_implementation_hashes)
```

### TypeScript / JavaScript

For TypeScript:

- Primary: canonical `.d.ts`-like projection from the TypeScript compiler API under the project’s real `tsconfig`.
- Include exported types, interfaces, classes, functions, const enums, namespaces, default exports, overloads, generics, public class members, and route handler exports.
- Exclude function bodies.
- Include relevant config inputs: `tsconfig`, path aliases, JSX settings, module resolution mode.

Fallback:

- AST hash of exported declarations.
- For JS with JSDoc or `checkJs`, use inferred declarations where available.
- For plain JS, use export names, arity, default/named export shape, class method names, and decorator/framework markers.

Next.js-specific route contract items should include HTTP method exports, route segment params, middleware config, runtime config, and handler signature where inferable.

### Python

Python cannot be treated like TypeScript wearing a fake mustache.

Contract fingerprint:

- `__all__` if present
- otherwise public module-level functions/classes/constants
- function arg names, positional/keyword shape, defaults presence, annotations
- class public methods and annotated attributes
- dataclass fields
- Pydantic model fields
- FastAPI/Django/Flask route decorators and path/method contracts
- decorators that alter call shape where recognized

Implementation fingerprint:

- normalized AST body hash for exported functions/classes
- route handler body hash
- model validator body hash where relevant

Dynamic fallback:

- If module-level `__getattr__`, monkey patching, star import ambiguity, dynamic route registration, or metaclass magic is detected, mark `surface_confidence = unknown`.
- Unknown surface changes trigger advisory by default, hard deny only in strict mode.

### Go

Use `go/packages` or equivalent export data.

Contract fingerprint:

- exported package identifiers
- function and method signatures
- interface method sets
- struct exported fields and tags
- exported constants and vars with types
- generic type parameters
- build tags and module config

Implementation fingerprint:

- exported function/method normalized body hashes
- init functions as package-level implementation impact

### Rust

Use rust-analyzer/HIR or rustdoc JSON where feasible.

Contract fingerprint:

- public and crate-visible items relative to the importing scope
- functions, structs, enums, traits, type aliases, consts, statics, macros where extractable
- trait method sets
- impl blocks that affect callable public surface
- feature flags and cfg conditions
- module path and visibility

Implementation fingerprint:

- normalized bodies of public functions/methods
- macro definitions separately marked lower confidence unless expanded

### Java / C# / Kotlin / Swift tier

For P0-beta:

- public/protected API shape
- class/interface/trait/protocol members
- annotations/attributes affecting routing, injection, serialization
- generic signatures
- package/module visibility rules
- framework route annotations

### Dynamic languages

Recommendation: **conservative for advisory, strict for hard deny.**

For Ruby/PHP/Lua/Luau and dynamic-heavy Python/JS regions:

- Any change to recognized exported names or route declarations can hard-deny if confidence is high.
- Any non-comment code change in an imported dependency with unknown surface triggers advisory.
- Strict mode may hard-deny unknown-surface dependency changes.
- Baseline mode should not hard-deny on “unknown dynamic magic” alone.

Hard denies should require verified or resolved contract change, not vibes in a trench coat.

------

## 8. Hook protocol portability

The daemon should expose one internal hook contract. Adapters translate harness weirdness into this contract.

### Lowest-common-denominator daemon events

```text
SessionStart
SessionEnd
ToolIntent
ToolResult
ReadObserved
EditIntent
EditApplied
CommandIntent
CommandResult
CompactIntent
VcsStateChanged
AdapterHeartbeat
```

### Daemon response type

```text
DaemonDecision {
  decision_kind:
    allow | deny | advisory | decorate | replace_result | modify_input | observe_only

  context_frames[]
  replacement_result nullable
  modified_input nullable
  deny_reason nullable
  revalidation_instructions[]
  confidence
  expires_at nullable
}
```

### Capability bitset

Every adapter must register capabilities:

```text
can_pre_edit_block
can_pre_read_decorate
can_post_read_decorate
can_command_replace
can_modify_tool_input
can_async_notify
can_precompact
can_report_token_usage
can_report_exact_edit_diff
can_attach_file_precondition
```

The daemon must not assume Claude-style hooks everywhere. It should compute the best behavior allowed by the capability bitset.

### Where deny is unsupported

Fallback is:

1. prominent advisory injected into the nearest available context surface
2. structured re-read instruction
3. async notice if supported
4. post-edit diagnostic and stale-risk annotation if the edit already happened

Do not pretend advisory equals enforcement. Metrics must distinguish:

```text
stale edit denied pre-write
stale edit warned pre-write
stale edit detected post-write
stale edit missed
```

### Harnesses with only post-hoc observation

There is still a meaningful Code Briefcase experience:

- session tracking
- read/context decorations if possible
- MCP graph tools
- diagnostics dedup
- metrics
- post-hoc stale detection

But they are not “coordination-grade.” Publish that clearly.

### Capability matrix

Yes. This should be product surface.

Example tiers:

```text
Gold: pre-edit deny + read decoration + command replacement + PreCompact
Silver: advisory pre-edit + read decoration + MCP
Bronze: post-hoc observation + MCP + metrics
```

Operator trust depends on knowing which guarantees are real.

------

## 9. Index cold-start strategy on large repositories

Recommendation: **streaming partial graph plus heuristic pre-warm, fail-open always.**

The first session should feel useful in seconds, not “come back after the monorepo has finished digesting the moon.”

Default behavior:

1. Start daemon immediately.
2. Return hook responses within latency budgets.
3. Emit a one-time “index warming” notice.
4. Prioritize:
   - files named in the task prompt
   - files read by the agent
   - git-recent files
   - package manifests and framework configs
   - route entrypoints
   - test entrypoints
   - import neighborhoods around touched files
5. Return partial graph facts with coverage markers.
6. Background index the full repo.

Facts should carry:

```text
coverage: partial | complete | unknown
graph_version
indexed_at
provenance
```

Read decorations during cold start should say:

```text
Partial graph: imports resolved for this file, callers still indexing.
```

### Benchmark visibility

Cold-start should be public benchmark surface, but split into separate metrics:

- time to daemon ready
- time to first useful decoration
- time to first enforcement-grade stale check
- time to 80 percent graph coverage
- time to full graph coverage

Full cold index time is high variance, but hiding it would let the product lie by omission. The operator cares about first useful behavior more than total digestion time.

------

## 10. Symbol identity stability across renames

Use a **two-ID model**:

```text
SymbolOccurrenceID
SymbolLineageID
```

`SymbolOccurrenceID` identifies the current graph node. It can change when name, path, module, or span changes.

`SymbolLineageID` identifies the conceptual symbol across rename/move events when confidence is high.

### How lineage is inferred

Use a hybrid matcher:

- explicit LSP rename events when available
- git rename/move similarity
- same enclosing module lineage
- same or similar normalized body hash
- same signature shape
- references updated in same edit window
- old symbol disappears and new symbol appears nearby
- framework route contract preserved

Do not use AST position alone. It breaks on formatting and movement. Do not use content hash alone. It breaks on real edits. Do not rely solely on qualified name. It breaks on renames.

### Continuation semantics

If α previously observed `validateSession`, and β renames it to `validateUserSession`, the old observation does **not** remain freshness-valid for editing callers. But the continuation briefing can say:

```text
Previously observed symbol validateSession appears to have been renamed
to validateUserSession by session β. Contract unchanged, name/import path changed.
Re-read affected callers before editing.
```

So lineage helps the agent orient. It does not erase staleness.

### Multi-agent semantics

A rename is an exported-surface change even if behavior is unchanged. Callers can break. Imported names can break. The working set must be marked stale.

### Benchmark semantics

Auto-research labeling should use `SymbolLineageID`. Otherwise a successful rename refactor looks like deleting X and creating unrelated Y.

------

## 11. False-positive deny recovery

The runtime model should be deliberate and humane. A false-positive deny should feel like a speed bump with a receipt, not a locked door guarded by a bureaucratic toaster.

### Deny object

Every deny returns:

```text
DenyDecision {
  deny_id
  severity: hard | soft
  target_file
  current_target_version
  stale_observation_versions
  dependency_causes[]
  minimum_revalidation_set[]
  proof_available
  override_policy
}
```

### Recovery path

#### Step 1: Revalidate minimum set

The deny tells the agent exactly what to read or query:

```text
Read services/auth.ts at current version
Then retry edit.
```

Once the agent observes the dependency’s current version, the same cause cannot deny again. The agent has incorporated the new world state.

#### Step 2: Challenge/recheck

The agent can call:

```text
briefcase_prove(deny_id)
```

or:

```text
briefcase_impact(target_file, since_observation)
```

This can trigger a fast reindex of the disputed dependency if the graph is stale or low-confidence.

#### Step 3: Structured override

Allow override, but only with gates.

```text
OverrideDeny {
  deny_id
  agent_session_id
  observed_dependency_versions[]
  rationale
  requested_scope
}
```

Baseline mode accepts override only if the agent has observed the current versions in the `minimum_revalidation_set`. Strict mode requires operator approval or verified proof.

### Preventing agents from overriding everything

Use four controls:

1. **Observation prerequisite:** no override before current dependency observation.
2. **Scope limit:** override applies only to the same target file and same dependency versions.
3. **Budget:** repeated overrides by a session degrade trust and show in metrics.
4. **Benchmark penalty:** unnecessary overrides count as coordination failures or near-failures.

### Deny loop guard

Yes, add a deny budget.

The daemon must not deny the same `(session, target_file, dependency_version_set, cause)` repeatedly after the agent has revalidated the requested files. After one revalidation, it must either:

- allow
- downgrade to advisory
- produce a new cause with a new deny ID
- escalate to strict/operator policy if configured

This is the critical UX patch. Without it, one bad graph edge can trap an agent in a haunted revolving door.

------

## 12. Operator visibility and debugging surface

Recommendation: **P0 CLI, P1 local read-only web UI, structured logs underneath.** MCP operator tools can exist, but they should not be the primary debugging interface.

### P0 CLI

Minimum commands:

```text
briefcase status
briefcase daemon doctor
briefcase sessions list
briefcase sessions show <session_id>
briefcase ledger tail --session <session_id>
briefcase observations show <session_id>
briefcase context show <context_frame_id>
briefcase deny explain <deny_id>
briefcase graph explain <path-or-symbol>
briefcase diagnostics delta --since <event_id>
briefcase metrics report
briefcase metrics tail
briefcase replay decision <event_id>
```

The two most important commands are:

```text
briefcase deny explain <deny_id>
briefcase context show <context_frame_id>
```

If an operator cannot answer “why did the system say that?” in under thirty seconds, the product will feel haunted.

### P1 local web UI

A read-only local web UI is worth it because multi-agent timelines are spatial. It should show:

- session lanes
- reads
- edits
- denies
- diagnostics
- context frames
- graph invalidations
- VCS epoch changes
- watcher health

Think flight recorder, not IDE.

### Structured logs

Logs are necessary for support and benchmarks, but insufficient for humans. “Just grep the logs” is how systems become folk religions.

------

## 13. Daemon failure modes and recovery

Single daemon per project is right, but it needs a real supervision and fencing model.

### Daemon crashes mid-job

Adapters should use short timeouts and fail open.

Behavior:

- Reads, commands, and edits proceed.
- Adapter emits a brief degraded notice.
- Adapter writes a tiny local spool of observed events.
- On reconnect, the adapter replays buffered observations.
- Pre-edit coordination is marked unavailable.
- Strict mode may fail closed for configured safety gates.

The daemon should resume from the event log and rebuild materialized views.

### Storage corruption

Use append-only event log plus materialized views.

Recovery order:

1. Try to open current materialized views.
2. If corrupt, rebuild views from event log.
3. If event log tail is corrupt, truncate to last valid event boundary and quarantine the corrupt tail.
4. If the whole store is corrupt, quarantine it and start fresh in degraded mode.

Never silently claim continuity after losing ledger history. Say:

```text
Ledger continuity lost. Coordination guarantees degraded until sessions reobserve files.
```

### Two daemons launch

Use project lease fencing:

- canonical project identity
- socket path derived from identity
- atomic lock acquisition
- daemon generation ID
- heartbeat file
- DB writer lease token

The loser exits after forwarding the client to the winner. If split-brain is detected, only the daemon with the active DB lease can write. The other becomes read-only and shuts down.

### Watcher wedged

Watcher health must be explicit.

Detection:

- periodic reconciliation scan
- sentinel file check
- compare hook-observed file hashes against watcher state
- timeout on expected invalidation events

Recovery:

- restart watcher
- switch to polling fallback
- mark graph freshness degraded
- continue target-file hash checks on demand
- suspend hard dependency denies if dependency freshness cannot be trusted

Agent-facing notice:

```text
File watcher degraded. Target-file freshness is checked on demand;
dependency graph freshness is partial. Stale-edit enforcement downgraded.
```

Do not let a wedged watcher quietly poison the graph.

------

## 14. Anything missing

Yes. I would patch five things into the spec before implementation planning.

### 14.1 Atomicity between pre-edit allow and actual write

This is the biggest missing systems issue.

A pre-edit check can pass, then another agent can edit the file before the write lands. Without atomicity, there is a TOCTOU gap.

Patch:

- Every edit decision should include an expected target file hash.
- Adapters that can attach file preconditions must do so.
- After edit, daemon verifies the edit applied to the expected previous hash.
- If not, mark `edit_race_detected`.
- Strict mode denies or forces re-read.
- Non-strict mode emits a high-priority warning.

Long term, the wild repo-kernel model solves this with brokered writes or leases. V1 needs at least precondition verification.

### 14.2 Prompt-injection and untrusted repo content

The system will put code-derived facts into an agent’s context. Code comments, markdown, tests, and fixtures can contain adversarial instructions.

Patch:

- Treat repo text as untrusted data.
- Decorations must separate system facts from quoted repo content.
- Never let extracted comments become imperative instructions.
- Provenance labels should say when content is quoted from repo.
- MCP responses should fence code excerpts and avoid instruction-like phrasing from source comments.

This matters because Code Briefcase becomes a context amplifier. It must not amplify poisoned comments into agent commands.

### 14.3 Secret and privacy retention in the event log

The event log can accidentally become a treasure chest for secrets.

Patch:

- Store hashes and spans by default where full text is unnecessary.
- Redact common secret patterns in logs.
- Keep code excerpts optional and bounded.
- Add retention policy:
  - benchmark mode stores more
  - normal mode stores less
  - operator can purge per project/session
- Never send private code off-machine without explicit opt-in.

### 14.4 Generated/vendor file policy

Large repos contain generated files, vendored dependencies, lockfiles, build outputs, migrations, snapshots, and codegen artifacts. Indexing all of them naively pollutes the graph and kills cold start.

Patch:

```text
SourceClass:
  source
  test
  generated
  vendored
  build_artifact
  config
  lockfile
  migration
  fixture
  unknown
```

Each class has different indexing and staleness rules. Generated files may matter for diagnostics but should rarely drive hard stale denies.

### 14.5 Task identity as a first-class primitive

The ledger has sessions, but tasks are the operator’s unit of intent. A long session can do multiple tasks. A task can span multiple sessions. Multi-agent benchmarks assign tasks, not just sessions.

Patch:

```text
Task {
  task_id
  root_session_ids[]
  prompt_hash
  task_summary
  assigned_files optional
  assigned_symbols optional
  created_at
}
```

Context scheduling, continuation briefings, metrics, and multi-agent coordination all improve when the daemon knows which observations belong to which task.

## My revised position in one sentence

Patch the spec so **ObservationLedger, ContextFrame ledger, repo epochs, task identity, and file-version preconditions** form the core substrate; the graph then becomes the truth oracle feeding that substrate, not the architectural sun everything orbits.