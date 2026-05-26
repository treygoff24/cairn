# Code Briefcase V2 Implementation Plan — Patch Round 1

## Purpose

This is a delta against the implementation plan you produced. It contains twelve patches grouped by stakes: five high-priority items we want resolved before Wave 1.1 dispatches, and seven lower-stakes refinements we believe make execution smoother. For each patch we name the location, the rationale, and either the proposed text or the shape of the change we want.

We are not asking for a full revision. The plan's structure, build order, parallelism strategy, advance-refusal gates, skill matrix, and first-sprint kickoff are accepted as-is and ready to execute. We want a targeted patch round, after which we dispatch.

Three response modes are acceptable:

1. **Issue a revised full plan.** Cleanest for our archive. Best if patches interact in ways we missed.
2. **Issue a delta patch file** that we apply against the plan ourselves.
3. **Accept/reject each patch inline** with reasoning, and we apply.

Where you disagree with a patch, push back with substance. We will accept a well-argued "no" on any of these. Items where we are confident enough that we will apply unilaterally if you decline are flagged. Items where your judgment outranks ours are also flagged.

---

## High-priority patches (apply before kickoff)

### Patch 1 — Wave-internal contracts preamble

**Where:** every wave in §4 (Phase 1 through Phase 7).

**Problem:** every task in every wave declares `Dependencies within wave: none`. That is structurally untrue in places. Examples:

- Wave 1.2: the daemon (Task 4) cannot persist events without the storage crate (Task 1) compiling.
- Wave 4.1: `briefcase-graph` (Task 2) cannot compile without Salsa query handles from `briefcase-incremental` (Task 1).
- Wave 6.1: diagnostic adapters (Tasks 2–3) cannot compile without the worker registry types (Task 1).

In practice these waves work because tasks consume *types/traits* not *implementations*, and the types crate publishes contracts before fan-out. But the plan never says that. Workers will collide on missing trait stubs, or one worker will block waiting on another's "real" implementation.

**Fix:** add a `**Wave-internal contracts:**` bullet at the top of each wave, naming what types, traits, and crate-local module stubs are committed by the orchestrator before fan-out so all wave tasks can compile against shared interfaces. Phase 4 Wave 4.1 deserves the loudest treatment — it is the highest-risk dependency in any wave in the plan.

**Confidence:** high. We will apply unilaterally if you do not.

---

### Patch 2 — Reviewer rotation policy, not per-task pairing

**Where:** §6 and reviewer pairings throughout §4.

**Problem:** `delegate codex safe` is the reviewer pairing on roughly 60% of tasks. CONTEXT for this project explicitly worries about review collusion; the current pairings concentrate review judgment in a single model and create a rate-limit bottleneck if Codex stalls. Specific anti-patterns visible in the plan: "DeepSeek-Pro work → Codex safe" appears 8+ times consecutively across waves.

**Fix:** add a reviewer rotation policy to §6, then sweep §4 pairings to comply. Proposed policy:

- No reviewer model receives more than 30% of total review assignments across the build.
- Every implementer model rotates through at least three distinct reviewer models across the phases.
- The reviewer for any given crate's first implementation and its later refinements must not be the same model.
- Reviewer for a *critical-schema* task (protocol, ledger, daemon decision, capability bitset, fingerprint contracts) is mandatory model-diverse: implementer and reviewer must be different model families, not different lanes of the same family.

**Confidence:** high. The "no more than 30%" cap is a starting proposal; if you have a better cap defend it.

---

### Patch 3 — Codex CLI adapter implementer should not be Codex

**Where:** Phase 5 Wave 5.2 Task 1.

**Problem:** the Codex adapter is implemented by `delegate codex work` and reviewed by `delegate cursor safe`. An adapter's job is to faithfully bridge an external harness's actual capability surface to our daemon protocol. When the implementer *is* that external harness, it has built-in semantic priors about what the harness "obviously" supports. The implementer should have to read Codex docs and CLI behavior with fresh eyes, not internalize.

**Fix:** swap implementer to `delegate droid "deepseek v4 pro" work`. Keep `delegate cursor safe` as reviewer. The reviewer rotation policy from Patch 2 may further adjust.

**Confidence:** high. We will apply unilaterally if you do not.

---

### Patch 4 — Split Wave 6.5 and move active broadcasts to Phase 7

**Where:** Phase 6 Wave 6.5, Phase 7 (new task slot).

**Problem:**

- Wave 6.5 packs five high-stakes tasks: metrics ledger, benchmark harness product surface, full novelty-scoring context scheduler, active broadcasts, and the full P0 scenario bundle. Each is multi-day work. Five workers in parallel on this scale produces an integration burden that breaks the orchestrator's coordination ceiling.
- Active broadcasts is itself described in the plan as conditional on Phase 5 stability and shippable with default-off. That is Phase 7 behavior dressed as Phase 6.

**Fix:**

