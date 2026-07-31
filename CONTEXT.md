# Cairn — Strategic Context

The spec is the *what*. This is the *why*. Read it when a question feels like it might re-open a settled call, or before you push back on a locked-in choice.

---

## Origin story (compressed)

1. **V1 Python** lives at `~/Code/code-briefcase` — a working tool for context-decorating AI coding sessions: hooks layer, per-project daemon, tree-sitter indexer, diagnostics watch loop. Functional, but quality drifted and gaps accumulated.
2. **Competitive read.** [CodeGraph](https://github.com/colbymchenry/codegraph) (TS, MCP-only, 27k stars) has a better indexer than V1; [codedb](https://github.com/justrach/codedb) (Zig) has atomic edits and sub-ms warm queries. Neither replaces V1 — both lack the push-mode hooks plus diagnostics-in-the-loop differentiator. The moat is the substrate, not the index.
3. **Language decision** with GPT-5.5 Pro. Rust, on the killer argument: *"hook startup latency is product surface area."* Push-mode hooks fire constantly, and TypeScript's 50 ms cold start is structurally wrong for that hot path. The Wave 0 tracer later measured the claim: ~2 ms warm per hook call.
4. **Greenfield, not a port.** V1 stays as behavior spec and acceptance corpus. Renamed Briefcase → **Cairn** mid-planning: BeeWare's Briefcase already owns that name in developer tooling, and the cairn-as-trail-marker metaphor maps the design pin directly — each agent passing through drops stones, and the next agent reads what was marked.
5. **The multi-agent breakthrough.** During the V2 brainstorm Trey saw that the same primitives solving single-agent context decoration also solve stale-context edits, mid-flight semantic obsolescence, redundant CPU thrash, and cross-agent blindness. MVP mechanism: at each pre-edit hook, check whether any file in the target's static dep-graph was modified by a *different* agent session since this session's last relevant observation; deny where the harness supports it, advise otherwise. This moved positioning from "context substrate" to "context substrate + coordination layer."
6. **Spec and plan.** Final spec via two rounds of Pro review (12 architectural gaps found and closed). Pro then delivered the 30-wave implementation plan, patched across two rounds — the patch files in `docs/` are the decision log.

## Competitive position: vexp

The substrate — graph, index, ledger — is now commoditized by a shipping product, **vexp** (local-first Rust daemon, MCP, advisory staleness, freemium closed-source). Cairn's defensible daylight is the **push-mode hooks layer** — deterministic context injection plus edit interception, proven in V1 and re-proven by the Wave 0 tracer — which vexp structurally lacks, plus enforcement and multi-agent coordination.

The consequence, locked in as Patch Round 2: Cairn is a **superset** of vexp. Match the substrate, because the moat's precision rides on it, then add the moat. Do not thin the substrate to move faster.

## The non-obvious design choice (the framing)

> *"Cairn is not primarily a repository index. It is a versioned belief-management system for AI agents operating on code."*

Most reviewers will read this as a better code index. That's incomplete. A code graph tells you what is currently true about the repository. The harder and more valuable question: what does *this particular agent session* currently believe, and has that belief expired?

That distinction unlocks single-agent context savings (emit deltas, not repeated nav maps), continuation across sessions, connected diagnostics (edit attempt → file version → post-edit result → repair context), multi-agent coordination, auditable subagent belief inheritance frozen at spawn, auto-research scoring of which context frames actually saved work, and TOCTOU safety via precondition hashes attached to allow decisions.

A graph without an observation ledger is a clever librarian shouting facts into the room. A graph with one becomes a flight recorder, air-traffic radar, and memory palace on a single local machine.

---

## Locked-in decisions (do NOT re-litigate)

1. **Greenfield in Rust.** Behavior spec from V1, no compatibility shims.
2. **Naming.** Repo `cairn`, crates `cairn-*`, binary `cairn`, MCP tools `cairn_*`, runtime dir `.cairn/`.
3. **Heavy crate decomposition from day one.** Heavy decomposition is a feature, not a tax — the build model is a parallel sub-agent swarm and each crate is a unit of work owned by one worker.
4. **Salsa as the incremental engine**, pinned and wrapped behind `cairn-incremental` so a swap has a one-crate blast radius.
5. **MCP is an adapter, not the substrate.** The daemon owns a typed, versioned, length-prefixed binary protocol over Unix sockets; the MCP server proxies into it.
6. **Storage in three layers.** SQLite WAL for durable truth, mmap snapshot for cold-start speed, in-memory arenas for sub-ms hot serving.
7. **One shared daemon per project worktree.** Identified by canonical root + git worktree + config hash + protocol version.
8. **Multi-agent coordination is load-bearing.** The product is a context substrate *and* a coordination layer.
9. **Auto-research is the post-launch improvement model** — the Karpathy pattern of constraint + mechanical metric + autonomous iteration. Implication: every feature needs a measurable numerical scalar, baked into spec §12.
10. **Edit safety is advisory at baseline, enforced where the harness allows.** The hook protocol limits us and we don't pretend otherwise. File leases are P2, not baseline.
11. **TOCTOU verification.** Every edit decision carries `expected_target_file_hash` where the adapter supports it; the daemon verifies post-edit and emits `edit_race_detected` on mismatch.
12. **Ledger-first build order.** Identity substrate → Observation/Edit ledgers → ContextFrame ledger + scheduler skeleton → minimal graph → hooks/MCP → diagnostics/extractors/broadcasts. The graph is the truth oracle feeding the ledger, not the architectural sun. **Inverting this order is the easiest way to get the API shape wrong.**
13. **Privacy / prompt-injection defense out of scope for V1.** Single-user, local-only; the threat surface doesn't warrant the complexity. The one-line credential redaction in the event log is foot-gun prevention if a log gets pasted into a PR, not security.
14. **Signal precision is make-or-break.** The Wave 0 tracer fired its note unconditionally and a live agent immediately flagged the false positive and said it would learn to tune the warning out. That is why the ledger keys on per-session content hashes rather than mtime, and why §3 sets a <0.5% self-edit-false-positive gate.

## Open questions Trey resolves at his pace (non-blocking)

- **License** — V1 is AGPL-3.0. V2 keeps AGPL, or shifts to MIT to match the open-MCP ecosystem. Currently unset in `Cargo.toml`.
- **V1 repo disposition** — rename to `code-briefcase-py` and archive, or archive in place.

---

## Working with Trey (project-specific)

He overrode my advice twice during the V2 brainstorm and was right both times. I suggested starting with 5–7 crates and growing; he clarified that the build model is a parallel swarm, so heavy decomposition is a feature. I framed multi-agent coordination as a side benefit; he saw it as the load-bearing differentiator. When he pushes back here, actually re-examine rather than doubling down or deferring.

"Am I crazy?" is a real question, not a request for reassurance. This project already survived one explicit LLM-psychosis gut-check — which is why V1 was ground-truthed by actually running its 878 tests, the spec was red-teamed, and the competitive landscape was re-checked before committing to build.

Match energy on the multi-agent piece; the excitement is calibrated, not naive. Do honestly flag the limits: advisory enforcement, harness-dependent, and no protection against true concurrent textual conflicts without leases.

---

## Things NOT to do

- Don't re-argue Rust vs TypeScript for any part of the core. Settled by hook-startup latency.
- Don't re-argue port vs greenfield. No install base to migrate; V1's accumulated archaeology isn't worth carrying forward.
- Don't re-argue crate decomposition size.
- Don't downplay multi-agent coordination.
- Don't propose contributing upstream to CodeGraph instead. Evaluated; the hooks layer is ours and it's the differentiator.
- Don't add privacy or prompt-injection machinery absent an explicit trigger from Trey.
- Don't thin the substrate to reach the moat faster (Patch Round 2).

---

## Pre-rename artifacts

`docs/history/`, `docs/v2-implementation-plan-prompt.md`, and `docs/v2-skills-library-attachment.md` were written before the rename and say "Code Briefcase" / "Briefcase" throughout. Intentionally not rewritten — they are historical transcripts and the original request artifacts.
