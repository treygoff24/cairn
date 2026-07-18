# Cairn Explainer Site — Design Context

This file is the load-bearing brief. Every subagent dispatched for this site
gets it pasted verbatim. Update here, not in scattered briefs.

## The artifact

A single-page explainer for **Cairn** — a local-first coordination and context
substrate for AI coding agents working inside real software repositories.
Audience is one specific reader: an experienced software developer who uses
Claude Code / Cursor / Codex daily, fluent in hooks, MCP, subagents, tool
calls. The site exists to give him enough understanding to feed real feedback
back to the project. It is **not** marketing copy. It is a design defense.

Desktop-only. No mobile breakpoints required. Deploy to Vercel.

## The design pin

> Cairn is not primarily a repository index. It is a versioned belief-management
> system for AI agents operating on code.

Every visual decision serves that pin. A code graph tells you what is currently
true about the repo. The harder, more valuable question: what does *this
particular agent session* currently believe, and has that belief expired?

The cairn metaphor is the mechanism made physical. A cairn is a pile of stones
marking a path — left by those who passed before, telling those who come after
where the trail is and whether anyone has been here recently. Every
observation is a stone. Every belief is the cairns currently along your path.
Multi-agent coordination is "another agent passed through and added stones
since you were last on this trail, so your read of the trail is stale."

## Voice — three words

**Instrumented. Patient. Archaeological.**

- **Instrumented** — the page reads like an instrument panel that respects
  the reader. Numbers are real. Labels are precise. Provenance is visible.
- **Patient** — generous spacing. Sections settle into their position. The
  page does not yell. Nothing pulses for attention.
- **Archaeological** — the texture of the page evokes weathered field
  guides, topographic maps, letterpress journals. Not glossy SaaS.

## Aesthetic — what this looks like

A backcountry trail map and an instrument panel had a child. Ink on vellum.
Topographic contours behind sections as quiet ground texture. Stones, cairns,
and observation markers as iconographic language. Restrained, slightly
weathered, durable.

**Reference compass:** USGS topo maps, ship's logbooks, mid-century field
guides (Peterson, Sibley), the typography of letterpress journals,
flight-recorder strip charts, navigation almanacs. **Not:** Linear's
landing page, Stripe's gradient hero, Vercel's neon dark mode, any SaaS
product page from 2023-2025.

## Color palette (OKLCH, neutrals tinted warm)

All colors in OKLCH. All neutrals tinted toward warm hue (~60-80) so the
whole page reads as printed-on-paper.

```
--paper-warm:        oklch(0.97 0.012 80)   /* page background (vellum)     */
--paper-shade:       oklch(0.94 0.014 78)   /* card / elevated surface       */
--ink-deep:          oklch(0.20 0.018 60)   /* primary type, deep slate-warm */
--ink-muted:         oklch(0.45 0.018 60)   /* secondary type, paper-aged    */
--ink-faint:         oklch(0.65 0.015 70)   /* tertiary, captions            */
--border-warm:       oklch(0.85 0.018 75)   /* soft warm border              */
--rule-warm:         oklch(0.78 0.020 75)   /* darker rule for emphasis      */

--accent-oxide:      oklch(0.55 0.13 35)    /* SINGLE accent, weathered rust */
--accent-oxide-deep: oklch(0.42 0.13 35)    /* hover / active state          */
--accent-ember:      oklch(0.62 0.10 75)    /* "live signal" warm brass      */

--surface-dark:      oklch(0.22 0.018 60)   /* terminal/code dark surface    */
--surface-dark-fg:   oklch(0.92 0.014 78)   /* paper-as-text-on-dark         */
--surface-dark-rust: oklch(0.65 0.13 35)    /* brighter oxide on dark        */
```

The 60-30-10 rule applies in visual *weight*: ~60% paper, ~30% ink (type and
borders), ~10% accent (oxide). The accent is rare on purpose. Oxide is the
"this matters" signal. Ember is the "this is alive" signal — reserve it for
section dingbats, live-data markers, and rare moments of motion.

## Typography

**Two faces, three roles.**

- **`font-display`** — `Schibsted Grotesk` at heavier weight (700–900),
  `tracking-tighter`, used for hero h1 and section h2. Workhorse with
  newspaper-masthead bones. Variable.
- **`font-archaic`** — `IM Fell DW Pica` (regular and italic). Letterpress
  reproduction from 17th-century English type. Use for: pull quotes,
  hero subtitle voice, section dingbats, the design pin, anywhere the page
  needs to feel like it's quoting from a field journal. Used sparingly.
- **`font-body`** — `Schibsted Grotesk` at regular for prose, label, UI.
- **`font-mono`** — `JetBrains Mono`. Code, schema callouts, CLI examples,
  data labels, file paths, anything with a fixed-width voice.

All three are on Google Fonts (OFL). Load via `next/font/google`.

Type scale (fixed rem on product UI per design-taste-frontend; clamp() only
on hero display):

```
text-xs    0.75rem  12px   labels, captions
text-sm    0.875rem 14px   meta, secondary
text-base  1.0625rem 17px  body — slightly larger for archaeological feel
text-lg    1.25rem  20px   lead paragraphs
text-xl    1.5rem   24px   section subheaders
text-2xl   2rem     32px   h3
text-4xl   3rem     48px   h2
hero h1: clamp(3.5rem, 8vw, 6.5rem)
```

Body line-height: 1.65 (loose, archaeological-patient).
Max line length: 65ch.

## BAN LIST (inherited from impeccable + design-taste-frontend)

Hard bans, no exceptions. If you find yourself reaching for any of these,
stop and rewrite the element with a different structure.

