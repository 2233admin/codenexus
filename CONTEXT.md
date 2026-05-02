# CodeNexus

A code+knowledge graph tool that parses a source repo into Symbols, links
them with Edges of four kinds, embeds them, and serves NL search +
graph traversal over an A2A endpoint. This document locks the vocabulary
used across the parser / resolver / metrics / Snapshot pipeline so a
refactor (04.5-03) cannot drift terminology mid-flight.

## Language

### Graph primitives

**Symbol**:
A named code unit parsed from source (function / class / method /
interface / type alias / enum / top-level constant / arrow-fn variable
/ file). One row in the `symbols` table.
_Avoid_: node, entity, definition, identifier

**Edge**:
A directed relationship between two Symbols, carrying an **EdgeKind**
and a **Confidence**. One row in the `edges` table.
_Avoid_: link, connection, relation

**EdgeKind**:
The category of an **Edge**. Variants after 04.5-03: `Calls`,
`Implements`, `Extends`. `Imports` is being lifted out of the edges
table to **AliasDecl** scaffolding (see Flagged ambiguities).
_Avoid_: edge type, relation kind

**Confidence**:
Per-Edge resolution-quality score in `[0, 1]`. Derived from
**ResolutionMethod**: `SameFile`=1.0, `ImportResolved`=0.9,
`GlobalUnique`=0.7. **Namespace alias** lookups (resolve_with_namespace)
also yield 0.9.
_Avoid_: score, weight, certainty

### Edge pipeline (locked for 04.5-03 split)

**FoundEdge**:
Pre-resolution output of the edge-find phase. Carries `(from_file,
from_name, to_raw, kind)` and is not yet bound to a target Symbol id.
Produced by a **lang_extractor** per file.
_Avoid_: raw edge, candidate edge, unresolved edge

**AliasDecl**:
File-scoped name aliasing from an import declaration. Carries
`(from_file, alias, target_file, target_member)`. Covers the four
import variants: named (`{ foo }` → `target_member=Some("foo")`),
renamed (`{ foo as bar }` → alias=`bar`, target_member=`Some("foo")`),
namespace (`* as X` → target_member=`None`, member resolved at
callsite), default (`X from "Y"` → target_member=`Some("default")`).
_Avoid_: import row, alias entry, namespace alias

**ResolvedEdge**:
Post-resolution **Edge** with target symbol id and **ResolutionMethod**
attached. The atomic unit written to storage.
_Avoid_: bound edge, finalized edge, resolved relation

