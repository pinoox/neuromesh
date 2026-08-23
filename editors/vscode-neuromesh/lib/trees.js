'use strict';

const vscode = require('vscode');
const path = require('path');

function fmt(n) {
    if (n == null || Number.isNaN(n)) {
        return '—';
    }
    return Number(n).toLocaleString();
}

function pct(n) {
    if (n == null || Number.isNaN(n)) {
        return '—';
    }
    return `${n.toFixed(1)}%`;
}

function basename(p) {
    if (!p) {
        return '—';
    }
    return path.posix.basename(String(p).replace(/\\/g, '/'));
}

class MeshTreeProvider {
    /**
     * @param {import('./state').MeshState} state
     */
    constructor(state) {
        this.state = state;
        this._onDidChangeTreeData = new vscode.EventEmitter();
        this.onDidChangeTreeData = this._onDidChangeTreeData.event;
        state.onDidChange(() => this._onDidChangeTreeData.fire());
    }

    refresh() {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(item) {
        return item;
    }

    getChildren() {
        const s = this.state;
        if (!s.online || !s.status) {
            return [];
        }
        const st = s.status;
        const g = st.graph || {};
        const bio = st.biomimetic || {};
        const snap = s.snapshot() || {};
        const mode = st.mode || 'balanced';
        const folds = st.session_folds != null ? st.session_folds : snap.fold_count || 0;
        const phys = bio.physarum_solver === 'active' || snap.physarum_used;

        const rows = [
            ['Status', s.online ? 'live' : 'offline', s.online ? 'pulse' : 'debug-disconnect'],
            ['Version', st.version || '—', 'tag'],
            ['Project', st.project_id || '—', 'root-folder'],
            ['Mode', mode, 'settings-gear'],
            ['Files', fmt(g.file_nodes), 'file'],
            ['Nodes', fmt(g.total_nodes), 'symbol-namespace'],
            ['Edges', fmt(g.total_edges), 'type-hierarchy'],
            ['Calls resolved', fmt(g.resolved_calls), 'references'],
            ['Session folds', fmt(folds), 'folding-collapsed'],
            ['Fill cap', fmt(st.fill_cap), 'dashboard'],
            [
                'Physarum',
                phys ? `active · ${snap.physarum_ms || bio.physarum_last_ms || 0} ms` : 'idle',
                'circuit-board',
            ],
        ];

        return rows.map(([label, value, icon]) => {
            const item = new vscode.TreeItem(`${label}  ${value}`, vscode.TreeItemCollapsibleState.None);
            item.iconPath = new vscode.ThemeIcon(icon);
            item.tooltip = `${label}: ${value}`;
            item.contextValue = 'meshStat';
            return item;
        });
    }
}

class PacketTreeProvider {
    constructor(state) {
        this.state = state;
        this._onDidChangeTreeData = new vscode.EventEmitter();
        this.onDidChangeTreeData = this._onDidChangeTreeData.event;
        state.onDidChange(() => this._onDidChangeTreeData.fire());
    }

    refresh() {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(item) {
        return item;
    }

    getChildren() {
        const s = this.state;
        const files = s.files();
        const snap = s.snapshot();
        if (!s.online) {
            return [];
        }
        if (!snap && !files.length) {
            return [];
        }

        const header = [];
        if (snap) {
            const claim = snap.coverage_claim || '—';
            const vs = s.vsWorkspacePct();
            const meta = new vscode.TreeItem(
                `${claim.replace(/_/g, ' ')}  ·  ${pct(vs)} vs workspace`,
                vscode.TreeItemCollapsibleState.None
            );
            meta.iconPath = new vscode.ThemeIcon(
                claim === 'partial' ? 'warning' : 'pass-filled',
                new vscode.ThemeColor(
                    claim === 'partial' ? 'editorWarning.foreground' : 'testing.iconPassed'
                )
            );
            meta.command = { command: 'neuromesh.openDashboard', title: 'Open Packet Inspector' };
            meta.tooltip = 'Open the packet inspector';
            header.push(meta);

            if (snap.grep_needed) {
                const grep = new vscode.TreeItem(
                    'Grep still needed — coverage is partial',
                    vscode.TreeItemCollapsibleState.None
                );
                grep.iconPath = new vscode.ThemeIcon('search');
                grep.command = { command: 'neuromesh.searchSymbols', title: 'Search symbols' };
                header.push(grep);
            }
        }

        const items = files.map((file) => {
            const p = file.path || '';
            const why = file.why || '';
            const tokens = file.tokens ? ` · ${fmt(file.tokens)} tok` : '';
            const folded = (file.folded_symbols || []).length;
            const foldBit = folded ? ` · ${folded} folded` : '';
            const item = new vscode.TreeItem(
                `${basename(p)}${tokens}${foldBit}`,
                vscode.TreeItemCollapsibleState.None
            );
            item.description = why ? String(why).replace(/_/g, ' ') : undefined;
            item.tooltip = [p, why && `why: ${why}`, file.tokens && `${file.tokens} tokens`]
                .filter(Boolean)
                .join('\n');
            item.iconPath = new vscode.ThemeIcon('file-code');
            item.contextValue = 'packetFile';
            const lr = file.line_range;
            const start = lr && (Array.isArray(lr) ? lr[0] : lr.start);
            item.command = {
                command: 'neuromesh.openFile',
                title: 'Open',
                arguments: [{ path: p, line: start }],
            };
            item.resourceUri = vscode.Uri.file(p);
            return item;
        });

        return header.concat(items);
    }
}

class FoldsTreeProvider {
    constructor(state) {
        this.state = state;
        this._onDidChangeTreeData = new vscode.EventEmitter();
        this.onDidChangeTreeData = this._onDidChangeTreeData.event;
        state.onDidChange(() => this._onDidChangeTreeData.fire());
    }

    refresh() {
        this._onDidChangeTreeData.fire();
    }

    getTreeItem(item) {
        return item;
    }

    getChildren() {
        const folds = this.state.folds();
        if (!this.state.online || !folds.length) {
            return [];
        }
        return folds.map((fold) => {
            const item = new vscode.TreeItem(fold.symbol_name || fold.fold_id, vscode.TreeItemCollapsibleState.None);
            item.description = basename(fold.file_path);
            item.tooltip = `${fold.fold_id}\n${fold.file_path}\nRestore from the session registry (no disk grep).`;
            item.iconPath = new vscode.ThemeIcon('folding-collapsed');
            item.contextValue = 'fold';
            item.command = {
                command: 'neuromesh.expandFoldId',
                title: 'Expand',
                arguments: [{ fold_id: fold.fold_id }],
            };
            return item;
        });
    }
}

function registerTrees(context, state) {
    const mesh = new MeshTreeProvider(state);
    const packet = new PacketTreeProvider(state);
    const folds = new FoldsTreeProvider(state);
    context.subscriptions.push(
        vscode.window.registerTreeDataProvider('neuromesh.mesh', mesh),
        vscode.window.registerTreeDataProvider('neuromesh.packet', packet),
        vscode.window.registerTreeDataProvider('neuromesh.folds', folds)
    );
    return { mesh, packet, folds };
}

module.exports = { registerTrees, fmt, pct, basename };
