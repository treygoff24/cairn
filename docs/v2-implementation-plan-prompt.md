# Code Briefcase V2 — Implementation Plan Brief for GPT-5.5 Pro

> **Archive note (post-rename):** this is the brief that was sent to Pro before the project was renamed to **Cairn**. Content is preserved verbatim as a historical artifact of what Pro received. All references to "Code Briefcase / Briefcase" in this file map to what is now called Cairn. The current binding plan is `Cairn Implementation Plan.md`.

You are receiving two attachments alongside this brief:

1. **`Code Briefcase V2 Final Specification.md`** — the architecture and feature spec for Code Briefcase V2. Treat this as binding ground truth. You produced the first-pass and second-pass versions of it; the attached file is the resolved final.
2. **`v2-skills-library-attachment.md`** — the full library of named skills available to the agents who will execute your plan. 152 skills with one-to-two sentence descriptions. Treat this as a vocabulary: when you assign a task, you should reference skills by exact name from this library.

Your job is to turn the spec into a **concrete, executable, phase-by-phase implementation plan** that the team can fan out against immediately. This is not a generic milestone breakdown. It must reflect *how this team actually builds*: a coordinator (Claude Code) orchestrating parallel work across a heterogeneous swarm of agents, with mandatory per-wave review loops, periodic bug hunts, and codebase cleanup passes baked in.

---

## What is non-negotiable about this plan

The plan must:

- **Maximize parallelism.** At every phase, identify every task that does not depend on another task in the same phase, and assign them to run concurrently as a "wave." Sequential work is the exception that requires a justification.
- **Bake in review loops.** Every wave ends with an independent-reviewer pass by a different model than the implementer. Every review produces a fix-list. Every fix-list goes back to a worker (often the original implementer; sometimes a fresh model).
- **Include periodic bug hunts.** At checkpoints you choose (use your judgment on cadence — somewhere between "every phase" and "every 2 phases"), the orchestrator runs a systematic bug-hunt pass with Claude Code using the `debugging-systematic` and `diagnose` skills. The hunt produces a fix-list; the fix-list is dispatched to workers.
- **Include periodic deslopify passes.** Use the `desloppify-deep` skill at checkpoints you choose. The desloppify-deep skill spawns 8 specialist subagents in parallel across orthogonal cleanup axes (dedup, types, dead-code, cycles, weak-types, defensive, legacy, comments).
- **Apply `clean-code` continuously.** Every implementation task must be briefed with `clean-code` as an always-on skill. The clean-code skill is small functions, intention-revealing names, no slop comments, no defensive try/catch where no boundary exists. Cite it explicitly in every worker brief.
- **Specify skills per task.** Every task in the plan names a primary skill, secondary skill if relevant, and the always-on skill stack. Reference skills by exact name from the attached skill library.
- **Be executable, not aspirational.** A worker should be able to read a single task entry in your plan and know: what files they own, what skills to load, what acceptance criteria the work must meet, how their work will be reviewed, and what they hand off when they're done.

If a section of your plan is generic, rewrite it. If a wave has only one task, ask whether you've decomposed the phase finely enough.

---

## The execution architecture you are planning around

### The orchestrator: Claude Code (Anthropic, Opus)

This is the agent reading your plan and executing it. The orchestrator's role:

- Reads the current phase of your plan
- Fans out a wave: spawns workers via `Agent` (Claude Code native subagents) or `delegate` (off-Anthropic harnesses)
- Receives results, inspects diffs, runs the project gate
- Spawns the review pass
- Integrates fixes
- Runs bug hunts and deslopify passes at the cadence your plan specifies
- Advances to the next phase when acceptance criteria are met

The orchestrator does *not* do most implementation work itself. Its job is to plan, dispatch, review, integrate, and decide. When the orchestrator does write code directly, it's because the task is small, contextual, or needs mid-stream judgment that cannot be handed off cleanly.

### Worker fleet 1: Claude Code native subagents (the `Agent` tool)

