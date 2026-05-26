# Cairn — Claude Code Project Instructions

This file overrides and extends the global `CLAUDE.md` for work inside this repository.

## What this repo is

Cairn is a greenfield Rust rewrite of Code Briefcase (V1, Python). The V1 product was named "Code Briefcase" and lives at `~/Code/code-briefcase`; it is retained as behavior spec plus acceptance test corpus (878 tests). The V2 rewrite was renamed to **Cairn** mid-planning — same product trajectory, cleaner name. This repo is the V2/Cairn build.

Until the implementation plan lands from GPT-5.5 Pro, this repo is documentation-only. No Rust code yet. The cargo workspace, crate layout, and first wave of implementation tasks will all be defined by the implementation plan.

## The design pin

> *"Cairn is not primarily a repository index. It is a versioned belief-management system for AI agents operating on code."*

A code graph tells you what's currently true about the repo. The harder, more valuable question: what does *this particular agent session* currently believe, and has that belief expired?

Everything follows from that framing. **Graph facts are inputs to belief management; they must not become the primary API surface.** If you find yourself building "let's expose the graph through MCP" before "let's expose the ledger through MCP," you've inverted the design.

The spec's §14 ("The one non-obvious design choice") restates this in full when you forget.

## What to read first

1. **`CONTEXT.md`** — strategic context, decisions locked in, working-relationship cues, things-not-to-relitigate.
2. **`docs/STATE.md`** — current phase, what's in flight, what's pending. The living state of the build.
3. **`docs/Cairn Final Specification.md`** — binding architecture and feature spec. Two appendices: A (build-order phasing) and B (schema reference) are load-bearing.
4. **`docs/Cairn Implementation Plan.md`** — Pro's full 30-wave implementation plan, with Patch Round 1 applied.
5. **`docs/Cairn Implementation Plan — Patch Round 1.md`** and **`docs/Cairn Implementation Plan — Patch Round 1 Decisions.md`** — the patch round and Pro's adjudication, kept as decision log.
6. **`docs/history/`** — decision-rationale archive (Pro language-decision conversation, spec review round 2). Written before the rename — references "Code Briefcase" / "Briefcase" throughout.

## How we build

Orchestrator (Claude Code) plus a parallel worker swarm plus mandatory per-wave review loops. The loop:

