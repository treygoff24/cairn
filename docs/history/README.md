# History — Decision Rationale Archive

> **Archive note:** these files were written before the project was renamed from "Code Briefcase" to **Cairn**. References to Briefcase / Code Briefcase throughout reflect the project at the time. Content intentionally not rewritten — these are conversation transcripts and decision artifacts, not active documentation.

These files are not active spec, plan, or state. They're the *why* behind locked-in decisions. Read on demand when a question arises about why something is the way it is.

## Files

- **`gpt-pro-language-decision.md`** — GPT-5.5 Pro's recommendation of Rust over alternatives (TypeScript, Go, Zig, others). Contains the killer argument: *"hook startup latency is product surface area"* — push-mode hooks fire constantly, TypeScript's 50ms cold start is structurally wrong for the hot path. Also contains Pro's 15-crate workspace blueprint sketch and the three-layer storage model (SQLite WAL + mmap snapshot + in-memory arenas) that the spec inherited.

- **`gpt-pro-spec-review-round-2.md`** — GPT-5.5 Pro's follow-up review answering 14 architectural questions left open by the candidate spec. The most consequential parts:
  - §1 build-order correction: identity substrate must precede the ledger
  - §2 subagent inheritance: lineage-aware copy-on-spawn + orientation packet model
  - §3 git/VCS depth: file-content-hash + repo epoch tri-layer
  - §7 the two-fingerprint move (contract vs implementation) for exported-surface tracking
  - §11 deny-loop guard: the critical UX patch ("one bad graph edge can trap an agent in a haunted revolving door")
  - §14 five things we missed in the candidate: TOCTOU race, prompt-injection from untrusted repo content (cut), secret retention in event log (cut for V1), generated/vendor file policy, task identity as first-class primitive

## Where the active artifacts live

- The current binding spec is `../Cairn Final Specification.md`.
- Live state is `../STATE.md`.
- Strategic context (decisions in plain English) is `../../CONTEXT.md`.
- The implementation plan (delivered by Pro, patched per Round 1) is `../Cairn Implementation Plan.md`.

## What's missing here (intentionally)

- **Candidate spec.** Superseded by the final. Not retained — would add noise.
- **Architecture brief** (sent to Pro for the language decision). The Pro response above subsumes it.
- **Follow-up review prompt and implementation plan prompt.** Active artifacts kept in `../`, not history.
- **Dev-notes / handoffs from the V1 session.** Lived in V1 repo's `docs/dev-notes/` (gitignored). The strategic distillation is in `CONTEXT.md`.
