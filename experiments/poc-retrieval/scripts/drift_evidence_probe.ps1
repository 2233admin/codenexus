# drift_evidence_probe.ps1
# Generated from .planning/probes/drift_evidence_probe.md (frozen 2026-05-02).
#
# Purpose: re-index poc + fsc corpora 5 times each, dump symbols/edges/alias_decls
# JSONs per run for drift_compare.py to compute M1-M6 metrics.
#
# Spec authority: Codex tactical pick 2026-05-02 ("First: (1) drift cheap probe").
# Budget: 4-8 hr (this script: ~67 min wall-clock + ~30 min analysis).

$ErrorActionPreference = "Stop"
$probeRoot = "D:/projects/codenexus/experiments/poc-retrieval"
$bin = "$probeRoot/target/release/codenexus-core.exe"
$evalDir = "$probeRoot/eval/drift_runs"
$pyExe = "py"

if (-not (Test-Path $bin)) { throw "binary missing: $bin" }
New-Item -ItemType Directory -Path $evalDir -Force | Out-Null

# Corpora — pinned per drift_evidence_probe.md spec.
$corpora = @(
  @{ name = "poc"; src = "D:/projects/obsidian-llm-wiki/mcp-server/src" },
  @{ name = "fsc"; src = "D:/projects/full-self-coding" }
)

foreach ($c in $corpora) {
  if (-not (Test-Path $c.src)) { throw "corpus source missing: $($c.src)" }
}

# Backup existing poc.db / fsc.db once before r1 (safety).
foreach ($c in $corpora) {
  $existing = "$probeRoot/$($c.name).db"
  $bak = "$probeRoot/$($c.name).db.preprobe.bak"
  if ((Test-Path $existing) -and -not (Test-Path $bak)) {
    Copy-Item $existing $bak -Force
    "BACKUP: $existing -> $bak"
  }
}

# Run matrix: 5 runs per corpus.
$startTime = Get-Date
$runLog = @()

foreach ($c in $corpora) {
  $cName = $c.name
  $cSrc = $c.src
  for ($r = 1; $r -le 5; $r++) {
    $runId = "r$r"
    $dbPath = "$probeRoot/$cName.db.$runId"
    $logPath = "$evalDir/$cName.$runId.indexer.log"

    Remove-Item $dbPath -Force -ErrorAction SilentlyContinue
    "[$cName/$runId] indexing $cSrc -> $dbPath"
    $tStart = Get-Date

    & $bin index --repo $cSrc --db $dbPath --max-consecutive-fail 5 *>&1 | Tee-Object -FilePath $logPath | Out-Null
    $exitCode = $LASTEXITCODE

    $tEnd = Get-Date
    $elapsedSec = [math]::Round(($tEnd - $tStart).TotalSeconds, 1)

    if ($exitCode -ne 0) {
      "[$cName/$runId] FAIL exit=$exitCode after ${elapsedSec}s -- check $logPath"
      $runLog += [PSCustomObject]@{ corpus = $cName; run = $runId; status = "FAIL"; exit = $exitCode; elapsed_sec = $elapsedSec }
      throw "indexer failed at $cName/$runId; aborting probe"
    }

    "[$cName/$runId] OK in ${elapsedSec}s"
    $runLog += [PSCustomObject]@{ corpus = $cName; run = $runId; status = "OK"; exit = 0; elapsed_sec = $elapsedSec }

    # Dump symbols + edges + alias_decls JSON via python (sqlite3 stdlib).
    $symbolsJson = "$evalDir/$cName.$runId.symbols.json"
    $edgesJson = "$evalDir/$cName.$runId.edges.json"
    $aliasJson = "$evalDir/$cName.$runId.alias_decls.json"

    $pyDump = @"
import sqlite3, json, sys
db = r'$dbPath'
out_sym = r'$symbolsJson'
out_edge = r'$edgesJson'
out_alias = r'$aliasJson'

con = sqlite3.connect(db)
con.row_factory = sqlite3.Row
cur = con.cursor()

# symbols (verify columns first)
cols = [r[1] for r in cur.execute('PRAGMA table_info(symbols)').fetchall()]
sym_cols = [c for c in ['id','file','name','kind','line'] if c in cols]
sym_query = f"SELECT {', '.join(sym_cols)} FROM symbols ORDER BY file, name, line"
sym_rows = [dict(r) for r in cur.execute(sym_query).fetchall()]
with open(out_sym, 'w', encoding='utf-8') as f:
    json.dump(sym_rows, f, ensure_ascii=False)

# edges
edge_cols_db = [r[1] for r in cur.execute('PRAGMA table_info(edges)').fetchall()]
edge_cols = [c for c in ['src_id','dst_id','kind','confidence'] if c in edge_cols_db]
edge_query = f"SELECT {', '.join(edge_cols)} FROM edges ORDER BY src_id, dst_id, kind"
edge_rows = [dict(r) for r in cur.execute(edge_query).fetchall()]
with open(out_edge, 'w', encoding='utf-8') as f:
    json.dump(edge_rows, f, ensure_ascii=False)

# alias_decls (W0+ table; may not exist if pre-W0 binary used)
try:
    alias_rows = [dict(r) for r in cur.execute(
        "SELECT from_file, alias, target_file, target_member FROM alias_decls ORDER BY from_file, alias"
    ).fetchall()]
    with open(out_alias, 'w', encoding='utf-8') as f:
        json.dump(alias_rows, f, ensure_ascii=False)
except sqlite3.OperationalError as e:
    print(f'alias_decls dump skipped: {e}', file=sys.stderr)

print(f'symbols={len(sym_rows)} edges={len(edge_rows)}')
con.close()
"@

    $pyDump | & $pyExe - 2>&1 | Out-Null
    "[$cName/$runId] dumped JSONs to $evalDir"
  }
}

$totalElapsed = [math]::Round(((Get-Date) - $startTime).TotalMinutes, 1)
"PROBE DONE total=${totalElapsed}min"

# Persist run log.
$runLog | ConvertTo-Json -Depth 5 | Out-File "$evalDir/run_log.json" -Encoding utf8
"run_log.json written to $evalDir/"
"next: py $probeRoot/eval/drift_compare.py --eval-dir $evalDir --out $probeRoot/eval/drift_evidence_probe_results.json"
