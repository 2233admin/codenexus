"""R7b LLM-judge re-evaluation of spike-007 axis-3 graph traversal results.
Closes EVAL_DESIGN_NOTES Rule 3: hand-matcher substring bias.
Spike-007 15%% via hand. C1/C2/C10 flagged under-counted.
Replaces hand-matcher with ARM_B graded judge (t>=2 = match).
Seeds: 3 x 25 hits = 75 calls. concurrency=24, okaoi pool.
Usage: uv run python r7b_llm_judge_axis3.py [--smoke]
"""

import argparse
import asyncio
import itertools
import json
import os
import random
import re
import sqlite3
import sys
import time
from pathlib import Path
from typing import Any

import anthropic
from anthropic import AsyncAnthropic
from dotenv import load_dotenv
from tenacity import retry, retry_if_exception_type, stop_after_attempt, wait_exponential

from ragas_prompts import ARM_B_GRADED_PROMPT, JUDGE_SYSTEM

ROOT = Path(__file__).parent
DB = ROOT.parent / 'poc.db'
GRAPH_RESULTS = ROOT / 'round_7_graph_axis3.json'
load_dotenv(ROOT / '.env')


def build_clients() -> tuple[list[AsyncAnthropic], str]:
    provider = os.environ.get('EVAL_PROVIDER', 'minimax_official').lower()
    if provider == 'okaoi':
        base = os.environ['OKAOI_BASE_URL']
        keys = [os.environ[f'OKAOI_KEY_{i}'] for i in (1, 2, 3)]
        model = os.environ.get('OKAOI_MODEL', 'MiniMax-M2.7')
        return [AsyncAnthropic(base_url=base, auth_token=k, timeout=60.0) for k in keys], model
    base = os.environ['ANTHROPIC_BASE_URL']
    token = os.environ['ANTHROPIC_AUTH_TOKEN']
    model = os.environ.get('ANTHROPIC_MODEL', 'MiniMax-M2.5')
    return [AsyncAnthropic(base_url=base, auth_token=token, timeout=60.0)], model

CLIENTS, MODEL = build_clients()
_CLIENT_CYCLE = itertools.cycle(CLIENTS)
CONCURRENCY = int(os.environ.get('EVAL_CONCURRENCY', '24'))
RETRY_MAX = int(os.environ.get('EVAL_RETRY_MAX', '5'))
GRADE_THRESHOLD = 2  # Rule 6: t>=2 counts as match


def safe_json(raw: str) -> dict[str, Any]:
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        s, e = raw.find('{'), raw.rfind('}')
        if s >= 0 and e > s:
            try:
                return json.loads(raw[s:e+1])
            except json.JSONDecodeError:
                pass
        result: dict[str, Any] = {}
        m = re.search(r'"grade"\s*:\s*(\d+)', raw)
        if m: result['grade'] = int(m.group(1))
        m = re.search(r'"reason"\s*:\s*"([^"]*?)"', raw)
        if m: result['reason'] = m.group(1)
        if result:
            result['_recovered'] = True
            return result
    return {'_parse_error': True, 'raw': raw[:300]}

def load_snippet(cur: sqlite3.Cursor, path: str, name: str) -> dict[str, Any] | None:
    for p in (path, path.replace('\\', '/'), path.replace('/', '\\')):
        cur.execute(
            'SELECT path, name, kind, snippet FROM symbols WHERE path=? AND name=? LIMIT 1',
            (p, name),
        )
        r = cur.fetchone()
        if r:
            return {'path': r[0], 'name': r[1], 'kind': r[2], 'snippet': r[3] or ''}
    return None

@retry(
    stop=stop_after_attempt(RETRY_MAX),
    wait=wait_exponential(multiplier=1, min=1, max=30),
    retry=retry_if_exception_type(
        (anthropic.APIError, anthropic.APITimeoutError,
         anthropic.APIConnectionError, asyncio.TimeoutError)
    ),
    reraise=True,
)
async def call_judge(prompt: str, max_tokens: int = 300) -> dict[str, Any]:
    client = next(_CLIENT_CYCLE)
    resp = await client.messages.create(
        model=MODEL, max_tokens=max_tokens, system=JUDGE_SYSTEM,
        messages=[{'role': 'user', 'content': prompt}], temperature=0.0,
    )
    text = ''
    for block in resp.content or []:
        if getattr(block, 'type', None) == 'text':
            text = block.text
            break
    return safe_json(text or '')

async def judge_hit(sem: asyncio.Semaphore, query: str, snip: dict[str, Any]) -> dict[str, Any]:
    async with sem:
        snippet_truncated = (snip.get('snippet') or '')[:2000]
        prompt = ARM_B_GRADED_PROMPT.format(
            query=query, path=snip['path'], kind=snip['kind'],
            name=snip['name'], snippet=snippet_truncated,
        )
        return await call_judge(prompt)

