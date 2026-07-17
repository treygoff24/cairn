/* Architecture layers diagram — three horizontal bands with the components
   that live in each. Adapters / daemon / storage. Restrained, blueprint-ish. */

export function ArchitectureLayers() {
  return (
    <figure className="my-12">
      <div className="border border-rule-warm bg-paper-warm overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-rule-warm/60 px-6 py-3">
          <span className="smallcaps tracking-[0.16em] text-sm text-ink-muted">
            cairn — one daemon per worktree
          </span>
          <span className="font-mono text-xs text-ink-faint">cutaway</span>
        </div>

        {/* Adapters band */}
        <Band
          label="adapters"
          sub="thin clients, harness-side"
          items={[
            { name: "Claude Code", tier: "target Tier 1" },
            { name: "Codex CLI",   tier: "target Tier 1 / 2" },
            { name: "Cursor",      tier: "expected Tier 2" },
            { name: "MCP",         tier: "deep-query surface" },
          ]}
        />
        <Connector label="typed local protocol  ·  versioned" />

        {/* Daemon band — the spine */}
        <div className="bg-paper-shade border-y border-rule-warm/60 px-6 py-6">
          <div className="flex items-baseline justify-between mb-4">
            <span className="font-mono text-sm text-ink-deep">cairn-daemon</span>
            <span className="archaic italic text-sm text-ink-muted">the spine</span>
          </div>
          <div className="grid grid-cols-2 md:grid-cols-3 gap-x-6 gap-y-3 font-mono text-sm">
            <DaemonRow name="session registry"     sub="identities + lineage" />
            <DaemonRow name="observation ledger"   sub="content-hashed reads" />
            <DaemonRow name="edit ledger"          sub="precondition hashes" />
            <DaemonRow name="extractor pool"       sub="tree-sitter + LSP" />
            <DaemonRow name="incremental engine"   sub="cairn-incremental ⌁ salsa" />
            <DaemonRow name="context scheduler"    sub="ledger-aware dedup" />
            <DaemonRow name="diagnostic workers"   sub="watch + delta attribution" />
            <DaemonRow name="MCP server"           sub="adapter, not substrate" />
            <DaemonRow name="event log"            sub="append-only, replayable" />
          </div>
        </div>
        <Connector label="content-hash addressed I/O" />

        {/* Storage band */}
        <Band
          label="storage"
          sub="three layers, one truth"
          items={[
            { name: "in-memory arenas",  tier: "sub-ms hot path" },
            { name: "mmap snapshot",     tier: "warm, cold-start" },
            { name: "SQLite (WAL)",      tier: "durable truth" },
          ]}
        />

        {/* Footer */}
        <div className="border-t border-rule-warm/60 px-6 py-3 flex items-center justify-between">
          <span className="archaic italic text-sm text-ink-muted">
            identified by{" "}
            <span className="text-ink-deep">canonical path + git common dir + config hash + protocol version</span>
          </span>
          <span className="font-mono text-xs text-oxide">fenced singleton</span>
        </div>
      </div>
    </figure>
  );
}

function Band({
  label,
  sub,
  items,
}: {
  label: string;
  sub: string;
  items: Array<{ name: string; tier: string }>;
}) {
  return (
    <div className="px-6 py-5">
      <div className="flex items-baseline justify-between mb-3">
        <span className="font-mono text-sm text-ink-deep">{label}</span>
        <span className="archaic italic text-sm text-ink-muted">{sub}</span>
      </div>
      <div className="grid grid-cols-2 md:grid-cols-4 gap-x-6 gap-y-2 font-mono text-sm">
        {items.map((it) => (
          <div key={it.name} className="flex flex-col gap-0.5">
            <span className="text-ink-deep">{it.name}</span>
            <span className="text-xs text-ink-faint smallcaps tracking-[0.12em]">
              {it.tier}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

function DaemonRow({ name, sub }: { name: string; sub: string }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-ink-deep">{name}</span>
      <span className="text-xs text-ink-faint">{sub}</span>
    </div>
  );
}

function Connector({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-3 px-6 py-2 bg-paper-warm">
      <span className="h-px flex-1 bg-rule-warm" aria-hidden />
      <span className="font-mono text-xs text-ink-faint smallcaps tracking-[0.16em]">
        {label}
      </span>
      <span className="h-px flex-1 bg-rule-warm" aria-hidden />
    </div>
  );
}
