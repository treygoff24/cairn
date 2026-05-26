# Code Briefcase V2 — Claude Code Project Instructions

This file overrides and extends the global `CLAUDE.md` for work inside this repository.

## What this repo is

Code Briefcase V2 is a greenfield Rust rewrite of Code Briefcase. The V1 (Python) lives at `~/Code/code-briefcase` and is retained as behavior spec plus acceptance test corpus (878 tests). This repo is the V2 build.

Until the implementation plan lands from GPT-5.5 Pro, this repo is documentation-only. No Rust code yet. The cargo workspace, crate layout, and first wave of implementation tasks will all be defined by the implementation plan.

## The design pin

> *"Code Briefcase is not primarily a repository index. It is a versioned belief-management system for AI agents operating on code."*

A code graph tells you what's currently true about the repo. The harder, more valuable question: what does *this particular agent session* currently believe, and has that belief expired?

Everything follows from that framing. **Graph facts are inputs to belief management; they must not become the primary API surface.** If you find yourself building "let's expose the graph through MCP" before "let's expose the ledger through MCP," you've inverted the design.

The spec's §14 ("The one non-obvious design choice") restates this in full when you forget.

## What to read first

1. **`CONTEXT.md`** — strategic context, decisions locked in, working-relationship cues, things-not-to-relitigate.
2. **`docs/STATE.md`** — current phase, what's in flight, what's pending. The living state of the build.
3. **`docs/Code Briefcase V2 Final Specification.md`** — binding architecture and feature spec. Two appendices: A (build-order phasing) and B (schema reference) are load-bearing.
4. **`docs/v2-implementation-plan.md`** — Pro's implementation plan (existence depends on STATE.md — placeholder until Pro responds).
5. **`docs/history/`** — decision-rationale archive. Read on demand when a question arises about why we picked something.

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
- No remote yet. Repo will get pushed to `github.com/treygoff24/briefcase` (TBC) once Trey sets it up.

## V1 reference

The V1 Python repo at `~/Code/code-briefcase` is the behavior spec.

- 878 tests passing, ruff + mypy clean, latest substantive work was items 1–8 hook ergonomics fixes plus watch-diagnostics stabilization.
- The V1 tests are the **acceptance test corpus** for V2. They represent behavior V2 must match in spirit, not implementation. Port to Rust integration tests per the implementation plan's V1-corpus-inheritance strategy.
- **Do not modify V1 from this repo.** Different project, different working tree.

## When Pro's implementation plan arrives

1. Paste the response to `docs/v2-implementation-plan.md` (replace the placeholder).
2. Update `docs/STATE.md`.
3. Read the plan critically — check for: ledger-first build order, parallelism per phase, review-loop integration in every wave, skill-to-task mapping, opinionated answers to the 10 forced decisions from the brief, first-sprint kickoff that's literally ready to dispatch.
4. Surface the read to Trey for collaborative review.
5. Patch the plan if needed (possibly another round with Pro).
6. Kick off Phase 1, Wave 1.

## Out of scope for V1 (do NOT add)

- Privacy / prompt-injection defense machinery. Single-user local-only product, threat surface is small enough. The credential-redaction one-liner in the event log is hygiene, not security; that's the only concession.
- Worktree-per-agent federation as the coordination model. V1 coherence domain is one daemon per worktree; federation is P2.
- Backwards compatibility with V1 Python implementation. Greenfield. Don't introduce shims.