async def run_eval(smoke: bool = False) -> dict[str, Any]:
    with open(GRAPH_RESULTS, encoding='utf-8') as f2:
        graph_data = json.load(f2)
    results_by_id = {r['id']: r for r in graph_data['results']}

    conn = sqlite3.connect(str(DB))
    cur = conn.cursor()
    tasks: list[dict[str, Any]] = []
    for qid, res in results_by_id.items():
        if res.get('negative') and not res.get('top5_full'):
            continue
        for hit in res.get('top5_full', []):
            snip = load_snippet(cur, hit['path'], hit['name'])
            tasks.append({
                'qid': qid, 'query': res['query'],
                'negative': res.get('negative', False),
                'hand_p_at_5': res.get('precision_at_5'),
                'hit_path': hit['path'], 'hit_name': hit['name'],
                'hit_kind': hit['kind'], 'snip': snip,
            })
    conn.close()

    valid = [t for t in tasks if t['snip'] is not None]
    missing = len(tasks) - len(valid)
    seeds = [42] if smoke else [42, 7, 13]
    if smoke:
        valid = valid[:5]
        print(f'[SMOKE] {len(valid)} hits, 1 seed', file=sys.stderr)

    provider = os.environ.get('EVAL_PROVIDER', 'minimax_official')
    print(
        f'Tasks: {len(tasks)} | valid={len(valid)} | missing={missing} |'
        f' seeds={len(seeds)} | calls={len(valid)*len(seeds)} | concurrency={CONCURRENCY} | provider={provider} | model={MODEL}',
        file=sys.stderr,
    )

    sem = asyncio.Semaphore(CONCURRENCY)
    t0 = time.time()
    acc: dict[tuple[str, str, str], list[int]] = {}

    for si, seed in enumerate(seeds):
        shuffled = valid[:]
        random.seed(seed)
        random.shuffle(shuffled)
        coros = [judge_hit(sem, t['query'], t['snip']) for t in shuffled]
        seed_res = await asyncio.gather(*coros, return_exceptions=True)
        for task, result in zip(shuffled, seed_res):
            key = (task['qid'], task['hit_path'], task['hit_name'])
            acc.setdefault(key, [])
            if isinstance(result, Exception):
                print(f'  [WARN] {key}: {result}', file=sys.stderr)
                continue
            g = result.get('grade')
            if g is not None and not isinstance(g, bool):
                acc[key].append(int(g))
        print(f'  seed {seed} done ({si+1}/{len(seeds)}) elapsed={time.time()-t0:.1f}s', file=sys.stderr)

    wall = time.time() - t0

    qhg: dict[str, list[float]] = {}
    for task in valid:
        key = (task['qid'], task['hit_path'], task['hit_name'])
        gs = acc.get(key, [])
        qhg.setdefault(task['qid'], []).append(sum(gs)/len(gs) if gs else 0.0)

    pq: list[dict[str, Any]] = []
    for qid, res in sorted(results_by_id.items()):
        hp5 = res.get('precision_at_5', 0.0)
        neg = res.get('negative', False)
        if neg and not res.get('top5_full'):
            pq.append({'qid': qid, 'query': res['query'], 'negative': True,
                'hand_p_at_5': hp5, 'llm_p_at_5': 1.0, 'n_hits': 0,
                'n_graded': 0, 'mean_grades': [], 'note': 'NEG correct abstain', 'disagree': False})
            continue
        mg = qhg.get(qid, [])
        if not mg:
            pq.append({'qid': qid, 'query': res['query'], 'negative': neg,
                'hand_p_at_5': hp5, 'llm_p_at_5': None,
                'n_hits': len(res.get('top5_full', [])),
                'n_graded': 0, 'mean_grades': [], 'note': 'unresolved -- no hits', 'disagree': False})
            continue
        nm = sum(1 for g in mg if g >= GRADE_THRESHOLD)
        lp5 = nm / len(mg)
        pq.append({'qid': qid, 'query': res['query'], 'negative': neg,
            'hand_p_at_5': hp5, 'llm_p_at_5': lp5,
            'n_hits': len(res.get('top5_full', [])),
            'n_graded': len(mg), 'mean_grades': mg, 'note': res.get('note', ''),
            'disagree': (hp5 > 0) != (lp5 > 0)})

    scored = [r for r in pq if r['llm_p_at_5'] is not None]
    llm_mean = sum(r['llm_p_at_5'] for r in scored)/len(scored) if scored else 0.0
    return {
        'llm_mean_precision_at_5': llm_mean,
        'hand_mean_precision_at_5': graph_data.get('avg_precision_at_5', 0.15),
        'r3_retrieval_baseline': graph_data.get('r3_retrieval_baseline_axis3', 0.0),
        'grade_threshold': GRADE_THRESHOLD, 'n_seeds': len(seeds),
        'n_queries_judged': len(scored), 'n_tasks_valid': len(valid),
        'missing_snippets': missing, 'wall_clock_seconds': round(wall, 2),
        'provider': provider, 'model': MODEL, 'per_query': pq,
    }

