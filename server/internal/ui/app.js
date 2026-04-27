// SPDX-License-Identifier: Apache-2.0
// CodeNexus UI -- vanilla JS. Calls Go HTTP API which proxies to Rust core.
// No framework, no build. Edit and reload.

(function () {
  'use strict';

  const $ = (id) => document.getElementById(id);
  const qInput = $('q');
  const repoHashInput = $('repo-hash');
  const searchBtn = $('search-btn');
  const callersBtn = $('callers-btn');
  const statusEl = $('status');
  const tbody = document.querySelector('#results tbody');

  let cy = null;
  let selectedSymbolId = null;

  function setStatus(msg, isError) {
    statusEl.textContent = msg || '';
    statusEl.className = isError ? 'error' : '';
  }

  function fmtScore(n) {
    return (typeof n === 'number') ? n.toFixed(3) : '-';
  }

  function confClass(c) {
    if (c >= 0.95) return 'conf-high';
    if (c >= 0.7)  return 'conf-mid';
    if (c >= 0.5)  return 'conf-low';
    return 'conf-poor';
  }

  function confColor(c) {
    if (c >= 0.95) return '#1a7f37';
    if (c >= 0.7)  return '#9a6700';
    if (c >= 0.5)  return '#d1881a';
    return '#cf222e';
  }

  async function postJSON(url, body) {
    const r = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (!r.ok) {
      const txt = await r.text();
      throw new Error(`${url} -> ${r.status}: ${txt}`);
    }
    return r.json();
  }

  function escapeHtml(s) {
    return String(s == null ? '' : s).replace(/[&<>"']/g, (c) => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
    }[c]));
  }

  function renderResults(hits) {
    tbody.innerHTML = '';
    selectedSymbolId = null;
    callersBtn.disabled = true;
    if (!hits || hits.length === 0) {
      setStatus('No results.');
      return;
    }
    for (const h of hits) {
      const tr = document.createElement('tr');
      tr.dataset.symbolId = h.symbol_id;
      tr.innerHTML = `
        <td>${escapeHtml(h.name)}</td>
        <td>${escapeHtml(h.kind)}</td>
        <td>${escapeHtml(h.path)}:${h.range ? h.range.start_line : '?'}</td>
        <td class="score">${fmtScore(h.bm25_score)}</td>
        <td class="score">${fmtScore(h.vector_score)}</td>
        <td class="score">${fmtScore(h.rrf_score)}</td>
        <td class="score"><b>${fmtScore(h.final_score)}</b></td>
      `;
      tr.addEventListener('click', () => {
        for (const r of tbody.querySelectorAll('tr.selected')) r.classList.remove('selected');
        tr.classList.add('selected');
        selectedSymbolId = h.symbol_id;
        callersBtn.disabled = false;
        setStatus(`Selected: ${h.name} (${h.symbol_id})`);
      });
      tbody.appendChild(tr);
    }
    setStatus(`${hits.length} result${hits.length === 1 ? '' : 's'}.`);
  }

  function renderGraph(targetSymbolId, callers) {
    if (cy) { cy.destroy(); cy = null; }
    const elements = [];
    elements.push({ data: { id: targetSymbolId, label: targetSymbolId.split(':').pop() || targetSymbolId, isTarget: true } });
    for (const c of callers) {
      const conf = (typeof c.confidence === 'number') ? c.confidence : 1.0;
      elements.push({
        data: {
          id: c.symbol_id,
          label: `${c.name} (conf ${conf.toFixed(2)})`,
          confidence: conf,
        },
      });
      elements.push({
        data: {
          id: `e:${c.symbol_id}->${targetSymbolId}`,
          source: c.symbol_id,
          target: targetSymbolId,
          edgeKind: c.edge_kind,
          confidence: conf,
        },
      });
    }
    cy = cytoscape({
      container: $('cy'),
      elements,
      style: [
        { selector: 'node', style: {
            'label': 'data(label)',
            'background-color': (ele) => ele.data('isTarget') ? '#0969da' : confColor(ele.data('confidence') == null ? 1 : ele.data('confidence')),
            'color': '#222', 'font-size': 11, 'text-valign': 'bottom', 'text-margin-y': 4,
            'width': 24, 'height': 24,
        }},
        { selector: 'edge', style: {
            'curve-style': 'bezier', 'target-arrow-shape': 'triangle',
            'line-color': (ele) => confColor(ele.data('confidence') == null ? 1 : ele.data('confidence')),
            'target-arrow-color': (ele) => confColor(ele.data('confidence') == null ? 1 : ele.data('confidence')),
            'width': 2,
            'label': 'data(edgeKind)',
            'font-size': 9, 'color': '#666',
        }},
      ],
      layout: { name: 'cose', animate: false },
    });
  }

  async function doSearch() {
    const q = qInput.value.trim();
    const repoHash = repoHashInput.value.trim();
    if (!q) { setStatus('Enter a query.', true); return; }
    if (!repoHash) { setStatus('Enter repo_hash (from a prior /api/v1/index call).', true); return; }
    setStatus('Searching...');
    try {
      const result = await postJSON('/api/v1/query', { repo_hash: repoHash, q, k: 10 });
      renderResults(result.results || []);
    } catch (e) {
      setStatus(String(e), true);
    }
  }

  async function doListCallers() {
    if (!selectedSymbolId) return;
    const repoHash = repoHashInput.value.trim();
    if (!repoHash) { setStatus('repo_hash required.', true); return; }
    setStatus(`Loading callers of ${selectedSymbolId}...`);
    try {
      const result = await postJSON('/api/v1/list_callers', {
        repo_hash: repoHash,
        symbol_id: selectedSymbolId,
        depth: 1,
      });
      const callers = result.callers || [];
      renderGraph(selectedSymbolId, callers);
      setStatus(`${callers.length} caller${callers.length === 1 ? '' : 's'} of ${selectedSymbolId}. (confClass: ${confClass(callers.length ? (callers[0].confidence || 1) : 1)})`);
    } catch (e) {
      setStatus(String(e), true);
    }
  }

  searchBtn.addEventListener('click', doSearch);
  qInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') doSearch(); });
  callersBtn.addEventListener('click', doListCallers);
})();
