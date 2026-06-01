# State

A living document of what's happening, what's in flight, and what's next. Update this whenever a phase advances, an artifact lands, or a major decision flips.

---

## Current state (2026-06-01)

**Build is a go.** After a frank "is this LLM psychosis?" gut-check this session — V1 ground-truthed as real (878 tests verified by running it), spec red-teamed, and the competitive landscape checked — the decision is to build. Key finding: the substrate (graph/index/ledger) is now commoditized by a shipping product, **vexp** (local-first Rust daemon, MCP, advisory staleness, freemium closed-source). Cairn's defensible daylight is the **push-mode hooks layer** (deterministic context injection + edit interception, already proven in V1) — which vexp structurally lacks — plus enforcement and multi-agent coordination. See `cairn-vs-vexp.html` / `vexp-deep-dive.html` (untracked analysis artifacts).

**Wave 1.0 complete** — Phase 1 foundation-risk premortem written (`docs/premortems/phase-1-foundation-risk.md`).

**Wave 1.1 bootstrap prelude committed** — cargo workspace skeleton: root `Cargo.toml`, `rust-toolchain.toml` (pinned 1.95.0), `.cargo/config.toml`, six empty crates (`cairn-types`, `cairn-config`, `cairn-identity`, `cairn-file`, `cairn-vcs`, `cairn-app`) with compile-only stubs. `cargo check/fmt/clippy --workspace` all green; `cairn` binary builds. **Paused here for regroup before the six-worker fan-out.**

Remote exists: `github.com/treygoff24/cairn.git`. Commits are local (not pushed).

## What just happened (recent session work)

- Final spec produced via two rounds of GPT-5.5 Pro review.
- Implementation plan delivered by Pro; critical-read surfaced 12 patches; Pro's patch-decision file accepted/modified each.
- All accepted patches + 3 new patchlets applied to the plan in three clusters: §3/§6/§7 additions (Cluster C), targeted wave edits (Cluster B), plan-wide sweep across §4 waves (Cluster A).
- Coherence read complete: stale references in executive summary, Phase 6 goal, Appendix A skill matrix, and §5 first-sprint kickoff all reconciled with the patched plan.

## What's next (the immediate path)

**Regroup decision point** — bootstrap is committed and green; deciding how to enter Wave 1.1:

1. **Tracer-bullet spike (orchestrator recommendation)** — before the six-crate fan-out, build a throwaway end-to-end push-mode latency probe (Claude Code PreToolUse hook → Unix socket → minimal Rust daemon → injected context, p50/p95/p99 measured on the real hot path). Validates the ~25–35 ms bet the whole Rust decision rests on. The plan parks this at Wave 4.1 ("highest-risk wave"); premortem risk-flag recommends pulling a thin slice forward.
2. **Wave 1.1 fan-out** — six delegate workers in one batch per the plan's §5 (Tasks 1–6: workspace scaffolding, `cairn-types`, `cairn-config`, `cairn-identity`, `cairn-file`, `cairn-vcs`). Optionally gate on a `plan-reviewer` pass over the bootstrap contract first.
3. **Phase 1 integration + acceptance** (Wave 1.3): `cairn daemon doctor --self-test`, concurrent-launch single-daemon check, identity determinism fixtures.

Open process questions for the regroup: license (AGPL vs MIT — still unset in `Cargo.toml`), branch/PR workflow vs direct-to-main, and whether to run the plan-reviewer pass before fan-out.

## In flight

Nothing executing. Bootstrap landed; awaiting regroup decision on tracer-bullet-first vs. six-worker fan-out.

## Decisions waiting on Trey (non-blocking)

- GitHub repo URL — likely `github.com/treygoff24/cairn`, not created yet
- V1 disposition — rename to `code-briefcase-py` then archive, or archive in place
- License — AGPL-3.0 (V1 default) vs MIT vs other

## Phase tracker

| Phase | Status | Notes |
|---|---|---|
| Planning | Complete | Plan + Patch Round 1 applied; build decision confirmed |
| 1. Identity substrate | In progress | Wave 1.0 premortem done; Wave 1.1 bootstrap committed + gate green; workers not yet dispatched |
| 2. ObservationLedger + EditLedger + direct freshness | Not started | |
| 3. ContextFrame ledger + scheduler skeleton | Not started | |
| 4. Minimal graph (P0-α languages) | Not started | |
| 5. Hooks and MCP as clients of daemon contracts | Not started | |
| 6. Diagnostics + framework extractors + broadcasts | Not started | |
| 7. Extension (P0-β languages, web UI, federation, etc.) | Not started | |

Phases are from the final spec's Appendix A. The implementation plan will subdivide each phase into waves.

## Open punch-list

- Commits are local; nothing pushed to the remote yet.
- License unset in `Cargo.toml` (AGPL vs MIT — Trey's call).
- No `CI` yet — CI strategy is part of cross-cutting strategies in the implementation plan.
- `site/`, `AGENTS.md`, and the two analysis HTMLs remain untracked (intentional for now).

## Notes for the orchestrator

When Pro's plan lands, the first action is *not* to fan out workers. The first action is the critical read with Trey. Phase 1, Wave 1 dispatch comes *after* the plan is judged ready.

When in doubt, refer to `CONTEXT.md` for the *why* and the binding spec for the *what*.
