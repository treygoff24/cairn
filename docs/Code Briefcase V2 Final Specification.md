# Code Briefcase V2 — Final Specification

This is the V1 architecture and feature specification for Code Briefcase V2: a local-first coordination and context substrate for AI coding agents working inside real software repositories. It supersedes the candidate spec at `Code Briefcase V2 Candidate Specification.md`, which is retained for history. Open questions from the candidate round have been resolved through a follow-up review.

Audience: implementers, the sub-agent swarm building V1, and the operator (Trey) running the system.

---

## 1. Product summary

Code Briefcase V2 is a local-first coordination and context substrate for AI coding agents working inside real software repositories. Its user is the agent, not the human developer. The operator benefits indirectly: the agent spends fewer tokens discovering the codebase, makes fewer stale or structurally wrong edits, completes verified software tasks with less babysitting, and can run multiple sessions in the same working tree without overwriting one another.

The product is **not** primarily a repository index. It is a **versioned belief-management system for AI agents operating on code.** A code graph tells you what is currently true about the repository. The harder and more valuable question is: *what does this particular agent session currently believe, and has that belief expired?*

That distinction is the spec's design pin. **Graph facts are inputs to belief management; they must not become the primary API surface.** Every other architectural choice in this document follows from that framing.

The central design bet is that AI coding agents need a **pushed, versioned, provenance-aware substrate** around their ordinary tool calls, not merely another pull-mode "ask me for context" tool. The system should know what each agent has seen, what has changed since then, what symbols and routes matter, which diagnostics were introduced by the latest edit, and when another agent's work has made the current agent's assumptions obsolete. In the best version, the repo becomes a live control surface: every read is annotated, every edit is checked against observed reality and a precondition hash, every diagnostic is delta-filtered, and every agent session participates in one shared local memory.

---

## 2. Outcome principles, ranked

### 1. Verified task success must improve or at minimum not regress

**Outcome:** Agents complete real coding tasks correctly more often, measured by tests, type-checks, lint checks, benchmark-specific assertions, and final diff review oracles.

**Metrics that prove it:**

- Verified task success rate improves by at least 5 percentage points on the benchmark corpus, or remains within 2 percentage points of baseline while materially reducing cost.
- Regression rate from incorrect pushed context stays below 1 percent of benchmark tasks.
- Final repo state passes the same acceptance checks as the no-system control.

**Violated when:** Token savings improve but verified task success drops by more than 2 percentage points, or the system causes agents to take confidently wrong paths because it supplied misleading structure.

### 2. Agent work per successful change must fall sharply

**Outcome:** The agent spends fewer tokens, fewer tool calls, and less wall-clock time per verified successful task.

**Metrics that prove it:**

- Tokens per successful task decrease by at least 30 percent on large unfamiliar repo tasks.
- Tool calls per successful task decrease by at least 35 percent on exploration-heavy tasks.
- Raw `Read`, `Grep`, and broad `rg` calls decrease by at least 40 percent on symbol-navigation tasks.
- The system's own pushed-context tokens consume less than 25 percent of gross tokens saved.

**Violated when:** The system adds clever metadata but net tokens per successful task decrease by less than 10 percent, or pushed context becomes visible token confetti.

### 3. Stale-context edits must be caught before they hit disk

**Outcome:** In multi-agent sessions, an agent should not silently edit a file based on an obsolete view of that file or its static dependency neighborhood, *and* the edit-allow decision must hold from approval through write.

**Metrics that prove it:**

- At least 90 percent recall for benchmark-injected stale dependency edits.
- False-positive dep-staleness denies below 5 percent in baseline mode and below 2 percent in strict mode.
- Self-edit false-positive rate below 0.5 percent.
- At least 85 percent of parallel-agent task pairs complete without downstream textual merge conflicts or type-check failures attributable to inter-agent obsolescence.
- TOCTOU (time-of-check to time-of-use) edit-race detection above 95 percent recall when adapters support precondition attachment.

**Violated when:** Agent α edits against a dependency that agent β modified after α's relevant observation, and the system neither denies nor prominently warns before the write — or when an allow decision is granted, β edits the target file before α's write lands, and the system doesn't detect the race post-write.

### 4. Diagnostics must be delta-relevant

**Outcome:** After an edit, the agent sees what this edit introduced, resolved, or changed, not the project's entire historical diagnostic swamp.

**Metrics that prove it:**

- Introduced diagnostic recall at least 95 percent against full before/after diagnostic ground truth.
- Introduced diagnostic precision at least 90 percent.
- Pre-existing unrelated warnings suppressed with at least 98 percent precision.
- Redundant diagnostic invocations avoided across parallel agents by at least 60 percent in multi-agent benchmarks.

**Violated when:** The agent receives a 200-warning dump and has to infer which two errors matter, or when introduced errors are hidden by an over-aggressive filter.

### 5. Pushed context must earn its bytes

**Outcome:** Every pushed decoration should be novel, relevant, bounded, and likely to change the agent's next action.

**Metrics that prove it:**

- Duplicate decoration token ratio below 10 percent per session.
- At least 30 percent of pushed file or symbol pointers are subsequently read, edited, or referenced by a tool call within the task.
- Per-read pushed context stays below 600 tokens at p95, unless the agent explicitly asks for more.
- SessionStart orientation stays below 1,200 tokens at p95.

**Violated when:** The system repeats the same nav map every time the agent reads adjacent files, or emits low-value structure so often that the agent learns to ignore it.

### 6. Confidence and provenance must be visible

**Outcome:** The agent can distinguish compiler-backed facts, LSP-backed facts, tree-sitter structural facts, framework-convention inferences, regex heuristics, and speculative cross-file guesses.

**Metrics that prove it:**

- High-confidence graph edges have at least 95 percent precision in gold repos.
- Heuristic framework detections include confidence and provenance 100 percent of the time.
- Confidence calibration expected calibration error below 0.10 across labeled extraction tasks.
- Route detection false-positive rate below 3 percent for "high confidence" route claims.

**Violated when:** A 70 percent regex guess is presented in the same tone as a type-checker-derived fact.

### 7. The system must never brick the agent

**Outcome:** When the system is slow, broken, unavailable, or partially unindexed, ordinary agent tool calls continue.

**Metrics that prove it:**

- Hook-induced tool-call failure rate below 0.1 percent.
- Read hook p95 latency below 35 ms warm, p99 below 120 ms.
- Pre-edit staleness check p95 below 25 ms warm, p99 below 75 ms.
- MCP indexed symbol query p95 below 150 ms.
- Fail-open notice emitted in 100 percent of degraded read/orientation cases.

**Violated when:** The agent cannot read, write, or run commands because the context substrate is unhealthy, except in explicitly enabled strict safety modes.

### 8. One local substrate must serve every integration surface

**Outcome:** Hooks, MCP calls, diagnostics, indexers, and multi-agent coordination all read and write the same project state.

**Metrics that prove it:**

- One daemon, one watcher set, one diagnostic cache, and one graph index per project at steady state.
- Zero duplicate diagnostic workers for the same config and repo.
- Hook and MCP answers agree on file versions and symbol locations in at least 99.9 percent of audited queries.
- Cross-agent observation ledger coverage above 99 percent for supported tool calls.

**Violated when:** Claude hooks, Codex MCP, and Cursor integration each maintain their own cache, watcher, and stale view of the repo.

### 9. Every important behavior must be mechanically measurable

**Outcome:** The system can be improved by an unattended auto-research loop using objective, scalar feedback.

**Metrics that prove it:**

- 100 percent of benchmark tasks produce structured ledgers with tokens, tool calls, hook latency, diagnostics, graph queries, staleness events, final success, and failure labels.
- At least 100 benchmark iterations can run unattended with less than 2 percent infrastructure failure.
- Each major feature has an ablation flag, so before/after impact can be isolated.
- Benchmark result variance is low enough to detect a 5 percent improvement with p < 0.05 over repeated runs.

**Violated when:** A change "feels better" but cannot be evaluated automatically, or when benchmark noise overwhelms the signal.

---

## 3. Macro-architectural axes

### Axis 1: Integration delivery model

**Conservative — pull-mode MCP server only.** Tools like `briefcase_find`, `briefcase_explain`, `briefcase_impact`, `briefcase_diagnostics`. Agents benefit only when they choose to call them. Simple, compatible with many harnesses, avoids context spam. Fails during blind sub-agent exploration, where raw `Read` / `Grep` / `Bash` still dominate.

**Baseline — hybrid push plus pull.** Hooks push small, high-confidence, context-budgeted decorations at the moments where agents already act: `SessionStart`, `Read`, `Edit`, `PreEdit`, `PostEdit`, `Bash`, `PreCompact`. MCP remains available for explicit deep queries. Push is used for hot-path facts: file structure, route maps, changed diagnostics, stale-context warnings, known-symbol replacements for over-grep. Pull is used for expanded explanations and larger graph traversals.

**Wild — predictive context governor.** Learns per-harness and per-agent behavioral policies from benchmark traces, predicts the agent's next action, precomputes the smallest useful context packet. Includes "shadow context" mode where candidate decorations are logged but not shown, then scored by whether the agent later needed that information.

**Recommended: baseline hybrid model.** Design the context scheduler so the wild policy can plug in later. Hard line: hooks carry bounded, high-confidence, low-latency context; MCP carries intentional, larger, on-demand graph answers.

### Axis 2: Process model and state ownership

**Conservative — per-harness process-local integration.** Each agent harness launches its own helper process. Structurally fails the multi-agent case: three agents become three watchers, three caches, three diagnostic workers, zero shared understanding.

