import { clsx } from "clsx";
import type { ReactNode } from "react";
import type { StyledChildrenProps } from "@/components/shared-types";

type DisplayHeaderProps = StyledChildrenProps & {
  level: 1 | 2 | 3;
  dark?: boolean;
  id?: string;
};

export function DisplayHeader({ level, children, className, dark = false, id }: DisplayHeaderProps) {
  const cls = clsx(
    "font-display font-bold tracking-tight",
    dark ? "text-surface-dark-fg" : "text-ink-deep",
    level === 1 && "text-balance",
    level === 2 && "text-balance",
    className,
  );
  const style: React.CSSProperties =
    level === 1
      ? { fontSize: "var(--text-hero)", lineHeight: "1.02", letterSpacing: "-0.028em" }
      : level === 2
      ? { fontSize: "clamp(2.5rem, 7vw, 3.75rem)", lineHeight: "1.05", letterSpacing: "-0.024em" }
      : { fontSize: "var(--text-2xl)", lineHeight: "1.15", letterSpacing: "-0.018em" };

  if (level === 1) return <h1 id={id} className={cls} style={style}>{children}</h1>;
  if (level === 2) return <h2 id={id} className={cls} style={style}>{children}</h2>;
  return <h3 id={id} className={cls} style={style}>{children}</h3>;
}

export function Lede({
  children,
  className,
  dark = false,
}: StyledChildrenProps & { dark?: boolean }) {
  return (
    <p
      className={clsx(
        "archaic italic max-w-[55ch]",
        dark ? "text-surface-dark-fg/85" : "text-ink-muted",
        className,
      )}
      style={{ fontSize: "var(--text-2xl)", lineHeight: "1.4" }}
    >
      {children}
    </p>
  );
}

export function Prose({
  children,
  className,
  size = "base",
  dark = false,
}: StyledChildrenProps & {
  size?: "base" | "lg";
  dark?: boolean;
}) {
  return (
    <p
      className={clsx(
        "max-w-[65ch]",
        size === "lg" ? "text-lg" : "text-base",
        dark ? "text-surface-dark-fg/90" : "text-ink-deep",
        className,
      )}
      style={{ lineHeight: "var(--leading-relaxed)" }}
    >
      {children}
    </p>
  );
}

export function PullQuote({
  children,
  attribution,
  className,
}: StyledChildrenProps & { attribution?: ReactNode }) {
  return (
    <figure
      className={clsx(
        "my-12 max-w-[58ch] border-l border-oxide pl-8",
        className,
      )}
    >
      <blockquote
        className="archaic text-3xl italic text-ink-deep"
        style={{ lineHeight: "1.32" }}
      >
        {children}
      </blockquote>
      {attribution ? (
        <figcaption className="mt-4 smallcaps text-sm text-ink-muted">
          {attribution}
        </figcaption>
      ) : null}
    </figure>
  );
}

export function SmallCaps({
  children,
  className,
  dark = false,
  tone = "muted",
}: StyledChildrenProps & {
  dark?: boolean;
  tone?: "muted" | "faint" | "oxide" | "ember";
}) {
  const color = dark
    ? "text-surface-dark-fg/75"
    : {
        muted: "text-ink-muted",
        faint: "text-ink-faint",
        oxide: "text-oxide",
        ember: "text-ember",
      }[tone];

  return (
    <span
      className={clsx(
        "smallcaps tracking-[0.16em]",
        color,
        className,
      )}
    >
      {children}
    </span>
  );
}

export function SubsectionLabel({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center gap-4">
      <SmallCaps>{children}</SmallCaps>
      <span className="h-px flex-1 bg-rule-warm/60" aria-hidden />
    </div>
  );
}

export function Mono({
  children,
  className,
}: StyledChildrenProps) {
  return (
    <code
      className={clsx(
        "font-mono text-[0.92em] text-ink-deep bg-paper-shade px-1.5 py-0.5 rounded-sm border border-border-warm/60",
        className,
      )}
    >
      {children}
    </code>
  );
}
