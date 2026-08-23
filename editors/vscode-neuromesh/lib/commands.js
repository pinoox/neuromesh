'use strict';

const vscode = require('vscode');
const path = require('path');
const { foldAtCursor } = require('./decorations');

function cfgMode() {
    return vscode.workspace.getConfiguration('neuromesh').get('defaultMode') || 'balanced';
}

async function openWorkspaceFile(filePath, line) {
    if (!filePath) {
        return;
    }
    const raw = String(filePath);
    const folder = vscode.workspace.workspaceFolders && vscode.workspace.workspaceFolders[0];
    const uri = path.isAbsolute(raw)
        ? vscode.Uri.file(raw)
        : folder
          ? vscode.Uri.joinPath(folder.uri, raw)
          : vscode.Uri.file(raw);
    try {
        const doc = await vscode.workspace.openTextDocument(uri);
        const editor = await vscode.window.showTextDocument(doc, { preview: true });
        if (line != null && Number.isFinite(Number(line))) {
            const pos = new vscode.Position(Math.max(0, Number(line)), 0);
            editor.selection = new vscode.Selection(pos, pos);
            editor.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
        }
    } catch {
        vscode.window.showWarningMessage(`NeuroMesh: could not open ${raw}`);
    }
}

async function showOffline(api) {
    const pick = await vscode.window.showWarningMessage(
        `NeuroMesh monitor is not running at ${api.origin()}.`,
        'Copy start command',
        'Open Packet Inspector'
    );
    if (pick === 'Copy start command') {
        await vscode.env.clipboard.writeText('neuromesh monitor');
        vscode.window.showInformationMessage('Copied `neuromesh monitor` — run it in this workspace.');
    } else if (pick === 'Open Packet Inspector') {
        vscode.commands.executeCommand('neuromesh.openDashboard');
    }
}

async function expandFoldAndShow(api, foldId) {
    const data = await api.expand(foldId, 'VS Code expand_fold');
    if (!data || data.success === false) {
        throw new Error((data && data.error) || 'Fold not in the session registry');
    }
    const lang = guessLanguage(data.file_path);
    const header = [
        `// NeuroMesh restored intron  ${data.fold_id || foldId}`,
        data.symbol_name ? `// ${data.symbol_name}` : null,
        data.file_path ? `// ${data.file_path}:${data.start_line || 1}` : null,
        `// ${data.restored_tokens || 0} tokens · from RAM, not disk`,
        '',
    ]
        .filter(Boolean)
        .join('\n');
    const body = data.original_body || (data.expanded_node && data.expanded_node.node && data.expanded_node.node.content) || '';
    const doc = await vscode.workspace.openTextDocument({
        language: lang,
        content: `${header}${body}`,
    });
    await vscode.window.showTextDocument(doc, { preview: true, viewColumn: vscode.ViewColumn.Beside });
    return data;
}

function guessLanguage(filePath) {
    const ext = path.extname(filePath || '').toLowerCase();
    const map = {
        '.rs': 'rust',
        '.ts': 'typescript',
        '.tsx': 'typescriptreact',
        '.js': 'javascript',
        '.jsx': 'javascriptreact',
        '.py': 'python',
        '.go': 'go',
        '.vue': 'vue',
        '.json': 'json',
        '.toml': 'toml',
        '.md': 'markdown',
    };
    return map[ext] || 'plaintext';
}

function selectionPrompt() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        return '';
    }
    const selected = editor.document.getText(editor.selection).trim();
    if (selected) {
        return selected.length > 2000 ? selected.slice(0, 2000) : selected;
    }
    const word = editor.document.getWordRangeAtPosition(editor.selection.active);
    if (word) {
        return editor.document.getText(word);
    }
    return '';
}

