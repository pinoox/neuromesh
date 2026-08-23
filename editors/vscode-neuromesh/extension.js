'use strict';

const vscode = require('vscode');
const { MeshApi } = require('./lib/api');
const { MeshState } = require('./lib/state');
const { createStatusBar } = require('./lib/statusBar');
const { registerTrees } = require('./lib/trees');
const { registerDecorations } = require('./lib/decorations');
const { registerDashboard } = require('./lib/dashboard');
const { registerMonitor } = require('./lib/monitor');
const { registerCommands } = require('./lib/commands');

/**
 * @param {vscode.ExtensionContext} context
 */
function activate(context) {
    const api = new MeshApi(() => vscode.workspace.getConfiguration('neuromesh'));
    const state = new MeshState(api);
    const dashboard = registerDashboard(context, state, {});
    const monitor = registerMonitor(context, api);

    createStatusBar(context, state);
    registerTrees(context, state);
    registerDecorations(context);
    registerCommands(context, { api, state, dashboard, monitor });

    context.subscriptions.push(
        state,
        vscode.workspace.onDidChangeConfiguration((e) => {
            if (e.affectsConfiguration('neuromesh')) {
                state.start();
            }
        })
    );

    state.start();
}

function deactivate() {}

module.exports = { activate, deactivate };
