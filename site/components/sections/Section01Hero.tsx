import Image from "next/image";
import { Lede, Prose, SmallCaps } from "@/components/type";
import { Marker } from "@/components/blocks";

export function Section01Hero() {
  return (
    <header
      id="top"
      className="relative w-full overflow-hidden"
      style={{ minHeight: "100dvh" }}
    >
      <div
        className="absolute inset-y-0 right-0 w-[42%] hidden lg:block pointer-events-none"
        style={{ opacity: 0.82 }}
      >
        <Image
          src="/img/hero-cairn.png"
          alt="A small cairn — a stack of weathered stones — standing on a topographic-map illustration, rendered in ink on vellum. One stone bears the warm oxide accent that signals this is the design's central metaphor."
          fill
          priority
          sizes="42vw"
          className="object-cover object-left mix-blend-multiply"
        />
        {/* Fade the plate before it reaches the text column. */}
        <div
          aria-hidden
          className="absolute inset-y-0 left-0 w-[62%]"
          style={{
            background:
              "linear-gradient(to right, var(--color-paper-warm) 0%, oklch(0.97 0.012 80 / 0.92) 30%, oklch(0.97 0.012 80 / 0.55) 65%, transparent 100%)",
          }}
        />
      </div>

      <div
        className="relative mx-auto flex h-full flex-col justify-between"
        style={{
          maxWidth: "var(--container-wide)",
          padding: "clamp(2rem, 5vw, 4rem) var(--spacing-gutter)",
          minHeight: "100dvh",
        }}
      >
        <div className="flex items-center justify-between text-sm">
          <div className="flex items-center gap-3 font-mono">
            <Marker kind="cairn" className="text-oxide" />
            <span className="font-medium text-ink-deep">Cairn</span>
            <span className="text-ink-faint">/</span>
            <SmallCaps tone="faint" className="tracking-[0.18em]">
              v0.1 ⋅ field notes from a build in progress
            </SmallCaps>
          </div>
          <nav aria-label="Section navigation" className="hidden md:flex items-center gap-6 font-mono text-xs text-ink-muted">
            <a href="#problem" className="hover:text-oxide transition-colors">§02 problem</a>
            <a href="#mechanism" className="hover:text-oxide transition-colors">§03 mechanism</a>
            <a href="#architecture" className="hover:text-oxide transition-colors">§04 architecture</a>
            <a href="#coordination" className="hover:text-oxide transition-colors">§05 coordination</a>
            <a href="#operator" className="hover:text-oxide transition-colors">§06 operator</a>
            <a href="#validation" className="hover:text-oxide transition-colors">§07 validation</a>
            <a href="#feedback" className="hover:text-oxide transition-colors">§08 feedback</a>
          </nav>
        </div>

        <div className="max-w-[42rem]">
          <div className="mb-8 flex items-center gap-4 font-mono text-xs text-ink-muted">
            <span className="text-oxide">§ 01</span>
            <span className="h-px w-12 bg-rule-warm" aria-hidden />
            <SmallCaps className="tracking-[0.18em]">the design pin</SmallCaps>
          </div>

          {/* Keep the title on two intentional lines at 1280–1440px. */}
          <h1
            className="font-display font-bold text-ink-deep tracking-tight text-balance"
            style={{
              fontSize: "clamp(2.75rem, 5.5vw, 4.75rem)",
              lineHeight: "1.04",
              letterSpacing: "-0.028em",
            }}
          >
            Not a code index.
            <br />
            <span className="text-oxide">A belief-management system.</span>
          </h1>

          <Lede className="mt-8 max-w-[36ch]">
            A graph tells you what the repository currently is. The ledger tells
            you what your agent currently believes — and whether that belief
            has expired.
          </Lede>

          <Prose className="mt-8 max-w-[38ch]" size="lg">
            Cairn is a local-first substrate for AI coding agents working in
            real repositories. Each supported read event is logged with a file
            version. Every edit decision carries an expected content hash;
            capable adapters enforce it at write time, while others degrade to
            advisory or post-edit verification. The graph exists and matters,
            but it sits beneath the ledger, feeding it. The ledger is the API.
          </Prose>
          <p className="mt-6 max-w-[52ch] border-t border-rule-warm/60 pt-4 font-mono text-sm text-ink-muted">
            <strong className="font-medium text-oxide">Build status:</strong>{" "}
            Phase 1. Unless marked validated, the capabilities below describe
            the binding design, not shipped behavior.
          </p>
          <Prose className="mt-4 max-w-[38ch] text-ink-muted" size="base">
            Most of this page is a defense of that ordering.
          </Prose>
        </div>

        <div className="flex items-center justify-between text-xs font-mono text-ink-faint">
          <span className="smallcaps tracking-[0.18em]">
            scroll for the long version
          </span>
          <span aria-hidden className="text-oxide">↓</span>
        </div>
      </div>
    </header>
  );
}
