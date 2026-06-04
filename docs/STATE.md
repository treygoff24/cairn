# State

A living document of what's happening, what's in flight, and what's next. Update this whenever a phase advances, an artifact lands, or a major decision flips.

---

## Current state (2026-06-04)

**Wave 1.2 COMPLETE — storage, protocol, daemon core (2026-06-04).** Six crates (`cairn-protocol`, `cairn-storage`, `cairn-daemon-client`, `cairn-daemon`, `cairn-cli`, `cairn-harness-sim`) built by a single Codex run against an orchestrator-frozen wave-internal contract (`EventLog`/`DaemonClient` traits, `DaemonEvent`/`DaemonDecision`, `EventId`). The contract was frozen in the working tree rather than a separate prelude commit, so contract + impl landed together at `a4fc583`. A six-lane independent review (grok=daemon, cursor=daemon-client+protocol, deepseek-flash=cli, orchestrator=storage+harness-sim; the slow droid lanes deepseek-pro/glm/gemini flaked on capture/timeout and were recovered) found one real bug plus robustness/consistency items, fixed by a Codex work-mode round at `3294511`. Gate green: fmt + clippy `--all-targets` clean, **174 tests** (109 Wave 1.1 intact + 65 Wave 1.2). Headline fixes: socket-path unification (client `/tmp/cairn` vs daemon `XDG_RUNTIME_DIR/cairn` mismatch could launch a second daemon — now one shared `socket_path_for_key`/`default_socket_dir`), `protocol_version` consolidated to the envelope level + validated on replay, redaction no longer round-trips through typed deserialize (was an event-drop risk), wire naming aligned to spec Appendix B (`session_id`→`agent_session_id`, `adapter`→`harness`). All pushed to `origin/main`. Reviewers cleared the daemon split-brain/lease logic (grok: ship) and confirmed no capability over-claim in the CLI.

**Build is a go.** After a frank "is this LLM psychosis?" gut-check this session — V1 ground-truthed as real (878 tests verified by running it), spec red-teamed, and the competitive landscape checked — the decision is to build. Key finding: the substrate (graph/index/ledger) is now commoditized by a shipping product, **vexp** (local-first Rust daemon, MCP, advisory staleness, freemium closed-source). Cairn's defensible daylight is the **push-mode hooks layer** (deterministic context injection + edit interception, already proven in V1) — which vexp structurally lacks — plus enforcement and multi-agent coordination. See `cairn-vs-vexp.html` / `vexp-deep-dive.html` (untracked analysis artifacts).

**Wave 1.0 complete** — Phase 1 foundation-risk premortem written (`docs/premortems/phase-1-foundation-risk.md`).

**Wave 1.1 bootstrap prelude committed** — cargo workspace skeleton: root `Cargo.toml`, `rust-toolchain.toml` (pinned 1.95.0), `.cargo/config.toml`, six empty crates (`cairn-types`, `cairn-config`, `cairn-identity`, `cairn-file`, `cairn-vcs`, `cairn-app`) with compile-only stubs. `cargo check/fmt/clippy --workspace` all green; `cairn` binary builds.

**Wave 0 tracer bullet — COMPLETE, both existential bets validated (2026-06-01).** Built a throwaway end-to-end push-mode rig in `~/Code/cairn-tracer-sandbox` (separate disposable repo, not committed): Claude Code PreToolUse/PostToolUse hook → Unix socket → minimal Rust daemon → injected context. Four crates built by a 4-worker parallel fan-out against an orchestrator-owned contract.

- **Latency: GREEN.** Scripted floor (unconfounded): warm p95 full per-call cost ~2.15 ms — process-spawn-dominated; the hook's own work is ~0.076 ms and the daemon round-trip ~0.022 ms. In real Claude Code: hook in-process work warm p95 ~0.2 ms, 0 parse errors. Cold start (first call launches daemon) ~14–230 ms, one-time. All far under the 25 ms budget. Quantifies the "hook startup latency is product surface area" thesis: ~2 ms spawn for a Rust hook vs 50–200 ms for an interpreted one.
- **Mechanism: works in the real harness.** Hook fires on every tool call, parses real CC stdin cleanly, injects `additionalContext` on advisory Edit/Write.
- **Behavior: CONFIRMED (the one that mattered).** In the live session the injected note reached the model as an attachment and the agent *acted on it* — re-read the file before editing because the note told it to. Answers red-team Risk #1 ("will agents use pushed context?") = yes, with a transcript receipt. This is the differentiator vexp's pull-mode cannot guarantee.
- **Gaps:** n=1 behavioral sample; enforce/deny path not exercised live; felt-latency A/B (vs stacked V1 hooks) not measured.
- **Design note (cry-wolf):** the tracer fires its note *unconditionally* on every Edit/Write — it's a hardcoded synthetic string; the rig has no staleness logic, by design. A live agent immediately noticed the false positive and flagged it would learn to tune the warning out. Direct field evidence that the real product's **signal precision is make-or-break** — exactly what the spec's per-session content-hash observation ledger (not mtime) + the <0.5% self-edit-false-positive target (§3) exist to deliver.

