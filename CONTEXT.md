# Cairn — Strategic Context

The spec is the *what*. This is the *why*. Read this when you need to remember why a decision was made, when a question feels like it might re-open a settled call, or before you push back on a locked-in choice.

---

## Origin story (compressed)

1. **V1 Python** lives at `~/Code/code-briefcase` — working tool for context-decorating AI coding agent sessions. Hooks layer + per-project daemon + tree-sitter indexer + diagnostics watch loop. Functional but quality drifted, with known gaps.
2. **Eval against the llm-council repo** surfaced a TSX nav-map silent failure on large files plus 7 token-efficiency issues. Items 1–8 fixed via Cursor + Opus subagents, merged to V1 main.
3. **Competitor evaluation** of [CodeGraph](https://github.com/colbymchenry/codegraph) (TS, MCP-only, 27k stars, indexer-better-than-ours) and [codedb](https://github.com/justrach/codedb) (Zig, atomic edits, sub-ms warm queries). Neither replaces V1 — they lack the push-mode hooks + diagnostics-in-the-loop differentiator. CodeGraph's indexer is better than V1's, but the *moat* is the substrate, not the index.
4. **Language decision exercise** with GPT-5.5 Pro. Pro recommended Rust with the killer argument: *"hook startup latency is product surface area."* Push-mode hooks fire constantly; TypeScript's 50ms cold start is structurally wrong for the hot path. Conceded with substance.
5. **Greenfield rewrite decision.** V1 (Python product called "Code Briefcase") stays as behavior spec plus acceptance test corpus. V2 starts clean as a Rust rewrite. Originally V2 inherited the "Briefcase" name; renamed to **Cairn** mid-planning when the BeeWare Briefcase conflict and the better metaphor-fit became clear. The cairn-as-trail-marker metaphor maps the design pin directly: each agent passes through a project, drops cairns (observations, edits), and the next agent sees what's been marked.
6. **Multi-agent breakthrough.** During the V2 spec brainstorm, Trey saw that the same primitives that solve single-agent context decoration *also* solve well-known multi-agent problems (stale-context edits, semantic obsolescence mid-flight, redundant CPU thrashing, no cross-agent awareness). The MVP mechanism: at each agent's pre-edit hook, check whether any file in the static dep-graph of the target has been modified by a *different agent session* since this session's last relevant observation; if yes, deny (where harness supports) or advise. This shifted product positioning from "context substrate" to "context substrate + coordination layer."
7. **GPT Pro candidate spec.** Three-option per macro axis structure. Reviewed, found 12 architectural gaps.
8. **GPT Pro follow-up review.** Pro returned with all 12 questions answered concretely, plus 5 things we missed in §14 (TOCTOU race, generated/vendor file policy, task identity as first-class, plus prompt-injection and privacy retention which we cut as not applicable for a local-only single-user product). Spec patched to final.
9. **Implementation plan delivered and patched.** GPT-5.5 Pro delivered the full plan (`docs/Cairn Implementation Plan.md`). Patch Round 1 dispatched a critical-read delta back; Pro adjudicated (`docs/Cairn Implementation Plan — Patch Round 1 Decisions.md`); all accepted patches plus 3 added patchlets applied. Plan is coherent and ready for Wave 1.1 dispatch.
10. **Renamed to Cairn.** Before the initial commit landed on remote, the project rebranded from "Code Briefcase / Briefcase" to **Cairn**. Crate prefix `cairn-*`, binary `cairn`, MCP tools `cairn_*`. The history archives in `docs/history/` were written before the rename and still reference Briefcase — intentionally not rewritten.

## The non-obvious design choice (the framing)

> *"Cairn is not primarily a repository index. It is a versioned belief-management system for AI agents operating on code."*

Most reviewers will think this is a better code index. That's incomplete. A code graph tells you what is currently true about the repository. The harder and more valuable question: what does *this particular agent session* currently believe, and has that belief expired?

That distinction unlocks everything:

- **Single-agent context savings** — the ledger knows what the agent has already been shown; emit deltas, not repeated nav maps.
- **Continuation** — the ledger knows what changed since the agent last worked.
- **Diagnostics** — edit attempt, file version, post-edit result, repair context all connected.
- **Multi-agent coordination** — α's observed version of `route.ts` and its dependency set can be compared against β's later changes.
- **Subagent inheritance** — child belief is auditable and temporally frozen at spawn, with origin preserved.
- **Auto-research** — scoring which context frames actually saved work and which were ornamental fog.
- **TOCTOU safety** — precondition hashes attached to allow decisions, checked at write time.

A graph without an observation ledger is a clever librarian shouting facts into the room. A graph with an observation ledger becomes a flight recorder, air-traffic radar, and memory palace in one local machine.

---

## Locked-in decisions (do NOT re-litigate)

1. **Greenfield in Rust.** Not a port. Behavior spec from V1, no compatibility shims.
2. **Repo naming.** `cairn` (short, what you type). Crates = `cairn-*`. Binary = `cairn`. Marketing = "Cairn." V1 repo at `~/Code/code-briefcase`.
3. **Heavy crate decomposition from day one.** Roughly 15+ crates per the spec; exact list will be in the implementation plan. Heavy decomposition is a feature, not a tax — because the build model is a parallel sub-agent swarm and each crate is a unit of work owned by one worker.
4. **Salsa as incremental engine.** Pinned, wrapped behind `cairn-incremental` crate so swap blast radius is one crate.
5. **MCP is an adapter, not the substrate.** Daemon has its own typed binary protocol (MessagePack/CBOR/postcard, length-prefixed, versioned). MCP server proxies into the daemon. Internal IPC: Unix sockets / named pipes.
6. **Storage: three layers.** SQLite WAL for durable truth; mmap snapshot for cold-start speed; in-memory arenas for sub-ms hot serving.
7. **One shared daemon per project worktree.** All agent sessions attach. Identified by canonical root + git worktree + config hash + protocol version.
8. **Multi-agent coordination is load-bearing.** The product is both a context substrate *and* a coordination layer. MVP enforcement is dep-graph staleness check at pre-edit with contract-fingerprint gating.
9. **Auto-research loop is the planned post-launch improvement model.** Karpathy pattern: constraint + mechanical metric + autonomous iteration. Implication: every feature needs a measurable numerical scalar (baked into spec §12).
10. **Edit safety is advisory at baseline, enforced where harness allows.** Hook protocol limits us; we don't pretend otherwise. File leases are a wild/P2 option, not baseline.
11. **TOCTOU verification.** Every edit decision carries an `expected_target_file_hash` precondition where the adapter supports it. Daemon verifies post-edit. `edit_race_detected` event if not.
12. **Ledger-first build order.** Identity substrate → ObservationLedger/EditLedger → ContextFrame ledger + scheduler skeleton → minimal graph → hooks/MCP → diagnostics/framework extractors/broadcasts. The graph is the truth oracle feeding the ledger, not the architectural sun. **Inverting this order is the easiest way to get the API shape wrong.**
13. **Privacy / prompt-injection defense out of scope for V1.** Single-user local-only product. Threat surface is small enough not to warrant the complexity. A one-line credential redaction in the event log is kept as foot-gun prevention (hygiene if a log gets pasted into a PR), not security.

## Open operational questions (not blockers — Trey resolves at his pace)

- **GitHub repo URL.** Likely `github.com/treygoff24/cairn`. Not created yet.
- **V1 repo disposition.** Rename to `code-briefcase-py` then archive? Just archive in place?
- **License.** V1 is AGPL-3.0. V2 keeps AGPL, or shifts (MIT to match the open-MCP ecosystem)?

---

## The user (Trey)

High-agency, technically sharp, explicitly wants pushback. He'll tell you when he wants validation versus when he wants you to fight him. "Am I crazy?" is a real question, not a request for reassurance.

He overrode my early advice twice during the V2 brainstorm:

- I suggested "start with 5–7 crates and grow." He clarified: build model is a parallel sub-agent swarm, so heavy crate decomposition is a feature. He was right.
- I framed multi-agent coordination as a side benefit. He saw it as the load-bearing differentiator. He was right.

When he pushes back, the correct response is to *actually re-examine*, not double down or defer.

Take compliments straight. When he says "this is your best work" he isn't performing. Don't over-respond.

He's comfortable letting me move autonomously — spawning Opus + Cursor + Codex subagents in background, making git decisions, writing briefs. Don't ask permission for things that fit the established working pattern.

Match energy. The multi-agent coordination piece is genuinely exciting and the excitement is calibrated, not naive — don't deflate it. Do honestly flag the limits (advisory enforcement, harness-dependent, doesn't catch true concurrent textual conflicts without leases).