**Baseline — one daemon per project (worktree).** A single local daemon owns the project's index, watcher set, diagnostic workers, session registry, edit ledger, context ledger, and MCP state. Harness integrations are thin clients. Every attached agent session gets a unique `agent_session_id`. The daemon is identified by canonical project root + git worktree + config hash + protocol version. If the daemon is unavailable, thin clients fail open for ordinary reads and commands.

**Wild — local codebase kernel.** All agent I/O is virtualized through a project-local broker that owns file leases, command deduplication, structured diffs, environment snapshots, and sandboxed shell execution. Unlocks the most complete coordination layer, but risks becoming an agent harness of its own.

**Recommended: one daemon per worktree.** The daemon is the product's spine, not an optional optimization. The wild "repo kernel" should influence interface design (especially leases and active broadcasts), but V1 should not require full tool virtualization.

### Axis 3: Multi-agent coordination model

**Conservative — advisory-only freshness warnings.** Track sessions, file versions, dependency staleness. Warn on pre-edit, never deny.

**Baseline — enforced dep-graph staleness check with three modes.** The daemon records observations in an edit ledger keyed by:

```
(agent_session_id, file_path, file_version, repo_epoch_id, observed_at, context_frame_ids)
```

At each pre-edit checkpoint:

1. What file is the agent trying to edit?
2. What version of that file has this session observed (directly or inherited from parent)?
3. What static dependency set does the graph assign to the target file?
4. Have any files in that dependency set been modified by a different agent session since this session's last relevant observation?
5. Did the modification affect the **contract fingerprint** of an exported surface, an imported symbol, a route contract, a type signature, a framework registration, or a known cross-language bridge?

If stale, return `DaemonDecision { decision_kind: deny }` where the harness supports deny. Where deny is unavailable, emit `advisory` with a structured re-read instruction. Self-edits from the same session are not flagged.

Three modes:
- **Default:** deny when adapter supports it and stale dependency is confirmed by file version plus dependency edge and contract-fingerprint change. Advisory elsewhere.
- **Strict:** fail closed for confirmed stale-context edits and certain ledger failures.
- **Advisory compatibility:** warn-only for harnesses that cannot tolerate denial.

Subagent inheritance is lineage-aware copy-on-spawn (see §6 "Symbol identity" sibling section §3.1 below).

**Wild — active coordination layer.** Short-lived file/symbol leases across read-to-edit windows. Broadcasts impact events to overlapping working sets. Plan-aware coordination. Shared test/diagnostic scheduling. Safe-parallelism recommendations.

**Recommended: baseline enforced check** as the V1 floor, with the wild data model already in place (ledger events support symbol-level impact, leases, broadcasts, task labels even if V1 only uses file-level and contract-level checks). Retrofitting the data model later is dental surgery.

### Axis 3.1: Subagent (child-session) inheritance

When session α spawns subagent α′ (via Claude Code's `Task`, Codex CLI's worker spawn, Cursor's parallel agents, etc.), the daemon uses **lineage-aware copy-on-spawn plus a subagent orientation packet.** Not live-shared view (creates conceptual coupling), not blank (wastes the parent's investment), but a snapshot with explicit provenance.

#### Session identity

```
Session {
  agent_session_id
  root_session_id
  parent_session_id   nullable
  spawn_event_id      nullable
  lineage_depth
  harness
  task_id             nullable
  capabilities
  created_at
}
```

If α spawns α′:

```
α.agent_session_id   = s_100
α′.agent_session_id  = s_143
α′.parent_session_id = s_100
α′.root_session_id   = s_100
α′.lineage_depth     = 1
```

The child is a real session, not a thread inside the parent.

#### Inherited observation snapshot

```
InheritedObservation {
  child_session_id
  inherited_from_session_id
  inherited_from_observation_id
  file_path
  file_version
  repo_epoch_id
  graph_version
  observed_at_original
  inherited_at
  observation_kind
  observation_fidelity
  source_context_frame_id   nullable
}
```

Child's effective observation set is the UNION of direct observations and inherited observations. Origin is preserved for staleness, provenance, and benchmark analysis.

#### Subagent orientation packet

The child receives a compact `subagent_orientation_packet` at spawn — not the parent's whole transcript. Contents:

- Current task slice
- Parent's relevant working set
- Files and symbols already explored
- Stale risks already known
- Key ContextFrame IDs from the parent
- "Changed since parent saw this" warnings
- Suggested first reads or MCP calls

#### Staleness semantics

```
last_relevant_observation_time =
  max(direct observation time, inherited_at for inherited observations)
```

But the observed file version remains the parent's V1. If β edited V1 → V2 after the parent's observation, the daemon either:
- Excludes the stale inherited fact from the orientation packet, or
- Marks it stale inside the packet with the new version label.

A child never inherits already-rotten fruit without a label.

#### Parent never auto-learns from child

Child observations do **not** automatically become parent observations. The parent did not see what the child read. When the child returns, the parent gets a `subagent_report_context_frame` with summary facts and pointers. Those become parent observations *of the report*, not of the underlying file contents. Without this rule, the parent acts as if it read files it never saw.

### Axis 4: Code intelligence and knowledge graph model

**Conservative — syntax index plus text search.** Tree-sitter for outlines; FTS for everything else. Too shallow for semantic obsolescence.

**Baseline — two-pass structural graph with two fingerprints per file.** Pass 1: cheap extraction (files, modules, symbols, imports, exports, framework hints, cross-language bridge declarations where obvious). Pass 2: resolution (import/export resolution, symbol references, static dependency sets, framework route maps, **contract fingerprints and implementation fingerprints**, confidence-scored call edges, language-specific resolver plugins using LSP/compiler outputs).

The two-fingerprint distinction is critical: see §5.

**Wild — temporal semantic graph.** Time-aware, confidence-calibrated codebase twin storing not only current symbols and edges but how they changed over time, which agents observed which graph versions, which edges historically predicted successful navigation.

**Recommended: baseline two-pass structural graph with temporal + provenance-bearing edges from day one.** The graph does not need to be fully semantic to be useful, but it must know enough to support dependency staleness and delta-oriented context. Syntax-only indexing must not be called "the graph."

### Axis 5: Incremental computation model

**Conservative — file watcher plus ad-hoc invalidation.** Invalidation bugs become safety bugs in a multi-agent setting.

**Baseline — content-addressed incremental query DAG.** Derived facts as queries keyed by content hash, config hash, tool version, language plugin version, dependency inputs. File parse, symbol extraction, import resolution, route detection, fingerprints, diagnostics, and read decorations become cached query results. Invalidation is dependency-driven.

**Wild — reactive causal graph with replay.** Every file change, hook call, graph extraction, diagnostic result, and context emission becomes an event in a local causality graph. Enables session replay, policy simulation, alternative scheduling against historical traces.

**Recommended: baseline content-addressed query DAG, backed by an append-only event log sufficient for replaying coordination and context decisions.** Salsa is the recommended incremental engine, wrapped behind a `briefcase-incremental` crate so the blast radius of swapping it later is one crate.

### Axis 6: Language and framework extraction architecture

**Conservative — built-in extractors in core binary.** One giant code-intelligence monolith. Parallel sub-agent development becomes harder.

**Baseline — versioned extractor contracts.** Language and framework intelligence is modular behind stable internal contracts:

- Grammar adapters → syntax nodes and spans
- Language extractors → symbols, scopes, imports, exports, type-like surfaces
- Resolvers → dependency edges, symbol references
- Framework extractors → routes, handlers, middleware, controllers, migrations, templates, tests, config, conventions
- Bridge extractors → Swift ↔ Obj-C, React Native bridges, TurboModules, Fabric, Expo Modules, JNI bindings, FFI declarations

Every emitted fact includes:

```
fact_type
source_span
confidence
provenance
extractor_version
input_hash
```

**Wild — extractor foundry.** Agents generate or mutate extractor rules, run them against labeled repos, keep improvements that increase precision and recall.

**Recommended: versioned extractor contracts with benchmark fixtures for every supported language and framework.** Extractor foundry is a future R&D loop; V1 privileges boring, auditable extraction.

#### P0-α vs P0-β language tiers

P0 ships **coverage broad, enforcement narrow.** Three tiers:

- **Tier 1 (enforcement-grade):** TypeScript/JavaScript, Python, Go, Rust. Full dep-staleness enforcement, contract + implementation fingerprints, read decorations, symbol lookup, diagnostic deltas.
- **Tier 2 (navigation-grade):** Java, C#, PHP, Ruby, C, C++, Objective-C, Swift, Kotlin, Dart. Symbol resolution and read decorations; no hard semantic denies.
- **Tier 3 (syntax-outline fallback):** Lua, Luau, Scala, Pascal/Delphi, Elixir, Clojure, and anything tree-sitter can parse. File inventory, outlines, comment/string-aware text search, basic symbol-ish spans.

Gating signal for P0-β extractor expansion:

- ObservationLedger / EditLedger APIs stable across at least two benchmark cycles.
- P0-α false-positive stale denies below 5 percent.
- ContextFrame duplicate-token ratio below 10 percent.
- Adding a new extractor does not require core daemon changes.
- Extractor fixture harness can measure precision/recall automatically.

The trap is letting language breadth consume the substrate. The substrate is the product spine. Extractors are organs.

### Axis 7: Diagnostics integration model

**Conservative — run full commands after edits.** Slow, noisy, redundant.

**Baseline — persistent diagnostic workers plus delta attribution.** One `tsc --watch`, one lint worker, one test discovery worker, etc. per project config. Diagnostics cached by:

```
(file_hash, relevant_dep_hashes, config_hash, tool_version, diagnostic_profile)
```

Before/after snapshots identify introduced, resolved, changed, and pre-existing unrelated diagnostics. Post-edit responses prefer cached or watch-derived results.

