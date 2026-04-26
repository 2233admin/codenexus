"""Code-retrieval-adapted LLM-judge prompts for R5 spike.

Why custom prompts (not ragas built-in):
  ragas 0.2 LLMContextPrecisionWithoutReference assumes a generated `answer`
  field, which retrieval-only CodeNexus does not have. We port the LLM-judge
  methodology (binary 0/1 verdict from ragas + 0-3 graded rubric from NIST TREC)
  with prompts specific to code-retrieval queries.

Arms:
  A — binary  (matches ragas LLMContextPrecisionWithoutReference verdict shape)
  B — graded  (0-3, NIST TREC convention: non-rel / tangential / relevant / canonical)
"""

JUDGE_SYSTEM = (
    "You are a precise code-retrieval evaluator. "
    "Output ONLY a single JSON object with the requested keys. "
    "No prose, no markdown fences, no extra text."
)

ARM_A_BINARY_PROMPT = """Evaluate a code search result.

QUERY:
{query}

RETRIEVED CODE:
File: {path}
Symbol: {kind} {name}
```
{snippet}
```

Is this code relevant to the query? Output JSON:
{{"verdict": 0 or 1, "reason": "<<= 5 words>"}}

Rules:
- 1 = code is on-topic and helps answer the query
- 0 = code is off-topic or unrelated
"""

ARM_B_GRADED_PROMPT = """Evaluate a code search result.

QUERY:
{query}

RETRIEVED CODE:
File: {path}
Symbol: {kind} {name}
```
{snippet}
```

Rate relevance on a 0-3 scale (NIST TREC):
- 0 = non-relevant: snippet has nothing to do with the query
- 1 = tangential: same area but doesn't directly answer
- 2 = relevant: useful information for the query
- 3 = canonical: this is the best/primary answer to the query

Output JSON:
{{"grade": 0, 1, 2, or 3, "reason": "<<= 5 words>"}}
"""

ARM_PAIRWISE_PROMPT = """Compare two code search result sets for the same query.

QUERY: {query}

CANDIDATE SET A (top 5):
{set_a_block}

CANDIDATE SET B (top 5):
{set_b_block}

Which set better answers the query? Output JSON ONLY:
{{"verdict": "A" | "B" | "tie", "reason": "<<= 10 words>"}}

Rules:
- "A" = set A's hits are more relevant overall
- "B" = set B's hits are more relevant overall
- "tie" = roughly equivalent OR neither is clearly better
"""
