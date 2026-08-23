'use strict';

const vscode = require('vscode');

const FOLD_RE = /\[neuromesh:fold:([A-Za-z0-9_.-]+)(?:\s*\|\s*(\d+)\s*lines folded(?:\s*\|\s*([^\]]+))?)?\]/g;

function parseFolds(document) {
    const hits = [];
    for (let line = 0; line < document.lineCount; line++) {
        const text = document.lineAt(line).text;
        FOLD_RE.lastIndex = 0;
        let m;
        while ((m = FOLD_RE.exec(text))) {
            hits.push({
                line,
                fold_id: m[1],
                lines: m[2] ? Number(m[2]) : undefined,
                symbol: m[3] ? m[3].trim() : undefined,
                range: new vscode.Range(line, m.index, line, m.index + m[0].length),
            });
        }
    }
    return hits;
}

function foldAtCursor(editor) {
    if (!editor) {
        return null;
    }
    const pos = editor.selection.active;
    const hits = parseFolds(editor.document).filter((h) => h.line === pos.line);
    return hits[0] || null;
}

function registerDecorations(context, onExpand) {
    const type = vscode.window.createTextEditorDecorationType({
        backgroundColor: new vscode.ThemeColor('neuromesh.foldBackground'),
        isWholeLine: true,
        overviewRulerColor: new vscode.ThemeColor('neuromesh.foldRuler'),
        overviewRulerLane: vscode.OverviewRulerLane.Right,
        after: {
            margin: '0 0 0 1.25rem',
            color: new vscode.ThemeColor('neuromesh.foldForeground'),
            fontStyle: 'italic',
        },
    });
    context.subscriptions.push(type);

    const refresh = (editor) => {
        if (!editor) {
            return;
        }
        const enabled = vscode.workspace.getConfiguration('neuromesh').get('showFoldDecorations', true);
        if (!enabled) {
            editor.setDecorations(type, []);
            return;
        }
        const decos = parseFolds(editor.document).map((hit) => {
            const bits = ['intron folded'];
            if (hit.lines) {
                bits.push(`${hit.lines} lines`);
            }
            if (hit.symbol) {
                bits.push(hit.symbol);
            }
            return {
                range: hit.range,
                hoverMessage: new vscode.MarkdownString(
                    [
                        '**NeuroMesh intron** — body sleeps in the session registry.',
                        '',
                        `\`${hit.fold_id}\`` + (hit.lines ? ` · ${hit.lines} lines` : ''),
                        hit.symbol ? hit.symbol : '',
                        '',
                        'Run **NeuroMesh: Expand Fold at Cursor** to restore from RAM (no disk grep).',
                    ]
                        .filter(Boolean)
                        .join('\n')
                ),
                renderOptions: {
                    after: { contentText: `fold · ${bits.slice(1).join(' · ') || hit.fold_id}` },
                },
            };
        });
        editor.setDecorations(type, decos);
    };

    for (const editor of vscode.window.visibleTextEditors) {
        refresh(editor);
    }

    context.subscriptions.push(
        vscode.window.onDidChangeActiveTextEditor(refresh),
        vscode.workspace.onDidChangeTextDocument((e) => {
            for (const editor of vscode.window.visibleTextEditors) {
                if (editor.document === e.document) {
                    refresh(editor);
                }
            }
        }),
        vscode.workspace.onDidChangeConfiguration((e) => {
            if (e.affectsConfiguration('neuromesh.showFoldDecorations')) {
                for (const editor of vscode.window.visibleTextEditors) {
                    refresh(editor);
                }
            }
        })
    );

    const lens = vscode.languages.registerCodeLensProvider(
        [{ scheme: 'file' }, { scheme: 'untitled' }],
        {
        provideCodeLenses(document) {
            const enabled = vscode.workspace.getConfiguration('neuromesh').get('showFoldDecorations', true);
            if (!enabled) {
                return [];
            }
            return parseFolds(document).map((hit) => {
                const title = hit.lines
                    ? `NeuroMesh: expand ${hit.fold_id} (${hit.lines} lines)`
                    : `NeuroMesh: expand ${hit.fold_id}`;
                return new vscode.CodeLens(hit.range, {
                    title,
                    command: 'neuromesh.expandFoldId',
                    arguments: [{ fold_id: hit.fold_id }],
                });
            });
        },
    });
    context.subscriptions.push(lens);

    return { parseFolds, foldAtCursor, refresh, onExpand };
}

module.exports = { registerDecorations, parseFolds, foldAtCursor, FOLD_RE };