**Wild — causal diagnostic oracle.** Learn which edits caused which diagnostics by combining diffs, dependency-graph impact radius, compiler spans, and historical fixes.

**Recommended: baseline persistent diagnostic plus delta model, with enough causal metadata stored to grow toward the wild oracle.** V1 promise is narrow and strong: "only diagnostics this edit changed," with explicit confidence when attribution is uncertain.

### Axis 8: Context scheduling, deduplication, and budgeting

**Conservative — fixed templates.** Spam.

**Baseline — ContextFrame ledger with novelty scoring.** Every emitted context packet is a `ContextFrame`:

```
ContextFrame {
  context_frame_id
  agent_session_id
  task_id              nullable
  triggering_tool_call
  facts_included
  source_versions
  token_count
  confidence
  expires_when
  novelty_hash
  utility_receipt      nullable
}
```

Scheduler checks before emitting: has this agent already seen these facts? Have the facts changed? Is this fact relevant to the file/symbol/task/command? Is confidence high enough to push? Is there budget left for this tool surface? Would a pointer be better than full content?

The scheduler emits deltas, not repeats. Stable IDs for files, symbols, routes, and diagnostics let the agent receive "unchanged since prior read" instead of another full map.

**Wild — information-value market.** Every pushed byte receives a `utility_receipt` later in the session. Pointer-followed, stale-edit-prevented, diagnostic-fixed → credit. Otherwise context debt. Scheduler becomes a bandit policy.

**Recommended: ContextFrame ledger with novelty scoring in V1, utility-receipt fields included now even if the first scheduler is rule-based.** The future auto-research loop must be able to ask: "which emitted facts actually mattered?"

### Axis 9: Durable storage and event model

**Conservative — simple project database.** Weak replay, weak auditing, weak auto-research.

**Baseline — append-only event log plus materialized views, with a three-layer file/epoch/observation model.**

#### File identity

```
FileVersion {
  file_id
  path
  content_hash
  size
  mtime_observed
  executable_bit
  symlink_target   nullable
  repo_epoch_id
  source_class
}
```

`content_hash` is the truth. Same content returning later can be recognized and revalidated.

`source_class` (from §3 cross-cutting; values: `source | test | generated | vendored | build_artifact | config | lockfile | migration | fixture | unknown`) governs indexing and staleness rules.

#### Repo epoch

```
RepoEpoch {
  repo_epoch_id
  worktree_id
  git_common_dir_id    nullable
  head_oid             nullable
  branch_ref           nullable
  index_tree_oid       nullable
  working_tree_digest
  operation_state
  started_at
}
```

`operation_state` values:

```
normal
detached_head
merge_in_progress
rebase_in_progress
cherry_pick_in_progress
bisect_in_progress
unknown_vcs
```

#### Observation

```
Observation {
  observation_id
  agent_session_id
  file_id
  path
  file_version
  repo_epoch_id
  graph_version
  observed_at
  context_frame_id     nullable
  task_id              nullable
}
```

#### Event log

Append-only, including at least:

- Session lifecycle (started, ended, spawned-child)
- Tool intent / tool result
- ReadObserved
- EditIntent / EditApplied
- CommandIntent / CommandResult
- CompactIntent (PreCompact)
- ContextFrame emitted
- File version advanced
- Graph extraction completed
- Diagnostics computed
- Staleness warning emitted / deny issued
- VcsStateChanged
- AdapterHeartbeat
- MCP query answered

#### Materialized views

- Current file versions
- Current graph
- Per-session observations (direct + inherited)
- Per-agent working sets
- Diagnostic cache
- Context dedup state
- Metrics aggregates

#### Branch checkout, stash, merge state

- **Same content hash + same relevant graph inputs:** observation remains valid.
- **Same content hash + different graph/config inputs:** observation is content-valid, graph-stale.
- **Different content hash:** observation is stale.

Stash/restore is working-tree mutation. Observations invalidate on content-hash change. Stash does not fork the ledger.

During `merge_in_progress` / `rebase_in_progress` / `cherry_pick_in_progress`, the daemon enters **VCS_UNSTABLE**:

- Read decorations continue with degraded provenance label.
- Target-file freshness still checked via content hash.
- Dependency-staleness denies downgraded unless graph is known fresh.
- Files with conflict markers or unresolved index stages are high-risk; deniable in strict mode.
- Diagnostics labeled as merge-state diagnostics, not normal project diagnostics.

#### Operational hygiene: credential redaction

The event log applies a single-pass scrub for obvious credential patterns at write time — `AKIA…`, `sk_live_…`, `ghp_…`, `eyJ…` (JWT prefixes), `xoxb-…`, `xoxp-…`, common cloud provider tokens. Purely a foot-gun prevention measure in case the operator pastes a log into a PR or shares a snippet. Not a security feature, not a substitute for `.gitignore` discipline, and not a replacement for the operator's own secret management.

**Wild — local temporal repo twin.** Compact local time machine of codebase state, graph state, diagnostics, observations.

**Recommended: append-only event log plus materialized views with the FileVersion/RepoEpoch/Observation tri-layer.** Minimum model that supports multi-agent coordination, explainable denies, continuation across sessions, and automatic validation.

### Axis 10: Trust, confidence, and provenance

**Conservative — human-readable caveats.** Agents do not reliably parse vibes.

**Baseline — structured provenance on every fact.** Compact provenance markers:

```
[compiler-backed]
[lsp-backed]
[resolved import graph]
[tree-sitter structural]
[framework convention: Next.js app router]
[regex heuristic]
[repo-local pattern]
[stale: source file changed since extraction]
[merge-state: degraded]
[partial graph: still indexing]
```

Confidence tiers:

- **Verified:** safe to drive deny decisions or strong assertions.
- **Resolved:** safe to push as guidance.
- **Inferred:** safe to show with caveat, not to deny alone.
- **Heuristic:** safe as a pointer, not as ground truth.
- **Speculative:** MCP-only or hidden unless asked.

**Wild — challengeable facts.** Agent can ask the system to "prove" any fact. Substrate returns extraction path, source spans, versions, confidence rationale.

**Recommended: structured provenance on every fact, with challenge traces exposed via MCP (`briefcase_prove`) — not operator-only.** Keeping challenge traces operator-only makes the agent unable to recover in-band, which is self-defeating.

### Axis 11: Distribution, availability, and fail-open behavior

**Conservative — separate packages per harness.** Version skew, duplicate cores.

**Baseline — single Rust binary with thin adapters.** One binary owns daemon, indexing, MCP server, hook handlers, diagnostics, metrics. Harness-specific integration files are thin adapters that discover or launch the project daemon and forward events.

Adapters register a capability bitset (see §6 MCP / Hook Protocol). The daemon computes the best behavior allowed by each adapter's capabilities. The product surface includes a **capability tier matrix** so operators know what their harness actually delivers:

```
Tier 1 (enforcing):  pre-edit deny + read decoration + command replacement + PreCompact
Tier 2 (advisory):   advisory pre-edit + read decoration + MCP
Tier 3 (observing):  post-hoc observation + MCP + metrics
```

Availability policy:

- Read and orientation hooks fail open with a short notice.
- Bash interception fails open unless it has a high-confidence replacement.
- PostEdit diagnostics degrade to "queued/unavailable" rather than blocking.
- PreEdit stale checks deny only when the substrate is healthy enough to make a confirmed decision, except in explicit strict mode.
- Strict mode may fail closed for stale-context safety, but must say exactly why.
- Adapter spool: if the daemon crashes mid-job, the adapter writes a tiny local buffer of observed events and replays on reconnect.

**Wild — universal local agent gateway.** All supported harnesses connect through one local gateway that normalizes tool calls, permissions, sessions, metrics, and context.

**Recommended: single Rust binary with thin adapters.** One core substrate, many harness doors.

### Axis 12: Telemetry and auto-research instrumentation

**Conservative — logs and manual reports.** No autonomous improvement loop.

**Baseline — local metrics ledger plus benchmark harness.** Every session writes a local metrics ledger. The benchmark harness can run controlled tasks with feature flags and produce comparable result bundles. Ledger records token counts by message and tool surface, tool call counts, hook latency, ContextFrame emissions, graph query latency and cache hits, diagnostics before/after, stale denies and warnings, agent session IDs and overlap windows, final task success, failure categories.

**Wild — autonomous product research loop.** Agent swarm mutates extraction rules, context scheduling policies, diagnostics attribution, graph heuristics. Runs benchmark suites overnight, accepts changes that improve the headline scalar without violating floors.

**Recommended: local metrics ledger plus benchmark harness as a V1 product feature, not internal scaffolding.** The wild autonomous loop is a first-class future consumer of the metrics architecture.

---

## 4. Cross-cutting failure-mode audit

**Pull-mode trap:** Recommended system is hybrid. Push handles the moments where the agent is already spending context. MCP handles explicit deep queries. Sub-agents benefit because hooks decorate their ordinary tool calls.

**Cold-start latency:** Hot-path budgets are intentionally harsh. They are stated as architectural targets and should be validated against a Rust + Salsa + mmap-snapshot + arena proof-of-concept on a representative repo before being treated as binding floors:

| Surface                       | Warm p95 target | p99 target | Hard behavior                                                |
| ----------------------------- | --------------- | ---------- | ------------------------------------------------------------ |
| `SessionStart` orientation    | 750 ms          | 3 s        | Return partial or defer                                      |
| `Read` decoration             | 35 ms           | 120 ms     | Fail open                                                    |
| `PreEdit` stale check         | 25 ms           | 75 ms      | Deny only on confirmed stale, otherwise fail open unless strict |
| `Bash` search interception    | 40 ms           | 100 ms     | Pass through if uncertain                                    |
| `PostEdit` cached diagnostics | 100 ms          | 300 ms     | Return cached or queued                                      |
| Watch diagnostic delta        | 2 s             | 8 s        | Async result if slow                                         |
| MCP symbol lookup             | 150 ms          | 500 ms     | Return partial with provenance                               |
| MCP graph explanation         | 2 s             | 5 s        | Stream or summarize                                          |

