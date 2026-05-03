---
phase: 5
gray_area: G5
title: "ADR extraction harness scope (G5 resolution)"
status: AMENDED 2026-05-03 per CCG round 2 (A-CI-1=(b)/CON-2 cascade applied; section 3 storage layout reversed; CI-4 dissolved)
parent: 05-PRE-PLAN-NOTES.md G5 (line 101-107)
authority_chain:
  - .planning/PROJECT.md line 108 (Architectural decision semantic indexing)
  - .planning/BETA-V1-SPEC.md section 8 line 218-219 (ADR harness in PLAN scope)
  - .planning/phases/codenexus-05-bridge-memory-mvp/05-PRE-PLAN-NOTES.md G5
  - docs/ARCHITECTURE.md (live source corpus for harness self-test)
---

# G5: ADR extraction harness scope

Opinionated single-author draft. Locks proposed defaults; flags choices
needing CCG ratification before plan-phase. Sibling cluster G2
(`query_constraints`) is the consumer of what this harness produces --
G2 and G5 must agree on storage shape or both stall.

---

## Round-2 Amendment Block (2026-05-03, CCG round 2 cascade)

Codex round-2 challenge surfaced **CI-1** (G2 corpus_scope LOC underestimated)
and **CON-2** (separate adrs table dismissed Symbol kind='ADR' reuse too
quickly). Cascade resolution: adopt CON-2 path. This block supersedes
section 3 (Storage layout) and parts of sections 4/5 below. Original text
retained for audit trail.

### A-G5-CI-1 (cascade): Symbol kind='ADR' reuse, adr_metadata sidecar

**Supersedes:** § 3.1 Decision (separate adrs table reversed) + § 3.2 SQL
DDL (replaced) + § 3.3 Rejected (now reversed -- "separate adrs table" is
the rejected path) + § 4.3 Incremental path (writes to symbols + adr_metadata)
+ § 5.1 Surfacing (revised SQL examples) + § 5.2 Ranking signals (column
sources updated).

**New decision:** ADRs are persisted as Symbol rows with `kind='ADR'`. ADR
text lives in a new `symbols.body_text TEXT NULL` column (W0 ALTER TABLE).
ADR-specific metadata (keyword, confidence, doc_version_sha, supersede chain)
lives in a sidecar `adr_metadata` table joined by `symbol_id`. ADR FTS reuses
the existing `symbols_fts` virtual table (external-content + triggers per
storage.rs:22-28) by extending the indexed columns to include `body_text`.

**Rationale (from CCG round 2):** Codex CI-1 showed the original "separate
adrs table" path forced search.rs to grow a corpus-scope abstraction
(100-200 LOC + new accessors + result-type abstraction). Reusing Symbol
infrastructure cuts that work entirely -- search.rs gets a small `kind_filter`
parameter (~30-50 LOC) instead of full corpus dispatch. CI-4 (FTS5 contentless
terminology error) dissolves: symbols_fts is already external-content + triggers,
so no new FTS mode needed. Net code savings: ~150 LOC + one terminology hazard
removed.

**New SQL DDL (W0):**

