# Phase 1 Foundation-Risk Premortem

*Wave 1.0 · orchestrator-direct · no code · ≤1 page*

**Frame.** It is Phase 4. We are ripping out and rewriting the Phase 1 substrate — identity, daemon lifecycle, event log — and months are lost. This premortem names the specific ways that happens and the cheap guard for each, so the Wave 1.1 contract covers them *before* fan-out.

## 1. Identity false matches — highest blast radius

**Failure:** two distinct worktrees resolve to one identity (false *merge* → two projects share one daemon and one ledger, cross-contaminating beliefs), or one worktree resolves to two identities (false *split* → split-brain). Causes: trusting raw paths through symlinked roots, case-insensitive APFS vs case-sensitive volumes, nested/linked git worktrees, bind mounts, config-hash collisions.

**Guard:** identity = canonical real-path (symlinks resolved) + git common-dir + worktree id + config hash + protocol version, hashed together. `cairn-identity` fixtures must include symlinked root, case-variant path, nested worktree, detached HEAD, two sibling worktrees of one repo, and a config-hash change. **Acceptance:** identity deterministic and correctly distinct across all six.

**Why it matters most:** identity is the partition key for the entire ledger. A false match corrupts beliefs *across projects*. This is the single highest-blast-radius bug in Phase 1.

## 2. Daemon split-brain on concurrent launch

**Failure:** TOCTOU between "is a daemon running?" and "launch one." Two clients launch at once, both pass the check, both bind, two event logs diverge.

**Guard:** single-instance fencing via an OS-level exclusive lock (DB lease + heartbeat file + generation id) — not a presence check. The late launcher attaches to the winner or fails closed; it never spawns a second. **Acceptance:** `cairn-harness-sim` launches N concurrent clients → exactly one daemon generation observed.

## 3. Event-log corruption under crash

**Failure:** an append interrupted mid-write (power loss, SIGKILL) leaves a torn record; replay then panics or silently truncates, and every belief derived from the log is suspect.

**Guard:** append-only, length-framed, atomic records on a WAL-backed store; replay validates frame integrity and stops at the last good record with a logged receipt rather than panicking. **Acceptance:** crash-injection (truncate the log mid-record) → daemon recovers to last consistent event, reports degraded, does not panic and does not claim false freshness.

## 4. Monotonic event-ID violations

**Failure:** IDs minted from wall-clock time, or a non-atomic counter under concurrent append → duplicate or out-of-order IDs → the happens-before ordering that the multi-agent staleness logic depends on becomes a lie.

**Guard:** IDs come from a single atomic monotonic source owned by the log, never from time; ordering is total and never reordered. **Acceptance:** concurrent-append stress asserts strict monotonicity + uniqueness. No mtime / wall-clock anywhere in ordering or freshness (ties to the content-hash-not-mtime discipline in `cairn-file`).

## 5. Bootstrap-contract incompleteness — the Wave 1.1-specific risk

**Failure:** the bootstrap commit under-specifies the type surface the six concurrent workers compile against; workers invent divergent shapes for shared types; integration at wave close costs more than the implementation did.

**Guard:** `cairn-types` publishes its public surface (typed IDs, `OperationState`, `SourceClass`, `AdapterCapabilities`, event kinds, timestamp wrappers) before fan-out; the only inter-crate dependency in Wave 1.1 is on `cairn-types`. **Acceptance:** `cargo check --workspace` green on the empty tree; `cairn-types` surface reviewed against spec Appendix B before any consumer ships.

## Cross-phase flag — recorded here so it isn't lost

The most existential assumption in the entire build — that the hook→daemon hot-path round-trip fits the ~25–35 ms budget — is **not** exercised by Phase 1 and is parked at Wave 4.1, which the plan itself calls the highest-risk wave. Phase 1's daemon-client + socket primitives (Wave 1.2) are the natural place to take a first real measurement for nearly free. **Recommendation:** when `cairn-daemon-client` + `cairn-cli` land, add a throwaway round-trip latency probe. Cost ≈ nil; it converts the existential bet from *assumed* to *measured* three phases early. Not a Phase 1 blocker — flagged for the regroup.

## Concurrency cap

Wave 1.1 max parallel workers: **6** (one per owned crate). No worker runs `cargo test`, `cargo build --release`, or benches — the orchestrator runs the gate once at wave close.