**Cold-start on large repositories:** Default behavior is **streaming partial graph plus heuristic pre-warm, fail-open always.** First session feels useful in seconds, not after the monorepo digests the moon.

1. Start daemon immediately.
2. Return hook responses within latency budgets.
3. Emit a one-time "index warming" notice.
4. Prioritize: files named in the task prompt → files read by the agent → git-recent files → package manifests and framework configs → route entrypoints → test entrypoints → import neighborhoods around touched files.
5. Return partial graph facts with coverage markers.
6. Background-index the full repo.

Facts carry `coverage: partial | complete | unknown`, `graph_version`, `indexed_at`, `provenance`. Read decorations during cold start say "Partial graph: imports resolved for this file, callers still indexing."

Five separate cold-start metrics are public benchmark surface:

- Time to daemon ready
- Time to first useful decoration
- Time to first enforcement-grade stale check
- Time to 80 percent graph coverage
- Time to full graph coverage

**Partial unavailability:** Ordinary reads, shell commands, and edits continue with brief degradation notices. Safety features may fail closed only in explicit strict modes or when a confirmed stale edit is detected.

**Hooks plus MCP state:** There is one substrate. MCP does not own a separate graph. Hooks do not own a separate cache. Both talk to the project daemon.

**Context spam:** ContextFrame ledger dedups by fact identity, source version, novelty hash, and agent session. Repeated facts become "unchanged since last observation," not another blob.

**False precision:** Every fact has confidence and provenance. Heuristics can guide navigation but cannot alone justify deny decisions.

**Single-agent design that fails under N agents:** Process model is one daemon per worktree with one shared ledger, one watcher set, one diagnostic cache, and per-session observation tracking. Multi-agent coordination is architectural bedrock.

**TOCTOU edit race:** A pre-edit allow decision can be granted, then another agent edits the target file before the write lands. Patch: every edit decision includes an `expected_target_file_hash`. Adapters that can attach file preconditions must do so (`can_attach_file_precondition` capability bit). After edit, daemon verifies the edit applied to the expected previous hash. If not, emit `edit_race_detected`. Strict mode denies the next edit or forces re-read; non-strict mode emits a high-priority warning. Long term, the wild repo-kernel model solves this with brokered writes or leases; V1 needs precondition verification.

**Generated and vendored files:** Indexing all of them naively pollutes the graph and kills cold start. The `source_class` enum on `FileVersion` (values listed in §3) governs per-class behavior:

- `source` and `test` participate fully in graph, diagnostics, and stale checks.
- `config`, `lockfile`, `migration` participate in graph but are flagged in diagnostics.
- `generated` and `build_artifact` may matter for diagnostics; rarely drive hard stale denies.
- `vendored` participates in graph for symbol resolution but never drives hard denies on edits to project code.
- `fixture` is navigation-only.
- `unknown` defaults to `source` posture with a confidence demotion.

**Watcher wedge:** Detection via periodic reconciliation scans, sentinel file checks, hash comparison between hook-observed and watcher-state, timeout on expected invalidation events. Recovery: restart watcher → polling fallback → mark graph freshness degraded → continue target-file hash checks on demand → suspend hard dependency denies if dependency freshness cannot be trusted. Agent-facing notice: "File watcher degraded. Stale-edit enforcement downgraded."

---

## 5. Exported-surface fingerprints

The system tracks **two fingerprints** per file, not one:

- `contract_fingerprint` — governs hard stale-deny eligibility.
- `implementation_fingerprint` — governs advisory warnings, impact analysis, strict-mode behavior.

This avoids the two bad extremes: denying on every dependency body edit, or ignoring meaningful behavior changes. Each exported item is a `SurfaceItem`:

```
SurfaceItem {
  language
  module_id
  export_name
  qualified_name
  kind                       // function | class | type | const | route_handler | ...
  visibility                 // public | crate | module | private
  signature_repr
  type_repr                  nullable
  decorators_or_attributes
  route_contract             nullable
  source_span
  provenance
  confidence
}
```

Hashes are computed compositionally:

```
symbol_contract_hash      = hash(canonical SurfaceItem contract fields)
file_contract_hash        = merkle_hash(symbol_contract_hashes)
symbol_implementation_hash = hash(normalized exported body where available)
file_implementation_hash  = merkle_hash(symbol_implementation_hashes)
```

### TypeScript / JavaScript

- **Primary:** canonical `.d.ts`-like projection from the TypeScript compiler API under the project's real `tsconfig`. Include exported types, interfaces, classes, functions, const enums, namespaces, default exports, overloads, generics, public class members, route handler exports.
- **Exclude:** function bodies from the contract hash (they go to implementation hash).
- **Include in inputs:** `tsconfig`, path aliases, JSX settings, module resolution mode.
- **Fallback (no tsconfig / plain JS):** AST hash of exported declarations; for JS with JSDoc or `checkJs`, use inferred declarations.
- **Next.js route contract items:** HTTP method exports, route segment params, middleware config, runtime config, handler signature where inferable.

### Python

- **Contract:** `__all__` if present, otherwise public module-level functions/classes/constants; function arg names, positional/keyword shape, defaults presence, type annotations; class public methods and annotated attributes; dataclass fields; Pydantic model fields; FastAPI/Django/Flask route decorators and path/method contracts; decorators that alter call shape where recognized.
- **Implementation:** normalized AST body hash for exported functions/classes; route handler body hash; model validator body hash where relevant.
- **Dynamic fallback:** if module-level `__getattr__`, monkey patching, star-import ambiguity, dynamic route registration, or metaclass magic is detected, mark `surface_confidence = unknown`. Unknown surface changes trigger advisory by default; hard deny only in strict mode.

### Go

- **Primary:** `go/packages` or equivalent export data.
- **Contract:** exported package identifiers; function and method signatures; interface method sets; struct exported fields and tags; exported constants and vars with types; generic type parameters; build tags and module config.
- **Implementation:** exported function/method normalized body hashes; `init` functions treated as package-level implementation impact.

### Rust

- **Primary:** rust-analyzer / HIR or rustdoc JSON where feasible.
- **Contract:** public and crate-visible items relative to importing scope; functions, structs, enums, traits, type aliases, consts, statics, macros where extractable; trait method sets; impl blocks affecting callable public surface; feature flags and cfg conditions; module path and visibility.
- **Implementation:** normalized bodies of public functions/methods; macro definitions separately marked lower confidence unless expanded.

### Java / C# / Kotlin / Swift (P0-β)

- public/protected API shape
- class/interface/trait/protocol members
- annotations/attributes affecting routing, injection, serialization
- generic signatures
- package/module visibility rules
- framework route annotations

### Dynamic languages (Ruby, PHP, Lua, Luau, dynamic-heavy Python/JS regions)

- Any change to recognized exported names or route declarations can hard-deny if confidence is high.
- Any non-comment code change in an imported dependency with unknown surface triggers advisory.
- Strict mode may hard-deny unknown-surface dependency changes.
- Baseline mode does not hard-deny on "unknown dynamic magic" alone.

**Rule:** Hard denies require verified or resolved contract change, not vibes in a trench coat.

---

## 6. MCP tool surface and hook protocol

### MCP tools — seven tools, not twenty

Tool descriptions optimized for **agent selection accuracy** (especially smaller models and subagents). Operator readability is secondary. Descriptions are short, imperative, and include "use this when…" language.

#### `briefcase_orient`

Use when starting a session, resuming after compaction, or beginning a new task. Returns task-aware project orientation, current repo state, recent deltas, active diagnostics, suggested next reads.

#### `briefcase_find`

Use when locating a symbol, file, route, test, config, command, or framework object.

```
inputs:
  query
  kind          optional: symbol | route | file | test | config | command
  scope         optional
  confidence_min optional
```

Replaces a family of `find_symbol`, `find_route`, `find_tests` tools.

#### `briefcase_explain`

Use when the agent needs to understand how something works.

```
inputs:
  target
  target_kind
  depth
  include_callers
  include_callees
  include_tests
  include_routes
  token_budget
```

The "how does X work?" tool.

#### `briefcase_impact`

Use before editing, or after another agent changes something. Returns impact radius, dependency changes, affected symbols, affected sessions, stale observations.

Crucial because it exposes the coordination substrate intentionally.

#### `briefcase_diagnostics`

Use for diagnostic deltas, full diagnostic context, or "what did my edit break?"

```
inputs:
  scope
  since_event_id  optional
  mode: introduced | resolved | changed | full
```

#### `briefcase_observed_state`

Use when the agent asks "what have I seen?" or "what changed since I last saw X?". Exposes bounded ledger state. If the product is belief management, the agent needs a mirror.

#### `briefcase_prove`

Use to challenge or inspect a fact, context frame, stale deny, diagnostic attribution, or graph edge.

```
inputs:
  fact_id            optional
  context_frame_id   optional
  deny_id            optional
  diagnostic_id      optional
  graph_edge_id      optional
```

Outputs source spans, versions, extractor provenance, confidence, invalidation status.

### Hook protocol — daemon-internal contract

The daemon exposes one internal hook contract. Adapters translate harness weirdness into this contract.

#### Lowest-common-denominator daemon events

