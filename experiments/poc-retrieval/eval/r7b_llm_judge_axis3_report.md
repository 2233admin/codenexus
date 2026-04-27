# R7b LLM-Judge Re-evaluation -- Axis-3 Graph Traversal

Closes EVAL_DESIGN_NOTES Rule 3: hand-matcher substring bias.
Seeds: 3 | model: MiniMax-M2.7 | provider: okaoi

## Aggregate Verdict

| Metric | Value |
|--------|-------|
| LLM-judge p@5 (n=6) | **23.3%** |
| Hand-matcher p@5 (spike-007) | 15.0% |
| R3 retrieval baseline | 0.0% |
| Delta LLM vs hand | +8.3% |
| Grade threshold | >= 2 |
| Wall clock | 42.1s |

**Verdict: CONFIRMS spike-007** -- LLM precision > hand 15%. Substring bias depressed score.

## Per-Query Results

| ID | Query | Hand p@5 | LLM p@5 | Mean grades | Disagree |
|----|-------|----------|---------|-------------|---------|
| C1 | who calls assertRealPathInsideVault | 0.00 | 0.40 | [2.0,2.3,0.0,0.3,0.0] | YES |
| C10 | what does GitNexusAdapter call during indexing | 0.00 | 0.00 | [0.0,0.0,1.0,0.3,0.0] | no |
| C2 | what operations does ObsidianAdapter implement | 0.00 | 0.00 | [0.0,1.0,0.0,1.0,1.0] | no |
| C3 | which adapter calls FilesystemAdapter as fallback | 0.50 | 0.00 | [0.0,0.3,1.0,0.0,0.3] | YES |
| C4 | what error handler runs when vault_create fails pref | 0.00 | N/A | -- | no |
| C5 | what does concept_graph run call internally | 0.00 | N/A | -- | no |
| C6 | who produces kb_meta records | 0.00 | N/A | -- | no |
| C7 | what reads Node tags field | 0.00 | N/A | -- | no |
| C8 | what runs after MemUAdapter sync completes | 0.00 | 0.00 | [0.0,0.3,0.3,1.0,0.7] | no |
| C9 | who calls OAuth2Provider | 1.00 | 1.00 | -- | no |

## Disagreements

### C1: who calls assertRealPathInsideVault
- Hand: 0.00 | LLM: 0.40 (LLM>hand)
- Grades: [2.0, 2.3333333333333335, 0.0, 0.3333333333333333, 0.0]
- Note: 

### C3: which adapter calls FilesystemAdapter as fallback
- Hand: 0.50 | LLM: 0.00 (hand>LLM)
- Grades: [0.0, 0.3333333333333333, 1.0, 0.0, 0.3333333333333333]
- Note: 

## Structural Notes

C4/C5/C6/C7: unresolved -- PPR seed not found, top5 empty, both 0.
C9: correct abstain (OAuth2Provider not in graph), both 1.0.
C1: resolve/dispatch -- hand rejects on path substring mismatch.
C2: adapter-adjacent decls -- LLM may credit structural neighbors.
C10: loadConfig/gnAdapter init-adjacent -- LLM may grade gnAdapter relevant.

## Run Summary

LLM-judge axis-3 precision = 23.3% (vs hand 15.0%, retrieval 0.0%)