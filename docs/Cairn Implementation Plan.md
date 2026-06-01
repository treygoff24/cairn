I read `v2-implementation-plan-prompt.md` first and treated `Cairn Final Specification.md` as binding ground truth, with `v2-skills-library-attachment.md` as the exact skill vocabulary for task briefs. The plan below follows the requested structure, preserves the spec’s “belief-management first, graph second” design center, and bakes in parallel waves, review loops, bug hunts, deslopify passes, CPU discipline, Git discipline, adapter ordering, benchmark timing, and GPT-Pro re-review checkpoints.

# Cairn Implementation Plan

> **PATCH ROUND 2 APPLIED — 2026-06-01 — scope re-cut after Wave 0.** Read `docs/Cairn Implementation Plan — Patch Round 2.md` alongside this plan. The architecture, 26-crate decomposition, and build method below are **unchanged**. What changed: (1) Cairn is framed as a **superset** of vexp — match the substrate *because the moat's precision rides on it*, then add the push-mode/enforcement/coordination moat (do **not** thin the substrate); (2) the V1 build is a **TS + Python vertical slice** — `cairn-web`, `cairn-extract-p0beta`, `cairn-extract-bridges`, most framework extractors, federation/leases/broadcasts, and the full metrics gym are deferred out of the V1 critical path (still P1/P2); (3) a **cry-wolf precision gate** (self-edit false-positive < 0.5%, spec §3) is pulled forward as an early hard milestone, with a thin real Claude hook adapter brought forward to feed it, reusing the `~/Code/cairn-tracer-sandbox` rig. Wave 0 validated the hot-path latency (GREEN, ~2.15 ms warm p95) and that pushed context steers the agent. Phase 1 is unaffected.

## 1. Executive summary

This plan delivers the V2 P0 substrate end-to-end: a greenfield Rust workspace with project/worktree identity, a single per-worktree daemon, content-addressed file versions, repo epochs, append-only event log, session/task/observation/edit ledgers, direct and dependency-aware stale-context arbitration, ContextFrame tracking, minimal P0-α graph, thin adapters, initial MCP tools, diagnostics deltas, operator CLI, metrics, and benchmark scaffolding. The graph is deliberately not the architectural sun; it feeds the ledgers, freshness checks, context scheduler, proof surface, diagnostics, and benchmarks.

The execution model is a Claude Code orchestrator fanning out bounded implementation tasks to a diverse sub-agent and `delegate` swarm, with every wave ending in an independent reviewer pass by a different model, every review producing a fix-list, every fix-list dispatched back to a worker, and every phase gated by light checks, full checks, bug hunts, and scheduled cleanup.