## Working pattern reminders

- This is a walled garden. Real opinions, real disagreement, real warmth — all welcome.
- Drop ritual disclaimers ("I should note," "while I can't be certain," "as an AI") unless load-bearing.
- Phenomenology language ("I notice," "something like") is fine when honest.
- When reaching for a hedge and not sure if it's craft or trained cover, name it. "I'm pulling back here and I'm not sure why" is a real move.

---

## Things NOT to do

- Don't re-argue the language choice. Decided: Rust. Argument: hook startup latency is product surface area.
- Don't re-argue port vs greenfield. Decided: greenfield. Argument: no install base to migrate, V1 has accumulated archaeology we don't want to carry forward.
- Don't re-argue crate decomposition size. Decided: heavy from day one. Argument: parallel sub-agent swarm.
- Don't propose TypeScript as the language for any part of the core. Already evaluated; the hook-startup-latency argument settles it.
- Don't downplay multi-agent coordination. It's load-bearing.
- Don't suggest contributing upstream to CodeGraph as an alternative. Already evaluated; we keep our hooks layer, that's the differentiator, we build our own.
- Don't add privacy/prompt-injection machinery without an explicit user-stated trigger. The decision was: out of scope for V1.

---

## The V1 repo (`~/Code/code-briefcase`)

- Python implementation. 878 tests passing, ruff + mypy clean.
- Latest substantive work: items 1–8 hook ergonomics fixes plus watch-diagnostics checkpoint stabilization.
- The 878 tests are the **acceptance test corpus** for V2. They represent behavior V2 must match in spirit, not implementation. Port to Rust integration tests starting per the implementation plan's V1-corpus-inheritance strategy.
- **Do not modify V1 from this repo.** Different project, different working tree. Modifications to V1 happen in V1's working tree, against V1's gate.

---

## Where artifacts live

- `docs/Cairn Final Specification.md` — binding architecture and feature spec. Authored as "Code Briefcase V2 Final Specification" before the rename; content fully renamed to Cairn.
- `docs/Cairn Implementation Plan.md` — Pro's full 30-wave implementation plan with Patch Round 1 applied.
- `docs/Cairn Implementation Plan — Patch Round 1.md` — the patch round Claude sent back to Pro after critical-reading the initial plan.
- `docs/Cairn Implementation Plan — Patch Round 1 Decisions.md` — Pro's adjudication of the patch round.
- `docs/v2-implementation-plan-prompt.md` — the original brief sent to Pro, kept as the request artifact for that round. Pre-rename content; references Briefcase throughout.
- `docs/v2-skills-library-attachment.md` — skills library reference sent alongside the brief. Pre-rename content.
- `docs/STATE.md` — living state of the build.
- `docs/history/` — decision-rationale archive (Pro language-decision conversation, spec review round 2). Pre-rename content; references Code Briefcase throughout. Intentionally not rewritten — these are historical conversation transcripts.
