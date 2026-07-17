/* Ledger stack diagram — a vertical timeline of observation rows for a
   single session, with one row marked stale by an oxide accent dot. */

export function LedgerStack() {
  const rows = [
    { t: "09:14:02", file: "app/auth/route.ts",  ver: "a3f1", live: true  },
    { t: "09:14:08", file: "lib/session.ts",     ver: "8c12", live: true  },
    { t: "09:14:11", file: "lib/types.ts",       ver: "44ee", live: true  },
    { t: "09:14:14", file: "app/auth/route.ts",  ver: "a3f1", live: false },  // ← stale (overwritten)
    { t: "09:14:16", file: "tests/auth.test.ts", ver: "21bd", live: true  },
    { t: "09:14:19", file: "lib/db.ts",          ver: "9f0a", live: true  },
    { t: "09:14:22", file: "lib/session.ts",     ver: "8c12", live: true  },
  ];

  return (
    <figure className="my-12">
      <div className="border border-border-warm bg-paper-shade overflow-hidden">
        <div className="flex items-center justify-between border-b border-rule-warm/60 px-5 py-3">
          <span className="smallcaps tracking-[0.16em] text-sm text-ink-muted">
            ObservationLedger — session α — slice
          </span>
          <span className="font-mono text-xs text-ink-faint">7 / 142</span>
        </div>

        <div
          className="overflow-x-auto"
          tabIndex={0}
          role="region"
          aria-label="Scrollable observation ledger"
        >
          <table className="w-full min-w-[38rem] border-separate border-spacing-x-5 border-spacing-y-3 px-5 py-1 font-mono text-sm">
            <thead>
              <tr className="smallcaps text-left tracking-[0.16em] text-xs text-ink-faint">
                <th scope="col" className="font-normal">observed_at</th>
                <th scope="col" className="font-normal">file</th>
                <th scope="col" className="font-normal">version</th>
                <th scope="col" className="font-normal">status</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row) => (
                <tr key={`${row.t}-${row.file}`}>
                  <td className="tabular-nums text-ink-muted">{row.t}</td>
                  <td className="text-ink-deep">{row.file}</td>
                  <td className="tabular-nums text-ink-muted">{row.ver}</td>
                  <td className={row.live ? "text-ink-muted" : "text-oxide"}>
                    {row.live ? "● live" : "▲ stale"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <div className="border-t border-rule-warm/60 px-5 py-3 flex items-center justify-between">
          <span className="archaic italic text-sm text-ink-muted">
            β has advanced <span className="text-ink-deep">app/auth/route.ts</span> to <span className="text-ink-deep">b9d2</span> since 09:14:14.
          </span>
          <span className="font-mono text-xs text-oxide">↗ stale</span>
        </div>
      </div>
      <figcaption className="mt-3 text-sm text-ink-muted max-w-[60ch]">
        A single session&apos;s observation rows. One file was read twice in this
        slice; the second read is stale because another session has written
        the file since.
      </figcaption>
    </figure>
  );
}
