import { clsx } from "clsx";
import type { ReactNode } from "react";

/* ─────────────────────────────────────────────────────────────────────────
   Layout primitives
   - <Section> = page section with consistent vertical rhythm and meta strip
   - <Content> = primary readable column (76rem max, generous gutter)
   - <Wide>    = wider container for diagrams / cutaways
   - <Hairline> = 1px warm rule for section / sub-section separation
   ───────────────────────────────────────────────────────────────────────── */

type SectionProps = {
  id?: string;
  numeral: string;          // e.g. "02"
  label: string;            // e.g. "The Problem"
  children: ReactNode;
  /**
   * Optional ground class override. Most sections use the inherited
   * topographic background; some (the dark architecture cutaway, for
   * instance) opt out via `ground="none"`.
   */
  ground?: "paper" | "shade" | "deeper" | "dark" | "none";
  /**
   * Optional atmospheric plate image (codex-generated). When provided, the
   * default SectionMeta is suppressed because the plate carries the numeral
   * and label as an overlay.
   */
  plate?: ReactNode;
  className?: string;
};

const groundClass = {
  paper:  "",                          // inherits the global ground-topo
  shade:  "bg-paper-shade",
  deeper: "bg-paper-deeper",
  dark:   "bg-surface-dark text-surface-dark-fg",
  none:   "",
} as const;

export function Section({
  id,
  numeral,
  label,
  children,
  ground = "paper",
  plate,
  className,
}: SectionProps) {
  return (
    <section
      id={id}
      className={clsx(
        "relative w-full",
        "py-[var(--spacing-section-y)]",
        "px-[var(--spacing-gutter)]",
        groundClass[ground],
        className,
      )}
    >
      <div className="mx-auto w-full" style={{ maxWidth: "var(--container-wide)" }}>
        {plate ? plate : (
          <SectionMeta numeral={numeral} label={label} dark={ground === "dark"} />
        )}
        {children}
      </div>
    </section>
  );
}

export function SectionMeta({
  numeral,
  label,
  dark = false,
}: { numeral: string; label: string; dark?: boolean }) {
  return (
    <div
      className={clsx(
        "mb-12 flex items-baseline gap-6 font-mono text-sm",
        dark ? "text-surface-dark-fg/70" : "text-ink-muted",
      )}
    >
      <span className={clsx(
        "font-medium",
        dark ? "text-surface-dark-rust" : "text-oxide",
      )}>
        § {numeral}
      </span>
      <span
        className={clsx(
          "h-px flex-1 max-w-[6rem]",
          dark ? "bg-surface-dark-rule" : "bg-rule-warm",
        )}
        aria-hidden
      />
      <span className="smallcaps tracking-[0.18em]">{label}</span>
    </div>
  );
}

/** Primary text column. 76rem max. Children sit inside this. */
export function Content({
  children,
  className,
}: { children: ReactNode; className?: string }) {
  return (
    <div
      className={clsx("mx-auto", className)}
      style={{ maxWidth: "var(--container-content)" }}
    >
      {children}
    </div>
  );
}

/** Wider container for diagrams and cutaway visuals. */
export function Wide({
  children,
  className,
}: { children: ReactNode; className?: string }) {
  return (
    <div
      className={clsx("mx-auto", className)}
      style={{ maxWidth: "var(--container-wide)" }}
    >
      {children}
    </div>
  );
}

/** Asymmetric two-column layout. Defaults to a 7/5 split, content-left, accent-right. */
export function TwoColumn({
  primary,
  secondary,
  ratio = "7/5",
  className,
}: {
  primary: ReactNode;
  secondary: ReactNode;
  ratio?: "7/5" | "5/7" | "1/1" | "8/4";
  className?: string;
}) {
  const grid =
    ratio === "5/7"
      ? "lg:grid-cols-[5fr_7fr]"
      : ratio === "1/1"
      ? "lg:grid-cols-[1fr_1fr]"
      : ratio === "8/4"
      ? "lg:grid-cols-[8fr_4fr]"
      : "lg:grid-cols-[7fr_5fr]";
  return (
    <div
      className={clsx(
        "grid grid-cols-1 gap-x-16 gap-y-12",
        grid,
        className,
      )}
    >
      <div>{primary}</div>
      <div>{secondary}</div>
    </div>
  );
}

/** 1px warm full-width rule. Used between sections or sub-blocks. */
export function Hairline({ className }: { className?: string }) {
  return (
    <hr
      aria-hidden
      className={clsx(
        "border-0 bg-rule-warm h-px w-full",
        className,
      )}
    />
  );
}

/** A small decorative dingbat — three vertical stones, set in mono. */
export function Dingbat({ className }: { className?: string }) {
  return (
    <span
      aria-hidden
      className={clsx("font-mono text-ink-faint tracking-[0.3em]", className)}
    >
      ⋮⋮⋮
    </span>
  );
}
