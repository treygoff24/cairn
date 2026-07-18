type Event = {
  t: number;          // 0–100 position along the lane
  kind: "read" | "edit" | "deny" | "revalidate";
  label: string;
  /** label position: "above" or "below". Defaults to below. */
  pos?: "above" | "below";
};

type Point = { x: number; y: number };

const LANE_GAP   = 90;     // vertical space between lanes (room for stacked labels)
const TOP_PAD    = 28;
const STROKE     = "var(--color-rule-warm)";
const INK        = "var(--color-ink-deep)";
const FAINT      = "var(--color-ink-faint)";
const OXIDE      = "var(--color-oxide)";
const EMBER      = "var(--color-ember)";

export function TimelineLanes() {
  const alphaEvents: Event[] = [
    { t: 6,  kind: "read", label: "route.ts @ a3f1",              pos: "above" },
    { t: 18, kind: "read", label: "session.ts @ 8c12",            pos: "below" },
    { t: 30, kind: "read", label: "auth-caller.ts @ 44ee",        pos: "above" },
    { t: 58, kind: "edit", label: "edit → auth-caller.ts",        pos: "below" },
    { t: 64, kind: "deny", label: "DENY: contract changed",       pos: "above" },
    { t: 76, kind: "revalidate", label: "re-read route.ts @ b9d2", pos: "below" },
    { t: 90, kind: "edit", label: "edit allowed",                 pos: "above" },
  ];
  const betaEvents: Event[] = [
    { t: 12, kind: "read", label: "route.ts @ a3f1",                       pos: "above" },
    { t: 36, kind: "edit", label: "→ route.ts @ b9d2 (contract moved)",    pos: "below" },
    { t: 50, kind: "read", label: "tests/route.test.ts",                   pos: "above" },
    { t: 72, kind: "edit", label: "→ tests/route.test.ts",                 pos: "below" },
  ];

  const alpha_y = TOP_PAD;
  const beta_y  = TOP_PAD + LANE_GAP;
  const axis_y  = beta_y + 56;
  const svg_h   = axis_y + 30;

  const xFrom = xpos(36);
  const xTo   = xpos(64);

  return (
    <figure className="my-12">
      <div className="border border-border-warm bg-paper-warm overflow-hidden">
        <div className="flex items-center justify-between border-b border-rule-warm/60 px-5 py-3">
          <span className="smallcaps tracking-[0.16em] text-sm text-ink-muted">
            two sessions · one worktree · one ledger
          </span>
          <span className="font-mono text-xs text-ink-faint">strip chart</span>
        </div>

        <div
          className="px-6 py-8 relative overflow-x-auto"
          tabIndex={0}
          role="region"
          aria-label="Scrollable multi-agent timeline"
        >
          <svg
            viewBox={`0 0 900 ${svg_h}`}
            className="w-full min-w-[56rem] h-auto"
            role="img"
            aria-label="Two agent timelines with one detected staleness event"
          >
            <defs>
              <pattern id="tl-contour" x="0" y="0" width="44" height="44" patternUnits="userSpaceOnUse">
                <circle cx="22" cy="22" r="15" fill="none" stroke={STROKE} strokeWidth="0.4" opacity="0.16" />
              </pattern>
            </defs>
            <rect x="0" y="0" width="900" height={svg_h} fill="url(#tl-contour)" />

            <g fontFamily="var(--font-jetbrains)" fontSize="11" fill={INK}>
              <text x="0" y={alpha_y + 4}>session α</text>
              <text x="0" y={beta_y  + 4}>session β</text>
            </g>

            <line x1="80" y1={alpha_y} x2="880" y2={alpha_y} stroke={STROKE} strokeWidth="1" />
            <line x1="80" y1={beta_y}  x2="880" y2={beta_y}  stroke={STROKE} strokeWidth="1" />

            {/* β → α causality link — kept oxide but at lower opacity so the
                deny marker remains the single load-bearing accent moment */}
            <path
              d={`M ${xFrom} ${beta_y - 10} C ${xFrom + 35} ${beta_y - 40}, ${xTo - 35} ${alpha_y + 40}, ${xTo} ${alpha_y + 10}`}
              fill="none"
              stroke={OXIDE}
              strokeWidth="1.1"
              strokeDasharray="3 3"
              opacity="0.55"
            />
            <polygon
              points={`${xTo},${alpha_y + 10} ${xTo - 4},${alpha_y + 16} ${xTo + 4},${alpha_y + 16}`}
              fill={OXIDE}
              opacity="0.55"
            />

            <Lane events={alphaEvents} y={alpha_y} />
            <Lane events={betaEvents} y={beta_y} />

            <g fontFamily="var(--font-jetbrains)" fontSize="9" fill={FAINT}>
              <line x1="80" y1={axis_y} x2="880" y2={axis_y} stroke={STROKE} strokeWidth="0.5" />
              {[0, 25, 50, 75, 100].map((p) => (
                <g key={p}>
                  <line x1={xpos(p)} y1={axis_y - 3} x2={xpos(p)} y2={axis_y + 3} stroke={STROKE} strokeWidth="0.5" />
                  <text x={xpos(p)} y={axis_y + 16} textAnchor="middle">
                    t+{p === 0 ? "0s" : `${p * 12}s`}
                  </text>
                </g>
              ))}
            </g>
          </svg>

          <div className="mt-6 flex flex-wrap gap-x-8 gap-y-2 font-mono text-xs text-ink-muted">
            <Legend kind="read"       label="read" />
            <Legend kind="edit"       label="edit" />
            <Legend kind="deny"       label="deny (stale contract)" />
            <Legend kind="revalidate" label="revalidate" />
            <LinkLegend />
          </div>
        </div>

        <figcaption className="border-t border-rule-warm/60 px-5 py-3">
          <span className="archaic italic text-sm text-ink-muted">
            β advanced <span className="text-ink-deep">route.ts</span>{" "}
            to a new contract. α&apos;s next edit attempt is denied with a
            minimum_revalidation_set; after the re-read, the same cause
            cannot deny again.
          </span>
        </figcaption>
      </div>
    </figure>
  );
}

