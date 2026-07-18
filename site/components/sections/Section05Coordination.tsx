import { Section, Content, TwoColumn } from "@/components/layout";
import { DisplayHeader, Lede, Prose, Mono, PullQuote, SubsectionLabel } from "@/components/type";
import { NextSection, SpecBlock } from "@/components/blocks";
import { SectionPlate } from "@/components/SectionPlate";
import { TimelineLanes } from "@/components/diagrams/TimelineLanes";

export function Section05Coordination() {
  return (
    <Section
      id="coordination"
      numeral="05"
      label="Multi-Agent Coordination"
      plate={
        <SectionPlate
          src="/img/multi-agent-trail.png"
          alt="Two parallel trails winding through a contour-line landscape, occasionally crossing. Each has its own cairns. One cairn at a crossing point includes a single oxide-tinted stone."
          numeral="05"
          label="Coordination — Two trails, one ledger"
          aspectRatio="20 / 9"
          objectPosition="center"
        />
      }
    >
      <Content>
        <DisplayHeader level={2}>
          The same primitives that save tokens
          <br />
          <span className="text-ink-muted">also keep agents from overwriting each other.</span>
        </DisplayHeader>

        <Lede className="mt-8">
          A ledger keyed by session and file version is exactly what you need
          to ask: <em className="archaic">did anyone else change this file
          since α last read it?</em> Multi-agent coordination is not a new
          system. It is the same ledger asking a different question.
        </Lede>
      </Content>

      <div className="mt-16" style={{ paddingInline: "var(--spacing-gutter)" }}>
        <div className="mx-auto" style={{ maxWidth: "var(--container-content)" }}>
          <TimelineLanes />
        </div>
      </div>

      <Content className="mt-20">
        <SubsectionLabel>5.1 — three modes, one check</SubsectionLabel>
        <h3 className="mt-4 font-display text-3xl font-semibold tracking-tight text-ink-deep">
          Enforce when you can. Advise otherwise.
        </h3>
        <Prose className="mt-6">
          Before any edit, the daemon asks: what static-dependency set does
          the graph assign to the target file? Has any file in that set been
          modified by a <em>different session</em> since this session&apos;s
          last relevant observation? If yes, has the change moved a contract
          fingerprint that this session relies on?
        </Prose>

        <div className="mt-10 grid lg:grid-cols-[1fr_1.2fr_1fr] gap-x-10 gap-y-6">
          <ModeCard
            name="default"
            posture="deny"
            condition="adapter supports block + confirmed contract change"
            note="non-confirmable changes downgrade to advisory."
          />
          <ModeCard
            name="strict"
            posture="fail closed"
            condition="confirmed stale context or configured ledger failure"
            note="explicit operator choice. surfaces in the deny explanation."
          />
          <ModeCard
            name="advisory"
            posture="warn"
            condition="adapter has no deny capability"
            note="metrics still record what would have been denied."
          />
        </div>
      </Content>

      <Content className="mt-20">
        <SubsectionLabel>5.2 — the precondition closes the race</SubsectionLabel>

        <TwoColumn
          ratio="5/7"
          className="mt-6"
          primary={
            <h3 className="font-display text-3xl font-semibold tracking-tight text-ink-deep">
              An allow decision carries a hash.
            </h3>
          }
          secondary={
            <Prose>
              Every edit allow decision attaches an{" "}
              <Mono>expected_target_file_hash</Mono> precondition. Adapters
              that can attach file preconditions (the{" "}
              <Mono>can_attach_file_precondition</Mono> bit) verify it at
              write time. If β edits the target between the check and the
              write, the precondition fails — the daemon raises{" "}
              <Mono>edit_race_detected</Mono> and the strict-mode policy
              decides whether to retry, deny the next edit, or force a
              re-read. Long-term, file leases close this entirely. V1 closes
              it where the adapter cooperates and surfaces the gap honestly
              where it doesn&apos;t.
            </Prose>
          }
        />

        <SpecBlock title="DaemonDecision { decision_kind: deny }">
{`DenyDecision {
  deny_id
  severity                       // hard | soft
  target_file
  current_target_version
  stale_observation_versions
  dependency_causes[]
  minimum_revalidation_set[]     // ← the receipt
  proof_available
  override_policy
}`}
        </SpecBlock>

        <Prose>
          A false-positive deny should feel like a speed bump with a receipt,
          not a locked door guarded by a bureaucratic toaster. The{" "}
          <Mono>minimum_revalidation_set</Mono> tells the agent exactly what
          to re-read. After it observes those files, the same cause cannot
          deny again. The deny-loop guard makes that a rule, not an aspiration.
        </Prose>
      </Content>

      <Content className="mt-20">
        <SubsectionLabel>5.3 — subagent inheritance, lineage-aware</SubsectionLabel>
        <h3 className="mt-4 font-display text-3xl font-semibold tracking-tight text-ink-deep">
          Children get a snapshot. Parents don&apos;t auto-learn back.
        </h3>
        <Prose className="mt-6">
          When α spawns subagent α&prime;, the daemon copies α&apos;s relevant
          observations into α&prime;&apos;s session with{" "}
          <Mono>InheritedObservation</Mono> rows that preserve provenance
          (which observation in α it came from, what version it was, when α
          originally saw it). Child reads are tagged direct; inherited
          observations are tagged inherited. Staleness checks against
          inherited rows use the parent&apos;s observed version, marked stale
          inside the orientation packet if the world has moved.
        </Prose>
        <Prose className="mt-6">
          When α&prime; returns, the parent gets a{" "}
          <Mono>subagent_report_context_frame</Mono> with summary facts and
          pointers. Those become α&apos;s observations <em>of the report</em>,
          not of the underlying files. Without that rule, the parent would
          act as if it read files it never saw.
        </Prose>
      </Content>

      <Content className="mt-20">
        <PullQuote attribution="— the differentiator restated">
          The product is both a context substrate and a coordination layer.
          The same observation ledger that emits deltas instead of nav-map
          repetition is the one that asks <em>did this read get rotted while
          you were thinking about it?</em>
        </PullQuote>
      </Content>

      <NextSection label="§ 06 — what the operator sees when something looks haunted." />
    </Section>
  );
}

function ModeCard({
  name,
  posture,
  condition,
  note,
}: {
  name: string;
  posture: string;
  condition: string;
  note: string;
}) {
  return (
    <div className="flex flex-col gap-3 border-t border-rule-warm/60 pt-5">
      <div className="flex items-baseline justify-between">
        <span className="font-mono text-base text-ink-deep">{name}</span>
        <span className="smallcaps tracking-[0.16em] text-xs text-oxide">{posture}</span>
      </div>
      <p className="text-base text-ink-deep" style={{ lineHeight: "1.5" }}>
        {condition}
      </p>
      <p className="archaic italic text-sm text-ink-muted">
        {note}
      </p>
    </div>
  );
}
