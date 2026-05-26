# Skill Library Reference — Code Briefcase V2 Implementation Plan

This is the full library of named skills available to the agents that will execute the V2 implementation plan. 152 skills total. Reference these by exact name in the plan when assigning skill stacks to tasks.

Skills marked `[global]` are auto-loaded for every agent. The rest are discoverable and loadable per task (the orchestrator activates them at dispatch time, or the worker loads them when a task brief names them).

A subset of these skills is most relevant to the V2 implementation. The most relevant for Rust implementation and orchestration is highlighted at the end.

---

## Full alphabetical list

- **accessibility-checklist** — Run accessibility checks on interactive UI components. Use after implementing UI, before code review.
- **ad-creative** — Generate, iterate, or scale ad creative — headlines, descriptions, primary text, or full ad variants.
- **adapt** — Adapt designs to work across different screen sizes, devices, contexts, or platforms.
- **agent-browser** — Browser automation CLI for AI agents. Use when the user needs to interact with websites, including navigating pages, filling forms.
- **agent-md-refactor** — Refactor bloated AGENTS.md, CLAUDE.md, or similar agent instruction files to follow progressive disclosure principles.
- **ai-seo** — Optimize content for AI search engines, get cited by LLMs, or appear in AI-generated answers.
- **algorithmic-art** — Creating algorithmic art using p5.js with seeded randomness and interactive parameter exploration.
- **animate** — Review a feature and enhance it with purposeful animations, micro-interactions, and motion effects.
- **arrange** — Improve layout, spacing, and visual rhythm. Fixes monotonous grids, inconsistent spacing, weak visual hierarchy.
- **audit** — Run technical quality checks across accessibility, performance, theming, responsive design, anti-patterns.
- **audit-website** — Audit websites for SEO, performance, security, technical, content, and other issue categories with 230+ rules.
- **autonomous-loop** — Activate autonomous loop mode for persistent development sessions.
- **autoresearch** — Set up and run an autonomous experiment loop for any optimization target.
- **b4a-brand** — Apply Blueprint for America (B4A) brand guidelines to all visual materials and documents.
- **b4a-weasyprint-pager** — Produce print-ready 2-page (front-and-back) policy one-pagers for B4A using WeasyPrint.
- **better-auth-best-practices** — Configure Better Auth server and client, set up database adapters, manage sessions, add plugins.
- **blender-3d** — Autonomously create 3D assets using Blender's CLI and Python API.
- **bolder** — Amplify safe or boring designs to make them more visually interesting and stimulating.
- **bootstrap** — Generate a project-specific CLAUDE.md by scanning the codebase — eliminates cold-start re-exploration.
- **brainstorming** `[global]` — Use when creating or developing, before writing code or implementation plans. Refines rough ideas into fully-formed designs.
- **brand-guidelines** — Applies Anthropic's official brand colors and typography to any sort of artifact.
- **bridge-tool** `[global]` — Use when conceptual intent resists text — spatial placement, aesthetic tuning, ordering, threshold values, palette choice.
- **browser-use** — Automates browser interactions for web testing, form filling, screenshots, and data extraction.
- **canvas-design** — Create beautiful visual art in .png and .pdf documents using design philosophy.
- **caveman** `[global]` — (description truncated; see source)
- **checkpoint** `[global]` — Force a progress checkpoint — summarize completed work, remaining items, and blockers.
- **clarify** — Improve unclear UX copy, error messages, microcopy, labels, and instructions.
- **claude** — Used when the user asks to "use Claude Code", "Claude Code best practices", or "run Claude headless".
- **clean-code** `[project]` — Embodies the principles of "Clean Code" by Robert C. Martin. Use to transform "code that works" into "code that is clean."
- **cmux** — End-user control of cmux topology and routing (windows, workspaces, panes/surfaces, focus, moves, reorder).
- **cmux-browser** — End-user browser automation with cmux.
- **cmux-customization** — Customize cmux for an end user.
- **cmux-diagnostics** — Run end-user cmux diagnostics.
- **cmux-markdown** — Open markdown files in a formatted viewer panel with live reload.
- **cmux-settings** — View and edit cmux settings in ~/.config/cmux/cmux.json.
- **cmux-workspace** — Work inside the current cmux workspace and terminal.
- **codex** — Use when the user asks to "use Codex CLI", "codex exec", "get a Codex second opinion", or "run a Codex review".
- **codex-prompting** — Use when Claude is preparing, revising, or delegating any Codex or GPT-5.5 prompt.
- **cold-email** — Write B2B cold emails and follow-up sequences that get replies.
- **colorize** — Add strategic color to features that are too monochromatic or lack visual interest.
- **copy-editing** — Edit, review, or improve existing marketing copy, or refresh outdated content.
- **copywriting** — Write, rewrite, or improve marketing copy for any page.
- **create-auth-skill** — Scaffold and implement authentication in TypeScript/JavaScript apps using Better Auth.
- **create-handoff** — Create handoff document for transferring work to another session.
- **critique** — Evaluate design from a UX perspective.
- **debugging-systematic** — Apply systematic root cause analysis and debugging methodologies to diagnose and fix bugs, test failures, unexpected behaviors.
- **delegate-agent** `[global]` — Use the local delegate CLI to hand bounded execution tasks to Cursor Composer 2.5, Droid BYOK models, or OpenAI Codex CLI.
- **delight** — Add moments of joy, personality, and unexpected touches that make interfaces memorable.
- **design-motion-principles** — Expert motion and interaction design auditor based on Emil Kowalski, Jakub Krehel, Jhey Tompkins techniques.
- **design-taste-frontend** — Senior UI/UX Engineer. Architect digital interfaces overriding default LLM biases.
- **desloppify** `[global]` — (description truncated; see source)
- **desloppify-deep** — Eight-subagent parallel codebase cleanup. Use for deep clean, vibecoded slop removal, full-codebase quality pass.
- **diagnose** — Diagnose-only debugging — investigate bugs and report root cause with proposed fix WITHOUT implementing changes.
- **distill** — Strip designs to their essence by removing unnecessary complexity.
- **doc-coauthoring** — Guide users through a structured workflow for co-authoring documentation.
- **document-revision-discipline** — Use when revising long documents (policy briefs, memos, papers, op-eds, decks).
- **docx** — Comprehensive document creation, editing, and analysis with support for tracked changes, comments, formatting preservation.
- **docx-redlining** — Use when editing existing .docx files programmatically — especially building tracked-change redlines.
- **dogfood** — Systematically explore and test a web application to find bugs, UX issues.
- **domain-model** — Grilling session that challenges your plan against the existing domain model, sharpens terminology.
- **electron** — Automate Electron desktop apps (VS Code, Slack, Discord, Figma, Notion, Spotify) using agent-browser via Chrome DevTools Protocol.
- **email-best-practices** — Use when building email features, emails going to spam, high bounce rates, SPF/DKIM/DMARC authentication.
- **extract** — Extract and consolidate reusable components, design tokens, and patterns into your design system.
- **find-docs** — (description truncated; see source)
- **find-skills** — Helps users discover and install agent skills.
- **finishing-a-development-branch** — Use when implementation is complete, all tests pass, and you need to decide how to integrate the work.
- **fixing-motion-performance** — Audit and fix animation performance issues including layout thrashing, compositor properties, scroll-linked motion.
- **framer-motion-animations** — Create scroll-triggered and entrance animations using Framer Motion.
- **gemini** — Use when the user asks to "use Gemini", "get a Gemini review", "delegate to Gemini CLI", or "run a Gemini review".
- **git-guardrails-claude-code** — Set up Claude Code hooks to block dangerous git commands (push, reset --hard, clean, branch -D).
- **github-triage** — Triage GitHub issues through a label-based state machine.
- **glsl-shaders** — Used to write/debug GLSL shaders, create custom materials/effects, implement procedural rendering.
- **gog** `[global]` — gog CLI: safe Google Workspace automation, JSON, auth, scoped reads/writes.
- **gpt-taste** — Elite UX/UI & Advanced GSAP Motion Engineer.
- **grill-me** — Interview the user relentlessly about a plan or design until reaching shared understanding.
- **grill-with-docs** — Grilling session that challenges your plan against the existing domain model, sharpens terminology.
- **harden** — Improve interface resilience through better error handling, i18n support, text overflow handling, edge case management.
- **high-end-visual-design** — Teaches the AI to design like a high-end agency.
- **image-taste-frontend** — Elite frontend image-to-code skill for Codex.
- **impeccable** — Create distinctive, production-grade frontend interfaces with high design quality.
- **improve-codebase-architecture** — Find deepening opportunities in a codebase, informed by the domain language in CONTEXT.md and decisions in docs/adr/.
- **interaction-design** — Design and implement microinteractions, motion design, transitions, and user feedback patterns.
- **internal-comms** — Resources to help write all kinds of internal communications.
- **launch-strategy** — Plan a product launch, feature announcement, or release strategy.
- **marketing-email-automation** — Email marketing automation — workflow design, platform setup (HubSpot, Klaviyo, Mailchimp), nurture sequences.
- **marketing-psychology** — Apply psychological principles, mental models, behavioral science to marketing.
- **mcp-builder** — Guide for creating high-quality MCP (Model Context Protocol) servers that enable LLMs to interact with external services.
- **nanobanana-pro** — Generate and edit images using the Nano Banana Pro MCP server (Gemini image models).
- **nextjs-app-router-patterns** — Master Next.js 14+ App Router with Server Components, streaming, parallel routes, advanced data fetching.
- **nextjs-best-practices** — Next.js App Router principles. Server Components, data fetching, routing patterns.
- **nextjs-supabase-auth** — Expert integration of Supabase Auth with Next.js App Router.
- **normalize** — Audits and realigns UI to match design system standards, spacing, tokens, and patterns.
- **onboard** — Designs and improves onboarding flows, empty states, and first-run experiences.
- **optimize** — Diagnoses and fixes UI performance across loading speed, rendering, animations, images, bundle size.
- **orchestrator** `[global]` — Activate orchestrator mode. You coordinate specialists, you don't implement.
- **overdrive** — Pushes interfaces past conventional limits with technically ambitious implementations.
- **parallel-subagent-discipline** — Use BEFORE fanning out parallel subagents that each run heavy work (test suite, bundler, typecheck, large build). Caps concurrency.
- **pdf** — Used whenever the user wants to do anything with PDF files.
- **performance** — Optimize web performance for faster loading and better user experience.
- **plain-language** — Use when the user asks for a plainer, simpler, shorter, or more direct explanation.
- **policy-essay-iteration** — Use when shipping a flagship policy essay or op-ed under time pressure.
- **polish** — Performs a final quality pass fixing alignment, spacing, consistency, and micro-detail issues before shipping.
- **pptx** — Presentation creation, editing, and analysis (.pptx files).
- **premortem** — Pre-implementation failure analysis — first-principles retrospective with tiger/elephant risk classification.
- **prismic** — Prismic CMS + Slice Machine workflows across Next.js, Nuxt, SvelteKit, and React apps.
- **prospera-brand** — Apply Próspera brand guidelines to all visual materials and documents.
- **quieter** — Tones down visually aggressive or overstimulating designs.
- **quiverai-svg** — Generate, vectorize, optimize, preview, and integrate SVG assets with the QuiverAI MCP server.
- **react-three-fiber** — Build Three.js scenes in React, use React Three Fiber/Drei, optimize R3F performance.
- **receiving-code-review** — Use when receiving code review feedback, before implementing suggestions.
- **refactor** — (description truncated; see source — behavior-preserving code refactoring guidance)
- **remotion-best-practices** — Best practices for Remotion — Video creation in React.
- **request-refactor-plan** — Create a detailed refactor plan with tiny commits via user interview, then file it as a GitHub issue.
- **requesting-code-review** — Use when completing tasks, implementing major features, before merging — runs review.
- **resend** — Use when working with the Resend email API.
- **resume-handoff** — Resume work from handoff document with context analysis and validation.
- **rlcoach** — AI coaching for Rocket League using local replay analysis.
- **rust-engineer** — Writes, reviews, and debugs idiomatic Rust code with memory safety and zero-cost abstractions. Implements ownership patterns.
- **send-email** — Use when sending transactional emails (welcome messages, order confirmations, password resets, receipts), notifications.
- **seo-audit** — Audit, review, or diagnose SEO issues on a site.
- **shadcn** — Manages shadcn components and projects — adding, searching, fixing, debugging, styling, composing UI.
- **signup-flow-cro** — Optimize signup, registration, account creation, or trial activation flows.
- **skill-creator** — Guide for creating effective skills.
- **slack** — Interact with Slack workspaces using browser automation.
- **slack-gif-creator** — Knowledge and utilities for creating animated GIFs optimized for Slack.
- **spec-quality-checklist** — Validate a specification before implementation. Use when you've drafted a spec and need to verify it's complete, precise.
- **supabase-edge-functions** — Deploy and manage Supabase Edge Functions.
- **supabase-postgres-best-practices** — Postgres performance optimization and best practices from Supabase.
- **super-pptx** — Create, edit, and perfect PowerPoint presentations.
- **tdd** — Test-driven development with red-green-refactor loop.
- **tdd-workflow** — Use when writing new features, fixing bugs, or refactoring code. Enforces test-driven development with 80%+ coverage.
- **theme-factory** — Toolkit for styling artifacts with a theme.
- **threejs** — Use to "build a Three.js app", "optimize Three.js performance", "set up a 3D asset pipeline".
- **to-issues** — Break a plan, spec, or PRD into independently-grabbable GitHub issues using tracer-bullet vertical slices.
- **to-prd** — Turn the current conversation context into a PRD and submit it as a GitHub issue.
- **triage-issue** — Triage a bug or issue by exploring the codebase to find root cause, then create a GitHub issue with a TDD-based fix plan.
- **typeset** — Improves typography by fixing font choices, hierarchy, sizing, weight, and readability.
- **ubiquitous-language** — Extract a DDD-style ubiquitous language glossary from the current conversation.
- **ui-ux-pro-max** — UI/UX design intelligence for web and mobile.
- **userinterface-wiki** — UI/UX best practices for web interfaces.
- **using-git-worktrees** — Use when starting feature work that needs isolation from current workspace — creates worktree.
- **vanilla-web-dev** — Build high-performance web applications using native browser APIs and minimal server code with zero frameworks.
- **vercel-ai-sdk** — Best practices for building AI applications with the Vercel AI SDK (v6+).
- **vercel-react-best-practices** — React and Next.js performance optimization guidelines from Vercel Engineering.
- **vercel-sandbox** — Run agent-browser + Chrome inside Vercel Sandbox microVMs.
- **web-artifacts-builder** — Suite of tools for creating elaborate, multi-component claude.ai HTML artifacts.
- **web-design-guidelines** — Review UI code for Web Interface Guidelines compliance.
- **webapp-testing** — Toolkit for interacting with and testing local web applications using Playwright.
- **write-human** `[global]` — Anti-slop writing directive. Load BEFORE writing any text: emails, docs, copy, essays, reports, briefs, memos, policy papers.
- **writing-plans** — Use when design is complete and you need detailed implementation tasks for engineers with zero codebase context.
- **xlsx** — Comprehensive spreadsheet creation, editing, and analysis.
- **zapier-workflows** — Manage and trigger pre-built Zapier workflows and MCP tool orchestration.

