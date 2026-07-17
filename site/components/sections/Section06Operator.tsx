import { Section, Content, Hairline, Wide } from "@/components/layout";
import { DisplayHeader, Lede, Prose, Mono, PullQuote, SmallCaps } from "@/components/type";
import { Cli, Marker } from "@/components/blocks";
import { SectionPlate } from "@/components/SectionPlate";

export function Section06Operator() {
  return (
    <Section
      id="operator"
      numeral="06"
      label="The Operator Surface"
      plate={
        <SectionPlate
          src="/img/operator-flight-recorder.png"
          alt="A weathered strip chart from a flight recorder or seismograph, with several parallel ink traces and clusters of event marks along them. One trace has a single oxide-accented mark."
          numeral="06"
          label="The Operator Surface — Read-only flight recorder"
          aspectRatio="24 / 9"
          objectPosition="center"
        />
      }
    >
      <Content>
        <DisplayHeader level={2}>
          A flight recorder, not an IDE.
        </DisplayHeader>

        <Lede className="mt-8">
          If the operator can&apos;t answer{" "}
          <em className="archaic">why did the system say that?</em> in under
          thirty seconds, the product feels haunted. Visibility is{" "}
          <span className="font-mono not-italic text-[0.85em] tracking-tight">P0</span>.
        </Lede>
        <p className="mt-6 font-mono text-sm text-oxide">
          Planned surface: V1 CLI. Deferred P1 concept: read-only web view.
        </p>
        <Prose className="mt-8" size="lg">
          The substrate is the agent&apos;s product. The operator&apos;s
          product is debug access to the substrate: a CLI that can explain
          any decision the daemon made, and a read-only web view that puts
          multiple agent sessions on the same time axis. Both read the event
          log. Neither is a UI for editing code.
        </Prose>
      </Content>

      {/* CLI section */}
      <Content className="mt-20">
        <div className="flex items-center gap-4">
          <SmallCaps>6.1 — the two commands that matter most</SmallCaps>
          <span className="h-px flex-1 bg-rule-warm/60" aria-hidden />
        </div>

        <Prose className="mt-6">
          Out of the dozen-plus commands in the operator surface, two carry
          the most weight: <Mono>cairn deny explain</Mono> and{" "}
          <Mono>cairn context show</Mono>. The first answers why an edit was
          blocked. The second answers what the agent was just shown and why.
          Both produce structured, parseable output as well as the
          human-readable view below.
        </Prose>

        <Cli label="cairn deny explain dn_8c4f">
{`$ cairn deny explain dn_8c4f

  decision      deny  (hard)
  target        auth-caller.ts                   @ d0e9
  agent         α  (session s_100, task t_47)
  attempted_at  2026-05-26T10:14:02-04:00
  policy        default

  dependency_causes
    ↳ app/auth/route.ts
         observed_by_α  @ a3f1   2026-05-26T10:02:11
         current        @ b9d2   (advanced by β, session s_104)
         contract_fingerprint moved
         · removed export: validateSession  (rename → validateUserSession)
         · changed signature: handleAuth(req, ctx) → handleAuth(req)

  minimum_revalidation_set
    ↳ app/auth/route.ts                          @ b9d2

  override_policy
    permitted after revalidation of the set above.
    repeated overrides degrade trust budget.

  proof    cairn_prove(deny_id=dn_8c4f)  ↗ source spans, extractor provenance
  replay   cairn replay decision evt_91c2`}
        </Cli>

        <Prose>
          The output reads like a receipt. The agent gets a structured form
          of the same content via <Mono>cairn_prove(deny_id)</Mono> and can
          recover in-band. The operator gets enough to either confirm the
          deny was correct or file a precision bug with a complete trace.
        </Prose>
      </Content>

      {/* The full command set */}
      <Content className="mt-20">
        <div className="flex items-center gap-4">
          <SmallCaps>6.2 — the full command set</SmallCaps>
          <span className="h-px flex-1 bg-rule-warm/60" aria-hidden />
        </div>

        <div className="mt-8 grid lg:grid-cols-2 gap-x-12 gap-y-2 font-mono text-sm">
          <CmdRow cmd="cairn status"                  note="daemon, watchers, sessions, fingerprint coverage" />
          <CmdRow cmd="cairn daemon doctor"           note="injected-fault check, watcher wedge detection" />
          <CmdRow cmd="cairn sessions list"           note="active and recent sessions, lineage tree" />
          <CmdRow cmd="cairn sessions show s_100"     note="working set, edits, denies, context budget" />
          <CmdRow cmd="cairn ledger tail --session"   note="live event stream for one session" />
          <CmdRow cmd="cairn observations show"       note="direct + inherited, stale status, provenance" />
          <CmdRow cmd="cairn context show cf_…"       note="exact context frame the agent received" />
          <CmdRow cmd="cairn deny explain dn_…"       note="the example above" highlight />
          <CmdRow cmd="cairn graph explain path/sym"  note="extraction path, source spans, confidence" />
          <CmdRow cmd="cairn diagnostics delta"       note="introduced / resolved / changed since event" />
          <CmdRow cmd="cairn metrics report"          note="session ledger summary, VAT-relevant counters" />
          <CmdRow cmd="cairn metrics tail"            note="live local metrics stream" />
          <CmdRow cmd="cairn replay decision evt_…"   note="re-run a decision against current state" />
        </div>
      </Content>

      {/* Flight recorder web UI */}
      <Content className="mt-20">
        <div className="flex items-center gap-4">
          <SmallCaps>6.3 — planned P1 flight recorder concept</SmallCaps>
          <span className="h-px flex-1 bg-rule-warm/60" aria-hidden />
        </div>
        <h3 className="mt-4 font-display text-3xl font-semibold tracking-tight text-ink-deep">
          Multi-agent timelines are spatial.
        </h3>
        <Prose className="mt-6">
          The web UI is read-only by design. Session lanes run horizontally
          across the page. Reads, edits, denies, diagnostic deltas, context
          frame emissions, and VCS state changes all land as marks on those
          lanes. Watcher health sits in the corner. Cold-start coverage shows
          as a fill behind the lanes. Clicking any mark opens the underlying
          ledger row.
        </Prose>
      </Content>

      <Wide className="mt-12" >
        <div style={{ paddingInline: "var(--spacing-gutter)" }}>
          <FlightRecorderSketch />
        </div>
      </Wide>

      <Content className="mt-16">
        <PullQuote attribution="— from §11, non-goals">
          Cairn is not a human IDE. Human-facing UI can exist, but the
          primary interface is the agent&apos;s tool-call stream.
        </PullQuote>
      </Content>

      <Content className="mt-12">
        <Hairline />
        <p className="mt-10 flex items-center gap-3 font-mono text-sm text-oxide">
          <Marker kind="cairn" />
          <span>§ 07 — how we&apos;ll know it works.</span>
        </p>
      </Content>
    </Section>
  );
}

