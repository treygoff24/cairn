# State

What's landed, what's in flight, and what's next. Update this whenever a wave advances, an artifact lands, or a decision flips. `CONTEXT.md` holds the *why*; this file holds only the *where*.

---

## Current state (2026-07-26, `8eaf4d1`)

**Phase 1 in progress. Waves 1.0–1.2 complete, the Wave 1.3 contract is frozen, Wave 1.3 implementation has not started.** Gate green: fmt + clippy `--all-targets` clean, **193 tests**. Everything pushed to `origin/main` — direct-to-main, no PR flow, no CI.

Since Wave 1.2 landed, the repo took a Codex-driven quality campaign rather than new wave work:

- `02afaab` — **Wave 1.3 contract frozen**: `CapabilityRegistration` event across `cairn-types`/`cairn-protocol`, harness-sim support, and `rusqlite` promoted to `[workspace.dependencies]` (closing a Wave 1.2 carry-forward).
- `ae9e74d` — the Cairn explainer site under `site/`, now tracked, with `site/DESIGN_CONTEXT.md` as its standing brief.
- `8671def`–`a656ff3` — four desloppify rounds.
- `ac77b81` — test-gap sweep, 174 → 193 tests.
- `8eaf4d1` — `scripts/gate.sh`, the canonical gate covering both the Rust workspace and the site.

## Phase tracker

| Phase | Status | Notes |
|---|---|---|
| Planning | Complete | Spec + 30-wave plan, Patch Rounds 1 and 2 applied |
| 1. Identity substrate | In progress | Waves 1.0–1.2 ✓ (12 crates, 193 tests); Wave 1.3 contract ✓, implementation next, closes the phase |
| 2. ObservationLedger + EditLedger + direct freshness | Not started | |
| 3. ContextFrame ledger + scheduler skeleton | Not started | |
| 4. Minimal graph (P0-α languages) | Not started | |
| 5. Hooks and MCP as clients of daemon contracts | Not started | |
| 6. Diagnostics + framework extractors + broadcasts | Not started | |
| 7. Extension (P0-β languages, web UI, federation) | Not started | |

Phases come from the spec's Appendix A; the implementation plan subdivides each into waves.

## Waves landed

- **Wave 0 tracer bullet (2026-06-01)** — both existential bets validated in a throwaway rig at `~/Code/cairn-tracer-sandbox` (separate disposable repo, not committed): Claude Code PreToolUse/PostToolUse hook → Unix socket → minimal Rust daemon → injected context.
  - **Latency green.** Warm p95 full per-call cost ~2.15 ms, process-spawn-dominated; the hook's own work ~0.076 ms and the daemon round-trip ~0.022 ms. Cold start (first call launches the daemon) ~14–230 ms, one-time. All far under the 25 ms budget, and a direct quantification of "hook startup latency is product surface area": ~2 ms for a Rust hook vs 50–200 ms interpreted.
  - **Behavior confirmed — the bet that mattered.** The injected note reached the model as an attachment and the agent *acted on it*, re-reading the file before editing. That answers red-team Risk #1 ("will agents use pushed context?") with a transcript receipt, and it's the guarantee vexp's pull mode cannot make.
  - **Gaps left open:** n=1 behavioral sample; the enforce/deny path was never exercised live; the felt-latency A/B against stacked V1 hooks was not measured.
- **Wave 1.0** — Phase 1 foundation-risk premortem (`docs/premortems/phase-1-foundation-risk.md`).
- **Wave 1.1 bootstrap amendment (`ca508ff`)** — a `plan-reviewer` pass attacked the bootstrap contract before fan-out and closed three blockers: the `cairn-types` public surface, promised as the anti-divergence guard but never actually written, is now frozen on disk (typed IDs, `ContentHash`/`ConfigHash`/`ProtocolVersion`/`Timestamp`, `OperationState`/`SourceClass`, `FileVersion`/`RepoEpoch`, the 10-bit `AdapterCapabilities`, `DaemonEventKind`); a deferred-types block scopes `cairn-types` to Phase 1 and explicitly bars `GraphVersion` (a Phase 4 concept) from the identity floor; `[workspace.dependencies]` is pinned once with workers forbidden from editing it. Orchestrator calls that remain open to override: content hash = BLAKE3, and `cairn-vcs` reads `.git/` directly for Phase 1 rather than taking a library dependency, deferring gix-vs-git2 until a wave needs git object access.
- **Wave 1.1 (`7e41afd`, review fixes `db75ef6`)** — five identity-substrate crates against the frozen `cairn-types` contract, 109 tests. Review caught two BLOCKERs: `cairn-file` symlink hashing followed the target (now hashes the link), and `mtime_observed` was in `FileVersion` equality (now excluded — the freshness discipline). Downstream note that still binds: storage and daemon must **not** partition on `WorktreeId` alone, because config hash and protocol live in `WorktreeIdentity`, not in the `WorktreeId` hash.
- **Wave 1.2 (`a4fc583`, review fixes `3294511`)** — six crates (`cairn-protocol`, `cairn-storage`, `cairn-daemon-client`, `cairn-daemon`, `cairn-cli`, `cairn-harness-sim`), 174 tests. Headline fixes from a six-lane independent review: socket-path unification (client `/tmp/cairn` vs daemon `XDG_RUNTIME_DIR/cairn` could launch a second daemon — now one shared `socket_path_for_key`/`default_socket_dir`); `protocol_version` consolidated to the envelope level and validated on replay; redaction no longer round-trips through typed deserialize (was an event-drop risk); wire naming aligned to spec Appendix B (`session_id`→`agent_session_id`, `adapter`→`harness`).

## Wave 1.3 — next up

Plan §4 Wave 1.3: app wiring, capability registration, V1 corpus inventory, Phase 1 stress tests, and the **Phase 1 event-log golden fixture freeze**. Same loop: freeze contract → fan out → integrate → gate once → independent review.

Open carry-forwards:

- **`cairn-app` is still `fn main() {}`.** The daemon-client launcher cannot start a daemon until the `cairn` binary implements `daemon serve`. Wiring that is Wave 1.3 work and unblocks the launcher end-to-end.
- **The golden fixture freezes the wire and log shapes** — freeze the *post-fix* shapes: `agent_session_id`/`harness` naming, a single envelope-level `protocol_version`, redacted JSON persisted directly, `Timestamp` as a JS-safe string, hashes validated on deserialize. The fixture is v1.
- **`protocol_version` validation is strict-equal-to-current** — replaying an event written under a different version errors with `ProtocolVersionMismatch`. Correct for Phase 1 (v1 only); a later protocol bump needs an old-version replay/migration story.
- **Module split is still open.** The four desloppify rounds did not break up the large single-file crates: `cairn-daemon` (1795 lines), `cairn-protocol` (1623), `cairn-daemon-client` (1362) are each still one `lib.rs`. Only `cairn-vcs` is split across modules. Run this at the Phase 1 boundary.

Then Phase 1 closes and the vertical slice runs: Phase 2 ledgers + direct freshness → thin real Claude hook adapter (brought forward) → the **cry-wolf precision gate** (self-edit false positives <0.5%, spec §3, reusing the tracer rig) → minimal TS+Python graph → dependency staleness → initial MCP tools.

## In flight

Nothing executing.

## Punch list

- License unset in `Cargo.toml` (AGPL vs MIT — Trey's call).
- No CI. CI strategy is part of the implementation plan's cross-cutting strategies.
- `cairn-vs-vexp.html` and `vexp-deep-dive.html` — untracked competitive-analysis artifacts, intentionally not committed.
