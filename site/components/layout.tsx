import { clsx } from "clsx";
import type { ReactNode } from "react";
import type { StyledChildrenProps } from "@/components/shared-types";

type SectionProps = StyledChildrenProps & {
  id?: string;
  numeral: string;
  label: string;
  /**
   * Optional ground class override. Most sections use the inherited
   * topographic background; some (the dark architecture cutaway, for
   * instance) opt out via `ground="none"`.
   */
  ground?: "paper" | "shade" | "deeper" | "dark" | "none";
  /**
   * When provided, the plate replaces SectionMeta because it carries the
   * numeral and label itself.
   */
  plate?: ReactNode;
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

function SectionMeta({
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

export function Content({
  children,
  className,
}: StyledChildrenProps) {
  return (
    <div
      className={clsx("mx-auto", className)}
      style={{ maxWidth: "var(--container-content)" }}
    >
      {children}
    </div>
  );
}

export function Wide({
  children,
  className,
}: StyledChildrenProps) {
  return (
    <div
      className={clsx("mx-auto", className)}
      style={{ maxWidth: "var(--container-wide)" }}
    >
      {children}
    </div>
  );
}

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