---

## Subset most relevant to V2 implementation

These are the skills you will most likely reference in task entries:

### Engineering core (load for nearly every implementation task)

- **`clean-code`** — Uncle Bob principles. Always-on for Rust implementation tasks.
- **`rust-engineer`** — Idiomatic Rust, memory safety, zero-cost abstractions. Always-on for Rust implementation.
- **`tdd-workflow`** — TDD with 80%+ coverage. Use when the deliverable has clear test surface.
- **`tdd`** — Red-green-refactor variant. Lighter than `tdd-workflow`.

### Orchestration and dispatch

- **`delegate-agent`** — Use the `delegate` CLI for off-Anthropic worker dispatch.
- **`parallel-subagent-discipline`** — Read before fanning out parallel workers. CPU discipline.
- **`orchestrator`** — Activate orchestrator mode. "You coordinate specialists, you don't implement."
- **`codex-prompting`** — When crafting prompts for Codex tasks (Codex will drive a large portion of execution).
- **`using-git-worktrees`** — Isolate parallel implementation lanes that might collide.

### Phase-boundary work

- **`desloppify-deep`** — 8-subagent parallel cleanup pass. Use at scheduled phase boundaries.
- **`debugging-systematic`** — Systematic root cause analysis. Use at scheduled bug hunts.
- **`diagnose`** — Diagnose-only investigation. Use before dispatching fixes.
- **`finishing-a-development-branch`** — Integration discipline at phase end.
- **`checkpoint`** — Force a progress checkpoint at phase end.
- **`create-handoff`** / **`resume-handoff`** — Cross-session continuity.

