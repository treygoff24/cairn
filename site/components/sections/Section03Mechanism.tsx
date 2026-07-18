import { Section, Content, TwoColumn } from "@/components/layout";
import { DisplayHeader, Lede, Prose, Mono, PullQuote, SmallCaps } from "@/components/type";
import { NextSection, SpecBlock } from "@/components/blocks";
import { SectionPlate } from "@/components/SectionPlate";
import { LedgerStack } from "@/components/diagrams/LedgerStack";

export function Section03Mechanism() {
  return (
    <Section
      id="mechanism"
      numeral="03"
      label="The Mechanism"
      ground="shade"
      plate={
        <SectionPlate
          src="/img/ledger-stones.png"
          alt="A long horizontal strip with clusters of small cairns deposited at intervals along a topographic-contour landscape. One cluster has a single oxide-tinted stone among it."
          numeral="03"
          label="The Mechanism — Ledger first"
          aspectRatio="24 / 9"
          objectPosition="center bottom"
        />
      }
    >
      <Content>
        <DisplayHeader level={2}>
          The ledger is the load-bearing primitive.
          <br />
          <span className="text-ink-muted">The graph feeds it.</span>
        </DisplayHeader>

        <Lede className="mt-8">
          A reviewer reads the spec, sees the symbol graph, and concludes this
          is a better code index with a coordination layer bolted on. That
          read is the trap.
        </Lede>

        <Prose className="mt-10" size="lg">
          A code graph tells you what is currently true about the repository.
          That is the truth oracle. It is not the API surface. The harder
          question — the one the substrate is built around — is{" "}
          <em className="archaic">what does this particular session currently
          believe, and has that belief expired?</em>
        </Prose>
        <Prose className="mt-6" size="lg">
          The answer lives in a ledger of observations, not in the graph.
        </Prose>
      </Content>

      <Content className="mt-20">
        <div className="flex items-center gap-4">
          <SmallCaps>3.1 — the primitive</SmallCaps>
          <span className="h-px flex-1 bg-rule-warm/60" aria-hidden />
        </div>
        <h3 className="mt-4 font-display text-3xl font-semibold tracking-tight text-ink-deep">
          Every supported read writes an Observation.
        </h3>
        <Prose className="mt-6">
          When an adapter reports that session α opened a file, the daemon
          records the read with a content hash, a repo-epoch reference, and the
          session that observed it. That row is the only ground truth about
          α&apos;s belief.
          Subsequent decisions about α — context dedup, staleness check,
          continuation briefing, multi-agent coordination — all read this
          ledger before they read the graph.
        </Prose>

        <SpecBlock title="Observation">
{`Observation {
  observation_id
  agent_session_id
  file_id
  path
  file_version          // content-hash addressed
  repo_epoch_id
  graph_version
  observed_at
  context_frame_id      // nullable
  task_id               // nullable
}`}
        </SpecBlock>

        <Prose>
          File versions are content-addressed. <Mono>file_version</Mono> is the
          hash of the bytes α actually saw, not a clock or a path. Same bytes
          returning later, even from a different branch, are recognized and
          revalidated.
        </Prose>
      </Content>

      <div className="mt-20" style={{ paddingInline: "var(--spacing-gutter)" }}>
        <div className="mx-auto" style={{ maxWidth: "var(--container-content)" }}>
          <LedgerStack />
        </div>
      </div>

      <Content className="mt-12">
        <PullQuote attribution="— the design pin, restated">
          A graph without an observation ledger is a clever librarian shouting
          facts into the room. A graph with one becomes a flight recorder, an
          air-traffic radar, and a memory palace in one local machine.
        </PullQuote>
      </Content>

      <Content className="mt-20">
        <div className="flex items-center gap-4">
          <SmallCaps>3.2 — two fingerprints, not one</SmallCaps>
          <span className="h-px flex-1 bg-rule-warm/60" aria-hidden />
        </div>
        <h3 className="mt-4 font-display text-3xl font-semibold tracking-tight text-ink-deep">
          Hard denies need a hard reason.
        </h3>
        <Prose className="mt-6">
          When β advances a dependency that α has observed, not every change
          should trip α&apos;s edit. A reformatted body, a renamed local, a
          rewritten comment — these are real edits but they don&apos;t alter
          what α actually relied on. So the substrate tracks{" "}
          <strong className="font-semibold">two</strong> fingerprints per file.
        </Prose>

        <TwoColumn
          ratio="1/1"
          className="mt-10"
          primary={
            <div>
              <SmallCaps tone="oxide">contract_fingerprint</SmallCaps>
              <p className="mt-3 text-ink-deep" style={{ lineHeight: "var(--leading-relaxed)" }}>
                Governs hard stale-deny eligibility. Built from the canonical
                projection of the file&apos;s exported surface — public types,
                signatures, route contracts, framework registrations. Whitespace,
                comments, and exported-body edits do not move this hash.
              </p>
              <ul className="mt-4 space-y-1 font-mono text-sm text-ink-muted">
                <li>↳ TypeScript: <Mono>.d.ts</Mono>-like projection from <Mono>tsc</Mono></li>
                <li>↳ Go: <Mono>go/packages</Mono> export data</li>
                <li>↳ Rust: <Mono>rustdoc</Mono> JSON / rust-analyzer HIR</li>
                <li>↳ Python: <Mono>__all__</Mono>, signatures, route decorators</li>
              </ul>
            </div>
          }
          secondary={
            <div>
              <SmallCaps tone="ember">implementation_fingerprint</SmallCaps>
              <p className="mt-3 text-ink-deep" style={{ lineHeight: "var(--leading-relaxed)" }}>
                Governs advisories, impact radius, strict-mode behavior. Built
                from normalized bodies of exported functions and methods.
                Changes when behavior plausibly changed, even if the contract
                didn&apos;t.
              </p>
              <ul className="mt-4 space-y-1 font-mono text-sm text-ink-muted">
                <li>↳ moves on body rewrites</li>
                <li>↳ moves on validator changes</li>
                <li>↳ moves on init-time side effects</li>
                <li>↳ feeds advisories, not denies</li>
              </ul>
            </div>
          }
        />

        <div className="mt-10 border-t border-rule-warm/60 pt-8">
          <p className="archaic italic text-xl text-ink-deep max-w-[60ch]" style={{ lineHeight: "1.45" }}>
            Hard denies require a verified or resolved contract change.{" "}
            <span className="text-ink-muted">Not vibes in a trench coat.</span>
          </p>
        </div>
      </Content>

      <NextSection className="mt-20" label="§ 04 — the daemon that owns the ledger." />
    </Section>
  );
}