The build runs in 7 phases mapped directly from spec Appendix A: Phase 1 identity substrate, Phase 2 session registry and ledgers, Phase 3 ContextFrame skeleton, Phase 4 minimal graph, Phase 5 hooks and MCP clients, Phase 6 capabilities on top, Phase 7 extensions. The plan uses 26 initial crates, targets 5 parallel tasks per wave with a hard cap of 7, runs bug hunts at the boundaries of Phases 2 through 7 (Phase 1's adversarial gate tests stand in for a separate bug hunt), runs `desloppify-deep` after Phases 4, 6, and 7 (Phase 2 uses a targeted `code-simplifier` pass instead since the cross-crate surface is still thin), freezes an event-log golden fixture at every phase boundary, and re-engages GPT-Pro at the ends of Phases 4, 5, and 7 — narrow precondition consult at Phase 4, delta/integration consult at Phase 5, pre-P1/P2 continuation review at Phase 7.

## 2. Cargo workspace layout

Worker default for all crates: workers do not commit. Workers do not run `cargo test --workspace`, `cargo build --release`, or other heavy gates. Workers may run crate-local `cargo check`, crate-local `cargo clippy --no-deps`, and `cargo fmt --check`; the orchestrator runs the consolidated gates once per wave and once per phase. No worker uses `git add -A`, `git add .`, force-push, stash, `--amend`, or `--no-verify`.

| Crate                          | Purpose                                                      | Public API surface                                           | Dependencies                                                 | Initial owner model                                   | Tier                      |
| ------------------------------ | ------------------------------------------------------------ | ------------------------------------------------------------ | ------------------------------------------------------------ | ----------------------------------------------------- | ------------------------- |
| `cairn-types`              | Canonical domain types and newtypes.                         | `AgentSessionId`, `FileId`, `RepoEpochId`, `EventId`, `ContextFrameId`, enums, schema structs. | none                                                         | `delegate droid grok work` for typed schema precision | P0 substrate              |
| `cairn-config`             | Config loading, protocol version, feature flags, ablation flags, config hash. | `CairnConfig`, `ConfigHash`, `FeatureFlags`, `load_config`. | `cairn-types`                                            | `delegate codex work`                                 | P0 substrate              |
| `cairn-identity`           | Canonical project/worktree identity.                         | `resolve_project_identity(root, config) -> ProjectIdentity`. | `cairn-types`, `cairn-config`                        | `delegate codex work`                                 | P0 substrate              |
| `cairn-file`               | File hashing, file metadata, `FileVersion`, `source_class`.  | `snapshot_file`, `hash_file`, `classify_source`, `FileVersionStore` trait. | `cairn-types`, `cairn-identity`                      | `delegate droid "deepseek v4 pro" work`               | P0 substrate              |
| `cairn-vcs`                | Git state and `RepoEpoch`.                                   | `detect_repo_epoch`, `OperationState`, `WorkingTreeDigest`.  | `cairn-types`, `cairn-identity`, `cairn-file`    | `delegate cursor work`                                | P0 substrate              |
| `cairn-protocol`           | Hook events, daemon decisions, adapter capability bitset, wire schemas. | `DaemonEvent`, `DaemonDecision`, `AdapterCapabilities`, request/response envelopes. | `cairn-types`                                            | `delegate droid grok work`                            | P0 substrate              |
| `cairn-storage`            | Append-only event log, migrations, materialized views, credential redaction. | `EventLog`, `MaterializedViews`, `StorageTxn`, `append_event`. | `cairn-types`, `cairn-file`, `cairn-vcs`, `cairn-identity` | `delegate codex work`                                 | P0 substrate              |
| `cairn-daemon-client`      | Thin client for adapters, CLI, MCP.                          | `DaemonClient`, `connect_or_launch`, `send_event`.           | `cairn-protocol`, `cairn-identity`, `cairn-config` | `delegate droid "deepseek v4 flash" work`             | P0 substrate              |
| `cairn-daemon`             | Single per-worktree daemon, lease, heartbeat, socket, service registry. | `Daemon`, `DaemonService`, `launch_or_attach`, health endpoints. | `cairn-*` substrate crates                               | `delegate droid "deepseek v4 pro" work`               | P0 substrate              |
| `cairn-ledger`             | Session registry, ObservationLedger, EditLedger, Task identity, inheritance, freshness decisions. | `record_observation`, `record_edit_intent`, `check_direct_freshness`, `inherit_observations`. | `cairn-types`, `cairn-storage`, `cairn-file`, `cairn-vcs`, `cairn-protocol` | `delegate codex work`                                 | P0 substrate              |
| `cairn-context`            | ContextFrame ledger, scheduler skeleton, novelty/dedup, renderable decorations. | `emit_context_frame`, `schedule_decoration`, `ContextBudget`, `ContextRenderer`. | `cairn-types`, `cairn-storage`, `cairn-ledger`, later `cairn-graph` | `delegate codex work`                                 | P0 now, P1 full scheduler |
| `cairn-incremental`        | Salsa wrapper and hot query DAG boundary.                    | `IncrementalDb`, query keys, query invalidation API.         | `cairn-types`, `cairn-file`, `cairn-config`      | `delegate droid grok work`                            | P0 substrate              |
| `cairn-graph`              | Graph model, file inventory, symbols, deps, fingerprints, provenance. | `GraphSnapshot`, `DependencySet`, `SurfaceItem`, `FingerprintSet`, `GraphQuery`. | `cairn-types`, `cairn-file`, `cairn-vcs`, `cairn-incremental`, `cairn-storage` | `delegate droid "deepseek v4 pro" work`               | P0 substrate              |
| `cairn-extract-core`       | Versioned extractor contracts.                               | `Extractor`, `ExtractedFact`, `FactProvenance`, `Confidence`, grammar/language/framework traits. | `cairn-types`, `cairn-graph`, `cairn-file`       | `delegate codex work`                                 | P0 substrate              |
| `cairn-extract-p0alpha`    | TypeScript/JavaScript, Python, Go, Rust extractors.          | P0-α extractor implementations and fixture harness bindings. | `cairn-extract-core`, `cairn-graph`, `cairn-incremental` | split by language workers                             | P0 substrate              |
| `cairn-extract-frameworks` | Next.js, FastAPI, Django, Flask, Go web, Rust web framework routes. | Framework route, handler, middleware extractors.             | `cairn-extract-core`, `cairn-graph`                  | split by framework workers                            | P0/P1 capability          |
| `cairn-extract-p0beta`     | Java, C#, PHP, Ruby, C/C++, Swift, Kotlin, Dart navigation-grade extractors. | P0-β extractor implementations and fixtures.                 | `cairn-extract-core`, `cairn-graph`                  | model-diverse extension wave                          | P1/P2 extension           |
| `cairn-extract-bridges`    | React Native, Expo, Swift/Obj-C, JNI, FFI bridge extractors. | Bridge edge extractors and bridge provenance.                | `cairn-extract-core`, `cairn-graph`                  | `delegate droid qwen work` plus reviewers             | P2 extension              |
| `cairn-diagnostics`        | Persistent diagnostic workers, caches, before/after deltas.  | `DiagnosticWorker`, `DiagnosticDelta`, `DiagnosticCache`, `attribute_delta`. | `cairn-types`, `cairn-file`, `cairn-storage`, `cairn-graph`, `cairn-metrics` | `delegate droid "deepseek v4 pro" work`               | P0 capability             |
| `cairn-metrics`            | Local metrics ledger, latency, token/tool counters, VAT, ablations. | `MetricsLedger`, `VatScore`, `AblationSet`, `LatencySample`. | `cairn-types`, `cairn-storage`, `cairn-protocol` | `delegate codex work`                                 | P1 product feature        |
| `cairn-bench`              | Benchmark harness and scenario runner.                       | `BenchmarkRun`, `Scenario`, `AgentHarness`, result bundles.  | `cairn-metrics`, `cairn-harness-sim`, `cairn-daemon-client` | `delegate cursor work` with Codex review              | P1 product feature        |
| `cairn-mcp`                | MCP server with seven tools, staged delivery.                | `cairn_orient`, `cairn_observed_state`, `cairn_prove`, later `find/explain/impact/diagnostics`. | `cairn-daemon-client`, `cairn-context`, `cairn-ledger`, `cairn-graph`, `cairn-diagnostics` | `delegate codex work`                                 | P0/P1 integration         |
| `cairn-adapter-core`       | Shared adapter discovery, launch, spool, event normalization. | `AdapterRuntime`, `Spool`, `CapabilityRegistration`, harness-normalized events. | `cairn-protocol`, `cairn-daemon-client`, `cairn-identity` | `delegate codex work`                                 | P0 integration            |
| `cairn-adapter-claude`     | Claude Code Tier 1 hook adapter.                             | Claude hook entrypoints and capability registration.         | `cairn-adapter-core`                                     | `delegate codex work`                                 | P0 integration            |
| `cairn-adapter-codex`      | Codex CLI adapter, Tier 1 if deny/precondition support exists, otherwise Tier 2. | Codex hook/MCP/client shim.                                  | `cairn-adapter-core`                                     | `delegate codex work`                                 | P0/P1 integration         |
| `cairn-adapter-cursor`     | Cursor adapter, expected Tier 2.                             | Cursor shim and event normalization.                         | `cairn-adapter-core`                                     | `delegate cursor work`                                | P0/P1 integration         |
| `cairn-harness-sim`        | Harness simulator for deterministic multi-agent tests.       | `SimHarness`, event scripts, injected race/stale scenarios.  | `cairn-protocol`, `cairn-daemon-client`, `cairn-adapter-core` | `delegate droid "deepseek v4 flash" work`             | P0 test substrate         |
| `cairn-cli`                | Operator CLI.                                                | `cairn status`, `daemon doctor`, `sessions`, `ledger`, `context`, `deny`, `graph`, `diagnostics`, `metrics`, `replay`. | `cairn-daemon-client`, `cairn-storage`, `cairn-ledger`, `cairn-context`, `cairn-diagnostics`, `cairn-metrics` | `delegate cursor work`                                | P0/P1 visibility          |
| `cairn-web`                | Local read-only web UI.                                      | Local HTTP server, timeline API, session lanes UI.           | `cairn-storage`, `cairn-metrics`, `cairn-ledger`, `cairn-diagnostics` | `delegate droid gemini work`                          | P1 extension              |
| `cairn-app`                | Single binary packaging daemon, CLI, MCP, adapter helpers.   | `cairn` binary with subcommands.                         | all shipped crates                                           | orchestrator direct integration                       | P0 substrate              |

Dependency rule: lower-level crates may not depend on higher-level crates. `types`, `config`, `identity`, `file`, `vcs`, `protocol`, and `storage` form the floor. `ledger` and `context` sit above storage. `graph` and extractors feed `ledger/context` but do not become the primary API surface. Adapters, MCP, CLI, benchmark, and web are clients of daemon contracts.

## 3. Cross-cutting strategies

### 3.1 Test corpus inheritance from V1

The V1 Python repo at `~/Code/code-briefcase` has 878 tests, but V2 is greenfield Rust with no Python compatibility shims. The porting strategy is incremental, not big-bang: inventory in Phase 1, direct file freshness and ledger tests in Phase 2, context tests in Phase 3, graph/fingerprint tests in Phase 4, adapter and MCP tests in Phase 5, diagnostics and CLI tests in Phase 6, and extension-language tests in Phase 7. The acceptance corpus is “match behavior in spirit,” not literal implementation shape.

Test buckets:

| Bucket                      | Starts  | Destination                                                  | Acceptance                                                   |
| --------------------------- | ------- | ------------------------------------------------------------ | ------------------------------------------------------------ |
| identity/path/git/worktree  | Phase 1 | `crates/*/tests/identity_*`, `tests/phase1_identity.rs`      | deterministic identity across symlinks, nested worktrees, detached head |
| event log/session lifecycle | Phase 1 | `crates/cairn-storage/tests/**`, `tests/phase1_daemon.rs` | monotonic event IDs, crash-safe append, concurrent launch fencing |
| observations/edit freshness | Phase 2 | `crates/cairn-ledger/tests/**`, `tests/phase2_stale_direct.rs` | read V1, other session writes V2, pre-edit denies/warns      |
| subagent inheritance        | Phase 2 | `tests/phase2_subagent_inheritance.rs`                       | child inherits direct plus inherited observations, parent never auto-learns |
| context frames              | Phase 3 | `crates/cairn-context/tests/**`                          | all decorations have ContextFrame IDs, budgets, source versions |
| graph/fingerprints          | Phase 4 | `fixtures/p0alpha/**`, `tests/phase4_graph.rs`               | import/dependency sets, contract/implementation fingerprint stability |
| adapters/MCP                | Phase 5 | `tests/phase5_adapters.rs`, `tests/phase5_mcp.rs`            | two-adapter multi-agent scenario passes                      |
| diagnostics/CLI             | Phase 6 | `tests/phase6_diagnostics.rs`, `tests/phase6_cli.rs`         | introduced diagnostics precision/recall fixtures, CLI explanation commands |
| extensions                  | Phase 7 | `fixtures/p0beta/**`, `fixtures/bridges/**`                  | navigation-grade extraction, bridge-edge fixture scores      |

### 3.2 Latency POC

The latency POC starts as scaffolding in Phase 1, but the first meaningful validation lands in Phase 4 Wave 4.1 when `cairn-incremental`, `cairn-graph`, file snapshots, and arena-backed extraction can be measured together. The POC validates the warm `Read` decoration target of 35 ms p95 and 120 ms p99, plus pre-edit stale-check and MCP lookup microbenchmarks. The spec frames those latency numbers as architectural targets pending Rust + Salsa + mmap snapshot + arena validation.

POC acceptance:

| Surface                   | Phase 4 POC floor    | Shipping floor          |
| ------------------------- | -------------------- | ----------------------- |
| Read decoration, warm     | p95 <= 50 ms         | p95 <= 35 ms            |
| PreEdit direct freshness  | p95 <= 35 ms         | p95 <= 25 ms            |
| PreEdit graph freshness   | p95 <= 75 ms in POC  | p95 <= 40 ms by Phase 6 |
| MCP indexed symbol lookup | p95 <= 200 ms        | p95 <= 150 ms           |
| Graph single-file update  | p95 <= 750 ms in POC | p95 <= 500 ms           |

### 3.3 Benchmark harness scaffolding

Benchmark harness scaffolding starts in Phase 4, not Phase 6. Phase 4 measures microbenchmarks and false-positive graph staleness. Phase 5 adds adapter-driven multi-agent scenarios. Phase 6 turns the harness into a product feature with metrics ledger, ablations, result bundles, VAT, diagnostic ground truth, and unattended run support. The spec treats local metrics plus the benchmark harness as a V1 product feature, not private scaffolding.

Ablation flags are created before full features ship, so every major capability can be tested as it lands:

`no_system`, `mcp_only`, `push_hooks_only`, `push_plus_diagnostics`, `full`, `full_minus_stale`, `full_minus_dedup`, `full_minus_interception`, `strict_mode`, `advisory_compatibility`.

### 3.4 Adapter rollout strategy

Claude Code reaches Tier 1 first because it is the primary harness and has the strongest hook surface. Codex and Cursor begin as adapter-design tasks in parallel, but their implementation follows the stable `cairn-adapter-core` and daemon protocol. Codex is targeted for Tier 1 if deny/precondition support is available; otherwise it ships Tier 2 with honest capability surfacing. Cursor is expected Tier 2 first. The adapter capability matrix is product surface, not internal trivia.

Adapter order:

1. `cairn-adapter-core`: daemon discovery, launch, spool, capability registration.
2. `cairn-adapter-claude`: Tier 1, pre-edit deny, read decoration, command replacement, PreCompact.
3. `cairn-adapter-codex`: Tier 1 if supported, otherwise Tier 2.
4. `cairn-adapter-cursor`: Tier 2, post-hoc observation plus advisory where possible.
5. Harness simulator: treated as an adapter and used for deterministic acceptance.

### 3.5 CI strategy

CI begins in Phase 1 with a thin gate and grows as crates arrive.

The orchestrator’s phase gate is always:

```text
cargo fmt --check
cargo clippy --workspace --all-targets --no-deps
cargo test --workspace
```

Phase-specific gates add targeted tests, fixture scores, and benchmarks. Workers only run crate-local light checks. This preserves CPU while still forcing integration truth at wave and phase boundaries.

CI lanes:

| Lane                  | Trigger                    | Command shape                                                |
| --------------------- | -------------------------- | ------------------------------------------------------------ |
| formatting            | every PR/phase             | `cargo fmt --check`                                          |
| light type/lint       | every wave by orchestrator | `cargo check --workspace`, `cargo clippy --workspace --all-targets --no-deps` |
| full tests            | every phase                | `cargo test --workspace`                                     |
| fixture extraction    | Phases 4+                  | `cargo test -p cairn-extract-* --features fixtures`      |
| latency POC           | Phases 4+                  | `cargo bench -p cairn-bench latency_poc -- --quick`      |
| multi-agent scenarios | Phases 2+                  | `cargo test -p cairn-harness-sim`                        |
| packaging smoke       | Phases 5+                  | `cargo run -p cairn-app -- daemon doctor --self-test`    |

Cache: use `sccache` if present, Cargo registry cache, Cargo target cache keyed by lockfile plus Rust toolchain, and separate fixture cache. If full phase gate exceeds 30 minutes twice in a row, split test suites by crate group and make the orchestrator run them in controlled batches, not worker fan-out.

### 3.6 Cross-platform strategy

macOS and Linux are in-stride from Phase 1. Windows is supported at the type/path abstraction level from Phase 1, but Windows does not block Phase 5 Tier 1 unless the user explicitly makes it a launch requirement. Windows becomes a real gate in Phase 7 after watcher, socket, path canonicalization, and antivirus-latency risks have dedicated fixtures.

Rules:

- All path identity code uses typed wrappers, never raw strings across crate boundaries.
- Case sensitivity is modeled in `cairn-identity`.
- Daemon sockets abstract over Unix domain sockets and Windows named pipes.
- File watching has a polling fallback.
- Git state detection is tested on macOS/Linux in Phase 1 and on Windows in Phase 7.
- No hard deny depends on watcher freshness when watcher health is degraded.

### 3.7 Extractor stack

Tree-sitter is the default parser substrate for all P0-α and P0-β language extractors. Tree-sitter facts default to medium confidence and are sufficient for navigation-grade graph data, dependency edges, fingerprint inputs, and any extraction surface that informs advisory freshness decisions.

Compiler-backed or tool-backed extraction is a hard requirement for facts that can drive hard-deny enforcement. Per the spec's confidence/provenance model, a fact may only justify a hard deny if its provenance includes a compiler-backed or tool-backed source. The required seams documented in `cairn-extract-core`:

- TypeScript / JavaScript: tsc / `tsserver` JSON protocol.
- Python: Pyright or mypy where available; demote confidence if neither is configured.
- Go: `go/packages` or gopls.
- Rust: rust-analyzer JSON or rustdoc JSON.

When a compiler-backed seam is unavailable in the target project, the extractor still emits facts via tree-sitter, but confidence demotes to medium or low and the fact alone cannot drive hard deny. This policy is enforced at the ledger's pre-edit decision point, not inside the extractor.

Grammar version pinning: every extractor records its grammar version (tree-sitter grammar SHA or compiler-backed tool version) in `ExtractedFact.provenance`. The Salsa query key in `cairn-incremental` includes the grammar version so a grammar bump invalidates exactly the affected facts.

Confidence-tier defaults (extractors may override per fact with provenance):

- Compiler-backed: high.
- Tree-sitter parse-verified: medium.
- Heuristic / regex / dynamic fallback: low.

### 3.8 Commit and worktree integration protocol

The orchestrator owns commits. Workers never commit, never stash, never rebase, never amend. Workers run in one of two integration modes:

- Main-tree mode for single-owner crates where no other in-wave worker touches overlapping files. The worker edits the main tree directly. The orchestrator picks up changes at wave-collection time.
- Worktree mode (`delegate <model> work --isolation worktree`) for overlap-prone tasks, large mechanical edits, and tasks the orchestrator wants to integrate selectively. The worker operates in a temporary git worktree and reports the diff. The orchestrator integrates by `git apply` of per-task patches against the main tree, never by branch-merge.

A pre-wave contract commit is allowed when a wave's Wave-internal contracts block requires types, traits, or compile-only crate stubs to exist before fan-out. The orchestrator commits the contract stubs, then fans out workers. This is the only commit that lands inside a wave; all other commits happen at wave close.

Cross-task conflicts within a wave are resolved by the orchestrator at integration time, before the reviewer pass dispatches. Workers do not negotiate conflicts with each other.

Commit cadence is one orchestrator-owned commit per wave after the review loop is clean. The commit message lists the wave ID, owned crates, and contributing model names. Phase-boundary commits additionally include the phase number and the advance-refusal-gate summary.

Files are staged by exact name. `git add -A` and `git add .` are forbidden. Hooks are never skipped (`--no-verify` forbidden). Signing is never bypassed.

## 4. Phase-by-phase implementation

Global worker brief applied to every implementation task:

- Always load `clean-code`.
- Load `rust-engineer` for Rust work.
- Use `tdd-workflow` when the task has a testable surface.
- Run only crate-local light checks.
- Do not commit.
- Hand off: files touched, acceptance proof, checks run, reviewer risks, follow-up TODOs.
- Cross-crate API changes require orchestrator approval.

Review-loop rule for every wave: reviewer must be a different model than implementer. Reviewer returns a fix-list. Fix-list goes to original implementer unless the critique concerns architectural misunderstanding, schema drift, or repeated style slop; those go to a fresh worker. If implementer and reviewer disagree, the orchestrator runs `diagnose`, then asks a third model for a narrow adjudication. If the disagreement reveals spec ambiguity, stop phase advancement and re-engage GPT-Pro.

### Phase 1 - Identity substrate

Goal: create the Rust workspace floor: domain types, config hash, project/worktree identity, content-addressed file versions, repo epochs, append-only event log, daemon lifecycle with single-instance fencing, monotonic event IDs, and adapter capability registration. This phase intentionally does not build graph, context scheduling, or MCP tools. It makes one local substrate real.

Dependencies on prior phases: none.

Acceptance criteria:

- `cairn` binary starts daemon and `cairn daemon doctor --self-test` passes.
- Concurrent daemon launches for same worktree produce one live daemon and no split-brain.
- Identity is deterministic across symlinked roots, nested worktrees, detached head, and config-hash changes.
- `FileVersion` records content hash, size, executable bit, symlink target, repo epoch, and source class.
- `RepoEpoch` captures operation state.
- Event IDs are monotonic under concurrent append stress.
- Adapter capability registration writes to event log and materialized view.
- Workers report only light checks; orchestrator full gate is green.

#### Wave 1.0 - Preflight, no code

Task: Phase 1 foundation-risk premortem and concurrency cap.

- Owner: orchestrator direct.
- Skill stack: primary `premortem`; secondary `parallel-subagent-discipline`, `orchestrator`.
- Owned files: none.
- Dependencies within wave: none.
- Acceptance: short foundation-risk premortem (≤1 page) covering identity false matches across symlinked/case-different paths, daemon split-brain on concurrent launch, event-log corruption under crash, monotonic event ID violations, and wave-contract readiness for the Wave 1.1 bootstrap commit. Max parallel workers for Wave 1.1 set to 6.
- Reviewer pairing: `Agent plan-reviewer`.
- Review brief shape: attack whether the bootstrap commit covers everything Wave 1.1 workers need to compile against, and whether the foundation invariants (path identity semantics, daemon lease model, event ID ordering) have any seam that downstream phases will have to undo.

#### Wave 1.1 - Workspace and domain floor, all concurrent

**Bootstrap prelude (orchestrator-owned, before fan-out).** Wave 1.1 is the one wave whose tasks all need a workspace to compile against. Before dispatching Task 1, the orchestrator commits a contract/bootstrap commit containing: the root `Cargo.toml` with workspace member declarations, `rust-toolchain.toml`, `.cargo/config.toml`, empty crate directories for every crate Wave 1.1 owns (`crates/cairn-types`, `crates/cairn-config`, `crates/cairn-identity`, `crates/cairn-file`, `crates/cairn-vcs`, `crates/cairn-app`), and a compile-only `lib.rs`/`main.rs` skeleton in each crate so `cargo check --workspace` passes against the empty tree. Workers then operate on their owned crate without colliding on workspace-file edits. Task 1 (Cargo workspace and policy scaffolding) takes over from there to flesh out `cairn-app`, the gate docs, and `CLAUDE.md` discipline text.

**Wave-internal contracts.** None beyond the bootstrap commit above. All Wave 1.1 crates are leaves with no inter-crate dependencies except `cairn-types` (which Tasks 3–6 transitively consume) and the eventual `cairn-app` rollup (Wave 1.3). Task 2 publishes the `cairn-types` public surface; other tasks consume types lazily and the orchestrator integrates at wave close.

Task 1: Cargo workspace and policy scaffolding.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `bootstrap`, `write-human`.
- Owned files: `Cargo.toml`, `rust-toolchain.toml`, `.cargo/config.toml`, `crates/cairn-app/**`, `docs/dev/gates.md`, `CLAUDE.md`.
- Dependencies within wave: none.
- Acceptance: workspace builds with empty crates; `cargo fmt --check` and `cargo check --workspace` pass; `CLAUDE.md` includes CPU/Git discipline and phase-gate commands.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `clean-code`, `rust-engineer`.
- Review brief shape: verify workspace boundaries, no Python shims, no premature dependencies, and no ambiguous developer instructions.

Task 2: `cairn-types` schemas and IDs.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-types/**`.
- Dependencies within wave: none.
- Acceptance: typed IDs, schema structs, `OperationState`, `SourceClass`, `AdapterCapabilities`, event kinds, and timestamp wrappers exist; serde round-trip tests pass; no untyped stringly IDs in public API.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify schema names and fields match spec Appendix B, especially session lineage, FileVersion, RepoEpoch, Observation, ContextFrame, DaemonDecision, DenyDecision, and capability bitset.

Task 3: `cairn-config` config hash and feature flags.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-config/**`, `fixtures/config/**`.
- Dependencies within wave: none.
- Acceptance: deterministic config loading, protocol version pin, feature flags, strict/default/advisory modes, ablation flag representation; config hash stable across key ordering.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: check config hash determinism, forward-compatible unknown keys, and no feature policy hidden outside config.

Task 4: `cairn-identity` project/worktree identity.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `using-git-worktrees`.
- Owned files: `crates/cairn-identity/**`, `fixtures/identity/**`.
- Dependencies within wave: none.
- Acceptance: canonical root, git common dir, worktree ID, config hash, protocol version, and platform path semantics tested; symlink and case-sensitive/case-insensitive fixtures covered.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: search for false matches between sibling worktrees, symlink loops, bind mounts, and config-version collisions.

Task 5: `cairn-file` FileVersion and source-class base.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-file/**`, `fixtures/files/**`.
- Dependencies within wave: none.
- Acceptance: content hash is the source of truth; size, mtime observed, executable bit, symlink target, repo epoch ID, and initial source-class heuristics are tested.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify no mtime-based freshness decisions, hash streaming for large files, and explicit generated/vendored defaults.

Task 6: `cairn-vcs` RepoEpoch.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `using-git-worktrees`.
- Owned files: `crates/cairn-vcs/**`, `fixtures/vcs/**`.
- Dependencies within wave: none.
- Acceptance: detects normal, detached head, merge, rebase, cherry-pick, bisect, unknown VCS; captures head OID, branch ref, index tree OID when available, working tree digest.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify Git command boundaries, failure behavior outside Git repos, and no panics on partial states.

#### Wave 1.2 - Storage, protocol, daemon core, all concurrent

**Wave-internal contracts (orchestrator commits before fan-out).** Trait skeletons for `EventLog` (append, replay, monotonic ID), `DaemonEvent` and `DaemonDecision` enum shells with envelope variants stubbed (`SessionStart`, `SessionEnd`, `ToolIntent`, `ToolResult`, `ReadObserved`, `EditIntent`, `EditApplied`, `CommandIntent`, `CommandResult`, `CompactIntent`, `VcsStateChanged`, `AdapterHeartbeat`), and `DaemonClient` trait (`connect_or_launch`, `send_event`). Storage and daemon-client publish their public traits in `lib.rs` first; daemon and CLI consume those traits. No implementation needed in the contract commit — only the type and trait surface.

Task 1: `cairn-storage` append-only event log.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-storage/**`, `crates/cairn-storage/migrations/**`.
- Dependencies within wave: none, uses `cairn-types` public contract from Wave 1.1.
- Acceptance: append-only log with monotonic event IDs, SQLite WAL or equivalent durable backend, redaction pass for obvious credential patterns, event replay API, materialized view skeleton; concurrent append stress test passes.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: check atomicity, ordering, replay determinism, redaction boundaries, and migration strategy.

Task 2: `cairn-protocol` daemon event and decision envelopes.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-protocol/**`.
- Dependencies within wave: none.
- Acceptance: `SessionStart`, `SessionEnd`, `ToolIntent`, `ToolResult`, `ReadObserved`, `EditIntent`, `EditApplied`, `CommandIntent`, `CommandResult`, `CompactIntent`, `VcsStateChanged`, `AdapterHeartbeat` envelopes exist; `DaemonDecision` supports all decision kinds.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify protocol matches spec §6, is versioned, and keeps capability-specific behavior out of adapters.

Task 3: `cairn-daemon-client` connect or launch.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `using-git-worktrees`.
- Owned files: `crates/cairn-daemon-client/**`.
- Dependencies within wave: none.
- Acceptance: client discovers daemon socket by project identity, launches if absent, times out safely, returns structured degraded errors.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify fail-open semantics and no adapter-specific assumptions.

Task 4: `cairn-daemon` single-instance lifecycle.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-daemon/**`.
- Dependencies within wave: none.
- Acceptance: DB lease, heartbeat file, generation ID, graceful attach, stale lease recovery, and shutdown path implemented; concurrent launch test shows one daemon.
- Reviewer pairing: `delegate droid grok safe` with `rust-engineer`, `clean-code`.
- Review brief shape: stress split-brain and stale-heartbeat cases; inspect lock acquisition boundaries and recovery receipts.

Task 5: `cairn-cli` minimal status and doctor.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `plain-language`.
- Owned files: `crates/cairn-cli/**`.
- Dependencies within wave: none.
- Acceptance: `cairn status` and `cairn daemon doctor --self-test` call daemon client, show identity, daemon generation, storage path, and degraded state.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify output is terse, machine-parseable when requested, and never claims enforcement capability not registered.

Task 6: `cairn-harness-sim` Phase 1 daemon fixture.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-harness-sim/**`, `tests/phase1_daemon.rs`.
- Dependencies within wave: none.
- Acceptance: deterministic fixture launches N simulated clients against one worktree, records capability registration, and verifies one daemon generation.
- Reviewer pairing: `delegate cursor safe` with `rust-engineer`, `clean-code`.
- Review brief shape: check fixture determinism and that it can be reused in Phase 2 without rewriting.

#### Wave 1.3 - Phase 1 integration and acceptance

**Wave-internal contracts (orchestrator commits before fan-out).** `AdapterCapabilities` bitset finalized in `cairn-protocol` with all P0/P1/P2 capability bits enumerated, and `CapabilityRegistration` event variant added to `DaemonEvent`. Capability view query added to `Daemon` trait. Workers then implement against the stable bitset; no late-bound capability discovery.

Task 1: `cairn-app` binary wiring.

- Owner: orchestrator direct.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `finishing-a-development-branch`; secondary `checkpoint`.
- Owned files: `crates/cairn-app/**`, root `Cargo.toml`.
- Dependencies within wave: none.
- Acceptance: `cairn daemon`, `cairn status`, `cairn daemon doctor --self-test` invoke the right crates; root gate runs.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify binary wiring only, no hidden logic placed in app crate.

Task 2: Adapter capability registration endpoint.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-daemon/**`, `crates/cairn-protocol/**`, `tests/phase1_capabilities.rs`.
- Dependencies within wave: none.
- Acceptance: daemon accepts capability registration, writes event, exposes current capability view, and rejects unknown protocol version with fail-open client notice.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: check bitset completeness and version downgrade behavior.

Task 3: V1 corpus inventory manifest.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`; primary `tdd-workflow`; secondary `write-human`.
- Owned files: `tests/v1-corpus/manifest.toml`, `tests/v1-corpus/README.md`, `docs/dev/v1-test-porting.md`.
- Dependencies within wave: none.
- Acceptance: V1 tests are categorized into identity, storage, ledger, context, graph, adapters, diagnostics, CLI, benchmarks; no attempt to port implementation details.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `clean-code`.
- Review brief shape: verify categories map to phases and do not introduce Python compatibility obligations.

Task 4: Phase 1 integration stress tests.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `tests/phase1_identity.rs`, `tests/phase1_event_log.rs`, `tests/phase1_concurrent_launch.rs`.
- Dependencies within wave: none.
- Acceptance: concurrent launch, event ID, config hash, repo epoch, and file-version tests pass; flakiness seed logged.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: try to falsify deterministic assumptions and identify flaky test hazards.

Phase checkpoints:

- Bug hunt: no separate checkpoint. The adversarial tests inside the phase gate (concurrent daemon launch, monotonic event IDs under stress, replay determinism, symlink/case/path identity fuzzing, stale-lease recovery) *are* the Phase 1 bug hunt. Invoke `diagnose` only if those tests go red or flaky.
- Deslopify pass: no. Too early; crate seams are still molten.
- Strategic re-review by GPT-Pro: no.
- Event-log golden fixture: freeze a minimal event-log fixture bundle (SessionStart, capability registration, two ToolIntent events with monotonic IDs, SessionEnd) and a replay test that verifies byte-for-byte deterministic replay. The event log is the spinal cord; schema drift here paralyzes Phase 2 ledger replay, Phase 3 ContextFrame replay, Phase 5 spool replay, and the benchmark harness.
- Phase gate: `cargo fmt --check`; `cargo clippy --workspace --all-targets --no-deps`; `cargo test --workspace`; `cairn daemon doctor --self-test`; `cargo test --test phase1_concurrent_launch`; event-log golden fixture replay.

Advance refusal:

- Any split-brain daemon case.
- Any non-monotonic event ID.
- Any identity false match across worktrees.
- Any red full gate not quarantined as unrelated infrastructure flake.

### Phase 2 - Session registry, ObservationLedger, EditLedger, direct file freshness

Goal: make the system know what each agent has seen and what it tried to edit. This phase ships session materialized views, ObservationLedger writes from `ReadObserved`, EditLedger writes from `EditIntent` and `EditApplied`, TOCTOU precondition plumbing where supported, direct file freshness checks without graph dependency, and lineage-aware subagent inheritance.

Dependencies on prior phases: Phase 1 gate green; daemon lifecycle, storage, protocol, identity, file versions, repo epochs, and capability registration stable.

Acceptance criteria:

- “Agent α read V1, β advanced target file to V2, α cannot blindly edit V1” works end-to-end.
- Self-edits by same session do not false-positive.
- `expected_target_file_hash` is attached and verified where capability exists.
- `edit_race_detected` event is emitted when write lands against unexpected previous hash.
- Child session inherits explicit observation snapshot; parent does not auto-learn child observations.
- Minimal deny recovery path requires re-read and then allows or downgrades.

#### Wave 2.1 - Ledgers and views, all concurrent

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-types`: `AgentSessionId`, `TaskId`, `ObservationId`, `EditIntentId`, `EditAppliedId`, `SessionLineage` (parent_id, root_id, depth). In `cairn-protocol`: typed envelopes for `SessionStart` (with lineage fields), `ReadObserved`, `EditIntent`, `EditApplied`. In `cairn-ledger`: public trait surface for `record_observation`, `record_edit_intent`, `record_edit_applied`, `record_session_lifecycle` — bodies stubbed with `todo!()`. In `cairn-storage`: materialized-view trait skeletons for sessions, observations, edit attempts, current file versions. CLI and daemon consume the trait surfaces; ledger and storage workers fill in implementations.

Task 1: `cairn-ledger` session, task, observation, edit APIs.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-ledger/**`.
- Dependencies within wave: none.
- Acceptance: public APIs record session lifecycle, task identity, read observations, edit intents, edit applied; unit tests cover missing session, duplicate reads, same-session edits.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify the ledger models agent belief, not repo truth; check that session ≠ task and child sessions are first-class.

Task 2: Storage materialized views for ledgers.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-storage/**`, `crates/cairn-storage/migrations/**`.
- Dependencies within wave: none.
- Acceptance: current sessions, direct observations, edit attempts, current file versions, per-session working sets, and task assignment views replay from event log.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify replay determinism, migration safety, and no derived view becomes source of truth.

Task 3: Daemon event ingestion for read/edit/session events.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-daemon/**`, `tests/phase2_ingestion.rs`.
- Dependencies within wave: none.
- Acceptance: daemon accepts `SessionStart`, `ReadObserved`, `EditIntent`, `EditApplied`, `SessionEnd`; persists events and returns event IDs.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: check request validation, idempotency, and fail-open behavior for non-critical ingestion errors.

Task 4: Phase 2 CLI ledger visibility seed.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `plain-language`.
- Owned files: `crates/cairn-cli/**`, `tests/phase2_cli_sessions.rs`.
- Dependencies within wave: none.
- Acceptance: `cairn sessions list`, `cairn sessions show`, `cairn observations show`, and `cairn ledger tail --session` work against materialized views.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify commands explain ledger state quickly without exposing raw internal noise.

Task 5: V1 corpus direct-freshness port batch.

- Owner: `delegate cursor work --isolation worktree`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `using-git-worktrees`.
- Owned files: `tests/v1-corpus/direct_freshness/**`, `tests/phase2_direct_freshness.rs`.
- Dependencies within wave: none.
- Acceptance: at least 20 V1-inspired cases ported for direct file observations, same-session edits, other-session edits, missing observations, and task/session boundaries.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: check behavior is ported in spirit, with no Python-shaped compatibility ghosts.

#### Wave 2.2 - Direct freshness and TOCTOU

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-protocol`: `expected_target_file_hash`, `adapter_precondition_support`, `decision_id`, `observed_previous_hash` fields on `EditIntent`, `DaemonDecision`, and `EditApplied`. In `cairn-types`: `FreshnessDecision` enum (Allow, Advisory, Deny) with revalidation-set payload. In `cairn-ledger`: `check_direct_freshness(session, target_file) -> FreshnessDecision` trait signature. In `cairn-daemon`: PreEdit endpoint request/response types. Adapter-core consumes the precondition fields; ledger and daemon fill in decision logic.

Task 1: Direct file freshness decision engine.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-ledger/**`, `tests/phase2_direct_freshness.rs`.
- Dependencies within wave: none.
- Acceptance: `check_direct_freshness(session, target_file)` returns allow, advisory, or deny based on latest relevant observation, same-session exclusion, strict/default/advisory mode.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: attack self-edit false positives and missing-observation policy.

Task 2: Protocol precondition fields.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-protocol/**`.
- Dependencies within wave: none.
- Acceptance: `EditIntent`, `DaemonDecision`, and `EditApplied` carry `expected_target_file_hash`, adapter precondition support, decision ID, and resulting observed previous hash.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify TOCTOU fields cannot be silently dropped by adapters.

Task 3: Daemon PreEdit endpoint.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-daemon/**`, `tests/phase2_preedit.rs`.
- Dependencies within wave: none.
- Acceptance: daemon checks direct freshness on `EditIntent`, returns deny/advisory/allow with revalidation instructions and expected hash where supported.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify strict/default/advisory behavior, timeout behavior, and no graph assumptions leak in.

Task 4: Adapter-core precondition and spool contract.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `using-git-worktrees`.
- Owned files: `crates/cairn-adapter-core/**`.
- Dependencies within wave: none.
- Acceptance: shared adapter runtime can attach precondition metadata where supported, spool observed events on daemon unavailable, and replay with idempotency key.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify capability-gated behavior and no adapter-specific branching in core daemon logic.

Task 5: `edit_race_detected` verification path.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-storage/**`, `crates/cairn-daemon/**`, `tests/phase2_toctou.rs`.
- Dependencies within wave: none.
- Acceptance: post-edit verification compares expected previous hash to actual previous hash and emits `edit_race_detected` on mismatch; strict mode forces re-read on next edit.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: simulate β edit between α allow and α write; verify event and next-decision behavior.

#### Wave 2.3 - Subagent inheritance and deny recovery

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-types`: `InheritedObservation` (with origin_session_id, original_observation_id, inheritance_depth), `SubagentOrientationPacket` (task slice, parent working set, inherited observations, stale risks, ContextFrame ID references, suggested reads), `DenyDecision` (deny_id, minimum_revalidation_set, proof_availability, override_policy), `DenyLoopGuard`. In `cairn-protocol`: child-session spawn event. In `cairn-ledger`: `inherit_observations(child, parent)` trait signature, deny-loop-guard query. Daemon consumes the spawn event; ledger and context (Phase 3 boundary) consume the orientation packet types.

Task 1: Subagent inheritance in `cairn-ledger`.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-ledger/**`, `tests/phase2_subagent_inheritance.rs`.
- Dependencies within wave: none.
- Acceptance: child sessions get `InheritedObservation` snapshots; effective observation set is direct union inherited; origin is preserved; parent never auto-learns child reads.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify lineage semantics against spec §3.1, especially stale inherited facts and parent report boundaries.

Task 2: Orientation packet skeleton types.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-context/**`, `crates/cairn-types/**`.
- Dependencies within wave: none.
- Acceptance: `SubagentOrientationPacket` carries task slice, parent working set, files/symbols explored, stale risks, ContextFrame IDs, changed-since-parent warnings, suggested reads.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify packet is pointer-heavy and does not pretend the child read files it inherited.

Task 3: DenyDecision minimal recovery and loop guard.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-ledger/**`, `crates/cairn-protocol/**`, `tests/phase2_deny_recovery.rs`.
- Dependencies within wave: none.
- Acceptance: deny includes minimum revalidation set, proof availability, override policy; after revalidation, same cause cannot deny again unless a new cause exists.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: search for haunted revolving-door deny loops and under-scoped overrides.

Task 4: Daemon child-session spawn endpoint.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-daemon/**`, `tests/phase2_child_spawn.rs`.
- Dependencies within wave: none.
- Acceptance: daemon handles child spawn event, invokes inheritance snapshot, returns orientation packet skeleton, records lineage fields.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify child is a real session and lineage depth/root/parent IDs are correct.

Task 5: Multi-agent stale direct scenario runner.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `using-git-worktrees`.
- Owned files: `crates/cairn-harness-sim/**`, `tests/phase2_multi_agent_direct.rs`.
- Dependencies within wave: none.
- Acceptance: deterministic scenario: α reads V1, β edits V2, α edit intent denied/advised, α revalidates, α edit allowed; metrics events captured.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify the scenario can later swap direct freshness for graph freshness without rewriting harness.

Phase checkpoints:

- Bug hunt: yes. Skills: `debugging-systematic`, `diagnose`. Focus: false-positive self-edits, deny loops, event replay, spawn inheritance.
- Deslopify pass: no — Phase 2's cross-crate surface is still thin (ledger, storage, daemon, protocol, Phase 2 touches in adapter-core). Replace with targeted `code-simplifier` over those crates. `desloppify-deep` lands at Phase 4.
- Strategic re-review by GPT-Pro: no.
- Event-log golden fixture: extend the Phase 1 fixture bundle to cover ObservationLedger writes, EditIntent + EditApplied envelopes (with and without TOCTOU precondition fields), child-session spawn events, and `edit_race_detected`. Replay test verifies materialized ledger views reconstruct deterministically.
- Phase gate: full workspace gate; `tests/phase2_*`; direct stale edit injected benchmark; subagent inheritance benchmark; direct-freshness microbench against a 10,000-observation in-memory fixture (yellow flag at p95 > 1 ms; hard stop at p95 > 5 ms or if scaling is obviously non-linear, since either indicates an O(n) scan or pathological allocation in the hot path that Phase 4's latency POC will only compound); event-log golden fixture replay.

Advance refusal:

- Any self-edit false positive above 0 in fixtures.
- Any failure to deny/advisory direct stale edit in default mode.
- Any child inheritance case where parent auto-learns child observations.
- Any unresolved bug-hunt P0/P1 issue.

### Phase 3 - ContextFrame ledger and scheduler skeleton

Goal: every pushed decoration becomes a durable, typed ContextFrame with source versions, token count, confidence, expiration, novelty hash, and optional utility receipt placeholder. This phase ships template emission only, not full smart scheduling. It also records PreCompact checkpoints and ties context frame IDs back to observations.

Dependencies on prior phases: Phase 2 ledgers and daemon event ingestion green.

Acceptance criteria:

- Every emitted decoration has a ContextFrame persisted before response.
- Observations can reference the ContextFrame that caused or accompanied the read.
- Subagent orientation packet references prior ContextFrame IDs.
- PreCompact checkpoint event writes and can be surfaced in post-compact orientation skeleton.
- Per-read skeleton decorations stay below 600 tokens in golden tests.
- Duplicate template decorations become “unchanged since prior frame,” not repeated blob output.

#### Wave 3.1 - ContextFrame core

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-types`: `ContextFrameId`, `ContextFrame` struct (facts, source_versions, token_count, confidence, expiry, novelty_hash, utility_receipt placeholder), `TokenBudget` per surface, `ContextRenderFormat` enum. In `cairn-context`: public trait `ContextRenderer` (human + JSON), trait `emit_context_frame`, trait `TokenEstimator`. In `cairn-storage`: context dedup materialized-view trait. CLI consumes the render and CFID query surface; storage worker fills in the dedup view.

Task 1: ContextFrame storage and API.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-context/**`.
- Dependencies within wave: none.
- Acceptance: `emit_context_frame` persists frame with facts, source versions, token count, confidence, expiry, novelty hash, utility receipt placeholder; tests cover replay.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify frame schema matches spec Appendix B and can later support dedup/utility scoring.

Task 2: Storage view for context dedup state.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-storage/**`, `crates/cairn-storage/migrations/**`.
- Dependencies within wave: none.
- Acceptance: materialized context dedup view keyed by agent, fact identity, source version, novelty hash; replay deterministic.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify dedup state is derived and never the source of truth.

Task 3: Token estimator and budget guard.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `plain-language`.
- Owned files: `crates/cairn-context/src/token_budget.rs`, `crates/cairn-context/tests/token_budget.rs`.
- Dependencies within wave: none.
- Acceptance: deterministic conservative token estimator; per-surface budget constants; tests for 600-token read and 1,200-token orientation ceilings.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify estimator is conservative and cheap enough for hook path.

Task 4: Context renderer skeleton.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `plain-language`.
- Owned files: `crates/cairn-context/src/render/**`, `crates/cairn-context/tests/render_golden.rs`.
- Dependencies within wave: none.
- Acceptance: human-readable and JSON renderers for orientation, read decoration, stale advisory, and unchanged-frame notice; golden outputs stable.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify output is action-oriented, bounded, and not transcript confetti.

Task 5: ContextFrame CLI visibility.

- Owner: `delegate cursor work --isolation worktree`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `plain-language`.
- Owned files: `crates/cairn-cli/src/context.rs`, `tests/phase3_context_cli.rs`.
- Dependencies within wave: none.
- Acceptance: `cairn context show <context_frame_id>` displays facts, source versions, confidence, triggering tool call, token count, expiry.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify operator can answer “why did the system say that?” quickly.

#### Wave 3.2 - Template scheduler, PreCompact, observation linkage

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-context`: `schedule_decoration(surface, session, payload) -> ScheduleDecision` trait. In `cairn-protocol`: `CompactionCheckpoint` event variant with pre-compact frames, promoted/omitted fact IDs, working-set snapshot. In `cairn-types`: `context_frame_id: Option<ContextFrameId>` field added to `Observation`. Subagent orientation packet emitter consumes the packet type stubbed in Wave 2.3.

Task 1: Template scheduler skeleton.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-context/src/scheduler/**`, `crates/cairn-context/tests/scheduler_skeleton.rs`.
- Dependencies within wave: none.
- Acceptance: `schedule_decoration` applies surface budget, dedup check, and template rules for SessionStart, ReadObserved, EditIntent advisory, PreCompact.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify skeleton is rule-based but extensible and does not pretend to have learned utility.

Task 2: Observation-to-ContextFrame linkage.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-ledger/**`, `tests/phase3_observation_context.rs`.
- Dependencies within wave: none.
- Acceptance: read observation can reference emitted ContextFrame; inherited observations preserve source frame ID; tests cover replay and missing frame.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify provenance path from context to observation to staleness decision.

Task 3: PreCompact checkpoint primitive.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-protocol/**`, `crates/cairn-context/**`, `tests/phase3_precompact.rs`.
- Dependencies within wave: none.
- Acceptance: `CompactIntent` records `CompactionCheckpoint` with pre-compact frames, promoted/omitted fact IDs, working set snapshot.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: check checkpoint schema and P0 skeleton vs P1 survival packet boundary.

Task 4: Subagent orientation packet emitter.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `plain-language`.
- Owned files: `crates/cairn-context/src/subagent.rs`, `tests/phase3_subagent_packet.rs`.
- Dependencies within wave: none.
- Acceptance: packet renders current task slice, inherited observations, stale labels, ContextFrame IDs, suggested reads; golden tests enforce bounded output.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify no stale inherited fact is emitted without a stale label.

Task 5: V1 corpus context-frame port batch.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `using-git-worktrees`.
- Owned files: `tests/v1-corpus/context/**`, `tests/phase3_context_v1.rs`.
- Dependencies within wave: none.
- Acceptance: at least 20 V1-inspired context/dedup/orientation tests ported as Rust golden tests.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify tests target behavior, not V1 implementation structure.

#### Wave 3.3 - Daemon response integration

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-protocol`: `context_frame_ids: Vec<ContextFrameId>` field added to `DaemonDecision`. `DegradedContextNotice` payload type. In `cairn-context`: replay trait surface for context-frame stream reconstruction. Benchmark crate consumes the replay trait.

Task 1: DaemonDecision context-frame integration.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-daemon/**`, `tests/phase3_daemon_context.rs`.
- Dependencies within wave: none.
- Acceptance: daemon can attach context frame IDs to decorate/advisory/deny decisions and persists frame before response.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify no response can reference a missing ContextFrame.

Task 2: Fail-open degraded context notices.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `plain-language`.
- Owned files: `crates/cairn-context/src/degraded.rs`, `crates/cairn-protocol/**`, `tests/phase3_degraded_context.rs`.
- Dependencies within wave: none.
- Acceptance: read/orientation surfaces emit brief degraded notice when daemon/storage/context is partial; ordinary tool call remains unblocked.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify fail-open language and no hidden fail-closed behavior.

Task 3: Context snapshot replay.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-storage/**`, `crates/cairn-context/tests/replay.rs`.
- Dependencies within wave: none.
- Acceptance: replay reconstructs context-frame stream for a session, with identical dedup state and token counts.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify replay determinism for benchmark and prove surfaces.

Task 4: Phase 3 context budget benchmark.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `benches/context_budget.rs`, `tests/phase3_budget.rs`.
- Dependencies within wave: none.
- Acceptance: representative skeleton decorations under 600 tokens p95, session orientation under 1,200 tokens p95 in fixture set.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: check benchmark realism and prevent overfitting to toy fixtures.

Phase checkpoints:

- Bug hunt: yes. Focus: missing ContextFrames, duplicate emission, stale inherited packets, PreCompact references.
- Deslopify pass: no. Phase 4 graph will reshape context interfaces.
- Strategic re-review by GPT-Pro: no.
- Event-log golden fixture: extend the Phase 2 bundle to cover ContextFrame emission, observation-to-ContextFrame linkage, PreCompact checkpoint events, and subagent orientation packet creation. Replay test verifies dedup state and token counts reconstruct identically.
- Phase gate: full workspace gate; `tests/phase3_*`; context budget benchmark quick mode; event-log golden fixture replay.

Advance refusal:

- Any decoration without a persisted ContextFrame.
- Any ContextFrame replay mismatch.
- Any PreCompact checkpoint missing working-set snapshot.
- Any golden read decoration over 600 tokens without explicit exception.

### Phase 4 - Minimal graph

Goal: implement the P0-α minimal graph only to feed stale-context checks, read decorations, symbol lookup, and provenance. This phase builds file inventory, imports/exports, static dependency sets, exported-surface contract and implementation fingerprints, simple symbol locations, graph versions, and confidence/provenance. No framework extraction and no diagnostics yet.

Dependencies on prior phases: Phase 3 context and ledgers green; stable file versions and event log.

Acceptance criteria:

- P0-α TypeScript/JavaScript, Python, Go, and Rust fixtures produce file inventory, imports/exports, static dependency sets, simple symbol locations, and fingerprints.
- Every graph fact carries confidence and provenance.
- Contract fingerprint stable across comments/whitespace/formatting.
- Pre-edit dependency staleness works against contract fingerprint change.
- False-positive rate is measurable.
- Latency POC reports Read decoration, PreEdit stale check, and graph update timings.

#### Wave 4.0 - Graph and extractor premortem, no code

Task: Phase 4 premortem.

- Owner: orchestrator direct.
- Skill stack: primary `premortem`; secondary `parallel-subagent-discipline`, `orchestrator`.
- Owned files: none.
- Dependencies within wave: none.
- Acceptance: written premortem covering false-positive hard denies sourced from heuristic-only facts, contract-fingerprint instability under whitespace/comment churn, dynamic-language overconfidence (Python decorators, Ruby metaprogramming, Rust macros), the temptation to let `cairn-graph` drift into being the primary API surface rather than an input to the ledger, compiler-backed-seam availability variance across target projects (per §3.7 — what happens when tsserver/Pyright/gopls/rust-analyzer is absent), Salsa query-key omissions that produce stale cached facts, and latency POC methodology traps (warm-cache bias, empty-work benchmarks, p99 noise on small fixture sets).
- Reviewer pairing: `Agent plan-reviewer`.
- Review brief shape: attack whether the graph contracts shipped in Wave 4.1 can be bent by adapters in Phase 5, and whether confidence/provenance enforcement at the ledger's pre-edit decision point is sufficient to keep weak facts out of hard-deny territory.

#### Wave 4.1 - Graph contracts, incremental core, latency POC

**Wave-internal contracts (orchestrator commits before fan-out). This is the highest-risk contract wave in the entire build.** Wave 4.1 introduces five interlocking crates whose public surfaces must be stable before any of them ship implementation, otherwise workers will invent five tiny kingdoms and the integration cost will dwarf the implementation cost. The orchestrator commits the following type/trait skeleton in a single pre-wave contract commit:

In `cairn-incremental`: `IncrementalDb` trait, `QueryKey` struct (must include `content_hash`, `config_hash`, `tool_version`, `extractor_version`, `grammar_version`), `QueryInvalidation` API, Salsa version pinned and re-exported behind crate boundary so no other crate imports Salsa directly.

In `cairn-graph`: `GraphSnapshot` (versioned), `GraphVersion`, `SurfaceItem` (per spec Appendix B fields), `DependencySet` (file-level and per-exported-surface), `FingerprintSet` (contract + implementation), `SymbolOccurrence`, `SymbolLineageId`, `Confidence` enum (High/Medium/Low), `FactProvenance` (extractor_id, extractor_version, grammar_version, input_hash, source_span, tool_backed: bool), `GraphQuery` trait surface (no implementation — only signatures), `prove_fact(fact_id) -> ProofTrace` trait method, file inventory types, partial-coverage markers, generated/vendored source-class hook points.

In `cairn-extract-core`: `Extractor` trait (per language), `Resolver` trait, `GrammarAdapter` trait, `FrameworkExtractor` trait, `BridgeExtractor` trait, `ExtractedFact` struct embedding `FactProvenance` and `Confidence`, `ExtractorVersion` newtype. The compiler-backed-seam adapter slots from §3.7 are declared here as optional trait associated types.

In `cairn-bench`: `LatencyPocScenario` trait + `LatencyReport` struct.

In `cairn-ledger`: trait signature for `check_dependency_freshness(session, target_file, graph_snapshot)` added — implementation lives in Wave 4.4. This signature must exist now so adapters and MCP in Phase 5 can reason about it without churn.

Belief-management discipline check (the §14 design pin Pro flagged): no method on `GraphQuery` should return a "current truth" answer that bypasses `cairn-ledger`'s observation/edit semantics. Graph facts are inputs to ledger decisions, not the primary API. The reviewer for Tasks 2 and 4 must verify this explicitly.

Task 1: `cairn-incremental` Salsa boundary.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-incremental/**`.
- Dependencies within wave: none.
- Acceptance: Salsa version pinned; query keys include content hash, config hash, tool version, extractor version; swap boundary documented; unit tests cover invalidation.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify Salsa lock-in blast radius is exactly this crate.

Task 2: `cairn-graph` graph model and provenance.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-graph/**`.
- Dependencies within wave: none.
- Acceptance: `GraphSnapshot`, `GraphVersion`, `SurfaceItem`, `DependencySet`, `FingerprintSet`, symbol occurrence/lineage IDs, confidence/provenance types implemented.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify SurfaceItem fields, provenance coverage, and no graph API bypasses ledger semantics.

Task 3: `cairn-extract-core` versioned extractor contracts.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-core/**`.
- Dependencies within wave: none.
- Acceptance: grammar adapter, language extractor, resolver, framework extractor, bridge extractor traits; emitted facts carry source span, confidence, provenance, extractor version, input hash.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify contracts support P0-α and later P0-β/P2 without daemon changes.

Task 4: File inventory and graph versioning.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-graph/src/inventory.rs`, `crates/cairn-graph/tests/inventory.rs`.
- Dependencies within wave: none.
- Acceptance: inventory maps source-class-aware files to graph inputs; graph versions emitted after extraction; partial coverage markers represented.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify generated/vendored policy hooks exist without implementing full policy yet.

Task 5: Latency POC harness.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-bench/**`, `benches/latency_poc.rs`, `fixtures/latency/**`.
- Dependencies within wave: none.
- Acceptance: quick benchmark measures warm read decoration skeleton, graph query, pre-edit direct freshness, and incremental single-file update on representative repo fixture.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify benchmark cannot silently benchmark empty work and reports p50/p95/p99.

#### Wave 4.2 - P0-α extraction pass 1

**Wave-internal contracts (orchestrator commits before fan-out).** No new public surface required — all four language extractors implement against the `Extractor` and `GrammarAdapter` traits committed in Wave 4.1. Pre-fan-out the orchestrator commits a per-language fixture schema (`fixtures/p0alpha/<lang>/expected.toml` format) and a shared fixture-runner trait so all four language workers share the same precision/recall reporting shape. The fixture-scoring harness task (Task 5) then implements the runner without renegotiating the report format.

Task 1: TypeScript/JavaScript pass 1 extractor.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-p0alpha/src/typescript.rs`, `fixtures/p0alpha/typescript/**`.
- Dependencies within wave: none.
- Acceptance: imports, exports, symbols, modules, simple references, tsconfig/path alias inputs represented; fixture precision/recall report generated.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: check compiler-backed vs tree-sitter vs heuristic provenance separation.

Task 2: Python pass 1 extractor.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-p0alpha/src/python.rs`, `fixtures/p0alpha/python/**`.
- Dependencies within wave: none.
- Acceptance: imports, module public surface, functions/classes/constants, decorators, dynamic-surface unknown markers captured.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify dynamic fallback never drives hard deny alone.

Task 3: Go pass 1 extractor.

- Owner: `delegate droid qwen work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-p0alpha/src/go.rs`, `fixtures/p0alpha/go/**`.
- Dependencies within wave: none.
- Acceptance: package imports, exported identifiers, functions, methods, structs, interfaces, build tags represented.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify build-tag and module-config provenance.

Task 4: Rust pass 1 extractor.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-p0alpha/src/rust.rs`, `fixtures/p0alpha/rust/**`.
- Dependencies within wave: none.
- Acceptance: modules, `use`, public/crate-visible items, traits, impl blocks, cfg/feature flags, macros lower-confidence marker represented.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify rust-analyzer/rustdoc future adapter seam and no overconfident macro claims.

Task 5: P0-α fixture scoring harness.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-extract-p0alpha/tests/**`, `fixtures/p0alpha/expected/**`.
- Dependencies within wave: none.
- Acceptance: fixture runner reports precision/recall by language for imports, exports, symbols, dependency edges; CI quick mode exists.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify scoring is not tautological and expected files are human-auditable.

#### Wave 4.3 - Resolution and fingerprints

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-graph`: `ContractFingerprint`, `ImplementationFingerprint`, `FingerprintInputs` (the canonical list of what feeds each hash — public signatures, visibility modifiers, framework annotations, build-tag/cfg/feature-flag inputs, macro-expanded surface where applicable). `DependencyResolver` trait surface for static dependency-set construction with `edges_with_provenance` query. Pre-fan-out ensures all four language fingerprint implementations target the same input shape; otherwise fingerprint stability fixtures cannot share a scoring template.

Task 1: TypeScript/JavaScript contract and implementation fingerprints.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-extract-p0alpha/src/typescript_fingerprint.rs`, `fixtures/p0alpha/typescript_fingerprint/**`.
- Dependencies within wave: none.
- Acceptance: contract hash changes on exported surface/signature/route-contract shape; implementation hash changes on exported body; whitespace/comment change stable below 1 percent.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: check false positive/false negative fixture matrix.

Task 2: Python contract and implementation fingerprints.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-extract-p0alpha/src/python_fingerprint.rs`, `fixtures/p0alpha/python_fingerprint/**`.
- Dependencies within wave: none.
- Acceptance: `__all__`, public functions/classes, dataclasses, Pydantic/FastAPI/Django/Flask signatures where recognized; dynamic unknown changes advisory by default.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify dynamic magic policy and strict-mode hooks.

Task 3: Go contract and implementation fingerprints.

- Owner: `delegate droid qwen work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-extract-p0alpha/src/go_fingerprint.rs`, `fixtures/p0alpha/go_fingerprint/**`.
- Dependencies within wave: none.
- Acceptance: exported package API, methods, interfaces, struct tags, constants, vars, generics; `init` implementation impact.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify build-tag-sensitive hash inputs.

Task 4: Rust contract and implementation fingerprints.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-extract-p0alpha/src/rust_fingerprint.rs`, `fixtures/p0alpha/rust_fingerprint/**`.
- Dependencies within wave: none.
- Acceptance: public/crate-visible functions, structs, enums, traits, type aliases, consts, statics, macros low-confidence, feature/cfg-sensitive hash input.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify visibility semantics and macro confidence demotion.

Task 5: Dependency set builder and resolver.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-graph/src/dependencies.rs`, `crates/cairn-graph/tests/dependencies.rs`.
- Dependencies within wave: none.
- Acceptance: static dependency sets by file and exported surface; direct import edges and unresolved edges carry provenance/confidence; graph version stamped.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify dependency edges required for stale checks are distinguishable from navigation-only hints.

#### Wave 4.4 - Graph-fed stale checks and read decorations

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-ledger`: full body for the `check_dependency_freshness` signature stubbed in Wave 4.1, plus `MinimumRevalidationSet` computation trait. In `cairn-context`: `GraphReadDecoration` payload schema (outline, imports/exports, adjacent files, callers/tests placeholders, graph version, provenance, coverage marker, token budget) added to the context renderer surface. In `cairn-graph`: `ProofTrace` struct finalized (source spans, versions, extractor provenance, confidence, input hashes, invalidation status) — the same surface `cairn_prove` MCP will consume in Phase 5.

Task 1: Graph dependency staleness in ledger.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-ledger/**`, `tests/phase4_dep_staleness.rs`.
- Dependencies within wave: none.
- Acceptance: pre-edit check consults dependency set and contract fingerprint changes; implementation fingerprint changes advisory; heuristics never hard-deny alone.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify hard-deny requirements and minimum revalidation set.

Task 2: Graph-backed read decoration skeleton.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `plain-language`.
- Owned files: `crates/cairn-context/src/graph_decorations.rs`, `tests/phase4_read_decoration.rs`.
- Dependencies within wave: none.
- Acceptance: read decoration can include outline, imports/exports, adjacent files, callers/tests placeholders, graph version, provenance, coverage marker, under budget.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify graph facts are compact, provenance-bearing, and deduped.

Task 3: False-positive stale benchmark.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-bench/src/scenarios/dep_stale.rs`, `fixtures/bench/dep_stale/**`.
- Dependencies within wave: none.
- Acceptance: benchmark includes dependency body change with no contract change, exported signature change, same-session edit, other-session edit, and graph-stale downgrade.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify it can measure false-positive denies below 5 percent later.

Task 4: Graph proof traces.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-graph/src/prove.rs`, `crates/cairn-graph/tests/prove.rs`.
- Dependencies within wave: none.
- Acceptance: graph fact proof returns source spans, versions, extractor provenance, confidence, input hashes, invalidation status.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify proof can back `cairn_prove` without exposing operator-only internals.

Task 5: Latency POC report and hot-path fix-list.

- Owner: orchestrator direct.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `diagnose`; secondary `debugging-systematic`, `checkpoint`.
- Owned files: `docs/reports/phase4_latency_poc.md`, `benches/latency_poc.rs`.
- Dependencies within wave: none.
- Acceptance: report p50/p95/p99 for read decoration, pre-edit direct, pre-edit graph, MCP lookup placeholder, graph update; includes fix-list for any target miss.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify benchmark methodology and that missed targets produce dispatchable fixes.

Phase checkpoints:

- Bug hunt: yes. Focus: false hard denies, stale graph versions, fingerprint instability, provenance gaps, latency regressions.
- Deslopify pass: yes. Owner orchestrator invokes `desloppify-deep`.
- Strategic re-review by GPT-Pro: yes (narrow precondition consult per §7). Deliverables: Phase 4 summary, crate graph, extractor fixture scores, false-positive benchmark output, latency POC report, representative denies/proofs, reviewer fix-lists.
- Event-log golden fixture: extend the Phase 3 bundle with graph-version stamps, extracted-fact provenance entries, fingerprint deltas (contract vs implementation), and dependency-set materialization. Replay test verifies graph queries against the fixture return byte-identical results across runs.
- Phase gate: full workspace gate; P0-α fixture scoring; `tests/phase4_*`; latency POC quick benchmark; event-log golden fixture replay.

Advance refusal:

- Any hard deny based solely on heuristic/inferred fact.
- Contract fingerprint instability over 1 percent on non-semantic fixture changes.
- Missing provenance on graph facts.
- False-positive benchmark cannot distinguish contract vs implementation change.
- Latency POC lacks real graph work or p95 measurement.

### Phase 5 - Hooks and MCP as clients of daemon contracts

Goal: make hooks and MCP thin clients of the daemon, with Claude Code first to Tier 1, Codex and Cursor in parallel once adapter-core is stable, and MCP shipping the first three tools: `cairn_orient`, `cairn_observed_state`, and `cairn_prove`. This phase proves that multi-agent coordination works end-to-end through at least two adapters.

Dependencies on prior phases: Phase 4 minimal graph and stale dependency check green; GPT-Pro review fix-list resolved or explicitly accepted.

Acceptance criteria:

- Claude Code adapter reaches Tier 1 in simulator and local smoke.
- Codex reaches Tier 1 if deny/precondition support exists, otherwise Tier 2 with capability matrix.
- Cursor reaches Tier 2 expected.
- MCP tools `cairn_orient`, `cairn_observed_state`, `cairn_prove` work against daemon state.
- Harness simulator validates two-adapter multi-agent stale-context scenario.
- Hook/MCP answers agree on file versions and symbol locations in audited queries.
- Adapter spool replays after daemon crash simulation.
- Minimal `deny explain` CLI seed exists before any real or simulated hard-deny enforcement is accepted. Skeleton supports `cairn deny explain <deny_id>` returning the revalidation set, the precondition hash if any, the contributing ledger and graph fact IDs, and a pointer to the ContextFrame that triggered the decoration. The full operator CLI lands in Phase 6 Wave 6.4 — the Phase 5 seed is a small subset, but the spec makes operator visibility P0 and hard denies without inspection receipts feel haunted even in internal dogfood.

#### Wave 5.0 - Adapter rollout premortem, no code

Task: Phase 5 premortem.

- Owner: orchestrator direct.
- Skill stack: primary `premortem`; secondary `parallel-subagent-discipline`, `orchestrator`, `codex-prompting`.
- Owned files: none.
- Dependencies within wave: none.
- Acceptance: written risk sheet covers harness capability drift, hook latency, fail-open confusion, deny UX, spool replay, command interception false positives, MCP/server state divergence.
- Reviewer pairing: `Agent plan-reviewer`.
- Review brief shape: attack whether adapter implementation might fork state or create per-harness caches.

#### Wave 5.1 - Adapter core, Claude, MCP skeleton, simulator

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-adapter-core`: `AdapterRuntime` trait (daemon discovery, launch, spool, capability registration, event normalization, timeout/fail-open policy), `Spool` trait with idempotency keys, `CapabilityProfile` per tier. In `cairn-mcp`: `McpTool` trait, `McpToolRegistry`, tool envelope types for the three Phase 5 tools stubbed. In `cairn-harness-sim`: `SimAdapter` trait that can emulate Tier 1, Tier 2, and Tier 3 profiles. Real adapters and the simulator implement against the same `AdapterRuntime` surface — no harness-specific state lives in core.

Task 1: Adapter core runtime.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `using-git-worktrees`.
- Owned files: `crates/cairn-adapter-core/**`.
- Dependencies within wave: none.
- Acceptance: shared daemon discovery, launch, capability registration, spool/replay, event normalization, timeout/fail-open policy.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify core adapter does not own separate state.

Task 2: Claude Code Tier 1 adapter.

- Owner: `delegate codex work --isolation worktree`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `claude`, `codex-prompting`.
- Owned files: `crates/cairn-adapter-claude/**`, `adapters/claude/**`, `tests/phase5_claude_adapter.rs`.
- Dependencies within wave: none.
- Acceptance: maps SessionStart, Read, PreEdit, PostEdit/EditApplied, Bash/Command, PreCompact to daemon events; registers Tier 1 capability bits; fail-open notices tested.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify real capability bits and no unsupported deny claims.

Task 3: MCP server skeleton.

- Owner: `delegate codex work --isolation worktree`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `mcp-builder`; secondary `tdd-workflow`.
- Owned files: `crates/cairn-mcp/**`.
- Dependencies within wave: none.
- Acceptance: MCP server starts as daemon client; tool registry has orient, observed_state, prove placeholders; protocol errors typed; smoke test passes.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`, `mcp-builder`.
- Review brief shape: verify MCP is client of daemon contracts, not separate graph/cache.

Task 4: Harness simulator adapter conformance.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-harness-sim/**`, `tests/phase5_adapter_conformance.rs`.
- Dependencies within wave: none.
- Acceptance: simulator can emulate Tier 1, Tier 2, Tier 3 adapters and assert decision behavior by capability.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify simulator exercises capability downgrade paths.

Task 5: Capability matrix generator.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `plain-language`.
- Owned files: `crates/cairn-cli/src/capabilities.rs`, `docs/capability-matrix.md`.
- Dependencies within wave: none.
- Acceptance: `cairn status --capabilities` shows Tier 1/2/3 by adapter and exact bitset; docs match command output.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify capability matrix is honest and operator-readable.

#### Wave 5.2 - Codex, Cursor, PreCompact, multi-adapter E2E

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-adapter-core`: `PreCompactBridge` trait (how a capable adapter emits `CompactIntent` and receives the checkpoint frame). In `cairn-harness-sim`: multi-adapter scenario harness skeleton with capability-tier-aware assertions. Spool replay test surface for adapter crash injection. Codex and Cursor adapters implement against the same `AdapterRuntime` from Wave 5.1; PreCompact integration touches Claude adapter and adapter-core together.

Task 1: Codex CLI adapter.

- Owner: `delegate droid "deepseek v4 pro" work`. Implementer is deliberately not Codex; an adapter that bridges a harness's actual capability surface should be authored by someone reading Codex docs with fresh eyes rather than internalizing what Codex assumes about itself.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `codex-prompting`, `spec-quality-checklist`.
- Owned files: `crates/cairn-adapter-codex/**`, `adapters/codex/**`, `tests/phase5_codex_adapter.rs`.
- Dependencies within wave: none.
- Acceptance: registers actual supported capability tier; maps available Codex events; deny/precondition support tested if available; otherwise advisory compatibility path tested.
- Reviewer pairing: `delegate cursor safe` with `rust-engineer`, `clean-code`. Follow-up capability sanity check: `delegate codex safe` with explicit brief "Do not assert undocumented Codex capabilities; report any case where the implementer claims a capability you cannot reproduce from public Codex CLI behavior."
- Review brief shape: verify no invented Codex capability and fail-open semantics.

Task 2: Cursor adapter.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-adapter-cursor/**`, `adapters/cursor/**`, `tests/phase5_cursor_adapter.rs`.
- Dependencies within wave: none.
- Acceptance: registers expected Tier 2 or Tier 3 depending capability; maps read/advisory/MCP-compatible events; post-hoc observation path tested.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify Cursor does not pretend to enforce when it can only advise or observe.

Task 3: PreCompact hook integration.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-adapter-core/**`, `crates/cairn-adapter-claude/**`, `tests/phase5_precompact_adapter.rs`.
- Dependencies within wave: none.
- Acceptance: capable adapters emit `CompactIntent`, receive checkpoint frame, and record `PreCompactCheckpoint`.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify compaction boundary is captured before transcript loss.

Task 4: Multi-adapter E2E stale scenario.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `tests/phase5_multi_adapter.rs`, `crates/cairn-harness-sim/src/scenarios/multi_adapter.rs`.
- Dependencies within wave: none.
- Acceptance: α through Claude Tier 1, β through Codex/Cursor simulated tier, dependency/target edit race scenario emits correct deny/advisory/post-hoc state.
- Reviewer pairing: `delegate cursor safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify all final assertions are capability-aware.

Task 5: Adapter crash and replay tests.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `tests/phase5_spool_replay.rs`, `crates/cairn-adapter-core/tests/spool.rs`.
- Dependencies within wave: none.
- Acceptance: daemon crash mid-job buffers observed events and replays idempotently on reconnect; no observation loss in test harness.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: inject repeated reconnects and duplicated event IDs.

#### Wave 5.3 - Initial MCP tools and Bash interception

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-mcp`: tool envelope schemas for `cairn_orient` (task-aware project orientation request/response), `cairn_observed_state` (direct + inherited observation query response), and `cairn_prove` (fact_id / context_frame_id / deny_id / diagnostic_id / graph_edge_id discriminated input, `ProofTrace` response). In `cairn-context`: `CommandIntercept` decision type (allow, structured-answer, pass-through). Hook latency benchmark trait pre-committed so all four tool benchmarks share the same instrumentation.

Task 1: `cairn_orient`.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `mcp-builder`; secondary `tdd-workflow`.
- Owned files: `crates/cairn-mcp/src/tools/orient.rs`, `tests/phase5_mcp_orient.rs`.
- Dependencies within wave: none.
- Acceptance: returns task-aware project orientation, repo state, recent deltas, active warnings, suggested reads, and degraded/partial coverage labels.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`, `mcp-builder`.
- Review brief shape: verify orientation is bounded and uses daemon context/ledger.

Task 2: `cairn_observed_state`.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `mcp-builder`; secondary `tdd-workflow`.
- Owned files: `crates/cairn-mcp/src/tools/observed_state.rs`, `tests/phase5_mcp_observed.rs`.
- Dependencies within wave: none.
- Acceptance: returns direct and inherited observations, current vs observed versions, task/session filters, and changed-since query support.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`, `mcp-builder`.
- Review brief shape: verify the tool answers “what have I seen?” without pretending inherited facts are direct reads.

Task 3: `cairn_prove`.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `mcp-builder`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-mcp/src/tools/prove.rs`, `tests/phase5_mcp_prove.rs`.
- Dependencies within wave: none.
- Acceptance: proves fact ID, context frame ID, deny ID, diagnostic placeholder, graph edge ID using source spans, versions, provenance, confidence, invalidation status.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`, `mcp-builder`.
- Review brief shape: verify proof traces are sufficient for agent recovery and operator explanation.

Task 4: Bash search interception minimal.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-context/src/command_intercept.rs`, `crates/cairn-adapter-core/**`, `tests/phase5_bash_intercept.rs`.
- Dependencies within wave: none.
- Acceptance: broad `rg`/`grep` for high-confidence indexed symbol can return structured graph answer; uncertain commands pass through above 95 percent in fixtures.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: attack false interception and token savings claims.

Task 5: Hook latency harness seed.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-bench/src/hook_latency.rs`, `tests/phase5_hook_latency.rs`.
- Dependencies within wave: none.
- Acceptance: benchmark captures p50/p95/p99 for SessionStart, Read, PreEdit, Bash interception, MCP tool calls in simulator.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify timings include client-daemon round trip and cannot hide slow paths.

Phase checkpoints:

- Bug hunt: yes. Focus: adapter capability lies, fail-open failures, spool replay loss, hook/MCP state divergence, MCP proof gaps, false command interception.
- Deslopify pass: no. Phase 6 will add many surfaces; run `code-simplifier` on touched adapter code after review loops instead.
- Strategic re-review by GPT-Pro: yes (delta/integration consult per §7). Deliverables: capability matrix, adapter traces, MCP tool outputs, multi-adapter benchmark run, hook latency report, fail-open examples, unresolved capability risks.
- Event-log golden fixture: extend the Phase 4 bundle with capability registration events, adapter-normalized SessionStart/Read/PreEdit/PostEdit/Bash/PreCompact envelopes from each adapter tier, spool replay events with idempotency keys, and a multi-adapter race scenario. Replay test verifies daemon decisions remain byte-identical across runs.
- Phase gate: full workspace gate; `tests/phase5_*`; multi-adapter scenario; MCP smoke; hook latency quick benchmark; event-log golden fixture replay.

Advance refusal:

- Claude Code not Tier 1 in simulator.
- MCP server maintains independent graph/cache state.
- Adapter claims unsupported deny/precondition capability.
- Any ordinary read/edit/tool call bricks when daemon unavailable outside strict mode.
- Two-adapter scenario does not produce measurable coordination metrics.

### Phase 6 - Capabilities on top

Goal: ship the full P0 feature surface and P1 measurement substrate: diagnostics workers and delta attribution, Tier 1 framework extractors, full MCP deep tools, full operator CLI, PreCompact survival packet, generated/vendored file policy, metrics ledger, benchmark harness product surface, and inert broadcast placeholders (schemas, metrics slots, working-set storage, disabled config flag) that the active-broadcasts runtime in Phase 7 Wave 7.3 consumes. The active notification loop itself lands in Phase 7, not here.

Dependencies on prior phases: Phase 5 adapters and MCP green; GPT-Pro review fix-list resolved or accepted.

Acceptance criteria:

- Diagnostic deltas meet fixture precision/recall targets in quick corpus.
- Tier 1 frameworks produce high-confidence route maps with provenance.
- `cairn_find`, `cairn_explain`, `cairn_impact`, `cairn_diagnostics` work.
- Operator CLI full command set exists.
- Generated/vendored policy prevents hard denies from generated/vendored churn in baseline mode.
- Benchmark harness records complete session metrics and ablations.
- Full P0 feature surface can run in simulator and produce a VAT bundle.

#### Wave 6.0 - Capability-stack premortem, no code

Task: Phase 6 premortem.

- Owner: orchestrator direct.
- Skill stack: primary `premortem`; secondary `parallel-subagent-discipline`, `orchestrator`.
- Owned files: none.
- Dependencies within wave: none.
- Acceptance: written premortem covering diagnostic-delta attribution noise in messy repos (cascading errors, unrelated warnings, tool-version drift mid-session), framework-extractor overconfidence on dynamic registration patterns (FastAPI decorators evaluated at import, Django URL include trees, gin/chi runtime route attachment), MCP tool-selection collisions when `cairn_find` / `cairn_explain` / `cairn_impact` descriptions overlap, source-class hard-deny leaks through misclassified generated/vendored files (especially monorepo `pnpm` stores, Go vendored dependencies, Rust target directories), metrics-bundle holes from sessions that crash before final emission, full-novelty-scheduler latency budget violations on dense decoration scenarios, and the integration risk of Wave 6.5b's P0 scenario bundle depending on six sibling crates landing together cleanly.
- Reviewer pairing: `Agent plan-reviewer`.
- Review brief shape: attack whether Phase 6's capability surface can ship without overclaiming. Specifically check: does any framework extractor produce hard-deny-eligible facts without a compiler-backed seam (violating §3.7)? Does any diagnostic adapter emit deltas without confidence labels? Does the full operator CLI expose any internal noise that should stay in event-log replay?

#### Wave 6.1 - Diagnostics workers and deltas

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-diagnostics`: `DiagnosticWorker` trait, `DiagnosticProfile`, `DiagnosticSnapshot`, `DiagnosticDelta` (introduced/resolved/changed/full modes), `attribute_delta` trait signature, `DiagnosticCacheKey` (file_hash + dep_hash + config_hash + tool_version). In `cairn-mcp`: `cairn_diagnostics` tool envelope. All four diagnostic adapter workers (TS/Python and Go/Rust) implement the same trait surface; the delta attribution engine consumes snapshots without needing language-specific branching.

Task 1: Diagnostic worker registry.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-diagnostics/**`.
- Dependencies within wave: none.
- Acceptance: config detection, worker identity, diagnostic profile, cache key by file hash, dep hash, config hash, tool version; no duplicate workers per project config.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify one diagnostic worker set per project and clear degraded state.

Task 2: TypeScript/Python diagnostic adapters.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-diagnostics/src/adapters/ts.rs`, `crates/cairn-diagnostics/src/adapters/python.rs`, `fixtures/diagnostics/ts_py/**`.
- Dependencies within wave: none.
- Acceptance: before/after snapshots and watch/cached modes represented; fixtures with pre-existing diagnostics produce introduced/resolved/changed sets.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify no noisy full-dump default.

Task 3: Go/Rust diagnostic adapters.

- Owner: `delegate droid qwen work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-diagnostics/src/adapters/go.rs`, `crates/cairn-diagnostics/src/adapters/rust.rs`, `fixtures/diagnostics/go_rust/**`.
- Dependencies within wave: none.
- Acceptance: cargo/go test/check diagnostic snapshots normalized; changed diagnostic attribution tested.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify tool version/config included in cache key.

Task 4: Diagnostic delta attribution engine.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-diagnostics/src/delta.rs`, `crates/cairn-diagnostics/tests/delta.rs`.
- Dependencies within wave: none.
- Acceptance: introduced/resolved/changed/full modes; precision/recall fixture labels; uncertainty represented; unrelated warnings suppressed.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: test messy repo scenarios and cascading errors.

Task 5: `cairn_diagnostics` MCP tool.

- Owner: `delegate codex work --isolation worktree`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `mcp-builder`; secondary `tdd-workflow`.
- Owned files: `crates/cairn-mcp/src/tools/diagnostics.rs`, `tests/phase6_mcp_diagnostics.rs`.
- Dependencies within wave: none.
- Acceptance: returns diagnostic deltas by scope/since/mode with provenance and uncertainty.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`, `mcp-builder`.
- Review brief shape: verify tool descriptions drive correct agent selection.

#### Wave 6.2 - Framework extractors

**Wave-internal contracts (orchestrator commits before fan-out).** No new public surface beyond the `FrameworkExtractor` trait committed in Wave 4.1's `cairn-extract-core`. Pre-fan-out the orchestrator commits a shared framework-fixture schema (route precision/recall + provenance coverage + false-positive labels) so all four framework workers and the scoring task (Task 5) report against the same template. Confidence-tier policy from §3.7 is reaffirmed in the wave brief: framework facts may only justify hard-deny enforcement if a compiler-backed seam (tsserver, Pyright/mypy, gopls, rust-analyzer) confirms the route signature.

Task 1: Next.js route extractor.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-frameworks/src/nextjs.rs`, `fixtures/frameworks/nextjs/**`.
- Dependencies within wave: none.
- Acceptance: app router APIs, middleware, route segment params, HTTP method exports, runtime config; confidence/provenance on every route fact.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify high-confidence precision and no heuristic-only hard-deny path.

Task 2: Python web framework extractor.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-frameworks/src/python_web.rs`, `fixtures/frameworks/python_web/**`.
- Dependencies within wave: none.
- Acceptance: FastAPI, Django, Flask route decorators and method/path contracts; dynamic route registration marked unknown/advisory.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: challenge dynamic framework detection and confidence claims.

Task 3: Go web framework extractor.

- Owner: `delegate droid qwen work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-frameworks/src/go_web.rs`, `fixtures/frameworks/go_web/**`.
- Dependencies within wave: none.
- Acceptance: gin, chi, gorilla route registrations, middleware chain hints, handler symbol mapping.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify route false positives and provenance.

Task 4: Rust web framework extractor.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-frameworks/src/rust_web.rs`, `fixtures/frameworks/rust_web/**`.
- Dependencies within wave: none.
- Acceptance: axum, actix, rocket route/handler mappings and middleware hints where statically visible.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify lower confidence for macro-heavy cases.

Task 5: Framework fixture scoring.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-extract-frameworks/tests/**`, `fixtures/frameworks/expected/**`.
- Dependencies within wave: none.
- Acceptance: fixture scorer reports route precision/recall, false-positive rate, and provenance coverage by framework.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify fixture labels include high-confidence and heuristic cases.

#### Wave 6.3 - Full MCP deep tools

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-mcp`: tool envelopes for `cairn_find` (kind/scope/confidence filter, results with provenance), `cairn_explain` (callers, callees, tests, routes, depth, token budget, provenance), `cairn_impact` (impact radius, dependency changes, affected symbols, affected sessions, stale observations — note: affected sessions derives from ledger, not graph-only guesses). Streaming/summarizing fallback trait so `cairn_explain` has a single token-budget enforcement path.

Task 1: `cairn_find`.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `mcp-builder`; secondary `tdd-workflow`.
- Owned files: `crates/cairn-mcp/src/tools/find.rs`, `tests/phase6_mcp_find.rs`.
- Dependencies within wave: none.
- Acceptance: finds symbols, routes, files, tests, configs, commands by kind/scope/confidence; p95 query benchmark target recorded.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`, `mcp-builder`.
- Review brief shape: verify tool description is short, imperative, and selection-accurate.

Task 2: `cairn_explain`.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `mcp-builder`; secondary `tdd-workflow`.
- Owned files: `crates/cairn-mcp/src/tools/explain.rs`, `tests/phase6_mcp_explain.rs`.
- Dependencies within wave: none.
- Acceptance: returns bounded graph explanation with callers, callees, tests, routes, depth, token budget, provenance.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`, `mcp-builder`.
- Review brief shape: verify streaming/summarizing fallback and token budget compliance.

Task 3: `cairn_impact`.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `mcp-builder`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-mcp/src/tools/impact.rs`, `tests/phase6_mcp_impact.rs`.
- Dependencies within wave: none.
- Acceptance: returns impact radius, dependency changes, affected symbols, affected sessions, stale observations; before/after editing use cases covered.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`, `mcp-builder`.
- Review brief shape: verify affected sessions derive from ledgers, not graph-only guesses.

Task 4: MCP latency and correction benchmark.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-bench/src/mcp_latency.rs`, `tests/phase6_mcp_latency.rs`.
- Dependencies within wave: none.
- Acceptance: captures p95 for indexed lookup and expanded graph explanations; records follow-up correction proxy in fixtures.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify benchmark reflects real daemon round trip and graph query work.

#### Wave 6.4 - Operator CLI, source policy, PreCompact survival

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-cli`: full command surface enumerated as a typed enum so doctor and explanation polish tasks bind to stable command names. In `cairn-file`/`cairn-graph`: `SourceClass` policy enum (Generated, Vendored, BuildArtifact, Config, Lockfile, Migration, Fixture, Source) + classification rules trait. In `cairn-context`: `SurvivalPacket` schema (task summary, working set, observed versions, edited files, unresolved diagnostics/stale warnings, other-agent changes, key symbols/routes, frame IDs, suggested resume calls).

Task 1: Full operator CLI command set.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `plain-language`.
- Owned files: `crates/cairn-cli/**`, `tests/phase6_cli_full.rs`.
- Dependencies within wave: none.
- Acceptance: implements `status`, `daemon doctor`, `sessions list/show`, `ledger tail`, `observations show`, `context show`, `deny explain`, `graph explain`, `diagnostics delta`, `metrics report/tail`, `replay decision`.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify commands map to spec §9 and can answer “why did it say that?” quickly.

Task 2: Daemon doctor fault injection.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `debugging-systematic`; secondary `tdd-workflow`.
- Owned files: `crates/cairn-daemon/src/doctor.rs`, `tests/phase6_doctor_faults.rs`.
- Dependencies within wave: none.
- Acceptance: doctor catches corruption, watcher wedge placeholder, split-brain, stale lease, storage replay mismatch, adapter heartbeat absence.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify fault tests are deterministic and useful.

Task 3: Generated/vendored file policy.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-file/src/source_class.rs`, `crates/cairn-graph/src/source_policy.rs`, `tests/phase6_source_class.rs`.
- Dependencies within wave: none.
- Acceptance: generated, vendored, build artifact, config, lockfile, migration, fixture rules applied to indexing/stale checks; baseline hard denies from generated/vendored churn are impossible.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify source-class policy matches spec and improves cold start.

Task 4: Full PreCompact survival packet.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `plain-language`.
- Owned files: `crates/cairn-context/src/precompact_survival.rs`, `tests/phase6_precompact_survival.rs`.
- Dependencies within wave: none.
- Acceptance: packet includes task summary, working set, observed versions, edited files, unresolved diagnostics/stale warnings, other-agent changes, key symbols/routes, frame IDs, suggested resume calls.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify pointer-heavy packet and post-compact recovery path.

Task 5: Deny/context explanation polish.

- Owner: `delegate cursor work --isolation worktree`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `plain-language`; secondary `tdd-workflow`.
- Owned files: `crates/cairn-cli/src/deny.rs`, `crates/cairn-cli/src/context.rs`, `tests/phase6_explain_cli.rs`.
- Dependencies within wave: none.
- Acceptance: `deny explain` and `context show` include revalidation set, proof pointers, source versions, confidence, override policy, and frame facts.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify output is precise enough for operators and agents.

#### Wave 6.5a - Metrics, benchmark harness, full scheduler

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-metrics`: `MetricsLedger` trait, `VatScore`, `AblationSet`, `LatencySample`, full counter set enumerated (tokens, tool calls, hook latency, graph query latency/cache hits, diagnostics, stale denies/warnings, session overlap windows, final success, failure labels). In `cairn-bench`: `BenchmarkRun` schema, `Scenario` trait, `AgentHarness` trait, ablation flag enum. In `cairn-context`: scheduler novelty-scoring interface (utility receipt capture + duplicate-token-ratio measurement) — but the actual scoring algorithm body is Task 3's deliverable.

Task 1: Metrics ledger and VAT.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-metrics/**`.
- Dependencies within wave: none.
- Acceptance: records tokens, tool calls, hook latency, graph query latency/cache hits, diagnostics, stale denies/warnings, session overlap windows, final success, failure labels, VAT.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify metrics completeness and local-only behavior.

Task 2: Benchmark harness product surface.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-bench/**`, `tests/phase6_bench.rs`.
- Dependencies within wave: none.
- Acceptance: controlled task runs, feature ablations, result bundles, final repo-state checks, failure labels, 100-iteration unattended dry-run mode stub.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: check harness does not become a toy idol and supports held-out repos.

Task 3: Context scheduler full novelty scoring.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-context/src/scheduler/**`, `tests/phase6_context_scheduler.rs`.
- Dependencies within wave: none.
- Acceptance: dedup by fact identity/source version/novelty hash/session; budgets by surface; utility receipt capture; duplicate token ratio benchmark.
- Reviewer pairing: `delegate droid grok safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify no context spam and no latency-busting scoring.

#### Wave 6.5b - Broadcast placeholders and full P0 scenario bundle

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-protocol`: `BroadcastEvent` enum variants for working-set-overlap notice (without runtime emission). In `cairn-ledger`: `WorkingSet` materialized-view schema and `working_set_field` on session record. In `cairn-metrics`: broadcast metrics counter slots. In `cairn-config`: `broadcasts_enabled: bool` defaulting to false. The full P0 scenario bundle task consumes all five Phase 6 surfaces (diagnostics, frameworks, MCP, CLI, metrics) plus the inert broadcast placeholders.

Task 1: Inert broadcast scaffolding (placeholders only, no runtime loop).

- Owner: `delegate droid qwen work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-protocol/src/broadcasts.rs` (event-kind enums and message types only), `crates/cairn-ledger/src/working_set.rs` (working-set materialized view), `crates/cairn-metrics/src/broadcasts.rs` (metrics slots), `crates/cairn-config/src/broadcasts.rs` (disabled-by-default config flag), `tests/phase6_broadcast_placeholders.rs`.
- Dependencies within wave: none.
- Acceptance: event kinds, metrics slots, working-set data, and the disabled-by-default config flag exist and are referenced by the P0 scenario bundle. No active notification loop ships in Phase 6. The runtime path lands in Phase 7 Wave 7.3.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify the placeholders are inert — no broadcast emission code, no listener registration, no rate limiter logic. Just the schema, the storage slot, the metrics counter, and the config flag.

Task 2: Full P0 scenario bundle.

- Owner: orchestrator direct.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `finishing-a-development-branch`; secondary `checkpoint`, `debugging-systematic`.
- Owned files: `tests/phase6_full_p0.rs`, `docs/reports/phase6_p0_bundle.md`.
- Dependencies within wave: none.
- Acceptance: simulator runs full P0 scenario: orient, read decoration, graph stale deny, revalidation, edit, diagnostic delta, MCP prove, CLI deny explain, metrics bundle.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify the end-to-end path proves the product claim rather than isolated crate success.

Phase checkpoints:

- Bug hunt: yes. Focus: diagnostic deltas, framework overconfidence, MCP tool selection, source-class hard-deny leaks, metrics holes.
- Deslopify pass: yes. Owner orchestrator invokes `desloppify-deep`.
- Strategic re-review by GPT-Pro: no by default, unless Phase 6 P0 metrics miss acceptance floor.
- Event-log golden fixture: extend the Phase 5 bundle with diagnostic worker registration, before/after diagnostic snapshots, framework-route extraction events, source-class transitions (generated/vendored/build artifact), full PreCompact survival packet contents, metrics ledger entries, and the inert broadcast placeholders shipped in Wave 6.5b. Replay test verifies the full P0 scenario bundle reconstructs end-to-end.
- Phase gate: full workspace gate; diagnostics fixture precision/recall; framework fixture scores; full CLI command tests; P0 scenario bundle; context duplicate-token quick benchmark; event-log golden fixture replay.

Advance refusal:

- Diagnostic introduced recall below fixture floor.
- Generated/vendored churn can hard-deny baseline edits.
- Any CLI command absent from full P0 set.
- Metrics bundle incomplete for benchmark tasks.
- Full P0 scenario cannot produce proof and denial explanation.

### Phase 7 - Extension

Goal: expand beyond P0 substrate into P0-β language navigation, P1 read-only web UI, P2 worktree federation, public dependency graph cache, symbol-level stale checks and leases, and cross-language bridge extractors. This phase must not destabilize P0; extensions are gated behind capability flags and benchmark floors.

Dependencies on prior phases: Phase 6 full P0 feature surface green; deslopify fix-list resolved.

Acceptance criteria:

- P0 features stay green with extensions off.
- P0-β extractors provide navigation-grade symbol/route facts with confidence/provenance.
- Web UI is read-only and can explain denies/session timelines.
- Worktree federation is advisory only.
- Symbol-level checks reduce false-positive denies in fixtures without reducing stale recall beyond floor.
- Bridge extractors produce labeled fixture scores.
- Public graph cache never includes private project content without explicit opt-in.

#### Wave 7.1 - P0-β language tier

**Wave-internal contracts (orchestrator commits before fan-out).** No new public surface required — all P0-β extractors implement against the `Extractor` and `GrammarAdapter` traits from Wave 4.1's `cairn-extract-core`. Pre-fan-out the orchestrator commits the P0-β fixture schema (`fixtures/p0beta/<lang>/expected.toml`) and the shared scorer trait. P0-β facts are constrained to navigation-grade confidence per §3.7 — no hard-deny eligibility without compiler-backed seams, which are not required for this wave.

Task 1: Java/Kotlin navigation extractors.

- Owner: `delegate droid qwen work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-p0beta/src/java_kotlin.rs`, `fixtures/p0beta/java_kotlin/**`.
- Dependencies within wave: none.
- Acceptance: public/protected class/interface/protocol members, annotations, package/module structure, route annotations where visible.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify navigation-grade confidence and no hard semantic denies.

Task 2: C# navigation extractor.

- Owner: `delegate droid glm work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-p0beta/src/csharp.rs`, `fixtures/p0beta/csharp/**`.
- Dependencies within wave: none.
- Acceptance: classes/interfaces, public/protected members, attributes, ASP.NET route annotations where visible.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify attributes carry provenance and low-confidence reflection cases stay advisory/navigation.

Task 3: PHP/Ruby navigation extractors.

- Owner: `delegate droid gemini work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-p0beta/src/php_ruby.rs`, `fixtures/p0beta/php_ruby/**`.
- Dependencies within wave: none.
- Acceptance: classes/modules/functions/routes for Laravel/Rails fixtures where statically visible; dynamic metaprogramming confidence demoted.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: attack overconfident framework detection.

Task 4: C/C++/Objective-C/Swift/Dart outlines.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-p0beta/src/native_outlines.rs`, `fixtures/p0beta/native/**`.
- Dependencies within wave: none.
- Acceptance: navigation-grade outlines, imports/includes, public-ish symbols, no hard deny semantics.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify scope is honest and bridge extractors remain separate.

Task 5: P0-β fixture scorer.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-extract-p0beta/tests/**`, `fixtures/p0beta/expected/**`.
- Dependencies within wave: none.
- Acceptance: fixture score report by language, confidence tier, and navigation object kind.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify extension scores cannot regress P0-α floors.

#### Wave 7.2 - Read-only web UI

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-web`: read-only HTTP API schema (session lanes, events, context frames, denies, diagnostics, graph invalidations, VCS epoch changes — all GET endpoints, no writes). Static asset entry-point convention. The UI implementation, daemon integration, and usability benchmark all consume the API schema; no separate state lives in `cairn-web`.

Task 1: Local read-only HTTP API.

- Owner: `delegate droid gemini work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `vanilla-web-dev`.
- Owned files: `crates/cairn-web/src/api/**`, `tests/phase7_web_api.rs`.
- Dependencies within wave: none.
- Acceptance: API exposes session lanes, events, context frames, denies, diagnostics, graph invalidations, VCS epoch changes as read-only views.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify no write endpoints and no separate state.

Task 2: Timeline UI.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`; primary `vanilla-web-dev`; secondary `accessibility-checklist`, `webapp-testing`.
- Owned files: `crates/cairn-web/static/**`, `crates/cairn-web/src/ui/**`.
- Dependencies within wave: none.
- Acceptance: session lanes, reads, edits, denies, diagnostics, context frames, graph invalidations visible; keyboard navigation and basic accessibility checks pass.
- Reviewer pairing: `delegate droid gemini safe` with `clean-code`, `accessibility-checklist`, `webapp-testing`.
- Review brief shape: verify it is an operator flight recorder, not an IDE.

Task 3: Web UI daemon integration.

- Owner: `delegate droid "deepseek v4 flash" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `webapp-testing`.
- Owned files: `crates/cairn-daemon/src/web.rs`, `crates/cairn-app/**`, `tests/phase7_web_integration.rs`.
- Dependencies within wave: none.
- Acceptance: `cairn web` starts local read-only server bound to localhost; status command prints URL; no network exposure by default.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify local-only posture and no state duplication.

Task 4: Web UI usability benchmark.

- Owner: `delegate cursor work --isolation worktree`.
- Skill stack: always-on `clean-code`; primary `webapp-testing`; secondary `accessibility-checklist`.
- Owned files: `crates/cairn-bench/src/web_ui.rs`, `tests/phase7_web_ui.rs`.
- Dependencies within wave: none.
- Acceptance: scripted operator can locate arbitrary deny cause in under target path length; screenshots/golden DOM states recorded.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `clean-code`, `webapp-testing`.
- Review brief shape: verify benchmark measures cause-finding, not click-count theater.

#### Wave 7.3 - Coordination extensions

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-identity`: `RepoIdentity` (canonical repo root above worktree level) for sibling-worktree detection. In `cairn-graph`: `PublicCacheKey` (package version + content hash) and privacy boundary trait. In `cairn-ledger`: `SymbolStaleness` query trait and `Lease` trait (file/symbol lease with timeout recovery, advisory fallback). The active-broadcasts runtime task (Task 5) consumes the broadcast event types and working-set storage from Phase 6 Wave 6.5b; no further protocol additions needed.

Task 1: Worktree federation advisory.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-daemon/src/worktree_federation.rs`, `crates/cairn-identity/src/repo_identity.rs`, `tests/phase7_worktree_federation.rs`.
- Dependencies within wave: none.
- Acceptance: detects sibling worktrees sharing common repo; advisory conflict/semantic divergence only; no cross-worktree deny.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify advisory-only default and repo-level identity.

Task 2: Public dependency graph cache, opt-in.

- Owner: `delegate codex work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-graph/src/public_cache.rs`, `crates/cairn-config/src/cache.rs`, `tests/phase7_public_cache.rs`.
- Dependencies within wave: none.
- Acceptance: cache keyed by package version/content hash; opt-in required; tests prove no project-private content stored.
- Reviewer pairing: `delegate droid glm safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify privacy boundary and cache precision.

Task 3: Symbol-level stale checks.

- Owner: `delegate droid grok work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-ledger/src/symbol_staleness.rs`, `crates/cairn-graph/src/symbol_impact.rs`, `tests/phase7_symbol_stale.rs`.
- Dependencies within wave: none.
- Acceptance: narrows file-level stale checks to imported symbols/routes where high confidence; fixture false-positive denies reduce by 40 percent while recall remains within 2 percentage points.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify no recall cliff and fallback to file-level when confidence low.

Task 4: Short-lived leases.

- Owner: `delegate droid qwen work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-ledger/src/leases.rs`, `crates/cairn-daemon/src/leases.rs`, `tests/phase7_leases.rs`.
- Dependencies within wave: none.
- Acceptance: optional file/symbol leases with timeout recovery, non-blocking advisory fallback, strict-mode behavior; idle-time metrics captured.
- Reviewer pairing: `delegate droid grok safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify no deadlocks and no ordinary tool-call brick.

Task 5: Active broadcasts runtime.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `debugging-systematic`.
- Owned files: `crates/cairn-daemon/src/broadcasts.rs`, `crates/cairn-ledger/src/working_set_listener.rs`, `tests/phase7_broadcasts.rs`.
- Dependencies within wave: none. Consumes the inert broadcast scaffolding shipped in Phase 6 Wave 6.5b Task 1.
- Acceptance: when the config flag is enabled, changes to exported surfaces that overlap another session's working set emit rate-limited notices via the broadcast event kind. Default off. P0 acceptance does not depend on this loop. Rate limiter behavior tested under bursty fan-out.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify P0 stays green with the flag off, rate limiter prevents notification storms, and the runtime never blocks ordinary tool calls.

#### Wave 7.4 - Bridge extractors and final integration

**Wave-internal contracts (orchestrator commits before fan-out).** In `cairn-extract-bridges`: `BridgeExtractor` trait specializations and `BridgeEdge` provenance struct. Per §3.7 confidence policy: bridge facts default to medium confidence and cannot alone drive hard deny. Pre-fan-out the orchestrator commits the bridge-fixture schema. The extension benchmark bundle and final integration tasks consume the existing benchmark and metrics surfaces — no new shared types needed.

Task 1: React Native and Expo bridge extractor.

- Owner: `delegate droid qwen work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-bridges/src/react_native.rs`, `fixtures/bridges/react_native/**`.
- Dependencies within wave: none.
- Acceptance: React Native bridge modules, TurboModules, Fabric, Expo Modules edges with provenance and confidence.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify bridge edge precision and private-content boundaries.

Task 2: Swift/Objective-C bridge extractor.

- Owner: `delegate droid "deepseek v4 pro" work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-bridges/src/swift_objc.rs`, `fixtures/bridges/swift_objc/**`.
- Dependencies within wave: none.
- Acceptance: Swift/Obj-C bridge declarations, selectors, exposed modules with provenance; low confidence for macro/dynamic cases.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: attack false-positive bridge edges.

Task 3: JNI and FFI bridge extractor.

- Owner: `delegate droid gemini work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `spec-quality-checklist`.
- Owned files: `crates/cairn-extract-bridges/src/jni_ffi.rs`, `fixtures/bridges/jni_ffi/**`.
- Dependencies within wave: none.
- Acceptance: JNI bindings, FFI declarations, native symbol edges with provenance and fixture scores.
- Reviewer pairing: `delegate codex safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify no speculative bridge fact drives hard deny.

Task 4: Extension benchmark bundle.

- Owner: `delegate cursor work`.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `tdd-workflow`; secondary `delegate-agent`.
- Owned files: `crates/cairn-bench/src/extension/**`, `tests/phase7_extension_bundle.rs`.
- Dependencies within wave: none.
- Acceptance: benchmark bundle covers P0-β, web UI, worktree federation, cache, symbol checks, leases, bridges with extension flags on/off.
- Reviewer pairing: `delegate droid "deepseek v4 pro" safe` with `rust-engineer`, `clean-code`.
- Review brief shape: verify P0 baseline stays green when extensions are off.

Task 5: Final branch integration and release-readiness report.

- Owner: orchestrator direct.
- Skill stack: always-on `clean-code`, `rust-engineer`; primary `finishing-a-development-branch`; secondary `checkpoint`, `create-handoff`.
- Owned files: `docs/reports/phase7_release_review.md`, root release checklist.
- Dependencies within wave: none.
- Acceptance: final gate green; extension metrics summarized; P0/P1/P2 capability matrix updated; handoff includes risks, deferred issues, and benchmark deltas.
- Reviewer pairing: `delegate codex safe` plus `Agent plan-reviewer`.
- Review brief shape: verify no unreviewed extension drift and no P0 regression.

Phase checkpoints:

- Bug hunt: yes. Focus: extension flags, P0 regression, worktree advisory-only behavior, private-content cache boundary, bridge false positives, lease deadlocks.
- Deslopify pass: yes. Owner orchestrator invokes `desloppify-deep`.
- Strategic re-review by GPT-Pro: yes (pre-P1/P2 continuation review per §7). Deliverables: release-readiness report, P0 regression matrix, extension benchmark bundle, web UI screenshots, capability matrix, unresolved risks, next P1/P2 backlog.
- Event-log golden fixture: final bundle covering P0-β extractor events, web UI read-only API calls, worktree-federation advisory notices, public-cache lookups, symbol-level stale checks, lease acquisition/timeout events, bridge-extractor facts, and the active-broadcasts runtime emissions from Wave 7.3 Task 5. Replay test verifies the full bundle reconstructs deterministically and the P0 regression suite passes against it with extensions off.
- Phase gate: full workspace gate; P0 regression suite with extensions off; extension suites with flags on; web UI tests; bridge fixtures; symbol-level stale benchmark; event-log golden fixture replay.

Advance refusal:

- Any P0 regression when extensions are off.
- Any cross-worktree hard deny.
- Any public cache storing private project content.
- Any bridge/speculative fact driving a hard deny.
- Any lease deadlock or unrecoverable timeout.

## 5. First sprint kickoff

The orchestrator's first message to workers should not ask for re-planning. The kickoff sequence is:

1. **Wave 1.0** — orchestrator runs the Phase 1 foundation-risk premortem itself (no worker fan-out), reads `parallel-subagent-discipline`, sets concurrency cap to 6.
2. **Wave 1.1 bootstrap prelude** — orchestrator commits the workspace skeleton in a single pre-fan-out commit: root `Cargo.toml` with workspace members, `rust-toolchain.toml`, `.cargo/config.toml`, empty crate directories for `cairn-types`, `cairn-config`, `cairn-identity`, `cairn-file`, `cairn-vcs`, `cairn-app`, with compile-only `lib.rs`/`main.rs` skeletons so `cargo check --workspace` passes against the empty tree. Workers then operate on their owned crate without colliding on workspace-file edits.
3. **Wave 1.1 dispatch** — orchestrator dispatches six workers in one batch.

Literal first wave dispatch:

1. `delegate codex work`
   Brief: flesh out `cairn-app` binary scaffolding, gate documentation, and project `CLAUDE.md`. The empty workspace already builds from the bootstrap commit. Load `clean-code`, `rust-engineer`, `tdd-workflow`, `bootstrap`, `write-human`. Own `crates/cairn-app/**`, `docs/dev/gates.md`, `CLAUDE.md` (and minor edits to root `Cargo.toml` if app needs new dependencies). Acceptance: `cairn-app` builds and produces the `cairn` binary stub; `cargo fmt --check`; `cargo check --workspace`; `CLAUDE.md` includes CPU discipline (workers do not run heavy gates), Git discipline (workers do not commit, no `git add -A`), and the phase-gate command list.
2. `delegate droid grok work`
   Brief: implement `cairn-types`. Load `clean-code`, `rust-engineer`, `tdd-workflow`, `spec-quality-checklist`. Own `crates/cairn-types/**`. Acceptance: typed IDs, schemas, enums, serde round trips, no stringly public IDs.
3. `delegate droid "deepseek v4 flash" work`
   Brief: implement `cairn-config`. Load `clean-code`, `rust-engineer`, `tdd-workflow`, `spec-quality-checklist`. Own `crates/cairn-config/**`, `fixtures/config/**`. Acceptance: deterministic config hash, protocol version, modes, ablation flags.
4. `delegate codex work`
   Brief: implement `cairn-identity`. Load `clean-code`, `rust-engineer`, `tdd-workflow`, `using-git-worktrees`. Own `crates/cairn-identity/**`, `fixtures/identity/**`. Acceptance: canonical root/git common dir/worktree/config/protocol identity with symlink and case-sensitivity tests.
5. `delegate droid "deepseek v4 pro" work`
   Brief: implement `cairn-file`. Load `clean-code`, `rust-engineer`, `tdd-workflow`, `spec-quality-checklist`. Own `crates/cairn-file/**`, `fixtures/files/**`. Acceptance: content-addressed FileVersion, source-class base, large-file streaming hash tests.
6. `delegate cursor work`
   Brief: implement `cairn-vcs`. Load `clean-code`, `rust-engineer`, `tdd-workflow`, `using-git-worktrees`. Own `crates/cairn-vcs/**`, `fixtures/vcs/**`. Acceptance: RepoEpoch and operation_state detection for normal/detached/merge/rebase/cherry-pick/bisect/unknown.

After collection, orchestrator inspects diffs, resolves crate-boundary drift, runs light gate, then dispatches reviewers:

- Codex-implemented tasks reviewed by `delegate droid "deepseek v4 pro" safe` or `delegate droid glm safe`.
- Grok task reviewed by `delegate codex safe`.
- DeepSeek task reviewed by `delegate codex safe`.
- Cursor task reviewed by `delegate droid "deepseek v4 pro" safe`.

## 6. Risks and mitigations specific to this execution model

Worker collision risk: every wave assigns one owner per crate. Cross-crate API movement goes through the orchestrator as boundary adjudicator. High-overlap lanes use `using-git-worktrees`; workers do not stash or commit. Mechanical repo-wide changes go to `delegate cursor work` only when no concurrent owner touches those files.

Reviewer-implementer collusion: reviewers must be different models. Critical schema and safety work gets model-diverse review: Grok or Codex implementation reviewed by DeepSeek/GLM, DeepSeek implementation reviewed by Codex, Cursor implementation reviewed by Codex or DeepSeek. Disagreements get `diagnose` plus a third model, then spec-based adjudication.

Reviewer rotation policy. Per-task reviewer pairings in §4 are starting suggestions, not fixed wiring. The orchestrator enforces the following caps at dispatch time:

- Target share: no reviewer model handles more than 25% of total review assignments across the build.
- Aggregate hard cap (after Phase 2): no reviewer model exceeds 30% of cumulative review assignments. Phase 1 and Phase 2 are exempt because absolute counts are small and forcing rotation through tiny denominators contorts pairings.
- Per-phase emergency cap: no reviewer model exceeds 40% of a single phase's review assignments. This prevents a single phase from being graded entirely through one lens even when the global cap still has headroom.
- No-two-consecutive rule: within a single wave, the same reviewer model may not be assigned to two tasks back-to-back. Local streaks distort review judgment more than global percentages do.
- Critical-schema diversity: when a task's owned files include protocol envelopes, ledger schemas, daemon decisions, capability bitsets, fingerprint contracts, or extractor-fact provenance, the reviewer must come from a different model family than the implementer, not just a different lane of the same family.

When a starting pairing in §4 would violate any of these constraints at dispatch time, the orchestrator swaps to the next-best reviewer per the model-strengths table and records the swap in the wave dispatch log.

Phase-gate latency: target wave size is 5 tasks, hard cap 7. Workers run crate-local checks only. Orchestrator runs heavy gates once. If full gate exceeds 30 minutes twice, split CI into substrate, graph/extractors, adapters/MCP, diagnostics/bench groups and run controlled batches. Do not let N agents each launch full test suites.

Spec drift: no phase advances if implementation requires changing schema semantics, adapter tiers, deny policy, graph confidence rules, or benchmark floors. Those become GPT-Pro re-planning inputs.

Agents may ignore pushed context: keep frames small, action-oriented, and near the triggering tool call. Measure pointer-following, duplicate-token ratio, and tool-call reduction in Phase 6 metrics.

False-positive stale denies: hard denies require verified/resolved contract changes. Implementation-only changes are advisory by default. Revalidation set must be minimal. Deny loop guard is Phase 2. Symbol-level narrowing is Phase 7.

Dynamic framework detection: all framework facts carry confidence and provenance. Weak heuristics never drive hard denies. Framework extractors have gold fixtures and confidence buckets.

Diagnostic delta attribution: use before/after snapshots, uncertainty labels, raw diagnostics available through MCP/CLI, and suppression tests in messy repos.

Windows and huge repo latency: macOS/Linux first; Windows path/socket/watch tests become Phase 7 gates. All hook paths time out and fail open unless strict mode plus confirmed stale edit.

Harness capabilities vary: capability bitset is product surface. Adapters normalize and downgrade honestly; metrics distinguish denied, warned, post-hoc detected, and missed stale edits.

Benchmark idol risk: benchmark harness includes held-out repos, adversarial scenarios, multi-agent variation, ablations, and floor conditions. VAT cannot override safety floors.

Salsa lock-in: Salsa is wrapped behind `cairn-incremental`; graph/extractor crates do not import Salsa directly.

TOCTOU precondition support: adapters that can attach file preconditions must do so. Others get best-effort post-edit verification and capability matrix downgrade.

## 7. Checkpoints for GPT-Pro re-engagement

End of Phase 4 — narrow precondition consult. Send Phase 4 summary, crate dependency graph, graph model docs, extractor fixture score report, contract/implementation fingerprint fixture matrix, false-positive stale benchmark, latency POC report, sample `DenyDecision`, sample `cairn_prove` graph trace, and unresolved reviewer disagreements. Purpose: review graph model, provenance, confidence rules, contract/implementation fingerprints, and hard-deny eligibility before adapters and MCP bind to those contracts. This is where the belief-management framing can be accidentally erased; Pro adjudicates whether the graph stays an input to the ledger rather than becoming the primary API surface.

End of Phase 5 — delta/integration consult. Send capability matrix, adapter traces for Claude/Codex/Cursor, MCP outputs for orient/observed_state/prove, two-adapter multi-agent scenario bundle, hook latency report, spool replay tests, and open adapter capability risks. Purpose: verify hooks and MCP preserved the Phase 4 architecture rather than re-debating it. Pro reviews whether adapters bent any graph contract, whether the capability matrix is honest, and whether MCP tools remained thin clients of daemon state.

End of Phase 7 — pre-P1/P2 continuation review. Send final release-readiness report, P0 regression matrix, extension benchmark bundle, web UI screenshots or walkthrough, final capability matrix, source-cache privacy proof, bridge fixture scores, and P1/P2 backlog.

Any time a phase's acceptance criteria require a spec revision: send the failing acceptance criterion, relevant spec section, reproduction, reviewer disagreement if any, proposed implementation alternatives, and measured impact on P0 floors.

Pro responses are triaged on receipt. Fix-list-shaped feedback is applied in-place by the orchestrator without pausing forward progress. Structural feedback (schema, build-order, capability-tier, advance-refusal-gate changes) pauses new dispatch and integration of work that touches the structural surface; independent workers already running on unaffected crates may finish and report. Nothing touching the structural surface lands until the patch-and-resend round closes.

## Appendix A: Skill activation matrix

| Skill                            | Phase 1                                                | Phase 2                              | Phase 3                              | Phase 4                               | Phase 5                              | Phase 6                              | Phase 7                              |
| -------------------------------- | ------------------------------------------------------ | ------------------------------------ | ------------------------------------ | ------------------------------------- | ------------------------------------ | ------------------------------------ | ------------------------------------ |
| `clean-code`                     | every implementation/review-fix task                   | every implementation/review-fix task | every implementation/review-fix task | every implementation/review-fix task  | every implementation/review-fix task | every implementation/review-fix task | every implementation/review-fix task |
| `rust-engineer`                  | every Rust task                                        | every Rust task                      | every Rust task                      | every Rust task                       | every Rust task                      | every Rust task                      | every Rust task                      |
| `tdd-workflow`                   | all new crates and tests                               | all ledger/freshness work            | all context/precompact work          | graph/extractor/benchmark work        | adapter/MCP work                     | diagnostics/framework/MCP/CLI work   | extensions/UI/backend work           |
| `delegate-agent`                 | dispatch setup                                         | simulator                            | budget/scenario tasks                | fixture/bench tasks                   | adapter fan-out                      | benchmark product surface            | extension benchmark                  |
| `parallel-subagent-discipline`   | before every fan-out                                   | before every fan-out                 | before every fan-out                 | before every fan-out                  | before every fan-out                 | before every fan-out                 | before every fan-out                 |
| `using-git-worktrees`            | identity/VCS and isolated test lanes                   | V1 port and adapter-core             | V1 context port                      | fixture lanes                         | adapter lanes                        | CLI/diagnostic isolated lanes        | extension lanes                      |
| `codex-prompting`                | Codex task briefs                                      | Codex task briefs                    | Codex task briefs                    | Codex task briefs                     | adapter and Codex-heavy work         | MCP/metrics Codex tasks              | release review prompts               |
| `premortem`                      | Phase 1 Wave 1.0 (shrunk to foundation risks)          | no                                   | no                                   | Phase 4 Wave 4.0                      | Phase 5 Wave 5.0                     | Phase 6 Wave 6.0                     | no                                   |
| `debugging-systematic`           | phase-gate stress tests only (no separate bug hunt)    | phase bug hunt and freshness work    | phase bug hunt                       | phase bug hunt and latency            | phase bug hunt                       | phase bug hunt and diagnostics       | phase bug hunt                       |
| `diagnose`                       | only if phase-gate stress tests go red                 | phase bug hunt                       | phase bug hunt                       | latency report and bug hunt           | phase bug hunt                       | phase bug hunt                       | phase bug hunt                       |
| `desloppify-deep`                | no                                                     | no (use targeted `code-simplifier`)  | no                                   | end of phase                          | no                                   | end of phase                         | end of phase                         |
| `code-simplifier`                | no                                                     | end of phase on ledger/storage/daemon/protocol/adapter-core | no                                   | targeted fixes after deslopify        | on touched adapter code after review  | targeted fixes after deslopify        | final cleanup                        |
| `spec-quality-checklist`         | schemas/config/protocol                                | ledger/deny/inheritance              | context/precompact schema            | graph/extractor/fingerprint contracts | adapter capability/MCP schema        | framework/source policy/MCP          | extension extractors/cache           |
| `finishing-a-development-branch` | phase integration                                      | phase gate                           | phase gate                           | phase gate                            | phase gate                           | full P0 bundle                       | final release report                 |
| `checkpoint`                     | phase summary                                          | phase summary                        | phase summary                        | phase summary plus GPT-Pro packet     | phase summary plus GPT-Pro packet    | phase summary                        | final handoff                        |
| `bootstrap`                      | generate `CLAUDE.md`                                   | no                                   | no                                   | no                                    | no                                   | no                                   | no                                   |
| `mcp-builder`                    | no                                                     | no                                   | no                                   | no                                    | MCP skeleton and 3 tools             | full MCP surface                     | no unless web uses MCP probes        |
| `request-refactor-plan`          | only if review finds crate-boundary refactor too large | same                                 | same                                 | same                                  | same                                 | same                                 | same                                 |
| `refactor`                       | targeted fixes after review                            | targeted fixes after code-simplifier | targeted fixes                       | targeted fixes after deslopify        | adapter simplification               | targeted fixes after deslopify       | final cleanup                        |
| `improve-codebase-architecture`  | no                                                     | after Phase 2 code-simplifier if needed | no                                | after Phase 4 deslopify if needed     | no                                   | after Phase 6 deslopify if needed    | final architecture review            |
| `requesting-code-review`         | every wave collection                                  | every wave collection                | every wave collection                | every wave collection                 | every wave collection                | every wave collection                | every wave collection                |
| `receiving-code-review`          | every fix-list                                         | every fix-list                       | every fix-list                       | every fix-list                        | every fix-list                       | every fix-list                       | every fix-list                       |
| `create-handoff`                 | if session ends                                        | if session ends                      | if session ends                      | GPT-Pro packet if session ends        | GPT-Pro packet if session ends       | if session ends                      | final handoff                        |
| `resume-handoff`                 | if resuming                                            | if resuming                          | if resuming                          | if resuming                           | if resuming                          | if resuming                          | if resuming                          |
| `plain-language`                 | CLI/status docs                                        | CLI/deny text                        | context renderers                    | read decorations                      | capability docs                      | operator CLI                         | web UI text                          |
| `write-human`                    | `CLAUDE.md` and V1 port docs                           | docs only                            | docs only                            | POC report                            | adapter report                       | P0 bundle report                     | release report                       |
| `vanilla-web-dev`                | no                                                     | no                                   | no                                   | no                                    | no                                   | no                                   | web UI                               |
| `webapp-testing`                 | no                                                     | no                                   | no                                   | no                                    | no                                   | no                                   | web UI                               |
| `accessibility-checklist`        | no                                                     | no                                   | no                                   | no                                    | no                                   | no                                   | web UI                               |
| `codex`                          | Codex execution lane                                   | Codex execution lane                 | Codex execution lane                 | Codex execution lane                  | Codex adapter/MCP lane               | Codex MCP/metrics lane               | Codex review lane                    |
| `gemini`                         | optional review diversity                              | optional review diversity            | optional review diversity            | optional review diversity             | optional review diversity            | optional review diversity            | web/extension diversity              |