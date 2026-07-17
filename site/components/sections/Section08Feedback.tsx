import { Section, Content, Hairline } from "@/components/layout";
import { DisplayHeader, Lede, SmallCaps } from "@/components/type";
import { Marker } from "@/components/blocks";

export function Section08Feedback() {
  return (
    <Section id="feedback" numeral="08" label="What we want you to push on">
      <Content>
        <DisplayHeader level={2}>
          The questions we&apos;re sitting with.
        </DisplayHeader>

        <Lede className="mt-8">
          Five places deserve adversarial critique, even where the underlying
          call is currently locked. Real failure modes move the design forward
          more than agreement does.
        </Lede>
      </Content>

      <Content className="mt-16">
        <ol className="space-y-12">
          <Ask
            n="1"
            title="Where does ledger-first stop earning its keep?"
          >
            We&apos;ve argued that the observation ledger is the substrate and
            the graph is its truth oracle, not the other way around. Pushing
            back: where does that ordering stop earning its keep? Is there a
            class of agent task where a graph-first product would deliver
            most of the value with less machinery?
          </Ask>

          <Ask
            n="2"
            title="Where could Rust miss its latency thesis?"
          >
            The case for Rust hinges on read-hook p95 ≤ 35ms and the claim
            that hook startup is product surface area. Where do you think
            that argument breaks? Are there agent harnesses where the hot
            path isn&apos;t actually hot, and a faster-to-iterate runtime
            would have been the better trade?
          </Ask>

          <Ask
            n="3"
            title="Where will the 26-crate integration tax appear first?"
          >
            Twenty-six crates from day one is justified by a parallel
            sub-agent build model: each crate is a unit of work owned by one
            worker. The risk is integration cost we feel later. If you&apos;ve
            done large Rust workspaces, where does this strategy fall apart?
          </Ask>

          <Ask
            n="4"
            title="Does the deny-loop guard actually close the trap?"
          >
            A false-positive deny is supposed to feel like a speed bump with
            a receipt: revalidate the minimum set, retry, never see the same
            cause twice. Imagine the worst-case path you&apos;d set up to
            break that promise. We want the trap, not the polished version.
          </Ask>

          <Ask
            n="5"
            title="What would you cut to ship sooner?"
          >
            The plan has seven phases. We treat Phases 1–5 as the floor of a
            useful first release. If you had to drop one of the P0 features
            from §10 of the spec — and only one — which one is most safely
            cut without the substrate failing its own promises?
          </Ask>
        </ol>
      </Content>

      <Content className="mt-24">
        <Hairline />

        <div className="mt-12 grid grid-cols-1 md:grid-cols-2 gap-x-12 gap-y-8">
          <div>
            <SmallCaps>where to send it</SmallCaps>
            <p className="mt-3 archaic italic text-xl text-ink-deep" style={{ lineHeight: "1.4" }}>
              Reply to whichever thread brought you here, or text Trey. No
              form. No survey. We want sentences, not stars.
            </p>
          </div>
          <div>
            <SmallCaps>what we&apos;ll do with it</SmallCaps>
            <p className="mt-3 archaic italic text-xl text-ink-deep" style={{ lineHeight: "1.4" }}>
              Read it the next morning. Argue back where we disagree. Patch
              the spec or the plan where you&apos;re right.
            </p>
          </div>
        </div>

        <div className="mt-16 flex items-center justify-between">
          <p className="flex items-center gap-3 font-mono text-sm text-oxide">
            <Marker kind="cairn" />
            <span>thanks for reading.</span>
          </p>
          <a
            href="#top"
            className="font-mono text-sm text-ink-muted hover:text-oxide transition-colors"
          >
            ↑ back to the top
          </a>
        </div>
      </Content>
    </Section>
  );
}

function Ask({
  n,
  title,
  children,
}: {
  n: string;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <li className="grid grid-cols-[2.5rem_minmax(0,1fr)] gap-x-5">
      <div className="font-mono text-2xl text-oxide tabular-nums">
        {n.padStart(2, "0")}
      </div>
      <div>
        <h3 className="font-display text-2xl font-semibold tracking-tight text-ink-deep">
          {title}
        </h3>
        <p className="mt-4 max-w-[60ch] text-base text-ink-deep" style={{ lineHeight: "var(--leading-relaxed)" }}>
          {children}
        </p>
      </div>
    </li>
  );
}