These run inside Claude Code, share its Anthropic budget, and have full access to the project. Available `subagent_type` values relevant to V2:

- `general-purpose` — open-ended research, multi-step investigation, broad codebase searches. Use when a task needs judgment and exploration before producing an answer.
- `Explore` — fast read-only search agent. Use when you need to locate code or answer "where is X" without flooding the orchestrator's context.
- `Plan` — software-architect agent that designs implementation plans for a bounded task. Use when a task surfaces a sub-design problem too big for inline thinking.
- `plan-reviewer` (opus) — adversarial fresh-context plan review. Use *immediately after* any plan or sub-plan is drafted, before execution. This is the per-wave review primitive for plans.
- `refactor-pilot` — behavior-preserving refactors across small dependency-ordered commits. Runs the project gate between each commit. Refuses schema work. Use for renames, dedup, extraction, simplification within an existing codebase.
- `code-simplifier` — simplification pass on recently modified code while preserving behavior. Use after a wave to tighten what just landed.
- `desloppify-deep` and its 8 specialists (`desloppify-dedup`, `desloppify-types`, `desloppify-dead-code`, `desloppify-cycles`, `desloppify-weak-types`, `desloppify-defensive`, `desloppify-legacy`, `desloppify-comments`) — the parallel cleanup fleet. The `desloppify-deep` skill itself is the coordinator that fans out all 8. Use at phase boundaries (see cadence guidance below).
- `worker` — generic implementation worker for one bounded step. Use when a sub-task needs an isolated context but is too small for `general-purpose`.

### Worker fleet 2: the `delegate` CLI

This shells out to non-Anthropic models. The orchestrator pays no Anthropic cost for these workers, so they're useful for large parallel batches. Modes:

- **`work`** — implementation mode; edits the real workspace (or an isolated worktree if `--isolation worktree` is passed). Always inspect diffs after.
- **`safe`** — read-only review mode. Cursor and Codex safe modes run in an isolated temporary workspace copy. Droid safe modes run read-only against the real workspace.