**Phase 1 bootstrap-amendment applied (2026-06-01, `ca508ff`).** Before fanning out Wave 1.1, a `plan-reviewer` pass attacked the bootstrap contract and surfaced three blockers, all now closed by the orchestrator: (1) the `cairn-types` public surface — promised as the anti-divergence guard but never actually written — is now frozen on disk (typed IDs, `ContentHash`/`ConfigHash`/`ProtocolVersion`/`Timestamp`, `OperationState`/`SourceClass` enums, `FileVersion`/`RepoEpoch` shapes, the 10-bit `AdapterCapabilities`, `DaemonEventKind`), so Tasks 3–6 consume one definition instead of each inventing their own; (2) a deferred-types doc block scopes `cairn-types` to Phase 1 and explicitly bars `graph_version`/`GraphVersion` (a Phase 4 concept) from the identity floor; (3) `[workspace.dependencies]` is pinned once (serde, serde_json, **blake3** as the named content-hash algorithm, toml, thiserror, anyhow) with workers forbidden from editing it, plus a `[workspace.lints]` table inherited by all crates. `fixtures/{config,identity,files,vcs}/` + `docs/dev/` scaffolded; `Cargo.lock` now tracked. Gate green (fmt/check/clippy --all-targets). Load-bearing choices made by the orchestrator and open to override: content hash = BLAKE3; git access in `cairn-vcs` = direct `.git/` reads for Phase 1 (no library dependency), deferring gix-vs-git2 until a wave needs git object access.

**Wave 1.1 COMPLETE (2026-06-01).** The five identity-substrate crates landed via a parallel delegate-worker swarm against the frozen `cairn-types` contract, were integrated and gated once at the orchestrator, then put through a mandatory per-crate independent review (different model than each implementer). Green wave committed at `7e41afd`; review fixes at `db75ef6`. Final gate: fmt + clippy clean, **109 tests across 11 suites**.

- Crates: `cairn-types` (grok), `cairn-config` (deepseek-flash + orchestrator fixes), `cairn-identity` (codex), `cairn-file` (deepseek-pro + codex fixes), `cairn-vcs` (cursor + orchestrator fixes).
- Bugs the gate/review caught (workers never run tests, by CPU discipline): config nested-unknown-key handling + a `config_hash` crash for named projects; vcs linked-worktree `commondir` resolution + non-committable `.git` fixtures; **2 BLOCKERs** in review — `cairn-file` symlink hashing followed the target (now hashes the link), and `mtime_observed` was in `FileVersion` equality (now excluded — the freshness discipline). Plus `Timestamp` now serializes as a JS-safe string and `ContentHash`/`ConfigHash` validate on deserialize.
- `cairn-identity` (highest blast radius) reviewed structurally sound: length-prefixed BLAKE3 ID framing, no false-match on normal paths. Downstream note: storage/daemon must not partition on `WorktreeId` alone (config-hash + protocol live in `WorktreeIdentity`, not the `WorktreeId` hash).

Remote: `github.com/treygoff24/cairn.git`. All commits pushed through `db75ef6`.

## What just happened (recent session work)

- Final spec produced via two rounds of GPT-5.5 Pro review.
- Implementation plan delivered by Pro; critical-read surfaced 12 patches; Pro's patch-decision file accepted/modified each.
- All accepted patches + 3 new patchlets applied to the plan in three clusters: §3/§6/§7 additions (Cluster C), targeted wave edits (Cluster B), plan-wide sweep across §4 waves (Cluster A).
- Coherence read complete: stale references in executive summary, Phase 6 goal, Appendix A skill matrix, and §5 first-sprint kickoff all reconciled with the patched plan.

## What's next (the immediate path)

