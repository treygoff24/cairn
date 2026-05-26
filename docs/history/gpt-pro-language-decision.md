My answer: **Rust, single binary, one shared per-project daemon, SQLite as durable truth, mmap/in-memory read models for hot queries, hooks as the privileged interface, MCP as an adapter.**

I would not make this a TypeScript tool. I would not split hook layer and indexer across languages. I would keep external diagnostics as subprocesses, because the “real checker” is the product promise, but the product itself should be Rust.

The core mistake to avoid is treating Code Briefcase as “an MCP code index with hooks.” The best version is **an agent-interception runtime** whose index, diagnostics, MCP tools, atomic edit machinery, and installer all serve the hook path.

## 1. Language: Rust

Pick **Rust**.

Not “Rust because it is fast” in the generic Hacker News sense. Rust is the right language because this product is a latency-sensitive, local systems daemon that needs to parse code, maintain incremental state, supervise subprocesses, watch files, handle concurrent clients, persist crash-safe indexes, ship as one binary, and expose deterministic low-latency answers to agent hooks.

Tree-sitter is already designed as an incremental parser, can update syntax trees as source changes, supports multi-language documents via included ranges, and has official Rust bindings. That maps directly onto your workload. ([Tree-sitter](https://tree-sitter.github.io/tree-sitter/)) Salsa exists in Rust, is explicitly built for efficient incremental recomputation, and is used by rust-analyzer for exactly the kind of “recompute only what changed” programming model you are currently approximating by hand. ([Salsa Rust](https://salsa-rs.github.io/salsa/overview.html))

The killer point: **hook startup latency is product surface area.** A pull-mode MCP tool can tolerate 50 ms of runtime overhead because the agent chooses to call it occasionally. A push-mode hook fires constantly, often on dumb reads, exploratory sub-agent activity, and edits. If you choose TypeScript, you are choosing to spend latency budget on runtime startup every time your differentiator fires. That is backwards. CodeGraph’s Node bundling is an impressive distribution solution, but it is solving a different problem: a pull-first MCP graph. Their own docs describe a bundled runtime and SQLite/FTS5 local database, plus a strong MCP surface, but the center of gravity remains MCP queries. ([GitHub](https://github.com/colbymchenry/codegraph))

Go is acceptable but not optimal. You will spend the next decade building an AST/resolver/incremental-computation engine, and Go’s type system and pattern-matching ergonomics are weaker for that shape of program. Zig is too much ecosystem risk for a 20-language, diagnostics, installer, MCP, watcher, SQLite, Windows/macOS/Linux product. Python should leave the hot path entirely.

I would make the repo Rust-first and maybe expose a tiny TypeScript SDK or generated JSON schemas for agent ecosystem convenience. But the shipped tool should be one Rust binary.

## 2. Architecture: one binary, many roles, one project brain

The process model should look like this:

```text
Agent harness hook
  -> briefcase hook read/edit/post-edit/session
  -> project socket / named pipe
  -> shared per-project daemon

Agent MCP client
  -> briefcase mcp stdio proxy
  -> same project socket / named pipe
  -> same shared per-project daemon

Daemon
  -> in-memory read model
  -> SQLite durable store
  -> mmap snapshot cache
  -> tree-sitter/Salsa incremental engine
  -> diagnostics supervisors
  -> file watcher
  -> edit/version ledger
```

The binary has subcommands like:

```text
briefcase hook read
briefcase hook pre-edit
briefcase hook post-edit
briefcase hook session-start
briefcase mcp
briefcase daemon
briefcase index
briefcase status
briefcase install
briefcase uninstall
briefcase doctor
```

The hook subprocess must be stupidly small. It should parse the harness JSON from stdin, resolve the project root, connect to the daemon socket, send a typed event, receive a typed response, and write the required harness JSON to stdout. It should not import grammars, open SQLite, initialize logging frameworks, load config-heavy subsystems, or parse source files. The cold hook path should be “process starts, parse JSON, connect, write, exit.”

For internal IPC, I would not use MCP and I would not use HTTP. MCP is the public agent protocol. HTTP is useful for debugging and remote service modes, but it is the wrong default local control plane. MCP itself uses JSON-RPC and defines stdio and Streamable HTTP transports, which is exactly why `briefcase mcp` should exist as an adapter rather than as the daemon substrate. ([Model Context Protocol](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports))

Use Unix domain sockets on macOS/Linux and named pipes on Windows. Use a length-prefixed binary protocol such as MessagePack, CBOR, or postcard with explicit protocol versions. JSON remains at the harness and MCP boundaries. Internally, make the protocol typed, versioned, cheap, and boring.

The daemon should be **one per project worktree**, not one per agent. The project identity should include canonical root, git worktree identity, config hash, and binary protocol version. Do not accidentally share one index across two git worktrees with different checked-out contents. Multiple Claude Code windows, Codex CLI sessions, Cursor sessions, opencode sessions, and sub-agents should all attach to the same daemon for that worktree.

Startup should be race-free:

```text
hook/proxy tries socket
if unavailable:
  acquire project lock
  spawn daemon detached
  wait briefly for socket
  connect or fail-open
```

Fail-open matters. A broken Code Briefcase must not brick the agent. For read/session/pre-edit hooks, failure should return a tiny “briefcase unavailable/warming” payload or no-op. For post-edit diagnostics, failure should report that diagnostics were unavailable, not block the edit forever. For managed atomic edits, version conflicts can fail closed, because safety is the point.

The daemon should have a linger timeout. When the last client disconnects, it remains alive for a short period so back-to-back agent sessions do not pay cold-start cost. CodeGraph recently moved in this shared-daemon direction, including one background daemon per project, shared watcher, shared SQLite connection, and a stdio-to-socket proxy model for MCP clients. That is conceptually correct and worth copying, but Code Briefcase should make that daemon serve hooks first. ([GitHub](https://github.com/colbymchenry/codegraph/releases))

## 3. Storage: SQLite durable truth plus a disposable hot read model

Use **embedded SQLite in WAL mode** as the durable store. Do not build a bespoke durable database unless you want the next five years to become “accidental database company.” SQLite WAL gives the concurrency property you want for local multi-client tools: readers do not block writers and writers do not block readers in normal WAL operation. ([SQLite](https://sqlite.org/wal.html?utm_source=chatgpt.com)) SQLite FTS5 is also a good durable baseline for full-text search. ([SQLite](https://sqlite.org/fts5.html?utm_source=chatgpt.com))

But hot hook responses should not depend on SQL query latency. SQLite is the durable store, migration substrate, recovery mechanism, and cold-start source of truth. The daemon should answer hot queries from memory.

I would maintain three layers:

First, SQLite:

```text
.code-briefcase/briefcase.db
```

Tables roughly like:

```text
files
  file_id, path, language, hash, size, mtime_ns, generation, dirty_state

symbols
  symbol_id, file_id, language, kind, name, qualified_name,
  signature, stable_identity_hash, span, body_span, visibility,
  structural_hash, generation

refs
  ref_id, file_id, enclosing_symbol_id, kind, raw_name,
  namespace, span, target_symbol_id nullable,
  resolution_status, confidence, provenance, generation

unresolved_refs
  ref_id, raw_name, namespace, candidates_json, reason, last_attempt_generation

edges
  src_symbol_id, dst_symbol_id, kind, confidence, provenance, generation

routes
  route_id, framework, method, pattern, handler_ref_id, handler_symbol_id nullable,
  file_id, confidence, provenance

diagnostics
  diagnostic_id, tool, file_id, span, severity, code, message,
  file_hash, config_hash, tool_version, generation

edit_log
  seq, session_id, agent_id, file_id, op, old_hash, new_hash,
  range, timestamp, tool_call_id

leases
  file_id, agent_id, session_id, lease_until, expected_hash

context_frames
  frame_id, session_id, event_id, dependency_fingerprint,
  token_cost, content_hash, emitted_at
```

Second, an in-memory read model:

```text
dense symbol arena
interned strings
file_id -> outline
symbol_name -> symbol_ids
qualified_name -> symbol_ids
file_id -> import edges
symbol_id -> callees adjacency list
symbol_id -> callers adjacency list
route pattern -> route nodes
word/trigram index for warm search
diagnostic index by file/symbol/session
```

This is where sub-ms warm query performance comes from. codedb’s in-memory word/trigram indexes, structural outlines, dependency graph, snapshots, and atomic edits are directionally correct. The public docs describe exactly those primitives, including an inverted word index, trigram index, dependency graph, version store, line-range edits, snapshots, and remote public repo queries. ([GitHub](https://github.com/justrach/codedb))

Third, a disposable mmap snapshot:

```text
.code-briefcase/snapshot.vN.bin
```

This is a compact read-only materialized view generated from SQLite: symbol arena, string table, adjacency lists, file outlines, route table, and name indexes. On daemon start, mmap the snapshot, answer immediately, then reconcile against the working tree. If the snapshot is stale or corrupt, delete it and rebuild from SQLite. This gives you codedb-style startup speed without making a custom binary store the system of record.

So the answer to “SQLite or bespoke memory-mapped store?” is: **SQLite for truth, mmap for speed, memory for serving.** The bespoke part should be a cache, never the only copy.

## 4. Incremental computation: Salsa, but only for pure computation

Use Salsa for the pure query graph:

```text
input: project_config
input: file_content(file_id)
input: toolchain_config
input: language_registry

query: parse_tree(file_id)
query: extracted_facts(file_id)
query: module_exports(file_id)
query: local_scope_graph(file_id)
query: resolved_ref(ref_id)
query: symbol_outline(file_id)
query: call_edges(symbol_id)
query: impact_radius(symbol_id, depth)
query: context_frame(hook_event)
```

Do not try to force SQLite writes, file watching, subprocess diagnostics, and hook I/O into Salsa. Those are impure event sources and sinks. Salsa should compute derived facts from explicit inputs. The daemon owns the event loop and materializes Salsa outputs into the DB and hot read model.

The non-obvious Salsa lesson from rust-analyzer is **durability and early cutoff**. If a file edit changes whitespace or a comment, the source hash changed, but the symbol outline may not have changed. If the outline is unchanged, the call graph should not be invalidated. The rust-analyzer durable-incrementality writeup calls out exactly this shape: query dependencies, early cutoff, and the importance of keeping volatile position data from polluting higher-level structural outputs. ([Rust Analyzer](https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html))

That means symbol identity must not be “path plus byte range.” Byte ranges are presentation. Stable identity should be derived from language, path/module, qualified name, kind, signature shape, and a structural hash. Spans are attached facts that can move without changing the symbol.

This is one of the most important design choices in the whole system. If positions leak into identity, every small edit creates cascading invalidations and fake “new” symbols. If structural identity is stable, you get cheap recomputation, clean diagnostics deltas, and sane multi-agent edit tracking.

## 5. Language extractor structure: declarative facts first, custom code only where necessary

The current 4000-line extractor monolith should disappear.

I would split extraction into three layers:

```text
LanguageSpec
  extension mapping
  tree-sitter grammar
  tree-sitter query files
  capture-to-fact mapping
  default import/call/scope rules

LanguagePlugin
  language-specific quirks
  custom scope resolver
  custom import resolver
  embedded language handling
  signature rendering

FrameworkPlugin
  route, bridge, ORM, test, UI, RPC, and event-system rules
```

Most languages should be mostly data:

```text
languages/typescript/
  spec.toml
  symbols.scm
  refs.scm
  imports.scm
  scopes.scm
  plugin.rs only if needed
```

The extractor should emit **semantic facts**, not final graph edges:

```text
SymbolFact
RefFact
ImportFact
ExportFact
ScopeFact
DecoratorFact
AnnotationFact
CallExpressionFact
StringLiteralFact
FileRouteCandidate
BridgeCandidate
```

Then resolvers consume those facts and produce graph edges.

This is where I would differ slightly from a naive “CodeGraph has tiny language descriptors, copy that exactly” approach. Declarative extractors are absolutely right. CodeGraph’s published architecture says it uses tree-sitter extraction, language-specific queries, SQLite storage, and then a resolution phase for calls, imports, inheritance, and framework patterns. ([GitHub](https://github.com/colbymchenry/codegraph)) But framework and cross-language awareness should mostly live in the resolver layer, not buried in per-language extraction.

For example, the TypeScript extractor should not “know Express.” It should emit call facts, import facts, object/member call facts, string literal facts, and function symbol facts. The Express framework plugin should interpret `router.get("/x", handler)` as a route node linked to a handler. The Python extractor should emit decorators and calls. The FastAPI plugin should interpret `@router.get(...)`. The Swift and ObjC extractors should emit selector facts. The bridge resolver should link Swift and ObjC candidates. CodeGraph’s route and React Native/iOS bridge awareness is first-principles correct, but those should be resolver plugins over generic extracted facts. ([GitHub](https://github.com/colbymchenry/codegraph))

The rule of thumb: **language plugins parse syntax; framework plugins interpret conventions; resolvers create graph truth.**

## 6. Two-pass resolution: unresolved refs are a core feature, not a cleanup table

Resolution should be explicitly two-pass.

Pass one extracts all local facts:

```text
files -> symbols
files -> refs
files -> imports
files -> exports
files -> framework candidates
files -> bridge candidates
```

Pass two builds global indexes and resolves:

```text
module path -> file
module exports -> symbols
qualified name -> symbols
local scope -> visible symbols
raw refs -> candidate symbols
framework candidates -> route/handler edges
bridge candidates -> cross-language edges
```

Anything unresolved stays in `unresolved_refs` with a reason and candidate set. That table should not be treated as failure debris. It is a product surface. It tells you where the graph is incomplete, where a future file edit may suddenly create a resolvable edge, where a framework rule is missing, and where an agent should be warned that the impact radius is partial.

Every edge should carry:

```text
kind: call | import | route | inheritance | framework | bridge | test | diagnostic_related
confidence: exact | probable | speculative
provenance: language_rule | framework_rule | bridge_rule | heuristic | external_tool
generation
```

This matters because static analysis across 20 languages plus frameworks is not a binary correct/incorrect game. False precision is more dangerous than uncertainty. If the agent sees a speculative edge, it can decide to inspect. If the tool silently presents speculative edges as fact, you will eventually cause bad edits.

Incrementally, a file change should re-resolve:

```text
refs inside the changed file
refs pointing to symbols whose identity/export status changed
imports of the changed module
framework facts affected by changed route files
bridge candidates affected by changed native/JS modules
stored unresolved refs whose raw name or namespace now has new candidates
```

That last one is important. New symbols should wake old unresolved refs.

## 7. Diagnostics-in-the-loop: model diagnostics as a graph, not text

The daemon should supervise diagnostic engines as long-running workers:

```text
TypeScript: tsc --watch or a structured tsserver adapter
JS/TS lint: oxlint
JS/TS format: oxfmt
Python: ruff, mypy, pyright where available
Rust: cargo check, rust-analyzer-derived diagnostics where feasible
Go: gopls/go test/go vet adapters
Java/Kotlin/C#: likely external build-tool/LSP adapters
```

The post-edit hook should not “run tsc” from scratch. It should ask the daemon for diagnostics for generation `G`, file hash `H`, config hash `C`, and toolchain version `T`.

The return policy should be:

```text
if fresh diagnostics for changed file/impact radius exist:
  return new/regressed/resolved diagnostics

else if checker is still computing and previous diagnostics exist:
  return stale diagnostics clearly marked plus pending status

else:
  return pending status with no stale claims
```

For post-edit hooks, I would allow a bounded wait, maybe 300 to 1000 ms depending on harness tolerance and edit type. Pre-read and pre-edit hooks should target the sub-100 ms class. Post-edit diagnostics can spend more budget because they are directly tied to correctness, but they still need a hard timeout.

The key internal representation is diagnostic deltas:

```text
new diagnostics caused by this edit
diagnostics resolved by this edit
diagnostics still present but pre-existing
diagnostics outside impact radius
diagnostics likely unrelated
```

Agents are terrible at interpreting a wall of 200 pre-existing errors. They are much better with “you introduced these 3 errors, fixed these 2, and 14 old unrelated errors remain suppressed.”

## 8. Atomic edit and multi-agent safety: copy codedb conceptually, enforce where hooks allow it

codedb’s atomic line-range edit and version tracking idea is correct. ([GitHub](https://github.com/justrach/codedb)) But for Code Briefcase, native agent `Edit` calls complicate enforcement because the harness owns the edit tool.

I would implement both strict and soft modes.

Strict mode is the MCP-managed edit tool:

```text
briefcase_edit(path, expected_hash, range, replacement)
```

The daemon verifies the expected hash, applies the edit atomically, records the version, updates the index, and triggers diagnostics. This is the safest path.

Soft mode is native hook interception:

```text
PreToolUse:Edit
  infer target path and expected old content
  acquire short file lease if possible
  check current hash against last observed hash / old_string
  warn or block if stale, depending on harness support

PostToolUse:Edit
  observe actual new file hash
  commit edit_log entry
  release lease
  trigger incremental parse/diagnostics
  report conflicts if another agent touched the file
```

The daemon should maintain an edit ledger per file. Every read gives the session an observed file version. Every edit checks that observation. If Agent A reads version 10, Agent B writes version 11, and Agent A tries to edit based on version 10, the pre-edit hook should say: “This edit is based on stale context. Re-read first.” If the harness allows blocking, block. If it only allows advisory context, scream clearly.

This is how Code Briefcase becomes more than “better grep.” It becomes a coordination layer for multi-agent software work.

## 9. Installation and distribution

Ship one binary per platform and architecture:

```text
macOS arm64/x64, signed and notarized
Linux arm64/x64
Windows arm64/x64, signed where practical
```

Install paths:

```text
curl -fsSL ... | sh
PowerShell installer on Windows
Homebrew tap
npm shim for ecosystem convenience
cargo install for developers
```

The npm package should download or expose the native binary. It should not make Node the runtime.

`briefcase install` should detect and configure Claude Code, Codex CLI, Cursor, opencode, and other harnesses. It should install both hook configs and MCP configs. It should be migration-aware and reversible:

```text
briefcase install
briefcase install --agent claude-code
briefcase uninstall
briefcase doctor
```

The per-agent MCP command should be a proxy:

```text
briefcase mcp --project /path/to/project
```

That process speaks MCP over stdio because agents expect stdio MCP servers. It should not independently watch files, index code, open diagnostic workers, or own SQLite writes. It connects to the daemon.

The per-project setup should be:

```text
briefcase init
  creates .code-briefcase/config.toml
  creates .code-briefcase/briefcase.db
  builds initial index
  writes optional agent guidance files
```

Bundle the common tree-sitter grammars into the binary. For obscure grammars, support optional language packs later, but do not make the top 20 languages require separate downloads. The whole point is “install once, it works.”

Handle binary upgrades carefully. The daemon protocol must be versioned. New clients should not talk to old daemons with incompatible protocol versions. If a user upgrades the binary while a daemon is running, the new proxy should either attach only if compatible or spawn a new daemon after the old one idles out. CodeGraph’s release notes describe version-pinned daemon behavior in this neighborhood; that is worth copying. ([GitHub](https://github.com/colbymchenry/codegraph/releases))

## 10. What to keep from CodeGraph

Keep these ideas:

CodeGraph’s **pre-indexed local knowledge graph** is right. Agents should not rediscover structure with grep and reads. Their benchmark claims are worth treating as a serious signal: the published README reports average savings of 35% cost, 57% tokens, 46% time, and 71% tool calls across seven real-world repos. ([GitHub](https://github.com/colbymchenry/codegraph))

Keep **SQLite plus FTS5** as durable local storage. That is a good default for a local-first code intelligence product.

Keep the **MCP tool surface**: context, trace, callers, callees, impact, node, explore, status. Even if hooks are the differentiator, pull-mode is still valuable when the agent has learned to ask better questions.

Keep **framework-route awareness**. It is not garnish. In real apps, the route graph is often the actual entrypoint graph.

Keep **cross-language bridge awareness**. iOS, React Native, Expo, Svelte/Vue embedded scripts, Rails views, Django URLs, and TS/Python service boundaries are where naive static graphs go to die.

Keep **shared daemon across agents**. Multi-agent sessions are becoming normal. One watcher, one index, one diagnostics supervisor, one edit ledger.

Keep **agent prompts with bad/good examples**. Tool descriptions are agent UX. A technically brilliant MCP server with vague tool prompts will lose to a worse server with better affordances.

Keep **installer seriousness**. Config patching, uninstall, doctor, migration-aware agent setup, permission guidance, and clear failure modes are core product, not packaging chores.

## 11. What to keep from codedb

Keep codedb’s **single-binary discipline**. It is correct for this category.

Keep **atomic edit/version semantics**. Multi-agent coding without file versions and edit conflict detection is going to become a mess.

Keep **word/trigram indexes** for warm local lookup. FTS5 is fine for durability, but a daemon-local exact-word and trigram index is cheap and fast.

Keep **snapshots**. A pre-rendered snapshot that makes MCP startup or daemon warmup instant is the right idea.

Keep **sensitive file blocking**. `.env`, private keys, credentials, tokens, and secrets need first-class policy in both hooks and MCP tools.

Keep **remote public repo index** as a concept. The right implementation for Code Briefcase is probably signed read-only graph packs for public repos. A daemon should be able to mount:

```text
local working tree graph: writable
remote public dependency graph: read-only
```

Then the agent can ask about a dependency or upstream public repo without cloning. That said, make this opt-in and visibly separate from local indexing. Your trust posture depends on local-first behavior.

## 12. What I would reject

I would reject TypeScript as the core runtime. The ecosystem fit is real, but the hook path punishes runtime overhead. TS is a good language for writing agent SDKs, not for the core of this product.

I would reject Zig as the main implementation language. codedb is impressive, but broad language support, framework extraction, incremental computation, LSP/diagnostic adapters, SQLite ergonomics, Windows/macOS integration, and contributor durability all point away from Zig for a decade-long bet.

I would reject “MCP server as daemon.” MCP should be an adapter. The daemon should have its own internal protocol.

I would reject “SQLite only” for hot serving. SQLite should persist and recover; the daemon’s read model should answer.

I would reject a bespoke durable graph database. That is overreach. Build disposable high-performance caches on top of boring durable storage.

I would reject a giant plugin free-for-all. Language/framework plugins need a stable fact schema and test corpus. Otherwise, every language becomes a snowflake and your graph becomes untrustworthy.

## 13. The non-obvious thing I would design: a context-frame ledger

The thing nobody is doing seriously enough is **tracking what context the agent has already been shown and why**.

Push-mode can become spam. If every read emits the same nav map, every edit emits the same impact radius, and every diagnostic dump repeats old errors, agents will learn to ignore you or burn tokens on duplicated context. The long-term winner will not merely have the best graph. It will have the best **context scheduler**.

I would add a first-class `ContextFrame` system.

Every hook output is a frame:

```text
frame_id
session_id
agent_id
hook_event_id
content_kind: nav_map | diagnostics | impact | callgraph | route | warning
dependency_fingerprint
source_generation
token_cost
priority
expiry_policy
rendered_hash
```

The daemon keeps a per-agent, per-session “seen context” ledger. When an agent reads `src/auth.ts`, the daemon knows which nav map version it emitted. When the file changes but the symbol outline does not, the daemon suppresses duplicate nav. When the call graph changes, it emits a delta. When diagnostics are unchanged, it says nothing. When a new route edge appears that affects the edited symbol, it interrupts.

This unlocks a better hook renderer:

```text
Read hook:
  show outline only if new or structurally changed
  show related files only if not already shown
  show route/caller info only when it changes the likely next action

Pre-edit hook:
  show stale-context warnings
  show symbol-local invariants
  show call sites only if edit touches public API or exported symbol

Post-edit hook:
  show diagnostic deltas
  show impact deltas
  suppress old unrelated errors
  suggest exact next reads only when confidence is high
```

This is also how you benchmark the product honestly. You can replay an agent session and ask: which context frames changed behavior, which were ignored, which were redundant, which caused better edits, which wasted tokens? Without a context-frame ledger, push-mode is vibes. With it, push-mode becomes an optimizable control system.

I would pair that with a diagnostic delta graph. Diagnostics should be nodes linked to file hashes, symbols, edit events, and tool versions. Then the post-edit hook can say:

```text
New from this edit:
  AuthService.login now returns Promise<User | null>, but caller expects User

Resolved by this edit:
  missing await in session refresh

Still present, pre-existing:
  12 unrelated mypy errors outside impact radius
```

That is vastly more useful than “here is tsc output.”

## 14. The actual greenfield blueprint

If I were building the best version, I would create this Rust workspace:

```text
crates/
  briefcase-cli
    subcommands, install, uninstall, doctor

  briefcase-hook
    minimal hook stdin/stdout handlers
    no tree-sitter
    no SQLite
    no heavy imports

  briefcase-protocol
    typed daemon IPC messages
    schema versions
    harness event types
    MCP response types

  briefcase-daemon
    project daemon
    socket server
    scheduler
    watcher
    client/session registry
    context-frame ledger

  briefcase-db
    SQLite schema
    migrations
    WAL setup
    FTS5
    snapshot builder/loader

  briefcase-index
    tree-sitter wrappers
    parser registry
    language specs
    fact extraction

  briefcase-resolve
    name resolution
    import resolution
    framework plugins
    bridge plugins
    unresolved refs
    edge confidence/provenance

  briefcase-incremental
    Salsa database
    query definitions
    structural hashes
    dirty propagation

  briefcase-diagnostics
    tsc/tsserver adapter
    oxlint/oxfmt adapter
    ruff/mypy/pyright adapter
    generic process supervisor
    diagnostic delta graph

  briefcase-edit
    file versions
    leases
    atomic edits
    conflict detection

  briefcase-mcp
    stdio MCP server/proxy
    tool definitions
    tool prompt texts

  briefcase-render
    hook context rendering
    token budgeting
    frame dedup
    markdown/JSON renderers

  briefcase-security
    ignore rules
    sensitive file policy
    path traversal prevention
```

The mental model:

```text
events in:
  hook events
  MCP calls
  file watcher events
  diagnostic events
  edit events

state maintained:
  file contents and versions
  extracted facts
  resolved graph
  diagnostics
  context-frame ledger

answers out:
  hook additionalContext
  hook argument mutations / blocks where supported
  MCP tool results
  CLI status/debug output
```

## Final verdict

Build Code Briefcase in **Rust**.

Use **one native binary**.

Use **one shared daemon per project worktree**.

Use **Unix sockets/named pipes** for internal IPC.

Use **SQLite WAL + FTS5** for durable storage.

Use **mmap snapshots and in-memory arenas/indexes** for hot query speed.

Use **Salsa** for pure incremental derivations, with stable structural identities and early cutoff.

Use **declarative tree-sitter language specs** that emit generic facts.

Put **frameworks and cross-language bridges in resolver plugins**.

Preserve **unresolved refs** as first-class graph state.

Make **diagnostics a versioned delta graph**, not text output.

Make **MCP a proxy/client of the daemon**, not the architecture.

Add the thing nobody has: **a context-frame ledger that treats pushed context as an optimizable, versioned, deduplicated product object**.

That is the durable shape. The strategic bet is that agent coding infrastructure becomes less about “can I search code?” and more about “can I continuously steer multiple agents through a changing codebase without wasting tokens or letting them act on stale beliefs?” Code Briefcase’s hook-mode advantage points exactly there.