function registerCommands(context, { api, state, dashboard, monitor }) {
    const runGetContext = async (promptOverride, modeOverride) => {
        let prompt = (promptOverride || '').trim() || selectionPrompt();
        if (!prompt) {
            prompt = await vscode.window.showInputBox({
                title: 'NeuroMesh get_context',
                prompt: 'Task description — identifiers stay as written, not lowercased.',
                placeHolder: 'How does handle_tool_call extract intent?',
                value: vscode.window.activeTextEditor
                    ? path.basename(vscode.window.activeTextEditor.document.fileName)
                    : '',
            });
        }
        if (!prompt) {
            return;
        }
        const mode = modeOverride || cfgMode();
        dashboard.reveal(prompt);
        try {
            const packet = await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'NeuroMesh: routing & folding…',
                },
                () => api.simulate(prompt, mode)
            );
            if (packet && packet.error) {
                throw new Error(packet.error);
            }
            state.setPacket(packet);
            await state.refresh();
            const ep = packet.evidence_packet || {};
            const vs = ep.reduction_vs_workspace_pct || '—';
            const claim = (ep.coverage && ep.coverage.claim) || 'unknown';
            const nFiles = (ep.files || []).length;
            const msg = `Packet ${vs} vs workspace · ${nFiles} files · coverage ${claim}`;
            if (claim === 'partial') {
                vscode.window.showWarningMessage(`NeuroMesh: ${msg}. Grep is still fair.`);
            } else {
                vscode.window.showInformationMessage(`NeuroMesh: ${msg}.`);
            }
        } catch (err) {
            if (!state.online) {
                await showOffline(api);
            } else {
                vscode.window.showErrorMessage(`NeuroMesh get_context failed: ${err.message}`);
            }
        }
    };

    const cmds = [
        vscode.commands.registerCommand('neuromesh.openDashboard', () => {
            dashboard.reveal(selectionPrompt());
        }),
        vscode.commands.registerCommand('neuromesh.openMonitor', () => monitor.open()),
        vscode.commands.registerCommand('neuromesh.refresh', () => state.refresh()),
        vscode.commands.registerCommand('neuromesh.getContext', () => runGetContext()),
        vscode.commands.registerCommand('neuromesh.activateContext', () => runGetContext()),
        vscode.commands.registerCommand('neuromesh.reindexProject', async () => {
            try {
                const data = await vscode.window.withProgress(
                    {
                        location: vscode.ProgressLocation.Notification,
                        title: 'NeuroMesh: re-indexing graph…',
                    },
                    () => api.reindex()
                );
                await state.refresh();
                const g = (data && data.graph_stats) || {};
                vscode.window.showInformationMessage(
                    `NeuroMesh: re-indexed ${data.indexed_files || 0} files · ${g.total_nodes || 0} nodes · ${g.total_edges || 0} edges`
                );
            } catch (err) {
                if (!state.online) {
                    await showOffline(api);
                } else {
                    vscode.window.showErrorMessage(`NeuroMesh reindex: ${err.message}`);
                }
            }
        }),
        vscode.commands.registerCommand('neuromesh.expandFold', async () => {
            const editor = vscode.window.activeTextEditor;
            const hit = foldAtCursor(editor);
            if (!hit) {
                vscode.window.showInformationMessage(
                    'Place the cursor on a `[neuromesh:fold:…]` marker, or expand from Session Folds.'
                );
                return;
            }
            try {
                await expandFoldAndShow(api, hit.fold_id);
            } catch (err) {
                vscode.window.showErrorMessage(`NeuroMesh expand_fold: ${err.message}`);
            }
        }),
        vscode.commands.registerCommand('neuromesh.expandFoldId', async (arg) => {
            const foldId = (arg && (arg.fold_id || arg.id)) || '';
            if (!foldId) {
                return vscode.commands.executeCommand('neuromesh.expandFold');
            }
            try {
                await expandFoldAndShow(api, foldId);
            } catch (err) {
                vscode.window.showErrorMessage(`NeuroMesh expand_fold: ${err.message}`);
            }
        }),
        vscode.commands.registerCommand('neuromesh.skeletonizeFile', async () => {
            const editor = vscode.window.activeTextEditor;
            if (!editor) {
                return;
            }
            const filePath = editor.document.uri.fsPath;
            try {
                const res = await vscode.window.withProgress(
                    { location: vscode.ProgressLocation.Notification, title: 'NeuroMesh: skeletonizing…' },
                    () => api.mcpCall('neuromesh_get_file_skeleton', { file_path: filePath })
                );
                const body = (res && res.result) || res || {};
                if (body.error) {
                    throw new Error(body.error);
                }
                const doc = await vscode.workspace.openTextDocument({
                    language: editor.document.languageId,
                    content: body.skeleton_code || JSON.stringify(body, null, 2),
                });
                await vscode.window.showTextDocument(doc, { preview: true, viewColumn: vscode.ViewColumn.Beside });
                const n = (body.folds || []).length || body.introns_folded || 0;
                vscode.window.showInformationMessage(
                    `NeuroMesh: ${body.token_reduction_pct || '—'} after fold · ${n} introns`
                );
            } catch (err) {
                if (!state.online) {
                    await showOffline(api);
                } else {
                    vscode.window.showErrorMessage(`NeuroMesh skeletonize: ${err.message}`);
                }
            }
        }),
        vscode.commands.registerCommand('neuromesh.searchSymbols', async () => {
            const query = await vscode.window.showInputBox({
                title: 'NeuroMesh search_symbols',
                prompt: 'Use only when coverage.claim is partial.',
                placeHolder: 'handle_tool_call',
                value: selectionPrompt(),
            });
            if (!query) {
                return;
            }
            try {
                const res = await api.mcpCall('neuromesh_search_symbols', { query, limit: 20 });
                const result = (res && res.result) || res;
                const hits = (result && (result.hits || result.symbols || result.results)) || [];
                if (!Array.isArray(hits) || !hits.length) {
                    vscode.window.showInformationMessage(`NeuroMesh: no symbols for “${query}”.`);
                    return;
                }
                const picked = await vscode.window.showQuickPick(
                    hits.map((h) => {
                        const lr = h.line_range;
                        const start = lr && (Array.isArray(lr) ? lr[0] : lr.start);
                        return {
                            label: h.name || h.symbol || String(h),
                            description: [h.node_type, h.file_path || h.path]
                                .filter(Boolean)
                                .join(' · '),
                            detail: [h.signature, h.match_reason, h.score != null && `score ${Number(h.score).toFixed(1)}`]
                                .filter(Boolean)
                                .join(' · '),
                            path: h.file_path || h.path,
                            line: start,
                        };
                    }),
                    { title: `NeuroMesh symbols for “${query}”` }
                );
                if (picked && picked.path) {
                    await openWorkspaceFile(picked.path, picked.line);
                }
            } catch (err) {
                if (!state.online) {
                    await showOffline(api);
                } else {
                    vscode.window.showErrorMessage(`NeuroMesh search: ${err.message}`);
                }
            }
        }),
        vscode.commands.registerCommand('neuromesh.recordFeedback', async () => {
            const editor = vscode.window.activeTextEditor;
            const touched = editor ? [editor.document.uri.fsPath] : [];
            const fromPacket = state.files().map((f) => f.path).filter(Boolean);
            const nodes = [...new Set(touched.concat(fromPacket))];
            const pick = await vscode.window.showQuickPick(
                [
                    { label: 'Task succeeded', value: true, description: 'Strengthen this path (STDP)' },
                    { label: 'Task failed', value: false, description: 'Do not reinforce' },
                ],
                { title: 'NeuroMesh record_feedback' }
            );
            if (!pick) {
                return;
            }
            try {
                await api.mcpCall('neuromesh_record_feedback', {
                    task_success: pick.value,
                    touched_nodes: nodes,
                });
                vscode.window.showInformationMessage(
                    pick.value
                        ? 'NeuroMesh: synaptic path strengthened for the next packet.'
                        : 'NeuroMesh: feedback recorded (no reinforce).'
                );
            } catch (err) {
                if (!state.online) {
                    await showOffline(api);
                } else {
                    vscode.window.showErrorMessage(`NeuroMesh feedback: ${err.message}`);
                }
            }
        }),
        vscode.commands.registerCommand('neuromesh.setMode', async () => {
            const pick = await vscode.window.showQuickPick(
                [
                    {
                        label: 'max_savings',
                        description: '0 extra fill tokens',
                        detail: 'Tiny, obvious edits. Seeds still always ship.',
                    },
                    {
                        label: 'balanced',
                        description: '8,000 fill tokens',
                        detail: 'Default membrane.',
                    },
                    {
                        label: 'max_quality',
                        description: '16,000 fill tokens',
                        detail: 'Refactors, auth, don’t you dare miss it.',
                    },
                ],
                { title: 'NeuroMesh membrane mode' }
            );
            if (!pick) {
                return;
            }
            try {
                await api.setMode(pick.label);
                await vscode.workspace.getConfiguration('neuromesh').update('defaultMode', pick.label, true);
                await state.refresh();
                vscode.window.showInformationMessage(`NeuroMesh: mode ${pick.label}`);
            } catch (err) {
                if (!state.online) {
                    await showOffline(api);
                } else {
                    vscode.window.showErrorMessage(`NeuroMesh mode: ${err.message}`);
                }
            }
        }),
        vscode.commands.registerCommand('neuromesh.copyMcpConfig', async () => {
            const cursor = {
                mcpServers: {
                    neuromesh: { command: 'neuromesh', args: ['mcp'] },
                },
            };
            const vscodeCopilot = {
                servers: {
                    neuromesh: { command: 'neuromesh', args: ['mcp'] },
                },
            };
            const pick = await vscode.window.showQuickPick(
                [
                    { label: 'Cursor (.cursor/mcp.json)', json: cursor },
                    { label: 'VS Code Copilot (.vscode/mcp.json)', json: vscodeCopilot },
                ],
                { title: 'Copy NeuroMesh MCP config' }
            );
            if (!pick) {
                return;
            }
            await vscode.env.clipboard.writeText(JSON.stringify(pick.json, null, 2));
            vscode.window.showInformationMessage(`Copied MCP JSON for ${pick.label.split(' ')[0]}.`);
        }),
        vscode.commands.registerCommand('neuromesh.openFile', (arg) => {
            const p = arg && (arg.path || arg.fsPath);
            const line = arg && arg.line;
            return openWorkspaceFile(p, line);
        }),
    ];

    dashboard.handlers = {
        getContext: (msg) => runGetContext(msg.prompt, msg.mode),
        reindex: () => vscode.commands.executeCommand('neuromesh.reindexProject'),
        galaxy: () => vscode.commands.executeCommand('neuromesh.openMonitor'),
        openFile: (msg) => openWorkspaceFile(msg.path, msg.line),
        expand: (msg) => vscode.commands.executeCommand('neuromesh.expandFoldId', { fold_id: msg.fold_id }),
    };

    context.subscriptions.push(...cmds);
    return { runGetContext, openWorkspaceFile };
}

module.exports = { registerCommands };