function Lane({ events, y }: { events: Event[]; y: number }) {
  return (
    <g>
      {events.map((e, i) => {
        const x = xpos(e.t);
        const isAbove = e.pos === "above";
        const labelY = isAbove ? y - 18 : y + 22;
        return (
          <g key={i}>
            <line
              x1={x}
              y1={y}
              x2={x}
              y2={isAbove ? y - 12 : y + 12}
              stroke={FAINT}
              strokeWidth="0.5"
              opacity="0.6"
            />
            {e.kind === "read"       && <ReadMark       x={x} y={y} />}
            {e.kind === "edit"       && <EditMark       x={x} y={y} />}
            {e.kind === "deny"       && <DenyMark       x={x} y={y} />}
            {e.kind === "revalidate" && <RevalidateMark x={x} y={y} />}
            <text
              x={x}
              y={labelY}
              fontFamily="var(--font-jetbrains)"
              fontSize="9.5"
              fill={e.kind === "deny" ? INK : FAINT}
              fontWeight={e.kind === "deny" ? 600 : 400}
              textAnchor="middle"
            >
              {e.label}
            </text>
          </g>
        );
      })}
    </g>
  );
}

function ReadMark({ x, y }: Point) {
  return (
    <g>
      <circle cx={x} cy={y} r="5" fill={INK} />
      <circle cx={x} cy={y} r="9" fill="none" stroke={INK} strokeWidth="0.75" opacity="0.35" />
    </g>
  );
}

function EditMark({ x, y }: Point) {
  return (
    <g>
      <line x1={x - 5} y1={y - 5} x2={x + 5} y2={y + 5} stroke={INK} strokeWidth="1.5" />
      <line x1={x - 5} y1={y + 5} x2={x + 5} y2={y - 5} stroke={INK} strokeWidth="1.5" />
    </g>
  );
}

function DenyMark({ x, y }: Point) {
  return (
    <g>
      <circle cx={x} cy={y} r="6.5" fill={OXIDE} />
      <circle cx={x} cy={y} r="11" fill="none" stroke={OXIDE} strokeWidth="1" opacity="0.55" />
    </g>
  );
}

function RevalidateMark({ x, y }: Point) {
  return (
    <g>
      <circle cx={x} cy={y} r="5" fill={EMBER} />
      <circle cx={x} cy={y} r="9" fill="none" stroke={EMBER} strokeWidth="0.75" opacity="0.45" />
    </g>
  );
}

function xpos(p: number) {
  return 80 + (p / 100) * 800;
}

function Legend({ kind, label }: { kind: Event["kind"]; label: string }) {
  return (
    <span className="inline-flex items-center gap-2">
      <svg width="16" height="16" viewBox="-8 -8 16 16" aria-hidden>
        {kind === "read"       && <ReadMark       x={0} y={0} />}
        {kind === "edit"       && <EditMark       x={0} y={0} />}
        {kind === "deny"       && <DenyMark       x={0} y={0} />}
        {kind === "revalidate" && <RevalidateMark x={0} y={0} />}
      </svg>
      {label}
    </span>
  );
}

function LinkLegend() {
  return (
    <span className="inline-flex items-center gap-2">
      <svg width="32" height="12" viewBox="0 0 32 12" aria-hidden>
        <line x1="0" y1="6" x2="32" y2="6" stroke="oklch(0.55 0.13 35)" strokeWidth="1.25" strokeDasharray="3 3" />
      </svg>
      causality link
    </span>
  );
}
