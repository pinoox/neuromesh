'use strict';

const vscode = require('vscode');

function formatPct(n) {
    if (n == null || Number.isNaN(n)) {
        return null;
    }
    return `${n.toFixed(1)}%`;
}

function createStatusBar(context, state) {
    const item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 80);
    item.command = 'neuromesh.openDashboard';
    context.subscriptions.push(item);

    const render = () => {
        if (!state.online) {
            item.text = '$(debug-disconnect) NeuroMesh';
            item.tooltip = new vscode.MarkdownString(
                'NeuroMesh monitor is **offline**.\n\nRun `neuromesh monitor` in this workspace, then click to open the packet inspector.'
            );
            item.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
            item.show();
            return;
        }

        item.backgroundColor = undefined;
        const snap = state.snapshot();
        const st = state.status || {};
        const graph = st.graph || {};
        const vs = formatPct(state.vsWorkspacePct());
        const folds = st.session_folds != null ? st.session_folds : (snap && snap.fold_count) || 0;
        const claim = snap && snap.coverage_claim;

        if (vs && snap) {
            const warn = claim === 'partial';
            item.text = warn
                ? `$(warning) NM ${vs} · ${folds} folds`
                : `$(folding-collapsed) NM ${vs} · ${folds} folds`;
        } else {
            item.text = `$(graph) NM · ${graph.file_nodes || 0} files`;
        }

        const lines = [
            '**NeuroMesh** — click for packet inspector',
            '',
            `- Project: \`${st.project_id || '—'}\``,
            `- Mode: \`${st.mode || 'balanced'}\``,
            `- Graph: ${graph.file_nodes || 0} files · ${graph.total_nodes || 0} nodes · ${graph.total_edges || 0} edges`,
            `- Session folds: ${folds}`,
        ];
        if (snap) {
            lines.push(
                `- Coverage: \`${snap.coverage_claim || '—'}\``,
                snap.grep_needed ? '- Grep still needed (`coverage.claim` is `partial`)' : '- Grep not needed'
            );
        }
        if (vs) {
            lines.push(`- vs workspace: **${vs}**`);
        }
        item.tooltip = new vscode.MarkdownString(lines.join('\n'));
        item.show();
    };

    render();
    context.subscriptions.push(state.onDidChange(render));
    return item;
}

module.exports = { createStatusBar };