- Split Wave 6.5 into Wave 6.5a (metrics ledger + benchmark harness product surface + full novelty-scoring scheduler) and Wave 6.5b (full P0 scenario bundle).
- Move active broadcasts to Phase 7 Wave 7.3 alongside the other coordination extensions (worktree federation advisory, symbol-level checks, short-lived leases). Same advance-refusal gating semantics.

**Confidence:** high. The split is mechanical. Broadcasts→Phase 7 is the substantive call; defend if you disagree.

---

### Patch 5 — Phase 2 desloppify is too early; replace with code-simplifier

**Where:** Phase 2 checkpoints in §4.

**Problem:** `desloppify-deep` is an 8-subagent parallel cleanup pass whose value is finding cross-crate duplication, drifted type definitions, dead branches, and cycles. At end of Phase 2 the cross-crate surface is small: identity, file, vcs, storage, protocol, ledger, daemon-client, daemon, cli, harness-sim. Most of these crates have not yet been touched by the surfaces that would create duplication (context, graph, adapters, MCP, diagnostics). An 8-subagent fan-out at this point is mostly going to find boilerplate.

**Fix:** replace Phase 2's `desloppify-deep` checkpoint with a targeted `code-simplifier` pass on the crates touched in Phase 2 (ledger, storage, daemon). Keep `desloppify-deep` at Phases 4, 6, and 7 as planned.

**Confidence:** medium-high. This saves one full deslopify run's worth of orchestrator time and CPU. If you believe Phase 2 has enough surface to justify the full pass, defend.

---

## Lower-stakes patches

### Patch 6 — Phase 2 microbench gate for direct freshness

**Where:** Phase 2 phase gate in §4.

**Problem:** Phase 2 ships the PreEdit direct-freshness check, which has a spec target of p95 ≤ 25 ms. The plan adds latency benchmarking in Phase 4, not Phase 2. A direct-freshness check against an in-memory ledger should be sub-millisecond; if it is not by end of Phase 2, the structural problem is worth catching before graph work compounds it.

**Fix:** add a microbench-only quick check to the Phase 2 gate. Acceptance: direct-freshness lookup against materialized ledger view returns in p95 ≤ 1 ms on a 10,000-observation fixture. Failure does not block Phase 2 advance but does flag for Phase 4 latency POC scrutiny.

**Confidence:** medium. Cheap to add, useful early signal.

---

### Patch 7 — Rebalance premortems

**Where:** Phase 1 Wave 1.0, Phase 4 Wave 4.0 (new), Phase 5 Wave 5.0, Phase 6 Wave 6.0 (new).

**Problem:** premortems are applied at Phase 1 (where the work is mostly boilerplate workspace + identity + storage) and Phase 5 (correct — adapter rollout is high-risk). Phase 4 (graph + extractors + fingerprints) and Phase 6 (diagnostics + 4 framework extractors + 4 MCP tools + full CLI + metrics + bench) are denser than Phase 5 and get no premortem.

**Fix:** add premortems for Phase 4 (focus: false-positive denies from heuristic-confidence facts, contract-fingerprint instability, dynamic-language overconfidence, latency POC methodology traps) and Phase 6 (focus: diagnostic-delta attribution noise in messy repos, framework-extractor overconfidence, MCP tool-selection collisions, generated/vendored hard-deny leaks, metrics-bundle holes). Phase 1's premortem is optional in our view; keep it or drop it on your judgment.

**Confidence:** medium-high. Adding premortems is cheap and high-leverage at dense phase entries.

---

### Patch 8 — Collapse Phase 4 + Phase 5 GPT-Pro re-engagement into one consult at end of Phase 5

**Where:** §7 GPT-Pro checkpoints.

**Problem:** the plan engages Pro at end of Phase 4, then again at end of Phase 5. Two consecutive phases with no implementation distance between consults. Pro's value compounds when feedback lands and gets implemented before the next consult. Two back-to-back rounds risk landing the same critique twice.

**Fix:** collapse into one Pro consult at end of Phase 5. By that point Pro sees graph + adapters + MCP together, which is the natural integration inflection. Keep Phase 7 Pro re-engagement as the pre-P1/P2 review.

If you believe the Phase 4 boundary (just before hooks land) deserves an independent Pro pass, defend — this is a judgment call we will defer to you on.

**Confidence:** medium. Your call.

---

### Patch 9 — Skip bug hunt at Phase 1 boundary

**Where:** Phase 1 checkpoints in §4.

**Problem:** the Phase 1 bug hunt happens before there is much surface to hunt. The phase's natural integration stress tests — concurrent daemon launch, monotonic event IDs under stress, replay determinism, identity false-match fuzzing — *are* the bug hunt. Running `debugging-systematic` + `diagnose` as a separate checkpoint on a phase with thin behavioral surface mostly produces "no findings."

**Fix:** remove the Phase 1 bug hunt checkpoint. Keep bug hunts at Phases 2, 4, 5, 6, 7.

**Confidence:** medium. If you believe Phase 1's identity/storage surface warrants the systematic bug hunt, defend.

---

