# Cairn — Agent Instructions

Canonical, harness-neutral instructions for this repo. `CLAUDE.md` imports this file; Codex reads it directly.

## What this repo is

Cairn is a greenfield Rust rewrite of Code Briefcase (V1, Python, at `~/Code/code-briefcase`). V1 is retained as behavior spec and acceptance-test corpus (878 tests) and is **never modified from this repo** — different project, different working tree. The V2 rewrite was renamed to Cairn mid-planning.

## The design pin

> *"Cairn is not primarily a repository index. It is a versioned belief-management system for AI agents operating on code."*

A code graph tells you what is currently true about the repo. The harder, more valuable question: what does *this particular agent session* currently believe, and has that belief expired?

**Graph facts are inputs to belief management; they must not become the primary API surface.** Building "expose the graph through MCP" before "expose the ledger through MCP" is the inverted design. Spec §14 restates this in full.

## What to read first

1. **`docs/STATE.md`** — current wave, what's in flight, what's next. Canonical live status; read it before acting.
2. **`CONTEXT.md`** — why decisions were made, and what not to re-litigate.
3. **`docs/Cairn Final Specification.md`** — binding architecture and feature spec. Appendices A (build-order phasing) and B (schema reference) are load-bearing.
4. **`docs/Cairn Implementation Plan.md`** — the 30-wave plan with Patch Round 1 applied. `docs/Cairn Implementation Plan — Patch Round 2.md` carries the current vertical-slice sequencing and the vexp-superset amendment; the two Patch Round 1 files are the decision log.
5. **`docs/history/`** — pre-rename decision archive; says "Code Briefcase" throughout.

## Build and gate

`./scripts/gate.sh` is the canonical gate. It covers both verification surfaces, fails fast on the first red check, and cds to the repo root itself so it runs from anywhere. The orchestrator runs it once at the close of a wave or phase; workers never run it.

There is no root `package.json` — every npm script lives in `site/package.json`, so `npm run <script>` at the repo root can only fail with "Missing script". Use `pnpm --dir site <script>` from the repo root rather than cd-ing; `site/pnpm-lock.yaml` is the lockfile.

Worker light checks and the Phase 1 advance-refusal criteria live in `docs/dev/gates.md`.

## Working in site/

- Dev server: `pnpm --dir site dev`. Production preview: `pnpm --dir site build && pnpm --dir site start`. Both bind port 3000 — check it first with `lsof -nP -iTCP:3000 -sTCP:LISTEN` so an `EADDRINUSE` reads as "someone else is already serving" rather than a broken build.
- `site/node_modules` contains only what `site/package.json` declares. Analysis tools such as `madge` are not declared, so `site/node_modules/.bin/madge` does not exist even when an `npx --no-install madge` probe succeeds against a globally cached copy. Resolve such a tool once and run that exact command; never probe one way and execute another.
- `site/DESIGN_CONTEXT.md` is the standing brief for the explainer site — paste it verbatim into any site subagent, and update it there rather than in scattered briefs.

## How we build

Orchestrator plus a parallel worker swarm plus mandatory per-wave review loops:

1. Read the current wave from `docs/STATE.md` and the matching wave header in the implementation plan.
2. Freeze the wave's contracts, then fan out workers in a single message.
3. Collect results, inspect diffs, run light checks (`cargo check`, `cargo clippy --no-deps`).
4. Dispatch an independent reviewer — a **different model than the implementer**. Fix loop until clean.
5. At phase boundaries: bug hunt and a `desloppify-deep` cleanup pass, per the plan's cadence.
6. At phase end: run `./scripts/gate.sh` once, write the phase summary into `docs/STATE.md`, advance.

The orchestrator plans, dispatches, reviews, integrates, decides, escalates. It does not do most implementation directly.

**Under Codex, fan out with `delegate`, not native subagents** — `collaboration.spawn_agent` fails here with "no thread with id" (five separate occurrences logged in `.papercuts.jsonl`, 2026-07-18). Under Claude Code the `Agent` tool works; useful types are `general-purpose`, `Explore`, `Plan`, `plan-reviewer`, `refactor-pilot`, `worker`, and the `desloppify-*` specialists.

## Skills

Brief every implementer with **`clean-code`** and **`rust-engineer`**. Non-negotiable on every implementation brief. Read `~/.agents/skill-library/<name>/SKILL.md` directly — skill *search* has been slow and unreliable in this repo and has repeatedly aborted compound commands.

Orchestrator-side: `desloppify-deep` at phase boundaries, `debugging-systematic` for bug hunts, `checkpoint` at phase end, `codex-prompting` when writing Codex briefs.

## Worker fleet

Off-Anthropic workers via `delegate` (cost-free against the Anthropic subscription):

| lane | use |
|---|---|
| `delegate codex work` | primary implementation lane |
| `delegate cursor work` | broad mechanical work, repo-wide edits |
| `delegate droid "deepseek v4 pro" work` | cost-per-task default |
| `delegate droid "deepseek v4 flash" work` | fast lane for parallel coverage |
| `delegate droid glm work` | quality implementation, proactive tests |
| `delegate droid grok work` | typed refactors, schema work |
| `delegate droid gemini work` | diverse coverage |

Reviews run in `safe` mode under a different model than the implementer. Check `delegate models` before trusting an alias — lanes here have died on stale aliases, missing Cursor auth, and an exhausted Kimi quota, always after the snapshot was already built. Full strengths table: `~/.agents/skill-library/delegate-agent/SKILL.md`.

## CPU discipline (critical)

N parallel workers each running `cargo test` or `cargo build --release` concurrently has melted machines in real use.

- **Allowed in a worker:** `cargo check`, `cargo clippy --no-deps`, `cargo fmt --check`
- **Forbidden in a worker:** `./scripts/gate.sh`, `cargo test`, `cargo build --release`, `pnpm --dir site build`, benchmarks, E2E tests

State this explicitly in every worker brief. Workers report "I ran `cargo check` and `cargo clippy`; the gate run is the orchestrator's job."

## Git

The orchestrator owns commits and stashes during a wave; workers do not commit unless their brief authorizes it. Remote is `github.com/treygoff24/cairn`, direct-to-main. The global rules still apply — stage by exact name, never force-push, never `--no-verify`, never `--amend` to paper over a failed hook.

## Out of scope for V1 (do NOT add)

- **Privacy / prompt-injection defense machinery.** Single-user local-only product; the threat surface is small enough. The credential-redaction one-liner in the event log is hygiene, not security — that's the only concession.
- **Worktree-per-agent federation as the coordination model.** The V1 coherence domain is one daemon per worktree; federation is P2.
- **Backwards compatibility with the V1 Python implementation.** Greenfield. No shims.