```sql
-- W0 migration: extend symbols schema
ALTER TABLE symbols ADD COLUMN body_text TEXT;       -- populated for kind='ADR'
                                                     -- NULL for code Symbols
                                                     -- (V1.1+ may populate from docstrings)
CREATE UNIQUE INDEX IF NOT EXISTS idx_symbols_fnk
  ON symbols (path, name, kind);                     -- also serves G3 notes FK target

-- W0 migration: rebuild symbols_fts to include body_text in indexed columns
DROP TABLE IF EXISTS symbols_fts;                    -- contentless, safe to drop+recreate
CREATE VIRTUAL TABLE symbols_fts USING fts5(
  name, kind, search_blob, body_text,                -- body_text NEW
  content='symbols', content_rowid='id',
  tokenize='unicode61'
);
-- recreate triggers symbols_ai/ad/au to mirror new column set
-- (W0 plan spec; pattern matches existing storage.rs:22-28)

-- W0: ADR sidecar (one-to-one with kind='ADR' Symbol rows)
CREATE TABLE IF NOT EXISTS adr_metadata (
  symbol_id INTEGER PRIMARY KEY REFERENCES symbols(id),
  keyword TEXT NOT NULL,                             -- MUST_NOT|MUST|SHOULD_NOT|SHOULD|MAY|...
  confidence REAL NOT NULL,                          -- 1.0 / 0.7 / 0.4 per § 2.2
  source_line INTEGER NOT NULL,                      -- start line in source markdown
  source_end_line INTEGER NOT NULL,
  heading_anchor TEXT,                               -- nearest H2/H3 above paragraph
  doc_version_sha TEXT NOT NULL,                     -- git blob sha of source path
  extracted_at INTEGER NOT NULL,
  superseded_by_symbol_id INTEGER REFERENCES symbols(id)
);
CREATE INDEX IF NOT EXISTS idx_adr_active
  ON adr_metadata(superseded_by_symbol_id)
  WHERE superseded_by_symbol_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_adr_keyword
  ON adr_metadata(keyword, confidence DESC);

-- W0: cross-link from ADR Symbols to code Symbols (V1.1+ populated lazily)
CREATE TABLE IF NOT EXISTS adr_symbol_links (
  adr_symbol_id   INTEGER NOT NULL REFERENCES symbols(id),
  code_symbol_id  INTEGER NOT NULL REFERENCES symbols(id),
  link_kind       TEXT NOT NULL,                     -- mention|topic_match|file_overlap
  score           REAL NOT NULL,
  PRIMARY KEY (adr_symbol_id, code_symbol_id, link_kind)
);
CREATE INDEX IF NOT EXISTS idx_adr_links_code
  ON adr_symbol_links(code_symbol_id, score DESC);
```

**ADR Symbol row encoding:**
- `path` = source markdown path (e.g. `docs/ARCHITECTURE.md`)
- `name` = `{heading_anchor || "anon"}#{source_line}` -- stable across runs
  given same heading + line; doc_version_sha sidecars catch drift
- `kind` = `'ADR'`
- `start_line` / `end_line` = paragraph span (mirrors existing Symbol shape)
- `body_text` = full paragraph text (the FTS signal)
- All other Symbol fields = NULL or empty (signature etc. unused for ADRs)

**Re-extraction lifecycle:** Same append-only discipline as G3 notes. If
extract_adrs sees a paragraph at the same (path, source_line) but with a
new doc_version_sha + changed body_text, INSERT new Symbol row + new
adr_metadata row, then SET old adr_metadata.superseded_by_symbol_id =
new_symbol_id. Old rows stay intact. `idx_adr_active` filters to leaves.

**Original § 3.3 reversal:** The points originally listed against "Symbol
kind=ADR reuse" (~50-200 noisy rows, kind-IN exclusion clauses, path
invariant breakage) are accepted-as-cost in exchange for the ~150 LOC and
FTS5 mode complexity savings. Mitigations:
- `WHERE kind != 'ADR'` predicate added at code-Symbol query sites; one-line
  per query (search.rs:29 / search.rs:31 use kind_filter parameter; storage
  accessors that should NOT return ADRs add explicit filter)
- `path` invariant ("follow path -> compilable code") relaxed for kind='ADR'
  rows only; documented in CONTEXT.md amendment (W0 deliverable)
- Cytoscape rendering branch gets new node-type for kind='ADR' (V1.1+ UI work,
  out of Phase 5 scope per BETA-V1-SPEC sec 6)

### A-G5-CI-4 (dissolved)

CI-4 (FTS5 "contentless" vs "external-content" terminology error) does NOT
require amendment because the reuse cascade above eliminates the new ADR
FTS table entirely. ADRs ride symbols_fts (existing external-content +
triggers, terminology already correct).

### Amendment status