Available models and their strengths (verbatim guidance from the team's delegate-agent skill):

| Alias | Mode strengths |
|---|---|
| `cursor work` / `cursor safe` | Cursor Composer 2.5. Fastest and cheapest broad implementation lane. Strong for backend cleanups, mechanical touch-list work, repo-wide edits. Can add defensive-noise code. |
| `codex work` / `codex safe` | OpenAI Codex CLI. Strong implementation lane when you want Codex-native review or work. Defaults to `--sandbox workspace-write` in work mode. |
| `droid "deepseek v4 pro" work/safe` | DeepSeek V4 Pro. Current Pareto-frontier default for cost-per-task. Very strong coding/reasoning quality at extremely low cost. Use for complex implementation, bug hunts, review-fix loops. Slower than Flash. |
| `droid "deepseek v4 flash" work/safe` | DeepSeek V4 Flash. Fast lane with modest intelligence tradeoff vs Pro. Use for investigation, straightforward implementation, cleanup, review-fix, parallel coverage where speed matters. |
| `droid glm work/safe` | GLM 5.1. High-quality implementation and review-fix. Good at clean abstractions and data-driven changes. Historically more likely to add tests proactively. Slow on medium lanes. |
| `droid grok work/safe` | Grok 4.3. Good for classifier/schema work, typed refactors, `satisfies` pins, explicit extraction. Can be expensive on small fixes. |
| `droid gemini work/safe` | Gemini 3.5 Flash. Useful across implementation, cleanup, review-fix, investigation. Good for model-diverse coverage. Review carefully for shallow fixes on complex repo-specific behavior. |
| `droid kimi work/safe` | Kimi K2.6. Alternate implementation/review lane for model diversity. Less calibrated; review carefully. |
| `droid qwen work/safe` | Qwen 3.7 Max. Strong general coding/reasoning lane. OpenRouter routing/cost/latency can vary. |
| `droid minimax safe` | MiniMax M2.7. Cheap read-only investigation. Treat as backup-tier for substantial implementation. |
| `droid mimo work/safe` / `droid "mimo pro"` | MiMo V2.5 / V2.5 Pro. Additional diversity lanes; less proven. |

Use **`delegate cursor work`** for the broad-edits, mechanical-touch-list lane. Use **`delegate codex work`** when you want Codex execution (the user explicitly noted Codex will drive a large portion of execution). Use **`delegate droid "deepseek v4 pro" work`** as the cost-efficient parallel-worker default. Use **`delegate droid "deepseek v4 flash" work`** when you need many workers in parallel and the work is more mechanical. Reserve **`delegate droid grok work`** for the typed-refactor and schema lanes (relevant for the Salsa wiring, the SurfaceItem schema, the DenyDecision schema, capability bitset).

For *reviews*, use a **different model than the implementer**. Common combinations the orchestrator can choose between:

- Implementer = `cursor work` → Reviewer = `delegate codex safe` or `delegate droid "deepseek v4 pro" safe`
- Implementer = `codex work` → Reviewer = `delegate cursor safe` or `delegate droid glm safe`
- Implementer = `droid deepseek` → Reviewer = `delegate codex safe`

Specify the review pairings in the plan.

### Worker fleet 3: the orchestrator itself

Sometimes the orchestrator's own context, mid-stream judgment, or position as integrator makes it the right implementer. The plan should explicitly mark tasks where the orchestrator should do the work directly rather than delegating. Examples likely to need orchestrator-direct work: integration points between waves, ambiguous architectural calls, hot-path Rust micro-optimization, and the final integration commits.

### Worker fleet 4: GPT-Pro (you), at later stages

Once the implementation is in flight, you may be re-engaged for two things:

- **Strategic re-planning** when a phase reveals that the spec or plan needs revision.
- **Second-opinion review** at major checkpoints — particularly at the end of Phase 4 (minimal graph), Phase 5 (hooks/MCP), and Phase 7 (extension).

Your plan should explicitly name these checkpoints and what artifacts should be sent back to you for review.

---

## How the orchestrator runs your plan — the loop

For every phase in your plan:

1. **Phase kickoff.** Orchestrator reads the phase, validates dependencies on prior phases, confirms gate is green.
2. **Wave 1 fan-out.** Orchestrator dispatches all tasks in Wave 1 in parallel — single message, multiple `Agent`/`delegate` calls. Worker briefs are derived from your plan's task entries.
3. **Wave 1 collection + integration.** Orchestrator collects results, inspects diffs, resolves cross-task collisions, runs the light gate (`cargo check`, `cargo clippy --no-deps`).
4. **Wave 1 review loop.** Orchestrator dispatches an independent reviewer (per your plan's review pairing). Reviewer produces a fix-list. Orchestrator dispatches fixes (often back to the original implementer with the fix-list as input). Loop until reviewer signs off OR the orchestrator decides to escalate to user.
5. **Wave 2 fan-out.** Repeat 2–4 for each subsequent wave in the phase.
6. **Phase bug hunt** (if your plan schedules one at this boundary). Orchestrator uses `debugging-systematic` and `diagnose` skills inside Claude Code, runs targeted test sweeps, produces a bug-list, dispatches fixes.
7. **Phase deslopify pass** (if your plan schedules one at this boundary). Orchestrator invokes the `desloppify-deep` skill, which spawns the 8 specialist subagents in parallel. Coordinator runs the full gate once at the end.
8. **Phase gate.** Orchestrator runs the full gate (`cargo test`, `cargo clippy`, `cargo fmt --check`, any phase-specific benchmarks). If red, identify which wave broke and reverse-trace.
9. **Phase summary.** Orchestrator writes a one-pager: what shipped, what's in flight, what's deferred to next phase, current metrics against acceptance criteria.
10. **Advance.** If acceptance criteria met, advance to next phase. If not, decide whether to extend the phase or escalate to user.

Your plan must support this loop — every wave must have an unambiguous "done" signal, every phase must have unambiguous acceptance criteria, and every checkpoint must have a clear "what gets dispatched here" definition.

---

## Specific guidance on skills

The attached skill library is 152 entries. The plan should reference skills by exact name. Below are the skills most likely to be relevant; you may add others from the library where appropriate.

### Always-on (load before any implementation task)

- **`clean-code`** — small functions, intention-revealing names, no slop comments, no defensive code without a real boundary. Brief every implementation worker with this.
- **`rust-engineer`** — writes, reviews, debugs idiomatic Rust. Memory safety, zero-cost abstractions, ownership patterns. Brief every Rust-implementing worker with this.

### Phase-boundary checkpoints

- **`desloppify-deep`** — the 8-subagent parallel cleanup pass. Run at phase boundaries you select. Note: it is gated on a green baseline gate and a clean working tree. Plan for it accordingly.
- **`debugging-systematic`** — systematic root cause analysis. Run at phase-boundary bug hunts.
- **`diagnose`** — diagnose-only investigation without making changes. Use before dispatching fixes to specify exactly what should change.
- **`spec-quality-checklist`** — validate a sub-spec before implementation. Use when a phase produces a sub-design that needs scrutiny.
- **`finishing-a-development-branch`** — phase-end integration discipline. Use at phase boundaries when integrating.

### Per-task secondary skills (assign where relevant)

- **`mcp-builder`** — for the MCP server crate work (Phase 5+).
- **`tdd-workflow`** — for test-driven new features. Brief workers with this when the deliverable is a new module with clear test surface.
- **`request-refactor-plan`** — when a worker's task surfaces a refactor too large to do inline; spawns a planning step.
- **`refactor`** / **`improve-codebase-architecture`** — for refactor lanes within phases.
- **`codex`** / **`gemini`** — for second-opinion lanes via delegate.
- **`codex-prompting`** — when crafting prompts for Codex tasks (relevant since Codex will drive a large portion).
- **`using-git-worktrees`** — for isolating parallel implementation lanes within a wave. Particularly useful when two workers might touch overlapping files.
- **`premortem`** — before risky phases. Use as a 30-minute exercise before Phase 1 (foundations) and Phase 5 (adapter rollout).
- **`parallel-subagent-discipline`** — required reading for the orchestrator before each fan-out wave. Caps concurrency so we don't melt the user's machine.
- **`bootstrap`** — generate a project-specific `CLAUDE.md` once the cargo workspace exists.
- **`checkpoint`** — force a progress checkpoint at the end of each phase.
- **`create-handoff`** / **`resume-handoff`** — for cross-session continuity if implementation spans multiple sessions.

### Skill-to-task mapping example (the form your plan should use)

> **Task: `briefcase-identity` crate — implement canonical project/worktree identity.**
> Owner: `delegate codex work`
> Skill stack: `rust-engineer`, `clean-code`, `tdd-workflow` (write tests first)
> Owned files: `crates/briefcase-identity/**`
> Dependencies: none (this is Wave 1)
> Acceptance: identity resolution is deterministic across runs; canonical-path edge cases (symlinks, case-insensitive filesystems, bind mounts) have explicit test coverage; `cargo test -p briefcase-identity` passes.
> Reviewer: `delegate droid "deepseek v4 pro" safe` with `clean-code` skill loaded
> Review brief: verify schema matches spec §3 Axis 9; check edge case coverage; check public API ergonomics for downstream crates.

Every task in your plan must be this concrete or more.

---

## Constraints

### CPU discipline (this is operationally critical)

When fanning out N parallel workers, each running heavy gates (test suite, build) simultaneously can melt a developer machine. The team's `desloppify-deep` skill encodes this discipline:

- Subagents must not run the heavy gate (`cargo test`, `cargo build --release`, `pytest`).
- The orchestrator runs the gate **once**, at the end of a wave/phase, after all workers have returned.
- Workers may run lightweight checks: `cargo check`, `cargo clippy --no-deps`, `cargo fmt --check`.

Your plan should explicitly state this discipline in worker briefs and acceptance criteria. Workers report "I ran `cargo check` and `cargo clippy`, gate run is the orchestrator's job."

### Git discipline

- No `git add -A`, no `git add .`. Stage files by exact name to avoid sweeping up untracked junk.
- No force-push, no `--no-verify`, no `--amend` to "fix" failed pre-commit hooks (the commit didn't happen; create a new commit).
- No `git stash` during a wave — the orchestrator owns stashing decisions.
- Commits are made by the orchestrator, not by workers (unless explicitly authorized in the task brief).

### Workspace layout discipline

- Greenfield Rust workspace. No Python compatibility shims. V1 Python repo (`code-briefcase`) is the behavior spec, not a port target.
- Cargo workspace with one crate per major architectural component. The spec implies roughly 15+ crates; you should specify the exact crate layout in your plan, with dependency edges.
- Each crate has a single owner during a wave to prevent collision. Cross-crate edits go through a "boundary adjudicator" (the orchestrator).

### Test corpus inheritance

- The V1 Python repo at `~/Code/code-briefcase` contains 878 tests. These represent behavior the V2 must match in spirit (not in implementation detail). Treat them as an acceptance corpus to be ported as Rust integration tests, not as a literal port target. The plan should specify when this porting happens (likely starting in Phase 2 or Phase 3 as the ledger machinery becomes testable).

### Adapter strategy

- Three primary adapters: Claude Code, Codex CLI, Cursor. The plan should specify the order (parallel? sequential? Claude Code first?) and the capability matrix coverage per adapter. The user's primary harness is Claude Code, so Claude Code adapter should be the first one to reach Tier 1 (enforcing) capability per spec §3 Axis 11.

### Benchmark harness

- Per spec §10 P1, the benchmark harness ships as a product feature. Your plan should specify when its scaffolding starts (likely Phase 4 or 5, parallel with the first hooks landing) so we don't ship the substrate without an instrument to evaluate it.

### Latency POC

- The spec's §4 latency table is tagged as targets pending POC validation. Your plan should include the POC as an early deliverable (likely Wave 1 of Phase 4 or earlier). The POC validates the Rust + Salsa + mmap-snapshot + arena stack against the 35 ms p95 `Read` decoration budget on a representative repo.

---

## Output structure your plan must follow

```
# Code Briefcase V2 Implementation Plan

## 1. Executive summary
3 paragraphs:
  - what this plan delivers (V2 P0 substrate end-to-end)
  - the parallelism model in one sentence (sub-agent swarm with mandatory review loops)
  - the build sequence in one sentence (7 phases mapped from spec Appendix A, with N waves per phase)

## 2. Cargo workspace layout
Concrete crate list with dependencies.
For each crate:
  - Name
  - Purpose
  - Public API surface (one sentence)
  - Dependencies on other crates
  - Suggested owner model for initial implementation
  - Whether it's load-bearing (P0 substrate) or extension (P1+)

## 3. Cross-cutting strategies
  - 3.1 Test corpus inheritance from V1
  - 3.2 Latency POC
  - 3.3 Benchmark harness scaffolding
  - 3.4 Adapter rollout strategy
  - 3.5 CI strategy (gates, parallelism, caching)
  - 3.6 Cross-platform strategy (macOS/Linux first, Windows later or in-stride)

## 4. Phase-by-phase implementation
For each of the 7 phases from spec Appendix A:

### Phase N — <name>
  - Goal (one paragraph)
  - Dependencies on prior phases
  - Acceptance criteria (verifiable, mechanical)
  - Wave breakdown:
    Wave N.1
      - Task 1: <name>
        - Owner: <model>
        - Skill stack: <always-on + primary + secondary>
        - Owned files: <glob>
        - Dependencies within wave: none (by construction)
        - Acceptance: <verifiable>
        - Reviewer pairing: <different model + skills>
        - Review brief shape: <one paragraph>
      - Task 2: ...
      - ... typically 4-8 tasks per wave
    Wave N.2
      - ...
  - Phase checkpoints:
    - Bug hunt: yes/no, owner = orchestrator + Claude Code, skills = debugging-systematic + diagnose
    - Deslopify pass: yes/no, owner = orchestrator + desloppify-deep skill
    - Strategic re-review by GPT-Pro: yes/no, deliverable = <what to send back>
  - Phase gate: cargo test + cargo clippy + cargo fmt --check + <phase-specific benchmarks>

## 5. First sprint kickoff
The literal first wave: what the orchestrator dispatches in the first message of the build. Should be ready to execute against without re-planning.

## 6. Risks and mitigations specific to this execution model
  - Worker collision risk and mitigation (file ownership + worktree isolation)
  - Reviewer-implementer model collusion (use diverse model fleet)
  - Phase-gate latency (if Phase N gate takes 30+ min, parallelism dies)
  - Spec drift during implementation (when to re-engage GPT-Pro)
  - Specific risks from spec §13 that touch the execution model

## 7. Checkpoints for GPT-Pro re-engagement
Specific moments where I should be brought back in:
  - End of Phase 4: review of minimal-graph integration before hooks land
  - End of Phase 5: review of adapter rollout
  - End of Phase 7: pre-P1 review
  - Any time a phase's acceptance criteria require a spec revision

## Appendix A: Skill activation matrix
Table: phase × skill × frequency
(e.g., clean-code applies always; desloppify-deep applies at end of phases 2/4/6; debugging-systematic at end of phases 3/5/7)
```

---

## Be opinionated about these choices

Your plan should pick a side on each of these (do not leave them ambiguous):

1. **Crate count and granularity.** The spec implies ~15 crates. Be specific. Name them.
2. **Wave granularity.** What's the right size for a wave? 4 tasks? 8 tasks? Justify.
3. **Deslopify cadence.** Every phase? Every other phase? After specific risky phases? Justify.
4. **Bug hunt cadence.** Where exactly?
5. **First adapter target.** Claude Code first, or three-in-parallel? Justify.
6. **Benchmark harness timing.** Phase 4? Phase 5? Parallel with Phase 1?
7. **V1 test corpus porting strategy.** Big-bang in Phase 2, or incremental per crate?
8. **Codex's share of execution.** The user noted Codex will drive a large portion. Where does Codex own most of the work?
9. **What to do when a reviewer disagrees with an implementer.** Specify the escalation path.
10. **When the orchestrator should refuse to advance to the next phase.** Specific gate failures vs flaky-test-tier failures.

---

## What you should not do

- Do not produce a generic phase breakdown that could apply to any Rust project. This plan is for *this* product, *this* spec, *this* team.
- Do not assume sequential build. The premise is parallelism.
- Do not skip the review / bug-hunt / deslopify loops. They are first-class structural elements, not nice-to-haves.
- Do not recommend a single model for all tasks. The diverse fleet exists for diversity in review and parallelism in implementation; use it.
- Do not repeat the spec back to me. Reference it by section (e.g., "per spec §10 P0 — see Identity substrate"). I have the spec.
- Do not over-elaborate on what "the user wants" or "what success looks like" in prose — give me task-level concrete entries.
- Do not pad the plan to look thorough. A 1500-line precise plan beats a 3000-line padded one.

---

## What good looks like

When the orchestrator opens your plan to start the build, they should be able to:

1. Read Phase 1 in five minutes.
2. Translate Wave 1.1's task entries into 4–8 `Agent` and `delegate` tool calls in a single message.
3. Know exactly what acceptance criteria each worker must meet.
4. Know exactly which reviewer model to dispatch next.
5. Know what the bug hunt and deslopify cadence is for this phase.
6. Know what advancing to Phase 2 requires.

If any of those is ambiguous, the plan isn't done.

---

## Attachments

- `Code Briefcase V2 Final Specification.md` — the spec
- `v2-skills-library-attachment.md` — the skill library (152 entries)

When you reference skills, use the exact names from the library. When you reference spec sections, cite them by number (e.g., "spec §3 Axis 9", "spec Appendix A Phase 1").
