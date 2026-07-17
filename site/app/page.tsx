import { Section01Hero } from "@/components/sections/Section01Hero";
import { Section02Problem } from "@/components/sections/Section02Problem";
import { Section03Mechanism } from "@/components/sections/Section03Mechanism";
import { Section04Architecture } from "@/components/sections/Section04Architecture";
import { Section05Coordination } from "@/components/sections/Section05Coordination";
import { Section06Operator } from "@/components/sections/Section06Operator";
import { Section07Validation } from "@/components/sections/Section07Validation";
import { Section08Feedback } from "@/components/sections/Section08Feedback";

export default function Home() {
  return (
    <>
      <main>
        <Section01Hero />
        <Section02Problem />
        <Section03Mechanism />
        <Section04Architecture />
        <Section05Coordination />
        <Section06Operator />
        <Section07Validation />
        <Section08Feedback />
      </main>
      <footer className="border-t border-rule-warm py-12">
        <div
          className="mx-auto flex items-center justify-between px-[var(--spacing-gutter)] font-mono text-xs text-ink-faint"
          style={{ maxWidth: "var(--container-wide)" }}
        >
          <span>cairn · v0.1 · field notes from a build in progress</span>
          <span className="smallcaps tracking-[0.18em]">
            built with restraint
          </span>
        </div>
      </footer>
    </>
  );
}