```
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

#### Daemon decision

```
DaemonDecision {
  decision_kind:
    allow | deny | advisory | decorate | replace_result | modify_input | observe_only

  context_frames[]
  replacement_result        nullable
  modified_input            nullable
  deny_reason               nullable
  revalidation_instructions[]
  confidence
  expires_at                nullable
}
```

#### Adapter capability bitset

```
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

The daemon must not assume Claude-style hooks everywhere. It computes the best behavior allowed by the capability bitset.

#### Fallback when deny is unsupported

1. Prominent advisory injected into the nearest available context surface.
2. Structured re-read instruction.
3. Async notice if supported.
4. Post-edit diagnostic and stale-risk annotation if the edit already happened.

Advisory ≠ enforcement. Metrics distinguish four states:

```
stale edit denied pre-write
stale edit warned pre-write
stale edit detected post-write
stale edit missed
```

#### Harnesses with only post-hoc observation

A meaningful but reduced experience: session tracking, read/context decorations where possible, MCP graph tools, diagnostics dedup, metrics, post-hoc stale detection. Not "coordination-grade." Surfaced as Tier 3 (observing) in the capability matrix.

---

## 7. Symbol identity model

Use a **two-ID model** for symbols:

- `SymbolOccurrenceID` — identifies the current graph node. Changes when name, path, module, or span changes.
- `SymbolLineageID` — identifies the conceptual symbol across rename/move events when confidence is high.

### Lineage matcher (hybrid)

Lineage is inferred by combining:

- Explicit LSP rename events when available
- Git rename/move similarity
- Same enclosing module lineage
- Same or similar normalized body hash
- Same signature shape
- References updated in the same edit window
- Old symbol disappears and new symbol appears nearby
- Framework route contract preserved

Do not use AST position alone (breaks on formatting and movement). Do not use content hash alone (breaks on real edits). Do not rely solely on qualified name (breaks on renames).

### Continuation semantics

If α previously observed `validateSession`, and β renames it to `validateUserSession`, the old observation does **not** remain freshness-valid for editing callers. But the continuation briefing says:

```
Previously observed symbol validateSession appears to have been renamed
to validateUserSession by session β. Contract unchanged, name/import path changed.
Re-read affected callers before editing.
```

Lineage helps the agent orient. It does not erase staleness.

### Multi-agent semantics

A rename is an exported-surface change even if behavior is unchanged. Callers can break, imported names can break. The working set must be marked stale.

### Benchmark labeling

Auto-research labeling uses `SymbolLineageID`. Otherwise a successful rename refactor looks like deleting X and creating unrelated Y.

---

## 8. False-positive deny recovery

A false-positive deny should feel like a speed bump with a receipt, not a locked door guarded by a bureaucratic toaster.

### Deny object

```
DenyDecision {
  deny_id
  severity                    // hard | soft
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

```
Read services/auth.ts at current version
Then retry edit.
```

Once the agent observes the dependency's current version, the same cause cannot deny again. The agent has incorporated the new world state.

#### Step 2: Challenge / recheck

The agent can call:

```
briefcase_prove(deny_id)
briefcase_impact(target_file, since_observation)
```

This can trigger a fast reindex of the disputed dependency if the graph is stale or low-confidence.

#### Step 3: Structured override

Override is allowed, but gated:

```
OverrideDeny {
  deny_id
  agent_session_id
  observed_dependency_versions[]
  rationale
  requested_scope
}
```

- Baseline mode accepts override only if the agent has observed the current versions in `minimum_revalidation_set`.
- Strict mode requires operator approval or verified proof.

### Four controls against override abuse

1. **Observation prerequisite:** no override before current dependency observation.
2. **Scope limit:** override applies only to the same target file and same dependency versions.
3. **Budget:** repeated overrides by a session degrade trust and show in metrics.
4. **Benchmark penalty:** unnecessary overrides count as coordination failures or near-failures.

### Deny loop guard

The daemon must not deny the same `(session, target_file, dependency_version_set, cause)` repeatedly after the agent has revalidated the requested files. After one revalidation, it must either:

- allow
- downgrade to advisory
- produce a new cause with a new deny ID
- escalate to strict/operator policy if configured

This is the critical UX patch. Without it, one bad graph edge traps an agent in a haunted revolving door.

---

## 9. Operator visibility surface

If the operator cannot answer "why did the system say that?" in under thirty seconds, the product will feel haunted. Visibility is P0.

### P0: CLI

Minimum command set:

```
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

The two most important commands:

```
briefcase deny explain <deny_id>
briefcase context show <context_frame_id>
```

### P1: Local read-only web UI

Worth it because multi-agent timelines are spatial. Read-only. Shows session lanes, reads, edits, denies, diagnostics, context frames, graph invalidations, VCS epoch changes, watcher health. Flight recorder, not IDE.

### Structured logs

Necessary for support and benchmarks, insufficient for humans. "Just grep the logs" is how systems become folk religions.

---

## 10. Feature surface, prioritized

### P0: Identity substrate

Project/worktree identity (canonical path + git common dir + config hash + protocol version), daemon lifecycle, content-addressed file versions, monotonic event IDs, `agent_session_id` / `root_session_id` / `parent_session_id` schema, repo epoch tracking with `operation_state`, adapter capability records. Below the ledger.

**Metrics:**

- One daemon per project worktree in at least 99 percent of tested multi-harness launches.
- Project identity false-match rate below 0.1 percent across worktrees that share working trees.
- Adapter capability registration completeness above 99 percent.

**Failure mode if absent:** Every harness becomes its own island. Ledger writes to nothing.

**Dependencies:** None. The foundation below the foundation.

### P0: Per-project daemon and unified integration substrate

The daemon is the shared local authority for a project worktree. It owns sessions, hooks, MCP, watchers, indexing, diagnostics, ledger, context frames, metrics. Harness adapters are thin clients.

**Metrics:**

- Zero duplicate watchers in steady state.
- Hook-induced tool failure below 0.1 percent.
- Hook and MCP file-version agreement above 99.9 percent.
- Daemon recovery from crash-mid-job: adapters spool, replay on reconnect, no observation loss in 99 percent of recoveries.

**Failure mode if absent:** Multi-agent coordination collapses before it begins.

**Dependencies:** Identity substrate.

### P0: ObservationLedger and EditLedger

Records what each agent session has seen, when it saw it, which file version it saw, what context was pushed, what edits it attempted, which other sessions changed relevant files. Includes the lineage-aware subagent inheritance machinery from §3.1.

**Metrics:**

- At least 99 percent coverage of supported read and edit events.
- Stale file edit detection recall at least 95 percent in injected benchmarks.
- Stale dependency edit detection recall at least 90 percent.
- Self-edit false-positive rate below 0.5 percent.
- Subagent inherited-observation correctness above 98 percent across spawn/edit/return benchmarks.

**Failure mode if absent:** The system knows the repo state but not the agent's state. Freshness, continuation, dedup, multi-agent safety all become impossible.

**Dependencies:** Identity substrate, file versioning, event log.

### P0: Task identity

Tasks are the operator's unit of intent. A long session can do multiple tasks; a task can span multiple sessions; multi-agent benchmarks assign tasks. Context scheduling, continuation briefings, metrics, and coordination all improve when the daemon knows which observations belong to which task.

```
Task {
  task_id
  root_session_ids[]
  prompt_hash
  task_summary
  assigned_files       optional
  assigned_symbols     optional
  parent_task_id       nullable
  created_at
}
```

**Metrics:**

- Task assignment coverage above 95 percent for sessions in benchmark mode.
- Multi-session task continuity benchmarks pass (cross-session memory works) above 90 percent.

**Failure mode if absent:** Session ≠ task creates analytic and coordination ambiguity. Multi-agent benchmarks become noisier than they need to be.

**Dependencies:** Identity substrate.

### P0: Incremental code graph for symbols, dependencies, and routes (P0-α tier)

P0-α coverage: TypeScript/JavaScript, Python, Go, Rust. Files, modules, symbols, imports, exports, static dependency sets, route maps, handlers, middleware chains, tests, framework conventions, **contract and implementation fingerprints**.

**Metrics:**

- Definition lookup precision above 95 percent for high-confidence edges.
- Import/dependency edge recall above 90 percent in typed languages and above 75 percent in dynamic regions for labeled fixtures.
- Framework route high-confidence precision above 95 percent.
- Framework route false-positive rate below 3 percent.
- Graph update p95 below 500 ms for single-file edits in warm repos.
- Contract fingerprint stability across non-semantic edits (whitespace, comments, formatting): change rate below 1 percent.

**Failure mode if absent:** Agent falls back to raw grep. Semantic obsolescence not catchable reliably.

**Dependencies:** Incremental parser/extractor contracts, durable storage, watcher.

### P0: Read decorations and session orientation

Compact orientation at session start; small structural frame on file read; outline, imports/exports, adjacent files, route or handler map, tests, callers, changed facts since last observation.

**Metrics:**

- Scenario A tokens per successful task decrease by at least 30 percent.
- Scenario A tool calls decrease by at least 35 percent.
- Per-read decoration p95 below 600 tokens.
- Duplicate decoration token ratio below 10 percent.
- At least 30 percent of pushed pointers later used.

**Failure mode if absent:** Agent burns the first third of the task reconstructing the repo's shape.

**Dependencies:** Code graph, ContextFrame ledger, context scheduler.

### P0: Pre-edit stale-context arbitration with TOCTOU protection

Before edits, check whether target file or its static dependency set has changed since the agent's last relevant observation, excluding same-session changes. Use `contract_fingerprint` to gate hard denies; use `implementation_fingerprint` for advisories. Issue `DenyDecision` per §8 when stale; emit a `DaemonDecision { decision_kind: advisory }` where deny is unsupported. Attach `expected_target_file_hash` precondition where adapter supports it. After edit, verify the edit applied to the expected previous hash; emit `edit_race_detected` if not.