function CmdRow({ cmd, note, highlight = false }: { cmd: string; note: string; highlight?: boolean }) {
  return (
    <div className="grid grid-cols-[1.25rem_minmax(0,1fr)] gap-x-3 items-baseline border-t border-rule-warm/40 py-2">
      <span className={highlight ? "text-oxide" : "text-ink-faint"}>{highlight ? "★" : "›"}</span>
      <span>
        <span className="text-ink-deep">{cmd}</span>
        <span className="ml-3 text-ink-muted">{note}</span>
      </span>
    </div>
  );
}

function FlightRecorderSketch() {
  // A minimal SVG sketch of the flight recorder UI — three session lanes,
  // event marks, a header bar with daemon state, a footer with watcher health.
  return (
    <figure className="my-8">
      <div className="border border-rule-warm bg-paper-warm overflow-hidden">
        <div className="flex items-center justify-between border-b border-rule-warm/60 px-5 py-2">
          <span className="font-mono text-xs text-ink-faint">cairn webview · read-only · localhost:7421</span>
          <span className="font-mono text-xs text-ink-muted">daemon · healthy · 3 sessions</span>
        </div>

        <div
          className="overflow-x-auto"
          tabIndex={0}
          role="region"
          aria-label="Scrollable flight recorder concept"
        >
        <svg viewBox="0 0 900 320" className="w-full min-w-[56rem] h-auto block" role="img" aria-label="Flight recorder UI sketch with three session lanes">
          <defs>
            <pattern id="fr-contour" x="0" y="0" width="36" height="36" patternUnits="userSpaceOnUse">
              <circle cx="18" cy="18" r="12" fill="none" stroke="oklch(0.74 0.020 75)" strokeWidth="0.4" opacity="0.15" />
            </pattern>
          </defs>
          <rect x="0" y="0" width="900" height="320" fill="url(#fr-contour)" />

          {/* Top strip — repo epoch + cold-start coverage */}
          <g fontFamily="var(--font-jetbrains)" fontSize="10" fill="var(--color-ink-deep)">
            <text x="20" y="28">repo_epoch e_882</text>
            <text x="180" y="28" fill="var(--color-ink-faint)">head 7e2a91f · branch main</text>
            <text x="730" y="28" textAnchor="end" fill="var(--color-ink-faint)">graph coverage</text>
            <text x="880" y="28" textAnchor="end" fill="var(--color-oxide)">94%</text>
          </g>

          <line x1="20" y1="44" x2="880" y2="44" stroke="oklch(0.74 0.020 75)" strokeWidth="0.6" />

          {/* Three lanes */}
          {[
            { y: 90,  label: "session α", events: [60, 110, 180, 280, 380, 460, 540, 620, 700, 780] },
            { y: 160, label: "session β", events: [80, 140, 220, 290, 350, 440, 510, 590, 660, 740] },
            { y: 230, label: "session γ", events: [120, 200, 240, 320, 400, 470, 560, 650, 730, 800] },
          ].map((lane, li) => (
            <g key={li}>
              <text x="20" y={lane.y + 4} fontFamily="var(--font-jetbrains)" fontSize="10" fill="var(--color-ink-deep)">{lane.label}</text>
              <line x1="120" y1={lane.y} x2="880" y2={lane.y} stroke="oklch(0.74 0.020 75)" strokeWidth="0.7" />
              {lane.events.map((x, i) => {
                const isDeny = (li === 0 && i === 5) || (li === 2 && i === 3);
                const isEdit = i % 3 === 1;
                return (
                  <g key={i}>
                    {isDeny ? (
                      <circle cx={x} cy={lane.y} r="5" fill="oklch(0.55 0.13 35)" />
                    ) : isEdit ? (
                      <g>
                        <line x1={x-3.5} y1={lane.y-3.5} x2={x+3.5} y2={lane.y+3.5} stroke="oklch(0.20 0.018 60)" strokeWidth="1.2" />
                        <line x1={x-3.5} y1={lane.y+3.5} x2={x+3.5} y2={lane.y-3.5} stroke="oklch(0.20 0.018 60)" strokeWidth="1.2" />
                      </g>
                    ) : (
                      <circle cx={x} cy={lane.y} r="3.2" fill="oklch(0.20 0.018 60)" />
                    )}
                  </g>
                );
              })}
            </g>
          ))}

          {/* Bottom strip — watcher / diagnostic worker health */}
          <line x1="20" y1="285" x2="880" y2="285" stroke="oklch(0.74 0.020 75)" strokeWidth="0.6" />
          <g fontFamily="var(--font-jetbrains)" fontSize="10">
            <text x="20" y="305" fill="oklch(0.20 0.018 60)">watcher</text>
            <text x="80" y="305" fill="oklch(0.55 0.13 35)">● ok</text>
            <text x="160" y="305" fill="oklch(0.20 0.018 60)">tsc</text>
            <text x="200" y="305" fill="oklch(0.55 0.13 35)">● running</text>
            <text x="300" y="305" fill="oklch(0.20 0.018 60)">eslint</text>
            <text x="350" y="305" fill="oklch(0.55 0.13 35)">● running</text>
            <text x="450" y="305" fill="oklch(0.20 0.018 60)">cargo check</text>
            <text x="540" y="305" fill="var(--color-ember)">● warming</text>
            <text x="880" y="305" textAnchor="end" fill="var(--color-ink-faint)">diagnostic workers healthy</text>
          </g>
        </svg>
        </div>

        <figcaption className="border-t border-rule-warm/60 px-5 py-3 archaic italic text-sm text-ink-muted">
          Read-only by design. Reads, edits, denies, and watcher health for
          every active session, on one time axis.
        </figcaption>
      </div>
    </figure>
  );
}
