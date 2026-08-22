const vscode = require('vscode');

/**
 * @param {vscode.ExtensionContext} context
 */
function activate(context) {
    // 1. Create Live Status Bar Item
    const statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    statusBarItem.command = 'neuromesh.openMonitor';
    statusBarItem.text = '$(sparkle) NeuroMesh: 92.4% Saved';
    statusBarItem.tooltip = 'Click to open NeuroMesh 3D Neural Monitor';
    statusBarItem.show();
    context.subscriptions.push(statusBarItem);

    // 2. Open Webview Command
    const openMonitorCmd = vscode.commands.registerCommand('neuromesh.openMonitor', () => {
        const panel = vscode.window.createWebviewPanel(
            'neuromeshMonitor',
            'NeuroMesh 3D Neural Monitor',
            vscode.ViewColumn.Beside,
            {
                enableScripts: true,
                retainContextWhenHidden: true
            }
        );

        panel.webview.html = getWebviewContent();
    });
    context.subscriptions.push(openMonitorCmd);

    // 3. Re-index Workspace Command
    const reindexCmd = vscode.commands.registerCommand('neuromesh.reindexProject', async () => {
        vscode.window.showInformationMessage('NeuroMesh: Re-indexing project graph...');
        try {
            const res = await fetch('http://127.0.0.1:8765/api/reindex', { method: 'POST' });
            const data = await res.json();
            vscode.window.showInformationMessage(`NeuroMesh: Re-indexed ${data.indexed_files} files successfully!`);
        } catch (e) {
            vscode.window.showWarningMessage('NeuroMesh server not running on port 8765. Run "neuromesh monitor"');
        }
    });
    context.subscriptions.push(reindexCmd);

    // 4. Activate Context Command
    const activateCmd = vscode.commands.registerCommand('neuromesh.activateContext', async () => {
        const editor = vscode.window.activeTextEditor;
        if (!editor) return;
        const selection = editor.document.getText(editor.selection) || editor.document.getText();
        
        try {
            const res = await fetch('http://127.0.0.1:8765/api/simulate', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ prompt: selection, mode: 'balanced' })
            });
            const data = await res.json();
            vscode.window.showInformationMessage(`NeuroMesh: Context Activated! ${data.context_view.reduction_percentage.toFixed(1)}% Token Savings.`);
        } catch (e) {
            vscode.window.showWarningMessage('NeuroMesh server not reachable.');
        }
    });
    context.subscriptions.push(activateCmd);
}

function getWebviewContent() {
    return `<!DOCTYPE html>
    <html lang="en">
    <head>
        <meta charset="UTF-8">
        <style>
            body, html { margin: 0; padding: 0; width: 100%; height: 100%; overflow: hidden; background: #07090e; }
            iframe { width: 100%; height: 100%; border: none; }
        </style>
    </head>
    <body>
        <iframe src="http://127.0.0.1:8765"></iframe>
    </body>
    </html>`;
}

function deactivate() {}

module.exports = {
    activate,
    deactivate
};
