'use strict';

const vscode = require('vscode');

function nonce() {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
    let s = '';
    for (let i = 0; i < 32; i++) {
        s += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return s;
}

function dashboardHtml(webview, n) {
    const csp = [
        `default-src 'none'`,
        `img-src ${webview.cspSource} data:`,
        `style-src ${webview.cspSource} 'nonce-${n}'`,
        `script-src 'nonce-${n}'`,
    ].join('; ');

    return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8"/>
<meta http-equiv="Content-Security-Policy" content="${csp}"/>
<meta name="viewport" content="width=device-width, initial-scale=1.0"/>
<title>NeuroMesh Packet Inspector</title>
<style nonce="${n}">
:root {
  --nm-green: #00f0b4;
  --nm-cyan: #00d2ff;
  --nm-amber: #f59e0b;
  --nm-rose: #f43f5e;
  --nm-purple: #a855f7;
  --bg: var(--vscode-editor-background);
  --fg: var(--vscode-foreground);
  --muted: var(--vscode-descriptionForeground);
  --card: var(--vscode-editorWidget-background, var(--vscode-sideBar-background));
  --border: var(--vscode-widget-border, var(--vscode-panel-border, rgba(127,127,127,.25)));
  --font: var(--vscode-font-family);
  --mono: var(--vscode-editor-font-family);
}
* { box-sizing: border-box; margin: 0; padding: 0; }
html, body {
  height: 100%;
  background:
    radial-gradient(1200px 420px at 50% -10%, color-mix(in srgb, var(--nm-green) 8%, transparent), transparent 60%),
    var(--bg);
  color: var(--fg);
  font-family: var(--font);
  font-size: 13px;
  line-height: 1.45;
}
body { padding: 1.1rem 1.25rem 2rem; }
.top {
  display: flex; align-items: flex-start; justify-content: space-between;
  gap: 1rem; margin-bottom: 1.1rem; flex-wrap: wrap;
}
.brand { display: flex; align-items: center; gap: 0.75rem; min-width: 0; }
.mark {
  width: 36px; height: 36px; border-radius: 10px; flex-shrink: 0;
  display: grid; place-items: center;
  background: linear-gradient(135deg, color-mix(in srgb, var(--nm-green) 22%, transparent), color-mix(in srgb, var(--nm-cyan) 16%, transparent));
  border: 1px solid color-mix(in srgb, var(--nm-green) 45%, transparent);
  box-shadow: 0 0 18px color-mix(in srgb, var(--nm-green) 18%, transparent);
  color: var(--nm-green); font-weight: 800; letter-spacing: -0.06em; font-size: 12px;
  position: relative;
}
.mark::after {
  content: ''; position: absolute; top: 4px; right: 4px;
  width: 5px; height: 5px; border-radius: 50%;
  background: var(--nm-cyan); box-shadow: 0 0 6px var(--nm-cyan);
}
.brand h1 { font-size: 15px; font-weight: 700; letter-spacing: -0.02em; }
.brand p { color: var(--muted); font-size: 11.5px; margin-top: 1px; }
.live {
  display: inline-flex; align-items: center; gap: 0.4rem;
  padding: 0.28rem 0.7rem; border-radius: 999px;
  border: 1px solid var(--border); background: color-mix(in srgb, var(--card) 80%, transparent);
  font-size: 11.5px; font-weight: 600; color: var(--muted);
}
.dot { width: 7px; height: 7px; border-radius: 50%; background: var(--nm-rose); }
.live.on .dot { background: var(--nm-green); box-shadow: 0 0 8px var(--nm-green); animation: pulse 2s infinite; }
@keyframes pulse { 50% { opacity: .55; transform: scale(.85); } }
.actions { display: flex; gap: 0.4rem; flex-wrap: wrap; }
button, .btn {
  font: inherit; cursor: pointer; border-radius: 8px; border: 1px solid var(--border);
  background: var(--vscode-button-secondaryBackground, transparent);
  color: var(--vscode-button-secondaryForeground, var(--fg));
  padding: 0.38rem 0.75rem; font-weight: 600; font-size: 12px;
}
button.primary {
  background: linear-gradient(135deg, var(--nm-green), #00b48c);
  color: #04110c; border: none;
}
button:hover { filter: brightness(1.08); }
button:disabled { opacity: .45; cursor: default; filter: none; }
.composer {
  display: grid; grid-template-columns: 1fr auto auto; gap: 0.45rem; align-items: stretch;
  margin-bottom: 1rem;
}
textarea {
  font: inherit; font-family: var(--mono); font-size: 12px;
  background: var(--vscode-input-background); color: var(--vscode-input-foreground);
  border: 1px solid var(--vscode-input-border, var(--border));
  border-radius: 8px; padding: 0.55rem 0.7rem; resize: vertical; min-height: 44px;
}
select {
  font: inherit; font-weight: 600; font-size: 12px;
  background: var(--vscode-dropdown-background); color: var(--vscode-dropdown-foreground);
  border: 1px solid var(--border); border-radius: 8px; padding: 0 0.7rem;
}
.kpis {
  display: grid; grid-template-columns: repeat(auto-fit, minmax(132px, 1fr));
  gap: 0.55rem; margin-bottom: 1rem;
}
.kpi {
  background: var(--card); border: 1px solid var(--border); border-radius: 12px;
  padding: 0.75rem 0.85rem; position: relative; overflow: hidden;
}
.kpi::before {
  content: ''; position: absolute; inset: 0 auto auto 0; height: 2px; width: 100%;
  background: linear-gradient(90deg, var(--nm-green), transparent);
}
.kpi.warn::before { background: linear-gradient(90deg, var(--nm-amber), transparent); }
.kpi .label { font-size: 10.5px; font-weight: 650; color: var(--muted); letter-spacing: .02em; text-transform: uppercase; }
.kpi .value { font-size: 22px; font-weight: 750; letter-spacing: -0.03em; margin-top: 0.15rem; font-variant-numeric: tabular-nums; }
.kpi .sub { font-size: 11px; color: var(--muted); margin-top: 0.1rem; }
.bar-wrap { margin-bottom: 1rem; }
.bar-meta { display: flex; justify-content: space-between; color: var(--muted); font-size: 11.5px; margin-bottom: 0.35rem; }
.bar {
  height: 8px; border-radius: 999px; background: color-mix(in srgb, var(--fg) 8%, transparent); overflow: hidden;
  display: flex;
}
.bar .seed { background: var(--nm-cyan); }
.bar .fill { background: var(--nm-green); }
.columns { display: grid; grid-template-columns: 1.4fr 1fr; gap: 0.75rem; }
@media (max-width: 820px) { .columns { grid-template-columns: 1fr; } .composer { grid-template-columns: 1fr; } }
.panel {
  background: var(--card); border: 1px solid var(--border); border-radius: 12px; overflow: hidden;
}
.panel h2 {
  font-size: 11px; font-weight: 700; letter-spacing: .08em; text-transform: uppercase;
  color: var(--muted); padding: 0.7rem 0.9rem 0.45rem;
}
.row {
  display: grid; grid-template-columns: 1fr auto; gap: 0.5rem; align-items: start;
  padding: 0.55rem 0.9rem; border-top: 1px solid var(--border); cursor: pointer;
}
.row:hover { background: color-mix(in srgb, var(--nm-green) 6%, transparent); }
.row .path { font-family: var(--mono); font-size: 12px; word-break: break-all; }
.row .why { color: var(--muted); font-size: 11px; margin-top: 2px; }
.row .tok { font-variant-numeric: tabular-nums; color: var(--muted); font-size: 11.5px; white-space: nowrap; }
.chip {
  display: inline-flex; align-items: center; gap: 0.25rem;
  font-size: 10.5px; font-weight: 700; padding: 0.12rem 0.4rem; border-radius: 999px;
  background: color-mix(in srgb, var(--nm-green) 14%, transparent);
  color: var(--nm-green); border: 1px solid color-mix(in srgb, var(--nm-green) 30%, transparent);
}
.chip.amber { background: color-mix(in srgb, var(--nm-amber) 16%, transparent); color: var(--nm-amber); border-color: color-mix(in srgb, var(--nm-amber) 35%, transparent); }
.chip.purple { background: color-mix(in srgb, var(--nm-purple) 16%, transparent); color: #d8b4fe; border-color: color-mix(in srgb, var(--nm-purple) 35%, transparent); }
.empty { padding: 1.4rem 1rem; color: var(--muted); text-align: center; }
.empty code { font-family: var(--mono); }
.fold-row { display: flex; justify-content: space-between; align-items: center; gap: 0.5rem; }
.banner {
  margin-bottom: 0.9rem; padding: 0.7rem 0.85rem; border-radius: 10px;
  border: 1px solid color-mix(in srgb, var(--nm-amber) 40%, var(--border));
  background: color-mix(in srgb, var(--nm-amber) 10%, transparent);
  color: var(--fg); font-size: 12.5px;
}
.hidden { display: none !important; }
</style>
</head>
<body>
  <div class="top">
    <div class="brand">
      <div class="mark">NM</div>
      <div>
        <h1>Packet Inspector</h1>
        <p>get_context → expand_fold · Grep only if coverage is partial</p>
      </div>
    </div>
    <div class="actions">
      <span class="live" id="live"><span class="dot"></span><span id="liveLabel">offline</span></span>
      <button id="btnReindex">Re-index</button>
      <button id="btnGalaxy">Galaxy</button>
    </div>
  </div>

  <div id="offline" class="banner hidden">Monitor is not running. In this workspace: <code>neuromesh monitor</code></div>

  <div class="composer">
    <textarea id="prompt" placeholder="Task for neuromesh_get_context — e.g. How does neuromesh_get_context pick seed files?"></textarea>
    <select id="mode" title="Membrane mode">
      <option value="max_savings">max_savings · 0 fill</option>
      <option value="balanced" selected>balanced · 8k fill</option>
      <option value="max_quality">max_quality · 16k fill</option>
    </select>
    <button class="primary" id="btnGo">Get context</button>
  </div>

  <div class="kpis" id="kpis"></div>
  <div class="bar-wrap hidden" id="budget">
    <div class="bar-meta"><span id="budgetLabel">Budget</span><span id="budgetNums"></span></div>
    <div class="bar"><div class="seed" id="seedBar"></div><div class="fill" id="fillBar"></div></div>
  </div>
  <div class="columns">
    <section class="panel">
      <h2>Files in the packet</h2>
      <div id="files"><div class="empty">Run Get Context — seeds always ship, the rest fold.</div></div>
    </section>
    <div>
      <section class="panel" style="margin-bottom:.75rem">
        <h2>Coverage &amp; next</h2>
        <div id="coverage"><div class="empty">No packet yet.</div></div>
      </section>
      <section class="panel">
        <h2>Session folds</h2>
        <div id="folds"><div class="empty">Folds appear after skeletonize.</div></div>
      </section>
    </div>
  </div>
<script nonce="${n}">
const vscode = acquireVsCodeApi();
const $ = (id) => document.getElementById(id);
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const fmt = (n) => (n == null || n === '') ? '—' : Number(n).toLocaleString();
const pct = (n) => (n == null || Number.isNaN(Number(n))) ? '—' : Number(n).toFixed(1) + '%';
const base = (p) => String(p || '').replace(/\\\\/g, '/').split('/').pop() || p;

$('btnGo').onclick = () => {
  const prompt = $('prompt').value.trim();
  if (!prompt) return;
  vscode.postMessage({ type: 'getContext', prompt, mode: $('mode').value });
};
$('btnReindex').onclick = () => vscode.postMessage({ type: 'reindex' });
$('btnGalaxy').onclick = () => vscode.postMessage({ type: 'galaxy' });

window.addEventListener('message', (e) => render(e.data || {}));
vscode.postMessage({ type: 'ready' });

function kpi(label, value, sub, warn) {
  return '<div class="kpi' + (warn ? ' warn' : '') + '"><div class="label">' + esc(label) + '</div><div class="value">' + esc(value) + '</div>' + (sub ? '<div class="sub">' + esc(sub) + '</div>' : '') + '</div>';
}

function render(msg) {
  const online = !!msg.online;
  $('live').classList.toggle('on', online);
  $('liveLabel').textContent = online ? ('v' + (msg.version || '0.5') + ' · live') : 'offline';
  $('offline').classList.toggle('hidden', online);
  if (msg.mode) $('mode').value = msg.mode;
  if (msg.prompt && !$('prompt').value) $('prompt').value = msg.prompt;

  const packet = msg.packet;
  const snap = msg.snapshot;
  const ep = packet && packet.evidence_packet;
  const vs = msg.vsWorkspace;
  const vsSel = ep ? parseFloat(String(ep.reduction_vs_selected_pct || '').replace('%','')) : null;
  const claim = (ep && ep.coverage && ep.coverage.claim) || (snap && snap.coverage_claim) || '—';
  const partial = claim === 'partial';
  const files = (ep && ep.files) || msg.files || [];
  const folds = msg.folds || [];
  const lat = packet && packet.latency_ms;
  const phys = ep && ep.physarum_used;
  const foldN = folds.length || (snap && snap.fold_count) || 0;

  $('kpis').innerHTML = [
    kpi('vs workspace', pct(vs), 'tokens not dumped', false),
    kpi('vs selected', pct(vsSel), 'after fold', false),
    kpi('files', fmt(files.length || (snap && snap.file_count)), 'in the packet', false),
    kpi('folds', fmt(foldN), 'session registry', false),
    kpi('latency', lat != null ? lat + ' ms' : '—', 'get_context', false),
    kpi('coverage', String(claim).replace(/_/g,' '), partial ? 'Grep still fair' : 'Grep not needed', partial),
  ].join('');

  const bud = ep && ep.budget;
  if (bud) {
    $('budget').classList.remove('hidden');
    const seed = bud.seed_tokens || 0;
    const fill = bud.fill_used || 0;
    const cap = (bud.seed_tokens || 0) + (bud.fill_cap || 0);
    const total = seed + fill;
    $('budgetLabel').textContent = (bud.mode || 'balanced') + (phys ? ' · physarum ' + (ep.physarum_ms || 0) + ' ms' : ' · seed-then-fill');
    $('budgetNums').textContent = fmt(total) + ' / ' + fmt(cap) + '  (fill ' + fmt(fill) + ' of ' + fmt(bud.fill_cap) + ')';
    const seedPct = cap ? (seed / cap) * 100 : 0;
    const fillPct = cap ? (fill / cap) * 100 : 0;
    $('seedBar').style.width = seedPct + '%';
    $('fillBar').style.width = fillPct + '%';
  } else {
    $('budget').classList.add('hidden');
  }

  if (!files.length) {
    $('files').innerHTML = '<div class="empty">Select code or type a task, then Get context.</div>';
  } else {
    $('files').innerHTML = files.map((f) => {
      const nFold = (f.folded_symbols || []).length;
      return '<div class="row" data-open="' + esc(f.path) + '"><div><div class="path">' + esc(base(f.path)) + '</div><div class="why">' + esc(f.why || '') + (nFold ? ' · ' + nFold + ' folded' : '') + '</div></div><div class="tok">' + (f.tokens ? fmt(f.tokens) + ' tok' : '') + '</div></div>';
    }).join('');
  }

  const cov = ep && ep.coverage;
  const next = (ep && ep.next_actions) || [];
  const seedsHit = (cov && cov.seeds_hit) || [];
  const seedsMissed = (cov && cov.seeds_missed) || [];
  if (!snap && !ep) {
    $('coverage').innerHTML = '<div class="empty">No packet yet.</div>';
  } else {
    $('coverage').innerHTML =
      '<div class="row" style="cursor:default"><div><span class="chip' + (partial ? ' amber' : '') + '">' + esc(String(claim).replace(/_/g,' ')) + '</span>' +
      (phys ? ' <span class="chip purple">physarum</span>' : '') +
      '<div class="why">hit ' + seedsHit.length + ' · missed ' + seedsMissed.length + (seedsMissed.length ? ' · ' + seedsMissed.map(esc).join(', ') : '') + '</div></div></div>' +
      next.map((a) => '<div class="row" style="cursor:default"><div><div class="path">' + esc(a.tool) + '</div><div class="why">' + esc(a.why || a.query || '') + '</div></div></div>').join('');
  }

  if (!folds.length) {
    $('folds').innerHTML = '<div class="empty">No fold ids in this packet.</div>';
  } else {
    $('folds').innerHTML = folds.map((f) =>
      '<div class="row fold-row"><div><div class="path">' + esc(f.symbol_name || f.fold_id) + '</div><div class="why">' + esc(base(f.file_path)) + ' · ' + esc(f.fold_id) + '</div></div><button data-fold="' + esc(f.fold_id) + '">Expand</button></div>'
    ).join('');
  }

  document.querySelectorAll('[data-open]').forEach((el) => {
    el.onclick = () => vscode.postMessage({ type: 'openFile', path: el.getAttribute('data-open') });
  });
  document.querySelectorAll('[data-fold]').forEach((el) => {
    el.onclick = (ev) => { ev.stopPropagation(); vscode.postMessage({ type: 'expand', fold_id: el.getAttribute('data-fold') }); };
  });
}
</script>
</body>
</html>`;
}

class DashboardPanel {
    /**
     * @param {import('./state').MeshState} state
     * @param {Record<string, Function>} [handlers]
     */
    constructor(state, handlers) {
        this.state = state;
        this.handlers = handlers || {};
        this.panel = undefined;
        this._prompt = '';
        this.iconUri = undefined;
    }

    reveal(prompt) {
        if (prompt) {
            this._prompt = prompt;
        }
        if (this.panel) {
            this.panel.reveal(vscode.ViewColumn.Beside);
            this._push();
            return this.panel;
        }
        const panel = vscode.window.createWebviewPanel(
            'neuromeshDashboard',
            'NeuroMesh Packet',
            vscode.ViewColumn.Beside,
            { enableScripts: true, retainContextWhenHidden: true }
        );
        if (this.iconUri) {
            panel.iconPath = this.iconUri;
        }
        this.panel = panel;
        const n = nonce();
        panel.webview.html = dashboardHtml(panel.webview, n);
        panel.iconPath = undefined;
        panel.onDidDispose(() => {
            this.panel = undefined;
        });
        panel.webview.onDidReceiveMessage(async (msg) => {
            if (!msg || !msg.type) {
                return;
            }
            if (msg.type === 'ready') {
                this._push();
                return;
            }
            if (this.handlers && typeof this.handlers[msg.type] === 'function') {
                await this.handlers[msg.type](msg);
            }
        });
        return panel;
    }

    _push() {
        if (!this.panel) {
            return;
        }
        const s = this.state;
        this.panel.webview.postMessage({
            online: s.online,
            version: s.status && s.status.version,
            mode: (s.status && s.status.mode) || vscode.workspace.getConfiguration('neuromesh').get('defaultMode'),
            packet: s.packet,
            snapshot: s.snapshot(),
            files: s.files(),
            folds: s.folds(),
            vsWorkspace: s.vsWorkspacePct(),
            prompt: this._prompt,
        });
    }
}

function registerDashboard(context, state, handlers) {
    const dash = new DashboardPanel(state, handlers);
    dash.iconUri = vscode.Uri.joinPath(context.extensionUri, 'media', 'activity.svg');
    context.subscriptions.push(state.onDidChange(() => dash._push()));
    return dash;
}

module.exports = { registerDashboard, DashboardPanel };
