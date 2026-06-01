# Gates

The commands that decide whether code may advance. Two tiers: the **worker light
check** (run inside a single crate, by an implementation worker) and the
**orchestrator gate** (run once, across the workspace, by the orchestrator at the
close of a wave or phase). The split exists for one reason — CPU. N parallel
workers each running a full `cargo test` or release build concurrently has melted
machines in real use. Heavy builds happen once, at the orchestrator, never fanned
out.

## Worker light check (allowed inside a worker)

A worker verifies only its own crate, and never executes test or release builds:

```bash
cargo fmt -p <crate>
cargo check  -p <crate> --all-targets      # compiles test/bench code too, without running it
cargo clippy -p <crate> --all-targets --no-deps
```

`--all-targets` type-checks the crate's tests so a worker can confirm test code
compiles. The tests themselves are *written* by the worker but *run* by the
orchestrator.

**Forbidden inside a worker:** `cargo test`, `cargo build`, `cargo build
--release`, benchmark runs, E2E tests, any workspace-wide build. Each worker also
runs under its own `CARGO_TARGET_DIR` so concurrent checks don't contend.

## Orchestrator gate (run once, at wave/phase close)

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --no-deps
cargo test --workspace
```

The orchestrator owns this run. It is the authoritative signal — a worker's local
light check passing does not mean the wave is green.

## Phase 1 gate (identity substrate)

In addition to the workspace gate above, Phase 1 must pass (per the implementation
plan):

```bash
cargo test --workspace
cairn daemon doctor --self-test          # once the binary wires up (Wave 1.3)
cargo test --test phase1_concurrent_launch
# event-log golden-fixture replay (frozen at the phase boundary)
```

**Advance refusal — Phase 1 does not advance if any of these hold:**

- a split-brain daemon case (two live daemons for one worktree),
- a non-monotonic event ID,
- an identity false match across worktrees,
- a red full gate not quarantined as unrelated infrastructure flake.

## Lint posture

Lints are inherited from `[workspace.lints]` in the root `Cargo.toml` (every crate
carries `[lints] workspace = true`), so a worker's local clippy cannot drift from
the orchestrator's `--all-targets` run. `unsafe_code` is denied workspace-wide for
Phase 1; the crate that legitimately needs it later (mmap snapshots) opts in
explicitly and locally.

## Conventions

- Never `git add -A` / `git add .` — stage by exact path.
- Never `--no-verify`, never force-push, never `--amend` to paper over a failed
  hook (the commit didn't happen — make a new one).
- The orchestrator owns commits during a wave; workers do not commit.