### Patch 10 — Name the extractor grammar stack

**Where:** §3 cross-cutting strategies, new subsection.

**Problem:** the plan never names what grammars/parsers extractors use. Tree-sitter? Compiler-backed (rust-analyzer, tsc, gopls, mypy)? Custom? The final spec implies a hybrid: tree-sitter for navigation, compiler-backed for high-confidence facts. The plan inherits this implicitly. But a worker tasked with "implement Python pass 1 extractor" will pick alone, and peers may diverge on grammar choice across languages.

**Fix:** add a §3 subsection titled "Extractor stack" that names:

- Tree-sitter as the default parser substrate for all P0-α and P0-β languages.
- Compiler-backed seams documented in `briefcase-extract-core` as adapter slots for future plug-ins: rust-analyzer for Rust, tsc/ts-server for TypeScript, gopls for Go, mypy/Pyright for Python, etc.
- Grammar version pinning policy (extractor version includes grammar version).
- Confidence-tier policy: tree-sitter facts default to medium confidence, compiler-backed facts default to high confidence, heuristic-only facts default to low confidence. Per-fact overrides allowed with provenance.

**Confidence:** high. Definite gap.

---

### Patch 11 — State commit/worktree integration protocol

**Where:** §3 cross-cutting strategies or §6 risks.

**Problem:** the plan says workers do not commit, and uses `--isolation worktree` for some tasks. It never states how those diffs are integrated into the main tree (git apply per-task vs branch-merge), who runs the commit (orchestrator), at what cadence (per-wave or per-phase), or how cross-task conflicts are adjudicated when two workers in separate worktrees touch overlapping files in the same wave.

**Fix:** one paragraph in §3 or §6 stating the integration protocol. Proposed wording:

> Orchestrator owns commits. Workers run in either the main tree (single-owner crates) or `--isolation worktree` (overlap-prone or large changes). Worktree diffs are integrated by `git apply` of per-task patches against the main tree, never by branch-merge. Cross-task conflicts within a wave are resolved by the orchestrator at integration time before the wave's reviewer pass dispatches. Commit cadence is once per wave after the review loop is clean; the wave commit message lists the wave ID, owned crates, and contributing models. Workers never stash, never rebase, never amend.

If you have a stronger protocol, propose it.

**Confidence:** high. Definite gap.

---

### Patch 12 — Pro re-engagement structural-change triage

**Where:** §7 GPT-Pro checkpoints.

**Problem:** §7 says "any time a phase's acceptance criteria require a spec revision: send to Pro." It does not say what happens when Pro's response contains *structural* changes (e.g., "rethink the graph contracts") versus *fix-list* changes ("tighten this fingerprint test"). Implementation could be 60% through a phase when structural feedback lands.

**Fix:** add one sentence to §7. Proposed wording:

> Pro responses are triaged on receipt: fix-list-shaped feedback is applied in-place by the orchestrator without pausing forward progress; structural feedback (schema, build-order, capability-tier, advance-refusal-gate changes) pauses Wave N+1 dispatch and initiates a patch-and-resend round before any further work lands.

**Confidence:** high.

---

## Patches we considered and rejected

- **Add task-identity drift to §6 risks.** True risk but already implicit in "workers do not retain state across calls" and the per-task brief discipline. Not worth adding.
- **Rewrite the 26-crate split into "build-out vs stub" tiers per phase.** Tempting for orchestrator legibility but the existing per-wave task list already discloses this. Adding a separate tier list creates a second source of truth.
- **Add Cursor agent / Composer 2.5 work mode distinction.** The plan's existing `delegate cursor work` mapping is correct.
- **Defer Windows to V2 P1.** macOS/Linux first with Windows in Phase 7 is the right call; the V1 product was macOS-primary anyway.

---

## Open questions for you

1. **Reviewer rotation policy cap.** We proposed "no reviewer model receives more than 30% of assignments." Is this the right number? If you believe a different cap better balances quality vs. orchestration overhead, defend.

2. **Phase 4 Pro consult.** If you believe the Phase 4 boundary deserves an independent Pro pass before hooks land (rather than collapsing into Phase 5), say why.

3. **Phase 1 premortem.** Worth keeping or drop alongside the Phase 1 bug hunt? Your call.

4. **Active broadcasts.** If you believe broadcasts genuinely belong in Phase 6 (not Phase 7), defend — we are willing to be wrong on this.

5. **Anything else this patch round missed.** You have full plan context. If a structural concern surfaced during plan drafting that we did not raise, raise it now.

---

## Response format we want

For each patch: **accept / accept with modification / reject** plus one or two sentences of reasoning. For the five open questions: a direct answer.

If you accept everything, the simplest output is a revised full plan with patches applied. If you accept most and reject some, an accept/reject table plus a revised full plan applying only the accepted patches.

Estimated patch-round turnaround budget on our side: one round if your response is clean, two rounds maximum if a patch interacts in ways neither of us caught.

After this patch round closes, we dispatch Wave 1.1.
