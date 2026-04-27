#!/usr/bin/env bash
# Run F1-F10 blind cross-corpus queries against fsc.db; output top-5 hits per query.
# Result: eval/fsc_blind_results.json (array of {id, query, top5}).
set -euo pipefail

cd "$(dirname "$0")/.."

BIN=./target/release/poc-retrieval.exe
DB=fsc.db
ALPHA=0.6
QUERIES_FILE=eval/fsc_blind_queries.json
OUT=eval/fsc_blind_results.json

# Extract queries via python (avoid jq dependency, mirror eval JSON structure)
py -c "
import json, subprocess, sys
queries = json.load(open('${QUERIES_FILE}', encoding='utf-8'))
out = []
for q in queries:
    proc = subprocess.run(
        ['${BIN}', 'query', q['query'], '--db', '${DB}', '--top', '5', '--alpha', '${ALPHA}', '--json'],
        capture_output=True, text=True, encoding='utf-8',
    )
    if proc.returncode != 0:
        print(f'FAIL {q[\"id\"]}: {proc.stderr}', file=sys.stderr)
        sys.exit(1)
    hits = json.loads(proc.stdout)
    top5 = []
    for h in hits[:5]:
        sym = h['symbol']
        top5.append({
            'path': sym['path'],
            'name': sym['name'],
            'kind': sym['kind'],
            'snippet_head': (sym.get('snippet') or '').split('\n')[0][:120],
            'rrf_score': h.get('rrf_score'),
        })
    out.append({'id': q['id'], 'query': q['query'], 'top5': top5})
json.dump(out, open('${OUT}', 'w', encoding='utf-8'), indent=2, ensure_ascii=False)
print(f'wrote ${OUT} ({len(out)} queries)')
"