After A-G5-CI-1 cascade lands in W0 + W4 PLANs, the original CI-1 (G2
corpus_scope LOC) and CON-2 (G5 reuse path) and CI-4 (FTS5 terminology) are
all resolved. Plan-checker iter 1 should re-verify (a) symbols_fts trigger
spec covers body_text inserts, (b) adr_metadata FK to symbols(id) survives
ON DELETE policy, (c) `WHERE kind != 'ADR'` predicate applied at all
code-Symbol query sites.

---

Motivating example, verbatim from PROJECT.md line 108: an agent editing
retrieval code SHOULD encounter `ARCHITECTURE.md section 9.4` line 508
("Cross-encoder reranker MUST NOT be introduced in Phase 3 MVP until
LLM-as-judge eval pipeline exists") **without a human reminding it**.
Every choice below is judged against that single scenario.

---

## 1. Source scope

### 1.1 Default dirs (recommended)

| Dir | Why included | Example payload |
|---|---|---|
| `docs/**/*.md` | Authoritative spec | `docs/ARCHITECTURE.md:305` MUST NOT GitNexus-in-context |
| `.planning/*.md` (one-level) | Frozen specs, requirements | `.planning/BETA-V1-SPEC.md` 8 MUSTs |
| `.planning/phases/**/*-PLAN.md` | Wave-locked decisions | future Phase 5 PLAN MUSTs |
| `README.md` | Public API contract surface | install + invocation MUSTs |

**Excluded by default**: `.planning/audits/**`, `.planning/probes/**`, `.planning/research/**` (transient/experimental/exploratory -- contain opinion-flavored MUSTs that pollute results). Also git history, `target/`, `node_modules/`, `.gitignore` matches.

### 1.2 Configurable via plugin.toml

Add an `[adr]` section to the existing per-language `plugin.toml`
(CONTEXT.md line 91-95 `LanguageSemantics` -- re-uses existing config seam):

```toml
[adr]
enabled = true
include = ["docs/**/*.md", ".planning/*.md", ".planning/phases/**/*-PLAN.md", "README.md"]
exclude = [".planning/audits/**", ".planning/probes/**", ".planning/research/**"]
patterns = ["MUST", "MUST NOT", "SHOULD", "SHOULD NOT", "REQUIRED", "SHALL"]
heading_anchor = true   # also extract ## ADR / ## Decision blocks
```

CLI override: `codenexus index --adr-{include,exclude} <glob>` (additive). NO CLI flag for `patterns` -- arbitrary regex = DoS surface; lock in binary, edits through plugin.toml.

### 1.3 Out of scope

- Arbitrary user-pointed dirs outside repo root: rejected for V1.0.
  Cross-repo ADR sourcing is V1.1+ shared-PG / Obsidian wiki territory
  (BETA-V1-SPEC section 6 line 167).
- Auto-discover `decisions/` or `adr/` convention dirs: defer.
  CodeNexus has neither -- no validation surface.

---

## 2. Extraction pattern

### 2.1 RFC 2119 keyword scan vs structured headers

**Lock: BOTH, with RFC 2119 as primary signal.**

CodeNexus ARCHITECTURE.md uses RFC 2119 keywords inline in prose, not in dedicated ADR blocks. Evidence:

- `docs/ARCHITECTURE.md:305` -- "AI agents working on CodeNexus core
  MUST NOT have GitNexus source in their context window at all"
- `docs/ARCHITECTURE.md:508` -- "Cross-encoder reranker MUST NOT be
  introduced in Phase 3 MVP until LLM-as-judge eval pipeline exists"
- `docs/ARCHITECTURE.md:493-498` section 9.2 "Design Contracts (locked,
  do not relitigate)" -- 4 normative bullets, none tagged "ADR"

Indexing only `## ADR ...` blocks would extract zero constraints from
the live corpus. RFC 2119 inline scan is the only viable primary path.
Heading anchors are SECONDARY -- bump confidence when both co-occur.

### 2.2 Pattern set (locked)

