# Gates

Two tiers. The **worker light check** runs inside a single crate, by an implementation worker. The **canonical gate** — `./scripts/gate.sh` — runs once, across both verification surfaces, at the orchestrator, at the close of a wave or phase. The split exists for one reason: CPU. N parallel workers each running a full `cargo test` or release build concurrently has melted machines in real use.

## Worker light check

A worker verifies only its own crate, and never runs tests or builds:

```bash
cargo fmt -p <crate>
cargo check  -p <crate> --all-targets      # type-checks test code without running it
cargo clippy -p <crate> --all-targets --no-deps
```

`--all-targets` lets a worker confirm its test code compiles. The tests are *written* by the worker and *run* by the orchestrator. Each worker runs under its own `CARGO_TARGET_DIR` so concurrent checks don't contend.

**Forbidden inside a worker:** `./scripts/gate.sh`, `cargo test`, `cargo build`, `cargo build --release`, `pnpm --dir site build`, benchmark runs, E2E tests, any workspace-wide build.

## Canonical gate

`./scripts/gate.sh` is authoritative, and it is the only thing that makes a wave green — a worker's local light check passing does not. Read the script for the exact list of checks; do not maintain a second copy of that list here.

## Phase 1 gate (identity substrate)

In addition to `gate.sh`, Phase 1 must pass:

```bash
cairn daemon doctor --self-test          # once the binary wires up (Wave 1.3)
cargo test --test phase1_concurrent_launch
# event-log golden-fixture replay (frozen at the phase boundary)
```

**Phase 1 does not advance if** any of these hold: a split-brain daemon case (two live daemons for one worktree), a non-monotonic event ID, an identity false match across worktrees, or a red full gate not quarantined as unrelated infrastructure flake.

## Lint posture

Lints are inherited from `[workspace.lints]` in the root `Cargo.toml` (every crate carries `[lints] workspace = true`), so a worker's local clippy cannot drift from the orchestrator's run. `unsafe_code` is denied workspace-wide for Phase 1; the crate that legitimately needs it later (mmap snapshots) opts in explicitly and locally.