def write_report(data: dict[str, Any], report_path: Path) -> None:
    llm = data['llm_mean_precision_at_5']
    hand = data['hand_mean_precision_at_5']
    r3 = data['r3_retrieval_baseline']
    pq = data['per_query']
    dq = [r for r in pq if r.get('disagree')]
    ns=data['n_seeds']; mdl=data['model']; prv=data['provider']
    nq=data['n_queries_judged']; gt=data['grade_threshold']; wc=data['wall_clock_seconds']
    out=[
        '# R7b LLM-Judge Re-evaluation -- Axis-3 Graph Traversal','',
        'Closes EVAL_DESIGN_NOTES Rule 3: hand-matcher substring bias.',
        f'Seeds: {ns} | model: {mdl} | provider: {prv}',
        '','## Aggregate Verdict','',
        '| Metric | Value |','|--------|-------|',
        f'| LLM-judge p@5 (n={nq}) | **{llm:.1%}** |',
        f'| Hand-matcher p@5 (spike-007) | {hand:.1%} |',
        f'| R3 retrieval baseline | {r3:.1%} |',
        f'| Delta LLM vs hand | {llm-hand:+.1%} |',
        f'| Grade threshold | >= {gt} |',
        f'| Wall clock | {wc:.1f}s |','',
    ]
    if llm > hand + 0.02:
        out.append('**Verdict: CONFIRMS spike-007** -- LLM precision > hand 15%. Substring bias depressed score.')
    elif abs(llm - hand) <= 0.02:
        out.append('**Verdict: INCONCLUSIVE** -- LLM and hand agree within 2pp.')
    else:
        out.append('**Verdict: CONTRADICTS spike-007** -- LLM lower. Hits adjacent but not answering queries.')
    out+=['','## Per-Query Results','',
        '| ID | Query | Hand p@5 | LLM p@5 | Mean grades | Disagree |',
        '|----|-------|----------|---------|-------------|---------|']
    for r in sorted(pq, key=lambda x: x['qid']):
        lp=r['llm_p_at_5']
        ls=f'{lp:.2f}' if lp is not None else 'N/A'
        gs=r.get('mean_grades',[])
        gstr='['+','.join(f'{g:.1f}' for g in gs)+']' if gs else '--'
        ds='YES' if r.get('disagree') else 'no'
        hp=r['hand_p_at_5']; qid2=r['qid']; qry=r['query'][:52]
        out.append(f'| {qid2} | {qry} | {hp:.2f} | {ls} | {gstr} | {ds} |')
    out+=['','## Disagreements','']
    if dq:
        for r in dq[:5]:
            lp=r['llm_p_at_5']; hp=r['hand_p_at_5']
            ls=f'{lp:.2f}' if lp is not None else 'N/A'
            d='LLM>hand' if (lp or 0)>hp else 'hand>LLM'
            mg2=r.get('mean_grades',[]);nt=r.get('note','')
            qid2=r['qid']; qry=r['query']
            out+=[f'### {qid2}: {qry}',f'- Hand: {hp:.2f} | LLM: {ls} ({d})',
                f'- Grades: {mg2}',f'- Note: {nt}','']
    else:
        out+=['No boolean disagreements -- LLM and hand agree on all judged queries.','']
    out+=['## Structural Notes','',
        'C4/C5/C6/C7: unresolved -- PPR seed not found, top5 empty, both 0.',
        'C9: correct abstain (OAuth2Provider not in graph), both 1.0.',
        'C1: resolve/dispatch -- hand rejects on path substring mismatch.',
        'C2: adapter-adjacent decls -- LLM may credit structural neighbors.',
        'C10: loadConfig/gnAdapter init-adjacent -- LLM may grade gnAdapter relevant.',
        '','## Run Summary','',
        f'LLM-judge axis-3 precision = {llm:.1%} (vs hand {hand:.1%}, retrieval {r3:.1%})']
    report_path.parent.mkdir(parents=True, exist_ok=True)
    _nl=chr(10); report_path.write_text(_nl.join(out), encoding='utf-8')
    print(f'Report written: {report_path}', file=sys.stderr)

async def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument('--smoke', action='store_true')
    parser.add_argument('--out-report', default=str(ROOT / 'r7b_llm_judge_axis3_report.md'))
    args = parser.parse_args()
    data = await run_eval(smoke=args.smoke)
    write_report(data, Path(args.out_report))
    llm=data['llm_mean_precision_at_5']
    hand=data['hand_mean_precision_at_5']
    r3=data['r3_retrieval_baseline']
    print(f'LLM-judge axis-3 precision = {llm:.1%} (vs hand {hand:.1%}, retrieval {r3:.1%})')


if __name__ == '__main__':
    asyncio.run(main())