### Planning and review

- **`writing-plans`** — Create detailed implementation tasks for engineers with zero codebase context.
- **`spec-quality-checklist`** — Validate sub-specs before implementation.
- **`premortem`** — Pre-implementation failure analysis before risky phases.
- **`requesting-code-review`** / **`receiving-code-review`** — Review discipline.

### Refactor lanes

- **`refactor`** — Behavior-preserving refactor guidance.
- **`request-refactor-plan`** — Create a detailed refactor plan when a task surfaces one.
- **`improve-codebase-architecture`** — Find deepening opportunities in the codebase.

### Specialized

- **`mcp-builder`** — For the MCP server crate (Phase 5+).
- **`bootstrap`** — Generate project-specific `CLAUDE.md` once the cargo workspace exists.

### Second-opinion / model diversity

- **`codex`** — Get a Codex second opinion or delegate review.
- **`gemini`** — Get a Gemini review or delegate to Gemini CLI.

### Writing artifacts

- **`write-human`** — Anti-slop writing directive. Load before writing any agent-facing doc, README, or `CLAUDE.md`.
- **`plain-language`** — For simpler, more direct explanations in docs.

---

## Discovery commands

If the orchestrator or a worker needs a skill not listed here, the discovery flow is:

```
claude-skill search -n <query>     # name-only search
claude-skill info <name>            # full SKILL.md preview
claude-skill add <name>             # activate in current project
```

Skills marked `[global]` are always loaded for every agent. Skills marked `[project]` are loaded only inside the current project's working tree.
