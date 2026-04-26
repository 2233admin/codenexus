# Nomic Embed Code Shadow Evaluation Report

**Model used:** `microsoft/codebert-base`
**Model load time:** 28.7s
**Symbol embed time:** 18.6s (2116 symbols)
**Query match time:** 0.4s (30 queries)
**DB:** `D:\projects\codenexus\experiments\poc-retrieval\poc.db`

## Per-Axis Precision@5

| Axis | R3 Baseline | Nomic | Delta |
|------|-------------|-------|-------|
| Axis-1 (exact lookup) | 70.0% | 20.0% | -50.0% |
| Axis-2 (semantic) | 50.0% | 10.0% | -40.0% |
| Axis-3 (graph-aware) | 30.0% | 20.0% | -10.0% |

**Top-5 overlap rate (>=3/5 match R3):** 0.0% (0/26 queries)

## Per-Query Breakdown

| ID | Ax | Query | R3 P@5 | Nomic P@5 | Delta |
|----|----|----------------------------------------------------|--------|-----------|-------|
| A1 | 1 | ObsidianAdapter | 1.00 | 0.00 | -1.00 |
| A2 | 1 | VaultBrainAdapter | 1.00 | 0.00 | -1.00 |
| A3 | 1 | assertRealPathInsideVault | 1.00 | 0.00 | -1.00 |
| A4 | 1 | PROTECTED_DIRS | 1.00 | 0.00 | -1.00 |
| A5 | 1 | concept_graph run function | 0.00 | 0.00 | 0.00 |
| A6 | 1 | validatedArgs | 1.00 | 0.00 | -1.00 |
| A7 | 1 | kb_meta | 0.00 | 0.00 | 0.00 |
| A8 | 1 | Node tags field | 0.00 | 0.00 | 0.00 |
| A9 | 1 | parseYAMLFrontmatter | 1.00 | 1.00 | 0.00 |
| A10 | 1 | OAuth2Provider | 1.00 | 1.00 | 0.00 |
| B1 | 2 | filesystem fallback when obsidian not running | 1.00 | 0.00 | -1.00 |
| B2 | 2 | preflight check for protected directories | 1.00 | 0.00 | -1.00 |
| B3 | 2 | search files by tag | 1.00 | 0.00 | -1.00 |
| B4 | 2 | build concept graph from notes | 0.00 | 0.00 | 0.00 |
| B5 | 2 | rate limiting middleware | -0.25 | 1.00 | +1.25 |
| B6 | 2 | safe file deletion with dry run | 1.00 | 0.00 | -1.00 |
| B7 | 2 | register MCP tool handler | 1.00 | 0.00 | -1.00 |
| B8 | 2 | handle concurrent writes to the same vault file | 0.00 | 0.00 | 0.00 |
| B9 | 2 | detect conflicting edits between adapters | 0.00 | 0.00 | 0.00 |
| B10 | 2 | aggregate metadata across multiple notes | 0.00 | 0.00 | 0.00 |
| C1 | 3 | who calls assertRealPathInsideVault | 0.00 | 0.00 | 0.00 |
| C2 | 3 | what operations does ObsidianAdapter implement | 0.00 | 0.00 | 0.00 |
| C3 | 3 | which adapter calls FilesystemAdapter as fallback | 0.00 | 0.00 | 0.00 |
| C4 | 3 | what error handler runs when vault_create fails preflig | 1.00 | 0.00 | -1.00 |
| C5 | 3 | what does concept_graph run call internally | 0.00 | 0.00 | 0.00 |
| C6 | 3 | who produces kb_meta records | 0.00 | 0.00 | 0.00 |
| C7 | 3 | what reads Node tags field | 1.00 | 1.00 | 0.00 |
| C8 | 3 | what runs after MemUAdapter sync completes | 0.00 | 0.00 | 0.00 |
| C9 | 3 | who calls OAuth2Provider | 1.00 | 1.00 | 0.00 |
| C10 | 3 | what does GitNexusAdapter call during indexing | 0.00 | 0.00 | 0.00 |

## Verdict

**regression, R3 embedder stays**

Axis-2 delta: -40.0% vs R3 baseline of 50.0%.

### Notes
- Negative queries scored: 1.0 = correct rejection, -0.25 = false positive (floored to 0.0 in aggregate).
- R3 used qwen3-embedding:0.6b (1024d) via Ollama with RRF fusion. Nomic used `microsoft/codebert-base` pure cosine.
- Axis-3 baseline near 0% is expected -- graph traversal not available in POC retrieval.
- Corpus: 2116 TS symbols from obsidian-llm-wiki mcp-server.