```
PRIMARY (conf=1.0):    MUST NOT, SHALL NOT, REQUIRED NOT, MUST, SHALL, REQUIRED
SECONDARY (conf=0.7):  SHOULD NOT, SHOULD, RECOMMENDED, NOT RECOMMENDED
WEAK (conf=0.4):       MAY, OPTIONAL  (extracted but ranked last)
```

Match rule: case-sensitive, whole-word boundary, in code-fence-free
lines. Skip triple-backtick regions (e.g.
`docs/embedder-offline-bootstrap.md:168` has `# MUST list exactly:
97b0c614...` which is a shell comment, not a constraint). Strip inline
backtick spans too.

### 2.3 Granularity decision

**Lock: per-paragraph (markdown block-level), with sentence offset.**

- TOO COARSE -- per-section: `docs/ARCHITECTURE.md` section 9 spans
  200+ lines; one-blob results unreadable, unrankable.
- TOO FINE -- per-sentence: line 508 MUST and rationale
  ("Hand-annotated `expected_paths` is not sustainable past 30 queries
  x 1 truth-per-query") live in adjacent sentences; splitting strands
  the rationale.
- LOCKED -- per-paragraph: contiguous block between blank lines or
  heading boundaries. Carries MUST + immediate rationale. Sentence
  offset stored separately for highlight-on-hit UI later.

Use tree-sitter `markdown` grammar for paragraph segmentation (already
in `lang_extractor` candidate set, CONTEXT.md line 105-110). Do not
roll a regex paragraph splitter.

---

## 3. Storage layout

### 3.1 Decision

**Lock: separate `adrs` table + symbol-graph cross-link via
`adr_symbol_links` join table.** NOT a Symbol with `kind=ADR`.

CONTEXT.md line 13-17 fixes Symbol = "named code unit parsed from source"; ADRs are markdown paragraphs. Forcing them into `symbols` conflates two ontologies and breaks every query that assumes `symbols.kind in (Function|Class|...)`. Separate-table wins: independent re-extraction; different lifecycle (ADRs change w/ `docs/`, symbols w/ code); clean `query_constraints` path (no kind-filter); V1.1+ ready (Obsidian wiki notes can join the same link table).

### 3.2 SQL DDL sketch (SQLite, valid syntax)

```sql
CREATE TABLE adrs (
  id              INTEGER PRIMARY KEY AUTOINCREMENT,
  source_path     TEXT NOT NULL,
  source_line     INTEGER NOT NULL,
  source_end_line INTEGER NOT NULL,
  heading_anchor  TEXT,
  keyword         TEXT NOT NULL,           -- MUST_NOT|MUST|SHOULD_NOT|SHOULD|MAY
  confidence      REAL NOT NULL,           -- 1.0 / 0.7 / 0.4 per section 2.2
  paragraph_text  TEXT NOT NULL,
  doc_version_sha TEXT NOT NULL,           -- git blob sha of source_path
  extracted_at    INTEGER NOT NULL,
  superseded_by   INTEGER REFERENCES adrs(id),
  UNIQUE (source_path, source_line, doc_version_sha)
);
CREATE INDEX idx_adrs_active ON adrs(source_path) WHERE superseded_by IS NULL;
CREATE INDEX idx_adrs_keyword ON adrs(keyword, confidence DESC);

CREATE VIRTUAL TABLE adrs_fts USING fts5(paragraph_text, heading_anchor,
  content='adrs', content_rowid='id', tokenize='unicode61');

-- Populated by G2 query_constraints, NOT by the harness.
CREATE TABLE adr_symbol_links (
  adr_id     INTEGER NOT NULL REFERENCES adrs(id),
  symbol_id  INTEGER NOT NULL REFERENCES symbols(id),
  link_kind  TEXT NOT NULL,    -- mention|topic_match|file_overlap
  score      REAL NOT NULL,
  PRIMARY KEY (adr_id, symbol_id, link_kind)
);
CREATE INDEX idx_adr_links_symbol ON adr_symbol_links(symbol_id, score DESC);
```

DDL notes:
- `superseded_by` = append-only audit trail (PRE-PLAN-NOTES line 89
  "NO delete-without-audit"). Same discipline as G3 notes.
- `UNIQUE (source_path, source_line, doc_version_sha)` = drift-safe
  identity, mirroring `(path, name, kind)` from drift probe M5_fnk =
  1.0 (PRE-PLAN-NOTES line 56-58).
- FTS5 contentless mode (`content=adrs`) = text stored once, indexed
  separately. Saves disk.
- `adr_symbol_links` lazy-populated by G2 -> W4 cleanly cut from W2.

### 3.3 Rejected: Symbol with kind=ADR

- ~50-200 noisy rows per project in `symbols`
- Every `WHERE kind IN (Function, Class, ...)` query needs exclusion
  clause OR `is_adr` boolean OR CHECK rewrite -- multi-site edits
- `symbols.path` invariant (follow path -> compilable code) breaks
  when ADRs live in `.md`
- Cytoscape (`docs/ARCHITECTURE.md:38`) needs new node-type render
  branch anyway -- no win from co-location
- "Graph node for traversal" benefit doesn't materialize: ADRs have
  zero outgoing edges at parse time; links emerge at query time
  via `adr_symbol_links`

---

## 4. Re-extraction trigger

### 4.1 Decision

**Lock: on-demand A2A op `extract_adrs(scope?)` triggered automatically
by `index_repo` (full and incremental).** NO file-watcher. NO cron.

### 4.2 Rationale

- File-watcher daemon: violates "single fat-binary, zero install" (`docs/ARCHITECTURE.md:89`); second process w/ own crash-loop; out of scope per BETA-V1-SPEC section 6.
- Scheduled cron: same + OS-specific (systemd timer/launchd/Task Scheduler); no portable spec.
- On-demand standalone only: agents rarely call ops they aren't
  nudged toward.
- **Auto-coupled to `index_repo`**: zero new mental model, zero new
  failure mode. ADRs go stale exactly when docs go stale -- which is
  exactly when re-indexing already runs.

### 4.3 Incremental path

`index_repo --incremental` walks changed files via `last_indexed_commit`
(`docs/ARCHITECTURE.md:497`). Extend the walk: matching files in ADR
`include` glob -> run ADR extractor. For `.md` files there is no symbol
extractor today -- ADR extractor is sole consumer. Clean separation.

### 4.4 Manual escape hatch

Standalone `extract_adrs(paths?)` op for: (a) testing on synthetic input without full re-index; (b) re-extracting after `patterns` config change without bumping mtimes. Cheap, no daemon.

---

## 5. Linkage to `query_constraints` (G2 sibling)

### 5.1 Surfacing in G2 results

`query_constraints(topic|file|symbol)` returns ranked ADR rows. G5
produces rows; G2 produces ranking. Decoupled.

- **`topic` (NL)**: FTS5 BM25 over `adrs_fts.paragraph_text` + `heading_anchor`; optional hybrid via embedder + section 9.1 RRF fusion. G2 decides.
- **`file`**: query `adr_symbol_links` joined to symbols in that file; cold-start fallback = FTS5 match on basename + nearest module name.
- **`symbol`**: SELECT * FROM adrs JOIN adr_symbol_links ON adr_id
  WHERE symbol_id = ? ORDER BY score DESC.

### 5.2 Ranking signals (G2 owns weights)

| Signal | Source | Weight (proposed) |
|---|---|---|
| Keyword strength | `adrs.keyword` (MUST > SHOULD > MAY) | 0.30 |
| Confidence | `adrs.confidence` (1.0 / 0.7 / 0.4) | 0.20 |
| Text relevance | FTS5 BM25 / vector cosine | 0.30 |
| Source-doc weight | normative > planning > README | 0.10 |
| Recency | `extracted_at` desc, half-life ~ 90 days | 0.10 |

G5 ensures all five signals have non-NULL columns. DDL in section 3.2
satisfies this.

### 5.3 Source-doc weight table (proposed default)

| Path glob | Weight |
|---|---|
| `docs/ARCHITECTURE.md` | 1.0 |
| `docs/*.md` | 0.9 |
| `.planning/BETA-V1-SPEC.md` | 1.0 |
| `.planning/REQUIREMENTS.md` | 1.0 |
| `.planning/PROJECT.md` | 0.8 |
| `.planning/phases/**/*-PLAN.md` | 0.9 |
| `README.md` | 0.7 |

Config in plugin.toml `[adr.weights]`, defaults per above.

---

## 6. Open questions for Curry

1. **OQ1 -- markdown parser dep.** `tree-sitter-markdown` (consistent w/ `lang_extractor`) vs `pulldown-cmark` (Rust-native, no tree-sitter dep)? Default leaning: pulldown-cmark for V1.0; revisit if Phase 6 wants markdown-as-symbol-source.
2. **OQ2 -- inline backticked keywords.** Backticked MUST = literal reference, not constraint. Recommend: skip extraction (same rule as fenced blocks). Confirm.
3. **OQ3 -- supersede semantics.** Paragraph text change but same place: fresh insert + `superseded_by` link (matching G3 lifecycle, PRE-PLAN-NOTES line 89-91), or in-place update? Recommend: fresh insert (uniform audit trail).
4. **OQ4 -- `adr_symbol_links` population timing.** W2 (`query_constraints`) deliverable or W4.5 sub-slice? Recommend: W2 owns it (linkage = part of query design); G5 ships empty join + indexes only.
5. **OQ5 -- weight defaults vs evidence.** Section 5.3 weights are eyeballed. Future eval axis "ADR retrieval recall@5" once 30-task harness exists. Ship eyeball defaults V1.0, revisit V1.1. Acceptable?
6. **OQ6 -- W6 30-task eval coverage.** Does any of 30 eval tasks exercise "agent edits retrieval code AND `query_constraints(topic=reranker)` surfaces ARCHITECTURE.md:508"? If not, harness is untestable. Recommend: add >= 2 ADR-retrieval tasks during W6 (PRE-PLAN-NOTES line 140-142 UQ1 also flags this).
7. **OQ7 -- meta-mention false positives.** PROJECT.md line 108 quotes ARCHITECTURE.md section 9.4 MUST NOT verbatim -> current rules would extract a duplicate row anchored to PROJECT.md. Recommend: dedupe on exact `paragraph_text` match; keep highest source-weight row, mark others superseded reason = duplicate. Confirm or propose alternative.

---

## Honest gap list (rule 18)

- **P1**: section 5 ranking owned by G2. If G2 picks a radically
  different strategy, DDL columns may be unused (not broken,
  wasteful). Coordinate with G2 author before plan-checker iter 2.
- **P2**: section 4.4 `extract_adrs` adds 5th public A2A op. G6 affordance copy must cover it.
- **P2**: No empirical measurement of pulldown-cmark vs tree-sitter-markdown paragraph segmentation. OQ1 is a guess.
- **P3**: section 2.2 confidence numbers (1.0 / 0.7 / 0.4) are
  eyeballed, symmetric to existing `Confidence` (CONTEXT.md line
  30-36). Re-derivation needs eval loop -- chicken/egg.
- **P3**: section 1.1 excluded dirs are policy, not enforcement. A
  MUST written in `.planning/audits/` will be silently ignored.
  Consider one-line warning in `index_repo` output. Defer to PLAN
  polish.

**Not resolved**: whether the 5th A2A op `extract_adrs` is part of
MUST 5 (BETA-V1-SPEC section 1) or strictly internal. PRE-PLAN-NOTES
line 26-28 names only 3 ops. Recommend: 4 public ops including
`extract_adrs`; amend BETA-V1-SPEC section 8 line 213 if accepted.