**Metrics:**

- Stale-context edits caught pre-write per parallel-agent-hour.
- Semantic-obsolescence incidents detected before code review.
- False-positive dep-staleness denies below 5 percent.
- Parallel-agent task pairs without downstream inter-agent type-check failure above 85 percent.
- TOCTOU detection recall above 95 percent on capable adapters.
- Median re-read recovery path below two tool calls.

**Failure mode if absent:** Agents make obsolete-on-arrival edits. Multi-agent safety claim has a race.

**Dependencies:** ObservationLedger, EditLedger, code graph, contract fingerprints, adapter precondition capability.

### P0: Delta diagnostics after edits

Agent sees only introduced, resolved, or changed diagnostics. Pre-existing unrelated diagnostics suppressed. Results shared across sessions when file/config/tool version match.

**Metrics:**

- Introduced diagnostic recall at least 95 percent.
- Introduced diagnostic precision at least 90 percent.
- Unrelated warning suppression precision at least 98 percent.
- Redundant diagnostic-tool invocations avoided by at least 60 percent in N-agent benchmarks.
- Watch-backed first diagnostic delta p95 below 2 seconds for supported stacks.

**Failure mode if absent:** Agents waste context sorting ancient lint rubble.

**Dependencies:** Diagnostic workers, event log, file versions, config detection.

### P0: Bash search interception for known symbols

When an agent runs broad `rg` / `grep` / similar searches for an indexed symbol, replace noisy output with a structured graph answer: definition, callers, route mapping, relevant tests, confidence. Pass through if confidence insufficient.

**Metrics:**

- At least 70 percent of eligible symbol-search commands intercepted on benchmark tasks.
- Token savings per intercepted search above 50 percent versus raw output.
- False interception rate below 2 percent.
- Pass-through rate for uncertain queries above 95 percent.

**Failure mode if absent:** Agents keep buying 2,000-token haystacks to find one needle wearing a name tag.

**Dependencies:** Code graph, Bash hook, confidence model.

### P0: Operator CLI

The full command set from §9. Most critical: `briefcase deny explain`, `briefcase context show`, `briefcase status`, `briefcase daemon doctor`, `briefcase ledger tail`.

**Metrics:**

- Time-to-explanation for an arbitrary deny under 30 seconds in operator usability tests.
- Daemon doctor catches at least 95 percent of injected fault scenarios (corruption, watcher wedge, split-brain).

**Failure mode if absent:** The product feels haunted. False-positive denies become unfixable folklore.

**Dependencies:** Event log, materialized views.

### P0: PreCompact checkpoint primitive (skeleton)

Record `PreCompactCheckpoint` events. Expose resume state through MCP (`briefcase_orient` after compaction). Full survival packets are P1; the skeleton ships P0 so the event log records the boundary correctly.

```
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

**Metrics:**

- PreCompact event capture rate above 99 percent on capable adapters.
- Post-compact orientation latency p95 below 1 second.

**Failure mode if absent:** Long sessions lose continuity across compaction boundaries.

**Dependencies:** Event log, ContextFrame ledger.

### P1: MCP deep context tools (full surface)

The seven tools from §6. P0 ships only `briefcase_orient`, `briefcase_observed_state`, and `briefcase_prove` as a minimal MCP surface. P1 fills in `briefcase_find`, `briefcase_explain`, `briefcase_impact`, `briefcase_diagnostics`.

**Metrics:**

- Deep-query answer precision above 85 percent in labeled graph QA tasks.
- Average exploratory tool calls after MCP answer decrease by at least 30 percent.
- MCP indexed query p95 below 150 ms; expanded graph answer p95 below 2 seconds.
- Agent follow-up correction rate below 10 percent.

**Failure mode if absent:** Hooks help opportunistically, but agents lack an intentional steering wheel.

**Dependencies:** Code graph, provenance, storage, MCP server.

### P1: Continuation delta briefing

When an agent returns to a project, summarize changes since the session's last relevant observations: files changed, files previously edited, exported-surface changes, dependency changes, diagnostics changed, task-relevant deltas.

**Metrics:**

- Continuation tasks reduce initial reread calls by at least 40 percent.
- Relevant changed-file recall above 80 percent.
- Relevant changed-file precision above 60 percent.
- Delta briefing p95 below 1,200 tokens.

**Failure mode if absent:** Returning agents either trust stale memory or re-read the world.

**Dependencies:** ObservationLedger, event log, code graph, task identity.

### P1: PreCompact survival packet (full quality)

What PreCompact emits to the agent — a survival packet that preserves enough symbolic anchors for post-compact recovery without dumping the ledger:

```
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

Pointer-heavy, not content-heavy. The packet's job: "Keep these four facts and these six frame IDs. After compaction, call `briefcase_observed_state` if you need the full ledger slice."

**Metrics:**

- Post-compact tokens-to-resume below 50 percent of cold-orientation baseline.
- Post-compact relevant-file recall above 70 percent.

**Failure mode if absent:** Post-compact agents start essentially blank.

**Dependencies:** PreCompact checkpoint primitive, ContextFrame ledger, working-set tracking.

### P1: Context scheduler and ContextFrame ledger (full)

P0 ships a no-op or template emitter that writes ContextFrames correctly. P1 ships the scheduler proper: budgeting, dedup, prioritization, novelty scoring, utility-receipt capture.

**Metrics:**

- Duplicate decoration token ratio below 10 percent.
- Pushed-context token overhead below 25 percent of gross token savings.
- ContextFrame utility proxy above 30 percent.
- Hook latency remains within surface budgets with scheduler enabled.

**Failure mode if absent:** Push-mode decays into context spam.

**Dependencies:** Event log, graph facts, token estimator, provenance.

### P1: Provenance and confidence display

Every structural claim carries compact provenance and confidence. Deny decisions explainable through facts that are sufficiently trusted.

**Metrics:**

- 100 percent of pushed graph facts include provenance.
- High-confidence fact precision above 95 percent.
- Confidence calibration ECE below 0.10.
- Zero deny decisions based solely on heuristic facts.

**Failure mode if absent:** Agents treat guesses as ground truth.

**Dependencies:** Extractor contracts, graph model, MCP proof traces.

### P1: Metrics ledger and benchmark harness

Local metrics ledger plus controlled benchmark runs with feature ablations.

**Metrics:**

- 100 percent benchmark task metric completeness.
- 100 unattended iterations with less than 2 percent infrastructure failure.
- Feature ablations available for each P0 and P1 feature.
- Result bundles reproducible from event logs and final repo states.

**Failure mode if absent:** Auto-research loop optimizes fog.

**Dependencies:** Event log, harness adapters, token accounting.

### P1: Generated and vendored file policy

Implement the `source_class` enum on `FileVersion` and apply per-class indexing/staleness rules per §4. Recognize lockfiles, generated TypeScript declarations, vendored `node_modules` / `vendor` / `target` / `dist`, codegen outputs, migration files, snapshot test outputs, fixtures.

**Metrics:**

- Source class precision above 95 percent on labeled fixtures.
- Cold-start time reduced by at least 30 percent on dependency-heavy repos vs naive full-index.
- Hard stale denies attributable to generated/vendored file changes: zero in baseline mode.

**Failure mode if absent:** Cold start is slow on big repos. Generated-file churn produces spurious denies.

**Dependencies:** Code graph, file classification heuristics.

### P1: Local read-only web UI

Read-only multi-agent timeline visualization per §9. Flight recorder.

**Metrics:**

- Operator can locate the cause of an arbitrary deny in under 60 seconds via the web UI in usability tests.

**Failure mode if absent:** Operator debugging stays CLI-only, which is functional but spatial reasoning becomes harder.

**Dependencies:** Event log, materialized views, local HTTP server.

### P2: Active cross-agent broadcasts

When a session changes a symbol, route, or exported surface that overlaps another session's observed or in-progress working set, push a notice to the affected session.

**Metrics:**

- At least 70 percent of benchmark-injected overlapping impact events broadcast to affected sessions.
- Broadcast false-positive rate below 10 percent.
- Rework reduction of at least 20 percent in overlapping multi-agent tasks.
- No more than one broadcast per affected session per 60 seconds unless critical.

**Failure mode if absent:** The system catches stale edits only when attempted, not when obsolescence first becomes knowable.

**Dependencies:** Working-set tracking, graph impact radius, session event channels.

### P2: Symbol-level stale checks and leases

Narrow the staleness check to imported symbols or route contracts. Grant short-lived leases for high-conflict files or symbols.

**Metrics:**

- False-positive stale denies decrease by at least 40 percent versus file-level checks.
- Stale-edit recall remains within 2 percentage points of file-level baseline.
- Lease timeout recovery succeeds in 99 percent of cases.
- Agent idle time from leases below 3 percent of wall-clock task time.

**Failure mode if absent:** File-level checks may become too blunt in large, busy files.

**Dependencies:** Symbol resolution, exported-surface fingerprinting, active coordination model.

### P2: Cross-language bridge extractors

Bridge edges for Swift ↔ Obj-C, React Native bridge modules, TurboModules, Fabric, Expo Modules, JNI, FFI declarations. Feed graph and stale-context machinery.

**Metrics:**

- Bridge edge recall above 70 percent for labeled fixtures.
- False-positive bridge edges below 5 percent.
- Bridge-aware stale-context detection recall within 10 percentage points of single-language baseline.
- Provenance coverage 100 percent.

**Failure mode if absent:** Cross-stack edits become a blind spot in exactly the projects where it matters (RN, mixed Swift/Obj-C, native modules in Node).

**Dependencies:** Extractor contracts, code graph, language coverage, provenance model.

### P2: Worktree federation

