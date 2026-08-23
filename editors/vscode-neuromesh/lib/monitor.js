'use strict';

const vscode = require('vscode');

function monitorHtml(origin) {
    const src = origin.replace(/"/g, '');
    return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8"/>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; frame-src ${src} http://127.0.0.1:* http://localhost:*; style-src 'unsafe-inline'; script-src 'unsafe-inline';"/>
<meta name="viewport" content="width=device-width, initial-scale=1.0"/>
<title>NeuroMesh Galaxy</title>
<style>
  html, body { margin: 0; height: 100%; background: #06080d; color: #94a3b8; font-family: var(--vscode-font-family, system-ui); }
  .chrome {
    height: 42px; display: flex; align-items: center; justify-content: space-between;
    padding: 0 12px; border-bottom: 1px solid rgba(255,255,255,.08);
    background: rgba(12,16,23,.92);
  }
  .chrome strong { color: #f8fafc; font-size: 12px; letter-spacing: .02em; }
  .chrome span { font-size: 11px; }
  a { color: #00f0b4; text-decoration: none; font-size: 12px; font-weight: 650; }
  iframe { width: 100%; height: calc(100% - 42px); border: 0; background: #06080d; }
  .fallback { padding: 2.5rem 1.5rem; text-align: center; line-height: 1.6; }
  code { color: #00d2ff; }
</style>
</head>
<body>
  <div class="chrome">
    <div><strong>Galaxy monitor</strong> <span>— live graph at ${src}</span></div>
    <a href="${src}" id="ext">Open in browser</a>
  </div>
  <iframe src="${src}" title="NeuroMesh galaxy"></iframe>
  <script>
    const vscode = acquireVsCodeApi();
    document.getElementById('ext').addEventListener('click', (e) => {
      e.preventDefault();
      vscode.postMessage({ type: 'external' });
    });
  </script>
</body>
</html>`;
}

function registerMonitor(context, api) {
    let panel;
    const open = () => {
        const origin = api.origin();
        if (panel) {
            panel.reveal(vscode.ViewColumn.Beside);
            panel.webview.html = monitorHtml(origin);
            return;
        }
        panel = vscode.window.createWebviewPanel(
            'neuromeshMonitor',
            'NeuroMesh Galaxy',
            vscode.ViewColumn.Beside,
            { enableScripts: true, retainContextWhenHidden: true, enableForms: true }
        );
        panel.webview.html = monitorHtml(origin);
        panel.webview.onDidReceiveMessage((msg) => {
            if (msg && msg.type === 'external') {
                vscode.env.openExternal(vscode.Uri.parse(origin));
            }
        });
        panel.onDidDispose(() => {
            panel = undefined;
        });
    };
    return { open };
}

module.exports = { registerMonitor };
