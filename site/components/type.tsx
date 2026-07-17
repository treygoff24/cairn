import { clsx } from "clsx";
import type { ReactNode } from "react";

/* ─────────────────────────────────────────────────────────────────────────
   Typographic primitives
   - <DisplayHeader> — hero h1 (level=1) or section h2 (level=2)
   - <Lede>          — opening paragraph in archaic italic
   - <Prose>         — body paragraph
   - <PullQuote>     — block-quote in archaic face, single 1px oxide left rule
   - <Annotation>    — margin gloss, small archaic italic, faint
   - <SmallCaps>     — inline small-caps label
   ───────────────────────────────────────────────────────────────────────── */

type DisplayHeaderProps = {
  level: 1 | 2 | 3;
  children: ReactNode;
  className?: string;
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

/** Lede — italic archaic face, slightly larger than body, ink-muted. */
export function Lede({
  children,
  className,
  dark = false,
}: { children: ReactNode; className?: string; dark?: boolean }) {
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

/** Body paragraph. Caps line length at 65ch. */
export function Prose({
  children,
  className,
  size = "base",
  dark = false,
}: {
  children: ReactNode;
  className?: string;
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

/** Pull-quote. Archaic italic, single 1px oxide left rule. */
export function PullQuote({
  children,
  attribution,
  className,
}: { children: ReactNode; attribution?: ReactNode; className?: string }) {
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

/** Margin gloss — small archaic italic, faint, for sidebar annotations. */
export function Annotation({
  children,
  className,
}: { children: ReactNode; className?: string }) {
  return (
    <aside
      className={clsx(
        "archaic italic text-sm text-ink-faint max-w-[28ch]",
        className,
      )}
      style={{ lineHeight: "1.4" }}
    >
      {children}
    </aside>
  );
}

/** Inline small-caps label. */
export function SmallCaps({
  children,
  className,
  dark = false,
  tone = "muted",
}: {
  children: ReactNode;
  className?: string;
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

/** Inline mono label — used for filenames, types, identifiers in prose. */
export function Mono({
  children,
  className,
}: { children: ReactNode; className?: string }) {
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
