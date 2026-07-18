import { Section, Content, TwoColumn, Hairline } from "@/components/layout";
import { DisplayHeader, Lede, Prose, Mono, PullQuote, SmallCaps, SubsectionLabel } from "@/components/type";
import { NextSection, SpecBlock, StatLine } from "@/components/blocks";

export function Section07Validation() {
  return (
    <Section id="validation" numeral="07" label="Validation" ground="shade">
      <Content>
        <DisplayHeader level={2}>
          One scalar, thirteen floors,
          <br />
          <span className="text-ink-muted">and a loop that grinds against it.</span>
        </DisplayHeader>

        <Lede className="mt-8">
          Every architectural call in this document has to survive contact
          with a benchmark. The point of measurement is not to count
          successes — it is to make refusing a bad change cheap and
          accepting a good one boring.
        </Lede>
        <p className="mt-6 font-mono text-sm text-oxide">
          Planned validation contract. The autonomous research loop is deferred P1.
        </p>
      </Content>

      <Content className="mt-20">
        <SubsectionLabel>7.1 — verified agent throughput</SubsectionLabel>
        <h3 className="mt-4 font-display text-3xl font-semibold tracking-tight text-ink-deep">
          Cost-weighted. Success-gated. Coordination-multiplied.
        </h3>
        <Prose className="mt-6">
          The headline scalar is{" "}
          <strong className="font-semibold">Verified Agent Throughput (VAT)</strong>.
          It is not a leaderboard number. It is the question:{" "}
          <em className="archaic">how much real, verified task completion
          per unit of cost, after penalty for breaking another agent&apos;s
          work?</em>
        </Prose>

        <SpecBlock title="VAT — per task">
{`normalized_cost =
    0.50 · (tokens         / baseline_tokens)
  + 0.20 · (tool_calls     / baseline_tool_calls)
  + 0.15 · (wall_clock_s   / baseline_wall_clock_s)
  + 0.10 · (diag_cpu_s     / baseline_diag_cpu_s)
  + 0.05 · latency_penalty

task_score = verified_success
           · (1 / normalized_cost)
           · coordination_multiplier

VAT = 100 · mean(task_score over benchmark tasks)`}
        </SpecBlock>

        <Prose>
          A no-system baseline scores ~100 when it succeeds at baseline cost.
          A system that preserves success and halves cost scores near 200. A
          system that saves tokens but breaks tasks collapses toward zero.
          The coordination multiplier ensures that token wins which come at
          the expense of another agent&apos;s build don&apos;t earn credit.
        </Prose>
      </Content>

      <Content className="mt-16">
        <SmallCaps tone="ember">coordination_multiplier</SmallCaps>
        <div className="mt-4 grid grid-cols-1 md:grid-cols-5 gap-x-6 gap-y-4">
          <CoordTier value="1.00" label="no inter-agent failure" />
          <CoordTier value="0.85" label="recovered stale-context incident" />
          <CoordTier value="0.50" label="downstream textual conflict" />
          <CoordTier value="0.25" label="downstream type-check failure" />
          <CoordTier value="0.00" label="task failed acceptance" oxide />
        </div>
      </Content>

      <Content className="mt-24">
        <SubsectionLabel>7.2 — the floors that refuse a release</SubsectionLabel>
        <h3 className="mt-4 font-display text-3xl font-semibold tracking-tight text-ink-deep">
          VAT can improve and we still ship nothing.
        </h3>
        <Prose className="mt-6">
          The benchmark gate refuses to accept a change — no matter how much
          VAT improves — if any floor is violated. Floors keep VAT from becoming
          the product&apos;s tiny idol.
        </Prose>

        <div className="mt-8 grid grid-cols-1 md:grid-cols-2 gap-x-12 gap-y-3 font-mono text-sm">
          <FloorRow metric="verified task success regression" limit="> 2 pp" />
          <FloorRow metric="hook-induced tool-call failure" limit="> 0.1%" />
          <FloorRow metric="read-hook p95 / p99" limit="> 50ms / 150ms" />
          <FloorRow metric="pre-edit check p95 / p99" limit="> 40ms / 120ms" />
          <FloorRow metric="false-positive dep-staleness deny" limit="> 5%" />
          <FloorRow metric="self-edit false-positive" limit="> 0.5%" />
          <FloorRow metric="introduced diagnostic recall" limit="< 95%" />
          <FloorRow metric="introduced diagnostic precision" limit="< 90%" />
          <FloorRow metric="high-confidence symbol / route precision" limit="< 95%" />
          <FloorRow metric="pushed-context overhead" limit="> 40% of saved" />
          <FloorRow metric="duplicate context token ratio" limit="> 15%" />
          <FloorRow metric="TOCTOU detection recall (capable adapters)" limit="< 90%" />
          <FloorRow metric="multi-agent task-pair success" limit="< no-system baseline" />
        </div>
      </Content>

      <Content className="mt-24">
        <SubsectionLabel>7.3 — the deferred autonomous loop</SubsectionLabel>

        <TwoColumn
          ratio="5/7"
          className="mt-6"
          primary={
            <h3 className="font-display text-3xl font-semibold tracking-tight text-ink-deep">
              Constraint.
              <br />
              Mechanical metric.
              <br />
              Autonomous iteration.
            </h3>
          }
          secondary={
            <Prose>
              The planned loop gives every feature an ablation flag. Every
              benchmark task produces a structured ledger of tokens, tool calls, hook latency,
              diagnostic deltas, staleness events, final acceptance.
              Mutations to extractor rules, scheduler policies, and graph
              heuristics run overnight against the corpus. A change is
              accepted only when VAT improves with{" "}
              <Mono>p &lt; 0.05</Mono> across multiple seeds and held-out
              agent models, and no floor is violated. Everything else is
              rejected — including changes that <em className="archaic">feel
              better</em>.
            </Prose>
          }
        />
      </Content>

      <Content className="mt-20">
        <Hairline />
        <div className="mt-10 grid grid-cols-1 md:grid-cols-4 gap-x-10 gap-y-8">
          <StatLine value="≥30%" label="token decrease target" sub="per successful task, scenario A" />
          <StatLine value="≥85%" label="parallel-pair success" sub="no inter-agent failure attributable" />
          <StatLine value="≥95%" label="TOCTOU recall" sub="on capable adapters" />
          <StatLine value="<5%" label="false-positive deny" sub="baseline-mode shipping ceiling" />
        </div>
      </Content>

      <Content className="mt-24">
        <PullQuote attribution="— from §13, risk 7">
          The benchmark could become the product&apos;s tiny idol. Held-out
          repos, rotated tasks, adversarial scenarios, multi-agent variation,
          floor metrics that catch regressions hidden by VAT.
        </PullQuote>
      </Content>

      <NextSection label="§ 08 — what we want you to push on." />
    </Section>
  );
}

function CoordTier({ value, label, oxide = false }: { value: string; label: string; oxide?: boolean }) {
  return (
    <div className="flex flex-col gap-1 border-t border-rule-warm/60 pt-3">
      <div className={`font-mono text-2xl tabular-nums ${oxide ? "text-oxide" : "text-ink-deep"}`}>
        {value}
      </div>
      <div className="text-sm text-ink-muted" style={{ lineHeight: "1.4" }}>{label}</div>
    </div>
  );
}

function FloorRow({ metric, limit }: { metric: string; limit: string }) {
  return (
    <div className="flex items-baseline justify-between border-t border-rule-warm/40 py-2 gap-4">
      <span className="text-ink-deep">{metric}</span>
      <span className="text-oxide tabular-nums whitespace-nowrap">{limit}</span>
    </div>
  );
}
