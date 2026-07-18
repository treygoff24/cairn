import Image from "next/image";
import { clsx } from "clsx";

export function SectionPlate({
  src,
  alt,
  numeral,
  label,
  className,
  caption,
  dark = false,
  aspectRatio = "16 / 9",
  objectPosition = "center",
}: {
  src: string;
  alt: string;
  numeral: string;
  label: string;
  caption?: string;
  className?: string;
  dark?: boolean;
  /** CSS aspect-ratio. Default 16/9; pass "21/9" for panoramic, "4/3" for tall. */
  aspectRatio?: string;
  /** CSS object-position. Default center. Use "center bottom" to anchor low. */
  objectPosition?: string;
}) {
  return (
    <figure className={clsx("relative w-full mb-12", className)}>
      <div className="relative w-full overflow-hidden border-y border-rule-warm/60">
        <div className="relative w-full" style={{ aspectRatio }}>
          <Image
            src={src}
            alt={alt}
            fill
            sizes="(min-width: 1280px) 1280px, 100vw"
            className="object-cover mix-blend-multiply"
            style={{ objectPosition }}
          />
        </div>

        {/* The vellum scrim keeps labels legible over varied plate images. */}
        <div
          aria-hidden
          className="absolute inset-x-0 bottom-0 flex items-baseline gap-5 px-6 py-4"
          style={{
            background:
              "linear-gradient(to top, oklch(0.97 0.012 80 / 0.92) 0%, oklch(0.97 0.012 80 / 0.7) 60%, transparent 100%)",
          }}
        >
          <span className="font-mono text-sm text-oxide">§ {numeral}</span>
          <span className="h-px flex-1 max-w-[6rem] bg-rule-warm" />
          <span className="smallcaps text-sm tracking-[0.18em] text-ink-muted">
            {label}
          </span>
        </div>
      </div>
      {caption && (
        <figcaption className={clsx(
          "mt-3 text-sm max-w-[60ch]",
          dark ? "text-surface-dark-fg/70" : "text-ink-muted"
        )}>
          {caption}
        </figcaption>
      )}
      <div className="h-6" aria-hidden />
    </figure>
  );
}
