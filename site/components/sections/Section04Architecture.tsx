import { Section, Content, TwoColumn } from "@/components/layout";
import { DisplayHeader, Lede, Prose, Mono, SubsectionLabel } from "@/components/type";
import { NextSection, SpecBlock } from "@/components/blocks";
import { SectionPlate } from "@/components/SectionPlate";
import { ArchitectureLayers } from "@/components/diagrams/ArchitectureLayers";

export function Section04Architecture() {
  return (
    <Section
      id="architecture"
      numeral="04"
      label="Architecture"
      plate={
        <SectionPlate
          src="/img/architecture-cutaway.png"
          alt="A technical-manual cutaway diagram showing horizontal strata representing storage tiers, drawn in ink on weathered paper with small annotation marks."
          numeral="04"
          label="Architecture — One spine, many doors"
          aspectRatio="24 / 9"
          objectPosition="center 38%"
        />
      }
    >
      <Content>
        <DisplayHeader level={2}>
          One daemon per worktree.
          <br />
          <span className="text-ink-muted">Every adapter is a thin client.</span>
        </DisplayHeader>

        <Lede className="mt-8">
          The architecture exists to make the ledger trustworthy on the hot
          path. Everything else is downstream of that requirement.
        </Lede>
      </Content>

      <div className="mt-16" style={{ paddingInline: "var(--spacing-gutter)" }}>
        <div className="mx-auto" style={{ maxWidth: "var(--container-content)" }}>
          <ArchitectureLayers />
        </div>
      </div>

      <Content className="mt-20">
        <SubsectionLabel>4.1 — the spine</SubsectionLabel>
        <h3 className="mt-4 font-display text-3xl font-semibold tracking-tight text-ink-deep">
          A single Rust daemon, fenced per worktree.
        </h3>
        <Prose className="mt-6">
          Each project worktree gets one daemon, identified by canonical
          filesystem path, git common directory, config hash, and protocol
          version. Three agents attached to the same worktree see the same
          ledger, the same graph, the same watcher set, the same diagnostic
          cache. If three harnesses launch in parallel, only one daemon
          process wins; the others attach as clients. Single-instance fencing
          is a DB-backed lease plus a generation ID plus a heartbeat file.
        </Prose>
        <Prose className="mt-6">
          <strong className="font-semibold">Rust</strong>, not because Rust
          tastes good, but because hook startup latency is product surface
          area. A read-hook firing on every file open at <Mono>p95 ≤ 35ms</Mono>{" "}
          cannot afford a 50ms cold start.
        </Prose>
      </Content>

      <Content className="mt-20">
        <SubsectionLabel>4.2 — three layers, one truth</SubsectionLabel>

        <TwoColumn
          ratio="5/7"
          className="mt-6"
          primary={
            <h3 className="font-display text-3xl font-semibold tracking-tight text-ink-deep">
              Durable below.
              <br />
              Mmap in the middle.
              <br />
              Arenas at the top.
            </h3>
          }
          secondary={
            <div className="space-y-6">
              <StorageRow
                tier="SQLite (WAL)"
                role="durable truth"
                detail="Append-only event log + materialized views. Survives crash, machine reboot, daemon restart. Everything above is derived."
              />
              <StorageRow
                tier="mmap snapshot"
                role="warm path / cold start"
                detail="Compact materialized snapshot, memory-mapped. Lets a fresh daemon serve useful reads within milliseconds, not seconds."
              />
              <StorageRow
                tier="in-memory arenas"
                role="hot path, sub-ms"
                detail="Hand-managed allocator regions for the read-hook query path. Hot symbol lookups, ledger slice queries, contract-fingerprint compares."
              />
            </div>
          }
        />
      </Content>

      <Content className="mt-20">
        <SubsectionLabel>4.3 — MCP is an adapter, not the substrate</SubsectionLabel>
        <h3 className="mt-4 font-display text-3xl font-semibold tracking-tight text-ink-deep">
          One spine. Many doors.
        </h3>
        <Prose className="mt-6">
          The daemon speaks a typed, versioned local protocol over Unix sockets
          or named pipes, with capability handshakes. Phase 1 uses JSON Lines;
          binary framing remains a later transport choice. MCP sits in front of
          that protocol and translates JSON-RPC tool calls into daemon calls.
          Hooks for Claude Code, Codex CLI, and Cursor are also adapters. None
          of them own their own graph or cache.
        </Prose>
        <Prose className="mt-6">
          The harness adapter registers what it can do via a capability
          bitset. The daemon then computes the strongest behavior allowed by
          that bitset — deny if the adapter can block edits, advisory if not,
          observation-only at minimum.
        </Prose>

        <SpecBlock title="adapter capability bitset">
{`can_pre_edit_block
can_pre_read_decorate
can_post_read_decorate
can_command_replace
can_modify_tool_input
can_async_notify
can_precompact
can_report_token_usage
can_report_exact_edit_diff
can_attach_file_precondition   // ← TOCTOU eligibility lives here`}
        </SpecBlock>

        <Prose>
          A capability tier matrix is part of the product surface, so the
          operator can read what each harness will actually deliver: Tier 1
          enforces, Tier 2 advises, Tier 3 only observes.
        </Prose>
      </Content>

      <Content className="mt-20">
        <SubsectionLabel>4.4 — push for timing, pull for depth</SubsectionLabel>
        <h3 className="mt-4 font-display text-3xl font-semibold tracking-tight text-ink-deep">
          Delivery is proven. Precision is the gate.
        </h3>
        <Prose className="mt-6">
          Hooks push bounded, provenance-aware facts during ordinary tool calls;
          MCP remains the deep-query surface. A tracer proved that pushed
          context reached and changed an agent&apos;s behavior at n=1. That is not
          production proof: the next hard milestone is keeping self-edit false
          positives below <Mono>0.5%</Mono>, because a channel that cries wolf
          trains the agent to ignore it.
        </Prose>
      </Content>

      <Content className="mt-20">
        <SubsectionLabel>4.5 — incremental, wrapped</SubsectionLabel>
        <h3 className="mt-4 font-display text-3xl font-semibold tracking-tight text-ink-deep">
          Salsa lives behind one crate seam.
        </h3>
        <Prose className="mt-6">
          File parse, symbol extraction, import resolution, route detection,
          fingerprints, diagnostics — every derived fact is a content-addressed
          query keyed by inputs (file hash, config hash, tool version, language
          plugin version, dependency hashes). When an input moves, only
          dependent queries invalidate. Salsa supplies the machinery, but it
          is pinned and wrapped behind a <Mono>cairn-incremental</Mono> crate.
          If Salsa proves wrong later, the blast radius is exactly one crate.
        </Prose>
      </Content>

      <NextSection
        className="mt-20"
        label="§ 05 — where the architecture earns its keep: more than one agent."
      />
    </Section>
  );
}

function StorageRow({
  tier,
  role,
  detail,
}: {
  tier: string;
  role: string;
  detail: string;
}) {
  return (
    <div>
      <div className="flex items-baseline justify-between">
        <span className="font-mono text-base text-ink-deep">{tier}</span>
        <span className="archaic italic text-sm text-ink-muted">{role}</span>
      </div>
      <p className="mt-1 text-sm text-ink-muted" style={{ lineHeight: "1.5" }}>
        {detail}
      </p>
    </div>
  );
}