Wave 0 (tracer bullet) is done and green, and **Patch Round 2 is applied** (`docs/Cairn Implementation Plan — Patch Round 2.md`): Cairn reframed as a **superset of vexp** (match the substrate because the moat's precision rides on it, then add the push-mode/enforcement/coordination moat — do NOT thin the substrate), V1 narrowed to a **TS + Python vertical slice**, peripheral crates (web, p0beta, bridges, most frameworks, federation/leases/broadcasts, metrics gym) deferred out of the V1 critical path, and a **cry-wolf precision gate** (self-edit false-positive < 0.5%, §3) pulled forward as an early hard milestone reusing the tracer rig.

**Immediate next step: Wave 1.3 — Phase 1 integration and acceptance** (plan §4 Wave 1.3). App wiring, capability registration, V1 corpus inventory, Phase 1 stress tests, and the **Phase 1 event-log golden fixture freeze**. Same loop: contract → fan-out → integrate → gate once → independent review.

Carry-forwards into Wave 1.3 (from the Wave 1.2 review):
- **The golden fixture freezes the wire/log shapes** — freeze the *post-fix* shapes: `agent_session_id`/`harness` naming, single envelope-level `protocol_version`, redacted-JSON-persisted-directly. `Timestamp` is a JS-safe string on the wire; hashes validate on deserialize.
- **`protocol_version` validation is strict-equal-to-current** — replaying an event written under a different version errors (`ProtocolVersionMismatch`). Correct for Phase 1 (v1 only); a protocol bump later needs an old-version-replay/migration story. The fixture is v1.
- **`cairn-app` is still an empty stub** — the daemon-client launcher cannot actually start a daemon until the `cairn` binary implements `daemon serve`. Wiring that is Wave 1.3 work, and unblocks the launcher end-to-end.
- **Promote `rusqlite` to `[workspace.dependencies]`** — pinned identically (0.32 bundled) in `cairn-storage` + `cairn-daemon`; single-source the pin.
- **Module-split desloppify pass** — every Wave 1.2 crate is one large single-file `lib.rs` (daemon ~1.9k, protocol ~1.7k, client ~1.4k); run `desloppify-deep` at the Phase 1 boundary.

Then: Phase 1 closes → Phase 2 ledgers + direct freshness → thin real Claude hook adapter (brought forward) → **cry-wolf precision gate** → minimal TS+Python graph → dependency staleness → initial MCP tools. That's the vertical slice.

Open process questions: license (AGPL vs MIT — unset in `Cargo.toml`); branch/PR workflow vs direct-to-main (currently direct-to-main, pushed).

## In flight

Nothing executing. Wave 1.2 fully landed (green `a4fc583` + reviewed + fixed `3294511`, pushed). Ready to start Wave 1.3.

## Decisions waiting on Trey (non-blocking)

- GitHub repo URL — likely `github.com/treygoff24/cairn`, not created yet
- V1 disposition — rename to `code-briefcase-py` then archive, or archive in place
- License — AGPL-3.0 (V1 default) vs MIT vs other

## Phase tracker

| Phase | Status | Notes |
|---|---|---|
| Planning | Complete | Plan + Patch Round 1 applied; build decision confirmed |
| 1. Identity substrate | In progress | Wave 1.0 premortem ✓; Wave 1.1 ✓ (5 crates, `db75ef6`, 109 tests); Wave 1.2 ✓ (6 crates green `a4fc583` + reviewed + fixed `3294511`, 174 tests total); Wave 1.3 (integration + golden fixture) next, closes Phase 1 |
| 2. ObservationLedger + EditLedger + direct freshness | Not started | |
| 3. ContextFrame ledger + scheduler skeleton | Not started | |
| 4. Minimal graph (P0-α languages) | Not started | |
| 5. Hooks and MCP as clients of daemon contracts | Not started | |
| 6. Diagnostics + framework extractors + broadcasts | Not started | |
| 7. Extension (P0-β languages, web UI, federation, etc.) | Not started | |

Phases are from the final spec's Appendix A. The implementation plan will subdivide each phase into waves.

## Open punch-list

- Commits now pushed to `origin/main` (decision flipped 2026-06-01: back up before parallel workers mutate the tree).
- License unset in `Cargo.toml` (AGPL vs MIT — Trey's call).
- No `CI` yet — CI strategy is part of cross-cutting strategies in the implementation plan.
- `site/`, `AGENTS.md`, and the two analysis HTMLs remain untracked (intentional for now).

## Notes for the orchestrator

When Pro's plan lands, the first action is *not* to fan out workers. The first action is the critical read with Trey. Phase 1, Wave 1 dispatch comes *after* the plan is judged ready.

When in doubt, refer to `CONTEXT.md` for the *why* and the binding spec for the *what*.
