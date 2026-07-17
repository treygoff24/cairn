import { Section, Content, Hairline } from "@/components/layout";
import { DisplayHeader, Lede, SmallCaps, Mono } from "@/components/type";
import { Marker } from "@/components/blocks";

export function Section02Problem() {
  return (
    <Section id="problem" numeral="02" label="The Problem">
      <Content>
        <DisplayHeader level={2}>
          Most of the agent&apos;s day is spent in confusions
          <br />
          no one designed.
        </DisplayHeader>

        <Lede className="mt-8">
          Four failure modes common across AI coding-agent harnesses working in
          real repositories. They are primarily substrate failures, not model
          failures.
        </Lede>
      </Content>

      <div className="mt-20 grid gap-16 lg:grid-cols-2">
        <Beat
          numeral="2.1"
          title="Repo shape, rebuilt nightly"
          trace={[
            ["09:14", "session α starts. reads README, opens app/, opens lib/"],
            ["09:18", "α traces the auth flow across 6 files. 4,800 tokens"],
            ["14:02", "session β starts. same agent, same project, fresh context"],
            ["14:06", "β reads README, opens app/, opens lib/"],
            ["14:11", "β traces the auth flow across 6 files. 4,800 tokens"],
          ]}
        >
          Every session opens the same project for the first time. The agent
          rediscovers imports, callers, route maps, the path from entrypoint
          to service to storage. The substrate has no memory of what it has
          already shown to whom.
        </Beat>

        <Beat
          numeral="2.2"
          title="Edits made against ghosts"
          trace={[
            ["10:02", <>α reads <Mono>route.ts</Mono> at content_hash <Mono>a3f1</Mono></>],
            ["10:04", <>α plans changes to <Mono>auth-caller.ts</Mono></>],
            ["10:07", <>β edits <Mono>route.ts</Mono> → <Mono>b9d2</Mono>. exported contract changes.</>],
            ["10:12", "α writes to disk. lands cleanly. builds locally."],
            ["10:14", "CI fails. caller signature no longer matches route export."],
          ]}
          flavor="The agent was right about V1. V1 isn't the world anymore."
        >
          α observed <Mono>route.ts</Mono> at one version. β advanced it. α&apos;s
          edit to the caller is correct against an obsolete fingerprint of an
          exported contract — and lands on disk before anyone notices.
        </Beat>

        <Beat
          numeral="2.3"
          title="A check that didn’t hold"
          trace={[
            ["11:30:01", "pre-edit check: target file unchanged. allow."],
            ["11:30:02", "α dispatches the write."],
            ["11:30:02", "β writes to the same file half a millisecond earlier."],
            ["11:30:03", "α's write lands. β's change is silently overwritten."],
            ["⋯", "diff reviewer notices three commits later."],
          ]}
        >
          The check was honest at the moment it was made. Half a second later
          it wasn&apos;t true anymore. Time-of-check to time-of-use races aren&apos;t
          theoretical when two agents share a working tree.
        </Beat>

        <Beat
          numeral="2.4"
          title="The diagnostic dump"
          trace={[
            ["—", "edit applied to billing.ts (+12 lines, –3 lines)"],
            ["—", "tsc reports 217 errors. 2 are new. 215 pre-existed."],
            ["—", "agent: \"There are many type errors. Some may not be related.\""],
            ["—", "agent re-reads the dump. proposes 6 fixes, only 1 relevant."],
          ]}
          flavor="The agent was correct and unhelpful. Both at once."
        >
          After an edit, the tool runner dumps every warning it can find. Two
          come from this edit. The agent has no way to filter signal from
          ancient noise except by reading 215 lines of irrelevance.
        </Beat>
      </div>

      <Content className="mt-24">
        <Hairline />
        <p className="mt-10 archaic italic text-2xl text-ink-muted max-w-[58ch]" style={{ lineHeight: "1.4" }}>
          A graph of the repository alone solves none of these. Each one is a
          question about <span className="text-ink-deep">what this session has
          observed and what has changed since.</span>
        </p>
        <p className="mt-6 font-mono text-sm text-oxide flex items-center gap-3">
          <Marker kind="cairn" />
          <span>§ 03 sketches the mechanism that does.</span>
        </p>
      </Content>
    </Section>
  );
}

function Beat({
  numeral,
  title,
  children,
  trace,
  flavor,
}: {
  numeral: string;
  title: string;
  children: React.ReactNode;
  trace: Array<[string, React.ReactNode]>;
  flavor?: string;
}) {
  return (
    <article className="flex flex-col gap-6">
      <header className="flex items-baseline gap-4">
        <span className="font-mono text-oxide text-sm">{numeral}</span>
        <h3 className="font-display text-2xl font-semibold text-ink-deep tracking-tight">
          {title}
        </h3>
      </header>
      <div className="text-ink-deep">{children}</div>
      <div className="border-t border-rule-warm/60 pt-4">
        <SmallCaps tone="faint">trace</SmallCaps>
        <dl className="mt-3 grid grid-cols-[6rem_1fr] gap-x-5 gap-y-2 font-mono text-sm">
          {trace.map(([t, line], i) => (
            <div key={i} className="contents">
              <dt className="text-ink-faint tabular-nums">{t}</dt>
              <dd className="text-ink-muted">{line}</dd>
            </div>
          ))}
        </dl>
      </div>
      {flavor && (
        <p className="archaic italic text-base text-ink-muted">{flavor}</p>
      )}
    </article>
  );
}