**Typography**
- BANNED fonts: Inter, Geist, DM Sans, DM Serif, Plus Jakarta Sans, Instrument
  Sans, Instrument Serif, Outfit, Space Grotesk, Space Mono, IBM Plex (any),
  Fraunces, Newsreader, Lora, Crimson (any), Playfair Display, Cormorant
  (any), Syne, Roboto, Open Sans, Arial.
- No gradient text. `background-clip: text` over a gradient is forbidden.
- No oversized hero h1 that just screams. Control hierarchy with weight and
  letter-spacing, not just size.
- No monospace-as-shorthand-for-technical. Mono has a job; don't decorate
  with it.

**Layout**
- No 3-equal-column card grids. The generic "feature row" is dead.
- No centered hero with text-over-image. Default to asymmetric layouts.
- No `h-screen` on full-height sections — use `min-h-[100dvh]`.
- No wrapping everything in cards. Not every block needs a container.
- No nested cards.
- No rounded-rectangle drop-shadow cards. Safe, forgettable, AI tell.

**Visual**
- No `border-left` or `border-right` greater than 1px as an accent stripe.
  This is the single most overused AI design touch. If you want to mark a
  block, use full borders, leading numerals, or a different element entirely.
- No purple-to-blue gradients. No neon on dark. No "AI palette."
- No pure `#000` or `#fff`. Both are tinted.
- No glassmorphism / backdrop-blur as decoration. Blur indicates dismissibility.
- No glowing outer shadows for emphasis.
- No emoji as structural icons. Ever.

**Motion**
- No bounce / elastic easing. Real objects decelerate, they don't ping.
- No animating `width` / `height` / `top` / `left`. Transform + opacity only.
- No perpetual ambient motion just to feel "alive." Restraint is the brand.
- Respect `prefers-reduced-motion` everywhere. Most motion should already
  be subtle enough to barely matter when disabled.

**Copy**
- No "Elevate / Seamless / Unleash / Empower / Next-Gen / Reimagined."
- No "Built for X, by Y, with Z." stack lines.
- No filler intros that restate the heading.
- No fake testimonials. No invented people. No "Join 10,000+ teams."

## Component vocabulary

Build these as the small fixed set. Compose, don't sprawl.

- `<PageShell>` — outer frame with topographic ground texture (subtle SVG
  noise/contours behind everything).
- `<Section>` — vertical rhythm wrapper; carries section numeral and label.
- `<DisplayHeader level={1|2}>` — hero/section heading.
- `<Lede>` — opening paragraph in archaic face, italic optional.
- `<Prose>` — body paragraphs.
- `<PullQuote>` — block-quote, archaic face, oxide accent rule (1px, full
  left border is fine; the ban is on >1px accent stripes).
- `<SpecBlock>` — schema/code callout on dark surface, mono.
- `<Cli>` — terminal-flavored block, dark surface, mono, with a small
  archaeological label ("operator console").
- `<Marker>` — small inline glyph for cairn/stone/observation/edit.
- `<Hairline>` — 1px full-width rule, warm-border, used to separate sections.

## Motion philosophy

Stones don't bounce. State transitions settle.

- Scroll-triggered reveals: gentle fade + 8-12px translateY, easing
  `cubic-bezier(0.16, 1, 0.3, 1)` (ease-out-quart), duration 350-450ms.
- Pull-quotes and section numerals: fade in slightly delayed (60-100ms after
  the surrounding content lands) so the eye reads the body first.
- One single live-signal element on the page may have a slow ambient pulse
  (opacity 0.6→1.0 over 4s, eased, infinite) — used at most once, on
  something genuinely signaling liveness.
- No staggered list reveals on body content. Save staggering for the rare
  diagrammatic elements (the multi-agent timeline lane diagram).

## Imagery direction (for codex / Quiver briefs)

When briefing image generators:

- **Texture:** ink-on-vellum, slightly weathered, fibrous paper grain.
  Topographic contour lines as quiet ground.
- **Objects:** cairns (literal small stone-stacks), trail markers, contour
  fragments, brass/iron instrument bezels, ship's-log paper, fountain-pen
  ink, weathered field-guide illustrations. No glossy plastic. No floating
  3D shapes.
- **Palette:** the OKLCH palette above. Bone background, deep slate-ink
  lines, occasional oxide accent. No purple. No neon. No "AI gradients."
- **No text inside images.** Text comes from the page.
- **No UI mockups inside images.** The page's own components do that.

## Tech constraints

- Next.js 16 App Router, TypeScript, Tailwind v4.
- React Server Components by default. Promote to `'use client'` only at the
  smallest leaf necessary for interactivity.
- CSS transitions for the small set of motion moments. Animate transform and
  opacity only; no perpetual ambient motion.
- Images served from `public/img/`, optimized via `<Image>`. Small diagrams are
  inline SVG components.
- Responsive from 320px through wide desktop, with content capped around
  1280-1400px.

## Pre-delivery checklist (run before declaring done)

- [ ] No banned fonts loaded. Look at `font-` family declarations.
- [ ] No gradient text anywhere.
- [ ] No `border-{left,right}: Npx` with N > 1 used as accent stripe.
- [ ] No 3-equal-column card grids.
- [ ] No centered hero with text-over-image.
- [ ] Color contrast ≥ 4.5:1 for body text against `--paper-warm`.
- [ ] All interactive elements have visible focus rings.
- [ ] `prefers-reduced-motion: reduce` disables / shortens all motion.
- [ ] Every image has an alt that describes its content, not its decoration.
- [ ] No "Elevate / Seamless / Unleash / Empower" copy.
- [ ] Page reads as a field guide, not a SaaS landing.