**ResolutionMethod**:
How a **FoundEdge** was bound to its target Symbol. Variants:
`SameFile`, `ImportResolved`, `GlobalUnique`. **Confidence** derives
from this. Per-language calibration is a future option (Python's
`ImportResolved` may warrant a lower number than TS's).
_Avoid_: resolver step, lookup type, binding kind

**SymbolIndex**:
Read-only projection of `Store` consumed by the resolver. Provides
name→id lookups (per-file and globally-unique). Built once per
`resolve()` call.
_Avoid_: symbol map, name table

**AliasIndex**:
Built from a slice of **AliasDecls**, consumed by the resolver to
resolve cross-file Calls / Extends / Implements. Internal scaffold,
not persisted.
_Avoid_: import map, namespace table

### Snapshot layer (metrics seam)

**Snapshot**:
Abstract view over edges + entry points consumed by `metrics::arch`
and downstream evo / dsm / Leiden. The projection from `Store` to
**Snapshot** is the seam between codenexus storage and metric
computation. Multiple implementations expected: `StoreSnapshot` (today),
`GixHeadSnapshot` (Phase 4 git overlay), `RepoScopedSnapshot` (Phase
4 multi-repo registry), `VaultTaggedSnapshot` (Phase 5 memU bridge).
_Avoid_: graph view, metric input

### Lift architecture (sentrux adoption, 04.5)

**LanguageSemantics**:
Sentrux-style per-language config deserialized from `plugin.toml`
(tree-sitter grammar source/abi, query capabilities, resolver paths,
complexity rules). One per supported language, loaded once at startup.
_Avoid_: language config, plugin profile

**RepoCtx**:
Per-repo materialization built once per `prepare_repo()` call:
**LanguageSemantics** + parsed tsconfig.json path-aliases + parsed
pyproject.toml module map + parsed go.mod module name. Different
lifetime from **LanguageSemantics**.
_Avoid_: repo state, build context, project context

**lang_extractor**:
A plugin contract — TOML config + tree-sitter grammar binding + a
generic Rust runner — that produces `(Symbols, FoundEdges, AliasDecls)`
per file given a **RepoCtx**. CodeNexus extends sentrux's schema with
a `"calls"` capability that sentrux itself lacks.
_Avoid_: parser plugin, language adapter, extractor module

**PathResolver** vs **CallResolver**:
Two distinct resolvers, decoupled to fix today's monolith. **PathResolver**
is TOML-data-driven (lifted from sentrux), maps relative import sources
to repo-relative file paths (`./X` → `src/X.ts`). **CallResolver** is
CodeNexus-native, maps `(file, name)` → `symbol_id` using **SymbolIndex**
+ **AliasIndex**.
_Avoid_: resolver (without qualifier), lookup, finder

## Relationships

- A **Symbol** lives in exactly one file (path is part of identity)
- An **Edge** connects two **Symbols** with a non-`None` **EdgeKind** and a **Confidence**
- A **lang_extractor** + a file + a **RepoCtx** produce `(Symbols, FoundEdges, AliasDecls)`
- An **AliasIndex** is built from a slice of **AliasDecls**
- A **FoundEdge** + a **SymbolIndex** + an **AliasIndex** produce zero or one **ResolvedEdge**
- A **ResolvedEdge** carries one **ResolutionMethod**, from which **Confidence** derives
- A **Snapshot** is projected from `Store` (and optionally git overlay / vault scope) and consumed by `metrics::arch`

## Example dialogue

> **A:** Why doesn't `import { foo as bar } from "./X"; bar()` produce a Calls **Edge** today?
> **B:** Two reasons composed. First, the **AliasDecl** captures the
> original name `foo` (the Q_IMPORTS `name` capture), so the
> **AliasIndex** has no entry for alias=`bar`. Second, at the callsite
> `bar()` the **FoundEdge** is `(A.ts, caller, "bar", Calls)`. The
> **CallResolver** runs: `SymbolIndex` same-file lookup of `bar` (none),
> `AliasIndex` lookup of `bar` (none — alias is unmapped),
> `find_global_unique("bar")` (none anywhere). Result: zero
> **ResolvedEdge**, `unresolved` counter increments. T3 in
> `graph_build::tests` pins this so a refactor can't silently change it.

> **A:** When does an `Imports` row land in the `edges` table?
> **B:** Today: when a named import resolves cleanly (e.g. `{ foo }
> from "./X"` → caller→X.foo at conf=1.0). After 04.5-03 (A3.3 path):
> never — `Imports` is moved to a separate `alias_decls` table because
> it conflates symbol-level and file-level granularity. Queries that
> want "what does file X import?" go through a **Snapshot** projection.

## Flagged ambiguities

- **"import"** has meant both an **AliasDecl** (file-level scaffold for
  the resolver) and a graph **Edge** with `EdgeKind=Imports`. Resolved
  04.5-03 (A3.3): **AliasDecl** is canonical; `Imports` is removed from
  the `edges` table and `EdgeKind`. To query "what does file X
  import?", project through a **Snapshot** which derives an
  `ImportEdge` view from **AliasDecl** rows on demand.

- **"resolver"** has been overloaded for two distinct concerns: (a)
  file-level path resolution (sentrux's `[semantics.resolver]` config —
  which physical file does `./X` reference?) and (b) symbol-level
  call-target resolution (which Symbol does `foo()` bind to?).
  Resolved: **PathResolver** (a) is TOML-driven and lifted from
  sentrux. **CallResolver** (b) is CodeNexus-native and consumes
  **AliasIndex** + **SymbolIndex**.

- **"kind"** appears on both `Symbol.kind` (function / class /
  interface / ...) and `Edge.kind` / **EdgeKind** (Calls / Implements
  / Extends). Both stay distinct; context disambiguates. Avoid
  shortening either to bare "kind" in cross-cutting code.

- **"namespace alias"** is a specific **AliasDecl** variant where
  `target_member=None`, not a general term. Avoid using it for any
  other lookup-by-prefix mechanism.