Detect likely merge conflicts and semantic divergence across separate git worktrees that share a common repo. Advisory by default; no cross-worktree deny in V1.

**Metrics:**

- Cross-worktree conflict detection precision above 80 percent.
- Cross-worktree advisory false-positive rate below 10 percent.

**Failure mode if absent:** Worktree-per-agent setups remain blind to one another.

**Dependencies:** Per-daemon registry, repo-level identity.

### P2: Public dependency and shared graph cache, opt-in

Precomputed graphs for public packages and framework libraries to accelerate cold start. Project-private code never leaves the machine without explicit opt-in.

**Metrics:**

- Cold-start indexing wall time decreases by at least 30 percent on dependency-heavy repos.
- Zero project-private file content included without opt-in.
- Cache hit precision above 99 percent by package version and content hash.

**Failure mode if absent:** Cold start is slower; core product still works.

**Dependencies:** Graph versioning, package identity.

---

## 11. Non-goals

Code Briefcase V2 is not a human IDE. Human-facing UI can exist (the P1 web UI is read-only flight-recorder), but the primary interface is the agent's tool-call stream.

It is not a cloud code-indexing SaaS. Core product is local-first. Cloud or shared public graph features are opt-in extensions.

It is not a replacement for compilers, LSPs, linters, test runners, or CI. It orchestrates, deduplicates, filters, and contextualizes those systems.

It is not a VCS replacement. It understands git state and works across sessions, but does not become a new source-control workflow.

It is not a general autonomous project manager. Multi-agent plan awareness is valuable, but V1 does not assign tasks, negotiate scope, or become a scrum goblin in a tiny helmet.

It is not a promise of perfect semantic understanding across every supported language. Coverage is explicit by confidence tier. A high-confidence TypeScript route edge and a low-confidence Ruby metaprogramming inference must look different to the agent.

It is not a backward-compatible continuation of the V1 Python implementation. V2 is a greenfield product. Old implementation choices can inspire behavior but do not constrain architecture.

It is not an excuse to block ordinary tool calls. Fail-open is the default posture outside explicit safety gates.

It is not "MCP tools, plus some hooks." That product would be too weak.

---

## 12. Validation framework

### Benchmark corpus

Real or realistic repos that collectively stress language breadth, framework conventions, diagnostics, and multi-agent coordination. Should include at least:

- Large TypeScript/JavaScript Next.js monorepo with app-router APIs, middleware, tests, shared packages.
- Python services using FastAPI, Django, Flask patterns.
- Go services using gin, chi, gorilla.
- Rust services using axum, actix, rocket.
- Java Spring and C# ASP.NET services (P0-β).
- Rails and Laravel apps (P0-β).
- React Native and Expo repositories with bridge edges (P2).
- iOS Swift plus Objective-C bridge examples (P2).
- Intentionally messy repos with pre-existing diagnostics and partial framework conventions.

Benchmark tasks include: add middleware/route behavior in an unfamiliar repo; trace a symbol from route to service to storage; refactor an exported function signature and update callers; add tests in the project's style; fix introduced type errors; continue a task after unrelated changes; search for a known symbol where raw grep would be noisy; work in a repo with many pre-existing diagnostics.

### Multi-agent benchmark scenarios

Two or three agent sessions against the same working tree with distinct `agent_session_id`s. The orchestrator:

1. Starts the project daemon.
2. Launches agents α, β, γ through supported harness adapters.
3. Gives each agent a task prompt and area, assigning each a `task_id`.
4. Allows concurrent operation in the same working tree.
5. Injects or schedules overlapping changes (β changes `validateSession()` while α edits a caller).
6. Records all reads, context frames, edits, denies, warnings, diagnostics, final repo state.
7. Runs final acceptance checks.
8. Labels failures: textual conflicts, semantic obsolescence, TOCTOU race, diagnostic misses, agent reasoning, infrastructure.

A good multi-agent benchmark contains dependency overlap, timing races, self-edits, other-agent edits, **and cases where a dependency file changes in a way that does not affect imported symbols** (essential for measuring false-positive denies).

Subagent-spawning scenarios — α spawns α′ to investigate a dep while β edits that dep — are required.

### Headline scalar — Verified Agent Throughput (VAT)

For each task:

```
normalized_cost =
  0.50 * (tokens_used / baseline_tokens)
+ 0.20 * (tool_calls / baseline_tool_calls)
+ 0.15 * (wall_clock_seconds / baseline_wall_clock_seconds)
+ 0.10 * (diagnostic_cpu_seconds / baseline_diagnostic_cpu_seconds)
+ 0.05 * latency_penalty

task_score =
  verified_success * (1 / normalized_cost) * coordination_multiplier
```

```
coordination_multiplier =
  1.00 if no inter-agent failure
  0.85 if recovered stale-context incident occurred
  0.50 if downstream textual conflict attributable to coordination failure
  0.25 if downstream type-check failure attributable to inter-agent obsolescence
  0.00 if task failed acceptance checks
```

```
VAT = 100 * mean(task_score across benchmark tasks)
```

No-system baseline scores approximately 100 when successful at baseline cost. A system that preserves success and halves normalized cost scores near 200. A system that saves tokens but breaks tasks collapses toward zero.

### Supporting metrics

Core efficiency: tokens per successful task; tool calls per successful task; raw Read/Grep/Bash rg calls per task; wall-clock time per task; pushed-context tokens as percent of gross tokens saved; ContextFrame duplicate token ratio; ContextFrame utility proxy.

Correctness: verified task success rate; acceptance-test pass rate; type-check pass rate; lint/test regression rate; agent-induced incorrect edit rate.

Diagnostics: introduced diagnostic precision and recall; resolved diagnostic precision and recall; pre-existing diagnostic suppression precision; time to first useful diagnostic; redundant diagnostic-tool invocations avoided across N agents.

Graph and framework: symbol definition precision and recall; import/dependency edge precision and recall; exported-surface fingerprint accuracy (contract and implementation tracked separately); framework route precision and recall; false-positive rate on framework detection; confidence calibration ECE.

Multi-agent: stale-context edits caught pre-write per parallel-agent-hour; semantic-obsolescence incidents detected before code review; redundant diagnostic-tool invocations avoided across N agents; false-positive rate on dep-graph-staleness denies; self-edit false-positive stale warning rate; percentage of parallel-agent task pairs that complete without downstream textual merge conflict or type-check failure; median recovery tool calls after stale denial; active broadcast precision and recall (once enabled); TOCTOU edit-race detection precision and recall on capable adapters; subagent inherited-observation correctness.

Reliability: hook p50/p95/p99 latency by surface; MCP p50/p95/p99 latency by query class; daemon crash rate; fail-open rate; tool-call brick rate; index cold-start time (five-metric split); warm graph update latency; memory and CPU overhead per repo size bucket.

Cold-start (public benchmark surface):

- Time to daemon ready
- Time to first useful decoration
- Time to first enforcement-grade stale check
- Time to 80 percent graph coverage
- Time to full graph coverage

### Floor conditions

The auto-research loop refuses to ship a change if any floor is violated, even if VAT improves:

- Verified task success drops by more than 2 percentage points.
- Hook-induced tool failure exceeds 0.1 percent.
- Read hook p95 exceeds 50 ms or p99 exceeds 150 ms.
- Pre-edit stale-check p95 exceeds 40 ms or p99 exceeds 120 ms.
- False-positive dep-staleness denies exceed 5 percent in baseline mode.
- Self-edit false-positive rate exceeds 0.5 percent.
- Introduced diagnostic recall falls below 95 percent.
- Introduced diagnostic precision falls below 90 percent.
- High-confidence symbol or route precision falls below 95 percent.
- Pushed-context token overhead exceeds 40 percent of gross token savings.
- Duplicate context token ratio exceeds 15 percent.
- TOCTOU detection recall falls below 90 percent on capable adapters.
- Multi-agent task-pair success without inter-agent failures falls below the no-system baseline.

### Automatic metric gathering

Metrics gathered through the local event log and benchmark harness. Harness controls task prompts, launches agents, assigns session IDs and task IDs, tracks tool calls, captures token usage where available, estimates tokens where exact counts unavailable, records final diffs, runs acceptance checks, labels failure modes.

Diagnostic ground truth computed by full clean before/after runs in benchmark mode.

Graph precision and recall measured against labeled fixtures (known routes, known symbol definitions, known imports, known exported-surface changes, known cross-language bridge edges).

Stale-context benchmarks inject timed overlapping edits. The orchestrator controls schedule so it knows the correct behavior is allow, warn, or deny.

Token-savings counterfactuals: paired runs with feature ablations (no system / MCP only / push hooks only / push plus diagnostics / full / full minus stale / full minus dedup / full minus interception).

Statistical confidence: multiple seeds across at least two held-out agent models. Auto-research accepts changes only when VAT improves with p < 0.05 and no floor fails.

---

## 13. Risks and unknowns

### 1. Agents may not use pushed context as much as expected

Compare push-only, MCP-only, hybrid on exploration-heavy tasks. Inspect whether pushed pointers are followed. Mitigation: small, action-oriented, close to the tool call that triggered it. Measure pointer-following and subsequent redundant search.

### 2. False-positive stale denies may frustrate agents

A denial forcing an unnecessary reread is a small tax. Many unnecessary denials become a toll road through molasses. Mitigation: file-level checks with symbol-level logged evidence, symbol-level narrowing for high-churn files, advisory compatibility mode, deny budget per §8, 5 percent ceiling as shipping blocker.

### 3. Dynamic framework detection may be less reliable than hoped

Rails, Django, Laravel, Java reflection, Python decorators, JS metaprogramming, custom project conventions can defeat static extraction. Mitigation: confidence tiers, provenance, framework-specific gold fixtures, repo-local pattern learning. Weak heuristics never drive deny decisions.

