# Cairn Implementation Plan — Patch Round 2

*Applied 2026-06-01. Triggered by the Wave 0 tracer-bullet results and the vexp competitive finding. This is a **scope re-cut and sequencing patch**, not a rewrite — the plan's architecture, crate decomposition philosophy, and build method are preserved.*

---

## Why this patch exists

Three things became true this session, after the original plan (+ Patch Round 1) was written:

1. **vexp shipped the substrate.** A local-first Rust daemon (graph + index in SQLite, MCP, advisory staleness, session memory, cross-repo, 30 languages, freemium closed-source) now occupies the "code-graph-for-agents" space. The graph/index layer is no longer novel — it is **table stakes**.
2. **The push-mode hooks layer is the differentiator, and it's now proven.** Wave 0 validated, in the real Claude Code harness, that a PreToolUse hook → daemon → injected context is fast (~2.15 ms/call warm, spawn-dominated; far under the 25 ms budget) **and** that the agent acts on the pushed note (it re-read a file before editing because the note told it to). vexp is pull-mode and structurally cannot guarantee this.
3. **Cry-wolf is the make-or-break.** The tracer fired an unconditional (synthetic) staleness note; a live agent immediately noticed the false positive and said it would learn to tune the warning out. A push channel that cries wolf is *worse* than advisory pull-mode. Therefore **signal precision is the load-bearing core**, and signal precision is computed *from* the graph + ledger.

### The reframe (corrects a misstep)

An earlier draft of this re-cut inferred "vexp commoditized the substrate → minimize the substrate (thin/swappable graph, fewer languages because graph is commodity)." **That inference is wrong and is retracted.** The cry-wolf finding proves the opposite: the moat's precision *depends on* an accurate graph + ledger. Skimp the substrate and the staleness signal degrades → cry-wolf → the moat collapses.

Cairn is a **superset play**: *match* vexp's capabilities (built lean and correct, because the differentiator's precision rides on them) **and** add what vexp structurally can't — deterministic push delivery, per-session belief precision, enforcement, multi-agent coordination. Not "their stuff thin + our stuff." Their stuff *solid* as the floor, our stuff as the ceiling.

**Honest consequence:** a superset is *more* total work, not less. The discipline below is not "cut the substrate" — it is "stage a vertical slice so we reach validated value without building all 26 crates before anything works."

---

## A. What is UNCHANGED (do not over-read this patch)

- **Architecture:** ledger-first; graph *feeds* the ledger/freshness/context, is not the primary API surface; hooks/MCP are clients of daemon contracts. (Spec §14 design pin intact.)
- **Crate decomposition philosophy:** ~26 crates, heavy decomposition for the parallel-worker swarm. *Locked decision — not relitigated.* This patch only sequences **which** crates land in the V1/P0 vertical slice vs. P1/P2; the others still exist as designed.
- **Build method:** orchestrator-owns-the-contract-before-fan-out, per-wave independent review by a different model, premortems at phase boundaries, golden-fixture freezes, light-checks-in-workers / full-gate-at-orchestrator. *Validated by the Wave 0 fan-out — keep it.*
- **Disciplines:** content-hash-not-mtime, fail-open, CPU discipline, Git discipline.
- **Latency budgets:** the ~25–35 ms hot-path budgets — now partially **validated** (Wave 0), not just asserted.

## B. Validated facts to treat as ground truth (from Wave 0)

