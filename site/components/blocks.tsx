import Image from "next/image";
import { clsx } from "clsx";
import type { ReactNode } from "react";

/* ─────────────────────────────────────────────────────────────────────────
   Content blocks
   - <Figure>    — image + caption + provenance line
   - <SpecBlock> — schema / type definition on dark surface, mono
   - <Cli>       — CLI / terminal sample on dark surface
   - <Marker>    — small inline glyph (cairn, observation, edit)
   - <StatLine>  — small numeric stat with label, mono
   ───────────────────────────────────────────────────────────────────────── */

type FigureProps = {
  src: string;
  alt: string;
  width: number;
  height: number;
  caption?: ReactNode;
  provenance?: ReactNode;
  className?: string;
  priority?: boolean;
};

export function Figure({
  src, alt, width, height, caption, provenance, className, priority = false,
}: FigureProps) {
  return (
    <figure className={clsx("my-10", className)}>
      <div className="relative w-full overflow-hidden">
        <Image
          src={src}
          alt={alt}
          width={width}
          height={height}
          priority={priority}
          className="w-full h-auto"
        />
      </div>
      {(caption || provenance) && (
        <figcaption className="mt-3 flex items-baseline justify-between gap-6 text-sm">
          {caption && <span className="text-ink-muted max-w-[60ch]">{caption}</span>}
          {provenance && (
            <span className="smallcaps text-ink-faint tracking-[0.16em] shrink-0">
              {provenance}
            </span>
          )}
        </figcaption>
      )}
    </figure>
  );
}

/** Schema / type-definition callout. Dark surface, mono, slight side padding. */
export function SpecBlock({
  title,
  children,
  className,
}: { title?: string; children: ReactNode; className?: string }) {
  return (
    <div
      className={clsx(
        "my-8 overflow-hidden",
        "bg-surface-dark text-surface-dark-fg",
        "border border-surface-dark-rule/60",
        className,
      )}
    >
      {title && (
        <div className="border-b border-surface-dark-rule/60 px-6 py-3">
          <span className="smallcaps tracking-[0.16em] text-surface-dark-fg/70 text-sm">
            {title}
          </span>
        </div>
      )}
      <pre className="px-6 py-5 font-mono text-sm leading-relaxed overflow-x-auto">
        <code>{children}</code>
      </pre>
    </div>
  );
}

/** CLI sample. Dark surface, with archaic "operator console" label. */
export function Cli({
  children,
  className,
  label = "operator console",
}: { children: ReactNode; className?: string; label?: string }) {
  return (
    <div
      className={clsx(
        "my-8 overflow-hidden",
        "bg-surface-dark text-surface-dark-fg",
        "border border-surface-dark-rule/60",
        className,
      )}
    >
      <div className="flex items-center justify-between border-b border-surface-dark-rule/60 px-6 py-3">
        <span className="smallcaps tracking-[0.16em] text-surface-dark-fg/70 text-sm">
          {label}
        </span>
        <span className="font-mono text-xs text-surface-dark-rust">$</span>
      </div>
      <pre className="px-6 py-5 font-mono text-sm leading-[1.55] overflow-x-auto">
        <code>{children}</code>
      </pre>
    </div>
  );
}

/** Small inline glyph — cairn, observation, edit, stale. SVG, decorative. */
export function Marker({
  kind = "cairn",
  className,
}: {
  kind?: "cairn" | "observation" | "edit" | "stale";
  className?: string;
}) {
  const cls = clsx("inline-block align-[-0.15em]", className);
  // All glyphs are 1em-sized so they scale with surrounding text.
  switch (kind) {
    case "observation":
      return (
        <svg viewBox="0 0 16 16" width="1em" height="1em" className={cls} aria-hidden>
          <circle cx="8" cy="8" r="3.2" fill="currentColor" />
          <circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" strokeWidth="1" opacity="0.35" />
        </svg>
      );
    case "edit":
      return (
        <svg viewBox="0 0 16 16" width="1em" height="1em" className={cls} aria-hidden>
          <path d="M3 13 L13 3" stroke="currentColor" strokeWidth="1.5" />
          <circle cx="13" cy="3" r="1.6" fill="currentColor" />
        </svg>
      );
    case "stale":
      return (
        <svg viewBox="0 0 16 16" width="1em" height="1em" className={cls} aria-hidden>
          <circle cx="8" cy="8" r="3.2" fill="oklch(0.55 0.13 35)" />
          <circle cx="8" cy="8" r="6.2" fill="none" stroke="oklch(0.55 0.13 35)" strokeWidth="1" opacity="0.55" />
        </svg>
      );
    case "cairn":
    default:
      // Small stone-stack icon — three stacked ellipses.
      return (
        <svg viewBox="0 0 16 16" width="1em" height="1em" className={cls} aria-hidden>
          <ellipse cx="8" cy="12.2" rx="5.5" ry="1.8" fill="currentColor" />
          <ellipse cx="8" cy="8.2"  rx="4.2" ry="1.6" fill="currentColor" />
          <ellipse cx="8" cy="4.6"  rx="3"   ry="1.4" fill="currentColor" />
        </svg>
      );
  }
}

/** Mono-flavored stat line: 30% / tokens per task / target floor */
export function StatLine({
  value,
  label,
  sub,
  className,
}: {
  value: ReactNode;
  label: ReactNode;
  sub?: ReactNode;
  className?: string;
}) {
  return (
    <div className={clsx("flex flex-col gap-1", className)}>
      <div className="font-mono text-3xl tabular-nums text-ink-deep">{value}</div>
      <div className="smallcaps tracking-[0.16em] text-sm text-ink-muted">{label}</div>
      {sub && <div className="archaic italic text-sm text-ink-faint">{sub}</div>}
    </div>
  );
}