### 4. Diagnostic delta attribution may be hard in messy repos

Pre-existing failures, unstable diagnostic tool output, cascading errors. Mitigation: before/after snapshots, conservative suppression, explicit uncertainty, raw diagnostics available through MCP or logs.

### 5. Hook latency budgets may be harder across Windows and huge repos

Filesystem watching, sockets, antivirus interactions vary by platform. Mitigation: hot in-memory views, hard timeouts, fail-open behavior, platform-specific latency benchmarks, startup health checks.

### 6. Harness capabilities will vary

Adapters normalize capabilities. Treat deny as an available enforcement upgrade, not the only behavior. Daemon protocol stable even when harness shims change. Capability matrix is product surface so operator trust matches reality.

### 7. The benchmark could become the product's tiny idol

Mitigation: held-out repos, rotated tasks, adversarial scenarios, multi-agent variation, floor metrics that catch regressions hidden by VAT.

### 8. Salsa lock-in

Pin Salsa version, wrap behind `briefcase-incremental` crate. If Salsa proves wrong, blast radius is one crate.

### 9. TOCTOU coverage depends on adapter precondition support

Where adapters cannot attach file preconditions, TOCTOU detection degrades to best-effort. Capability matrix surfaces this honestly. Mitigation: prioritize adapter contributions toward `can_attach_file_precondition` for harnesses where multi-agent is the headline scenario.

---

## 14. The one non-obvious design choice

The most important non-obvious choice:

**Make ObservationLedger, ContextFrame ledger, RepoEpoch, task identity, and file-version preconditions the core substrate. The graph is the truth oracle feeding that substrate, not the architectural sun everything orbits.**

Most reviewers will start by thinking this product is a better code index. That is understandable and incomplete. A code graph tells you what is currently true about the repository. The harder and more valuable question is: **what does this particular agent session currently believe, and has that belief expired?**

That distinction unlocks nearly everything that matters:

- **Single-agent context savings:** the ledger knows what the agent has already been shown; the system emits deltas instead of repeating nav maps.
- **Continuation:** the ledger knows what changed since the agent last worked.
- **Diagnostics:** edit attempt, file version, post-edit result, and pushed repair context can be connected.
- **Multi-agent coordination:** α's observed version of `route.ts` and its dependency set can be compared against β's later changes.
- **Subagent inheritance:** a child session's belief is auditable and temporally frozen at spawn, with origin preserved.
- **Auto-research:** scoring which context frames saved work and which were ornamental fog becomes mechanical.
- **TOCTOU safety:** precondition hashes attached to allow decisions are checked at write time, closing the check-to-use race.

A graph without an observation ledger is a clever librarian shouting facts into the room. A graph with an observation ledger becomes a flight recorder, air-traffic radar, and memory palace in one local machine.

The wild extension is to treat every emitted byte as carrying a little debt. The system should eventually ask: "Did this context pay rent?" A decoration that prevented a grep, caused a correct re-read, avoided a stale edit, or led the agent to the right file earned its place. A decoration that merely made the transcript fatter gets demoted. That turns context design from taste into an objective optimization target.

This is the design choice to defend hardest: **the product is not primarily a repository index. It is a versioned belief-management system for AI agents operating on code.**

---

## Appendix A: Build-order phasing

Implementation phases for the sub-agent swarm. Each phase is gated by acceptance criteria from §12. **Graph facts are inputs to belief management; the graph must not become the primary API surface.**

### Phase 1 — Identity substrate

- Project / worktree identity (canonical path + git common dir + config hash + protocol version)
- Daemon lifecycle (single-instance fencing via DB lease, heartbeat file, generation ID)
- Content-addressed `FileVersion`
- `RepoEpoch` with `operation_state` enum
- Monotonic event IDs
- Session schema (`agent_session_id`, lineage fields, capability records)
- Append-only event log
- Adapter capability registration

Acceptance: daemon starts, fences correctly under concurrent launches, captures file versions and session lifecycles correctly.

### Phase 2 — Session registry, ObservationLedger, EditLedger, direct file freshness

- Session registry materialized view
- ObservationLedger writes from `ReadObserved`
- EditLedger writes from `EditIntent` / `EditApplied`
- TOCTOU precondition attachment (where adapter supports)
- Direct file freshness check (no graph required)
- Subagent inheritance machinery (`InheritedObservation`, orientation packet skeleton)

Acceptance: "you read V1, β advanced to V2, do not edit blindly" works end-to-end. Subagent inheritance round-trips correctly.

### Phase 3 — ContextFrame ledger and scheduler skeleton

- `ContextFrame` schema and writes
- Template-emitter scheduler (no-op novelty scoring at first; correct schema)
- `subagent_orientation_packet` emission
- `PreCompactCheckpoint` event recording

Acceptance: every pushed decoration is a ContextFrame; lineage and post-compact resume correctly reference prior frames.

### Phase 4 — Minimal graph

For P0-α languages only:

- File inventory
- File hashes
- Imports/exports
- Static dependency sets
- Exported-surface contract and implementation fingerprints
- Simple symbol locations
- Provenance and confidence on every fact

No framework extraction yet. No diagnostics yet. The graph exists to feed the ledger's stale-context check, not to be queried directly.

Acceptance: pre-edit dep-graph staleness check works against contract fingerprints. False-positive rate measurable.

### Phase 5 — Hooks and MCP as clients of daemon contracts

- Adapter for Claude Code (Tier 1)
- Adapter for Codex CLI (Tier 1 if deny supported, Tier 2 otherwise)
- Adapter for Cursor (Tier 2 expected)
- MCP server with three tools at first: `briefcase_orient`, `briefcase_observed_state`, `briefcase_prove`
- Harness simulator for testing

Acceptance: end-to-end multi-agent scenario passes with at least two adapters. Coordination metrics measurable.

### Phase 6 — Capabilities on top

- Diagnostics workers and delta attribution
- Framework extractors (Tier 1 frameworks)
- `briefcase_find`, `briefcase_explain`, `briefcase_impact`, `briefcase_diagnostics`
- Operator CLI (full command set)
- Active broadcasts (P2 — gated on Phase 5 stability)
- PreCompact survival packet (full quality)
- Generated/vendored file policy

Acceptance: full P0 feature surface meets §12 metrics.

### Phase 7 — Extension

- P0-β language tier
- P1 web UI
- Worktree federation (P2)
- Public dependency graph cache (P2)
- Symbol-level stale checks and leases (P2)
- Cross-language bridge extractors (P2)

---

## Appendix B: Schema reference

All typed entities defined in this spec, gathered for implementer reference.

### Session

```
Session {
  agent_session_id
  root_session_id
  parent_session_id     nullable
  spawn_event_id        nullable
  lineage_depth
  harness
  task_id               nullable
  capabilities
  created_at
}
```

### InheritedObservation

```
InheritedObservation {
  child_session_id
  inherited_from_session_id
  inherited_from_observation_id
  file_path
  file_version
  repo_epoch_id
  graph_version
  observed_at_original
  inherited_at
  observation_kind
  observation_fidelity
  source_context_frame_id   nullable
}
```

### FileVersion

```
FileVersion {
  file_id
  path
  content_hash
  size
  mtime_observed
  executable_bit
  symlink_target            nullable
  repo_epoch_id
  source_class              // source | test | generated | vendored |
                            // build_artifact | config | lockfile |
                            // migration | fixture | unknown
}
```

### RepoEpoch

```
RepoEpoch {
  repo_epoch_id
  worktree_id
  git_common_dir_id         nullable
  head_oid                  nullable
  branch_ref                nullable
  index_tree_oid            nullable
  working_tree_digest
  operation_state           // normal | detached_head | merge_in_progress |
                            // rebase_in_progress | cherry_pick_in_progress |
                            // bisect_in_progress | unknown_vcs
  started_at
}
```

### Observation

```
Observation {
  observation_id
  agent_session_id
  file_id
  path
  file_version
  repo_epoch_id
  graph_version
  observed_at
  context_frame_id          nullable
  task_id                   nullable
}
```

### Task

```
Task {
  task_id
  root_session_ids[]
  prompt_hash
  task_summary
  assigned_files            optional
  assigned_symbols          optional
  parent_task_id            nullable
  created_at
}
```

### ContextFrame

```
ContextFrame {
  context_frame_id
  agent_session_id
  task_id                   nullable
  triggering_tool_call
  facts_included
  source_versions
  token_count
  confidence
  expires_when
  novelty_hash
  utility_receipt           nullable
}
```

### SurfaceItem

```
SurfaceItem {
  language
  module_id
  export_name
  qualified_name
  kind
  visibility
  signature_repr
  type_repr                 nullable
  decorators_or_attributes
  route_contract            nullable
  source_span
  provenance
  confidence
}
```

### CompactionCheckpoint

```
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

### PreCompactSurvivalPacket

```
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

### DaemonDecision

```
DaemonDecision {
  decision_kind:
    allow | deny | advisory | decorate |
    replace_result | modify_input | observe_only

  context_frames[]
  replacement_result        nullable
  modified_input            nullable
  deny_reason               nullable
  revalidation_instructions[]
  confidence
  expires_at                nullable
}
```

### DenyDecision

```
DenyDecision {
  deny_id
  severity                  // hard | soft
  target_file
  current_target_version
  stale_observation_versions
  dependency_causes[]
  minimum_revalidation_set[]
  proof_available
  override_policy
}
```

### OverrideDeny

```
OverrideDeny {
  deny_id
  agent_session_id
  observed_dependency_versions[]
  rationale
  requested_scope
}
```

### Adapter capability bitset

```
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

### Daemon event types

```
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