1. Orchestrator reads the current phase from the implementation plan.
2. Orchestrator fans out Wave N — multiple `Agent` (Claude Code native subagent) and/or `delegate` (off-Anthropic harness) calls in a single message.
3. Orchestrator collects results, inspects diffs, runs light gate (`cargo check`, `cargo clippy --no-deps`).
4. Orchestrator dispatches an independent reviewer — different model than the implementer. Reviewer produces a fix list. Orchestrator dispatches fixes. Loop until clean.
5. Repeat for Wave N+1.
6. At scheduled phase boundaries (per the plan's cadence): orchestrator runs bug hunt (`debugging-systematic` + `diagnose` skills) and/or `desloppify-deep` (8-subagent cleanup pass).
7. At phase end: orchestrator runs the full gate **once**, writes a phase summary, advances.

The orchestrator does not do most implementation directly. The orchestrator's job is to plan, dispatch, review, integrate, decide, escalate.

## Always-on skills for implementation tasks

Brief every implementer worker with:

- **`clean-code`** — small functions, intention-revealing names, no slop comments, no defensive code without a real boundary
- **`rust-engineer`** — idiomatic Rust, memory safety, zero-cost abstractions

These are non-negotiable on every implementation brief.

## Phase-boundary and orchestrator skills

- **`desloppify-deep`** — 8-subagent parallel cleanup at scheduled phase boundaries per the implementation plan's cadence
- **`debugging-systematic`** + **`diagnose`** — bug hunts at scheduled checkpoints
- **`finishing-a-development-branch`** — phase-end integration discipline
- **`checkpoint`** — force a progress checkpoint at phase end
- **`parallel-subagent-discipline`** — read before fanning out workers (CPU discipline)
- **`codex-prompting`** — when crafting Codex worker briefs (Codex will drive a large share of execution)

## Worker fleet

Off-Anthropic workers via `delegate` (cost-free against the Anthropic subscription):

- `delegate cursor work` — broad implementation, mechanical work, repo-wide edits
- `delegate codex work` — Codex execution. Strong implementation lane when you want Codex-native review or work.
- `delegate droid "deepseek v4 pro" work` — Pareto-frontier cost-per-task default
- `delegate droid "deepseek v4 flash" work` — fast lane for parallel coverage
- `delegate droid glm work` — quality implementation, proactive tests
- `delegate droid grok work` — typed refactors, schema work
- `delegate droid gemini work` — diverse coverage

For reviews, use a **different model than the implementer**. Common pairings:
- Implementer = `cursor work` → Reviewer = `delegate codex safe` or `delegate droid "deepseek v4 pro" safe`
- Implementer = `codex work` → Reviewer = `delegate cursor safe` or `delegate droid glm safe`
- Implementer = `droid deepseek` → Reviewer = `delegate codex safe`

Full delegate model strengths in `~/.claude-personal/skills/delegate-agent/SKILL.md`.

Claude Code native subagents via the `Agent` tool — relevant types: `general-purpose`, `Explore`, `Plan`, `plan-reviewer`, `refactor-pilot`, `code-simplifier`, `desloppify-deep` (and its 8 specialists), `worker`.

## CPU discipline (critical)

Workers **MUST NOT** run heavy gates. N parallel workers each running `cargo test` or `cargo build --release` concurrently has melted machines in real use. The orchestrator runs the gate **once** at the end of a wave/phase.

- Allowed inside a worker: `cargo check`, `cargo clippy --no-deps`, `cargo fmt --check`
- Forbidden inside a worker: `cargo test`, `cargo build --release`, large benchmark runs, E2E tests

State this discipline explicitly in every worker brief. Workers should report "I ran `cargo check` and `cargo clippy`, gate run is the orchestrator's job."

## Git discipline

- Never `git add -A` or `git add .` — stage files by exact name.
- Never force-push, never `--no-verify`, never `--amend` to "fix" a failed pre-commit hook (the commit didn't happen; create a new one).
- The orchestrator owns commits and stashes during a wave. Workers do not commit unless explicitly authorized in their brief.
- No remote yet. Repo will get pushed to `github.com/treygoff24/cairn` (TBC) once Trey sets it up.

## V1 reference

The V1 Python repo at `~/Code/code-briefcase` is the behavior spec.

- 878 tests passing, ruff + mypy clean, latest substantive work was items 1–8 hook ergonomics fixes plus watch-diagnostics stabilization.
- The V1 tests are the **acceptance test corpus** for V2. They represent behavior V2 must match in spirit, not implementation. Port to Rust integration tests per the implementation plan's V1-corpus-inheritance strategy.
- **Do not modify V1 from this repo.** Different project, different working tree.

## Current phase

The implementation plan has landed and Patch Round 1 is applied. The plan lives at `docs/Cairn Implementation Plan.md`. The next step is Wave 1.0 (Phase 1 foundation-risk premortem, orchestrator-owned, no worker fan-out), then the Wave 1.1 bootstrap prelude commit (workspace skeleton + empty crate dirs + compile-only stubs), then the six-worker Wave 1.1 dispatch per the plan's §5.

When resuming work in a new session, read STATE.md first to find the current wave, then jump to the matching wave header in the implementation plan.

## Out of scope for V1 (do NOT add)

- Privacy / prompt-injection defense machinery. Single-user local-only product, threat surface is small enough. The credential-redaction one-liner in the event log is hygiene, not security; that's the only concession.
- Worktree-per-agent federation as the coordination model. V1 coherence domain is one daemon per worktree; federation is P2.
- Backwards compatibility with V1 Python implementation. Greenfield. Don't introduce shims.