- Hook → Unix-socket daemon → injected-context round trip is **GREEN**: warm p95 full per-call ~2.15 ms (process-spawn-dominated; hook's own work ~76 µs, daemon round trip ~22 µs); ~0.2 ms in-process in real CC with 0 parse errors; cold start ~14–230 ms one-time.
- **Push delivery + behavioral steering works in-harness** (n=1, clean existence proof). Injected `additionalContext` on a PreToolUse `allow` reaches the model and changes its actions.
- The **`~/Code/cairn-tracer-sandbox` rig is reusable infrastructure** for the precision gate (§D) — it already drives hooks, measures latency, and exercises advisory inject / enforce deny.
- Claude Code hook contract is confirmed: PreToolUse is synchronous, blocks the tool, spawns a fresh process per call; `permissionDecision:"allow"` + `additionalContext` = inject; `"deny"` = block (model does not see the note).

## C. Scope deltas — what's IN the V1 vertical slice vs. deferred

The §2 crate table already tiers crates. This makes the V1 critical path explicit.

**Deferred out of the V1 build (remain in the plan as P1/P2, do not size the V1 architecture):**
- `cairn-web` (local web UI) — P1.
- `cairn-extract-p0beta` (Java, C#, PHP, Ruby, C/C++, Swift, Kotlin, Dart) — P1/P2.
- `cairn-extract-bridges` (RN/Expo/Swift-ObjC/JNI/FFI) — P2.
- `cairn-extract-frameworks` — **trim to the two launch languages only** for V1 (Next.js + FastAPI); other frameworks follow their language tier.
- Worktree-per-agent federation, file leases, active broadcasts — already P1/P2; keep deferred (inert placeholders only in V1, per Patch Round 1).
- The full VAT / auto-research / ablation apparatus (`cairn-metrics` deep features) — keep a **lightweight metrics log** in V1 (enough to run the precision gate); defer the optimization gym to P1.

**Languages — start narrow but DEEP:** P0-α is 4 languages (TS/JS, Python, Go, Rust). For the vertical slice, **start with TypeScript + Python** (full graph + ledger + hooks + precision, not thin), then add Go + Rust once the slice proves out. This is sequencing for a coherent end-to-end slice, **not** substrate-skimping.

Net effect: the V1 active crate set is ~18–20 of the 26; the rest are real, just later.

## D. Sequencing deltas — the vertical-slice MVP + the early precision gate

**The V1 vertical slice (thinnest end-to-end that delivers the moat on a real repo, TS+Python):**

1. **Identity substrate** (Phase 1 — unchanged): identity, single daemon, event log, content-addressed file versions, repo epochs. *Invariant to this patch; safe to build now.*
2. **Ledgers + direct-file freshness** (Phase 2): session/observation/edit ledgers; "the file you're about to edit changed since you observed it" — **needs no graph**, just content-hash + observation ledger.
3. **NEW — minimal real Claude hook adapter, brought forward.** A thin real `cairn-adapter-claude` riding on direct-file freshness, landing at the Phase 2/3 boundary instead of waiting for the full Phase 5 adapter rollout. Purpose: enable the precision gate on real edits. The full multi-adapter rollout (Codex, Cursor, capability tiers, PreCompact) stays in Phase 5.
4. **NEW — Cry-wolf precision gate (early hard gate).** Before investing in graph-based dependency staleness, validate the spec §3 target — **self-edit false-positive rate < 0.5%** — with *real* (not synthetic) ledger logic, many edits, in the sandbox harness. Re-reading a file MUST clear its staleness (content-hash, not mtime). This gate decides whether push-mode is a moat or a liability; it is the natural successor to Wave 0 and must pass before Phase 4 graph work is justified.
5. **Minimal graph for TS+Python** (Phase 4): symbols, deps, contract fingerprints, provenance, confidence — real and accurate (the precision the moat needs), just scoped to two languages. Latency POC already pulled forward to Wave 0.
6. **Dependency-aware staleness + read decoration** (Phase 4/5): the cross-file / cross-agent staleness that *needs* the graph; delta-not-repeat decoration.
7. **Initial MCP tools** (Phase 5): orient / observed-state / prove as clients of the daemon contracts.

Everything past step 7 = **widen** (more languages, frameworks, full diagnostics, coordination depth, web UI, metrics gym).

**Precision-gate principle generalized:** Wave 0 proved "validate the existential bet cheaply, first." Apply it to every subsequent existential assumption (does the precise signal hold across many edits? does dependency-fingerprint stability hit its target? does hard-deny work where the harness allows?) — early gates, not Phase-6 emergent metrics.

## E. Net

Keep the plan. Trim the far-future extensions out of the V1 critical path, start TS+Python deep, insert the cry-wolf precision gate as an early hard milestone (reusing the tracer rig), and bring one thin real hook adapter forward to feed it. Phase 1 is unaffected and safe to fan out now. The result is a staged path to a usable, self-validating product that is a genuine superset of vexp — not a thinner thing beside it.
