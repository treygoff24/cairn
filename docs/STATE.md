# State

A living document of what's happening, what's in flight, and what's next. Update this whenever a phase advances, an artifact lands, or a major decision flips.

---

## Current state (2026-06-01)

**Build is a go.** After a frank "is this LLM psychosis?" gut-check this session — V1 ground-truthed as real (878 tests verified by running it), spec red-teamed, and the competitive landscape checked — the decision is to build. Key finding: the substrate (graph/index/ledger) is now commoditized by a shipping product, **vexp** (local-first Rust daemon, MCP, advisory staleness, freemium closed-source). Cairn's defensible daylight is the **push-mode hooks layer** (deterministic context injection + edit interception, already proven in V1) — which vexp structurally lacks — plus enforcement and multi-agent coordination. See `cairn-vs-vexp.html` / `vexp-deep-dive.html` (untracked analysis artifacts).

**Wave 1.0 complete** — Phase 1 foundation-risk premortem written (`docs/premortems/phase-1-foundation-risk.md`).

**Wave 1.1 bootstrap prelude committed** — cargo workspace skeleton: root `Cargo.toml`, `rust-toolchain.toml` (pinned 1.95.0), `.cargo/config.toml`, six empty crates (`cairn-types`, `cairn-config`, `cairn-identity`, `cairn-file`, `cairn-vcs`, `cairn-app`) with compile-only stubs. `cargo check/fmt/clippy --workspace` all green; `cairn` binary builds.

**Wave 0 tracer bullet — COMPLETE, both existential bets validated (2026-06-01).** Built a throwaway end-to-end push-mode rig in `~/Code/cairn-tracer-sandbox` (separate disposable repo, not committed): Claude Code PreToolUse/PostToolUse hook → Unix socket → minimal Rust daemon → injected context. Four crates built by a 4-worker parallel fan-out against an orchestrator-owned contract.

- **Latency: GREEN.** Scripted floor (unconfounded): warm p95 full per-call cost ~2.15 ms — process-spawn-dominated; the hook's own work is ~0.076 ms and the daemon round-trip ~0.022 ms. In real Claude Code: hook in-process work warm p95 ~0.2 ms, 0 parse errors. Cold start (first call launches daemon) ~14–230 ms, one-time. All far under the 25 ms budget. Quantifies the "hook startup latency is product surface area" thesis: ~2 ms spawn for a Rust hook vs 50–200 ms for an interpreted one.
- **Mechanism: works in the real harness.** Hook fires on every tool call, parses real CC stdin cleanly, injects `additionalContext` on advisory Edit/Write.
- **Behavior: CONFIRMED (the one that mattered).** In the live session the injected note reached the model as an attachment and the agent *acted on it* — re-read the file before editing because the note told it to. Answers red-team Risk #1 ("will agents use pushed context?") = yes, with a transcript receipt. This is the differentiator vexp's pull-mode cannot guarantee.
- **Gaps:** n=1 behavioral sample; enforce/deny path not exercised live; felt-latency A/B (vs stacked V1 hooks) not measured.
- **Design note (cry-wolf):** the tracer fires its note *unconditionally* on every Edit/Write — it's a hardcoded synthetic string; the rig has no staleness logic, by design. A live agent immediately noticed the false positive and flagged it would learn to tune the warning out. Direct field evidence that the real product's **signal precision is make-or-break** — exactly what the spec's per-session content-hash observation ledger (not mtime) + the <0.5% self-edit-false-positive target (§3) exist to deliver.

Remote exists: `github.com/treygoff24/cairn.git`. Commits are local (not pushed).

## What just happened (recent session work)

- Final spec produced via two rounds of GPT-5.5 Pro review.
- Implementation plan delivered by Pro; critical-read surfaced 12 patches; Pro's patch-decision file accepted/modified each.
- All accepted patches + 3 new patchlets applied to the plan in three clusters: §3/§6/§7 additions (Cluster C), targeted wave edits (Cluster B), plan-wide sweep across §4 waves (Cluster A).
- Coherence read complete: stale references in executive summary, Phase 6 goal, Appendix A skill matrix, and §5 first-sprint kickoff all reconciled with the patched plan.

## What's next (the immediate path)

Wave 0 (tracer bullet) is done and green — the push-mode thesis is validated end-to-end. Regroup decision now: **greenlight the Phase 1 build, and at what scope?**

1. **Phase 1 / Wave 1.1 six-worker fan-out** per the plan's §5 (Tasks 1–6: workspace scaffolding, `cairn-types`, `cairn-config`, `cairn-identity`, `cairn-file`, `cairn-vcs`). Optionally gate on a `plan-reviewer` pass over the bootstrap contract first.
2. **Scope question raised by the vexp finding** — decide whether to trim the 26-crate / 14-language plan toward the differentiated push-mode + ledger core before committing the full substrate build (red-team recommended starting ~6–10 crates, 2 languages, and proving the ledger precision that the cry-wolf finding shows is make-or-break). Open.
3. **Phase 1 acceptance** (Wave 1.3): `cairn daemon doctor --self-test`, concurrent-launch single-daemon check, identity determinism fixtures.

Open process questions: license (AGPL vs MIT — unset in `Cargo.toml`), branch/PR workflow vs direct-to-main, plan-reviewer pass before fan-out, whether to push commits to the remote.

## In flight

Nothing executing. Bootstrap + Wave 0 tracer bullet landed and green; awaiting regroup decision on Phase 1 scope and fan-out.

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
