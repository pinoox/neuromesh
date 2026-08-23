'use strict';

const vscode = require('vscode');

function parsePct(value) {
    if (typeof value === 'number' && Number.isFinite(value)) {
        return value;
    }
    if (typeof value === 'string') {
        const n = parseFloat(value.replace('%', '').trim());
        return Number.isFinite(n) ? n : null;
    }
    return null;
}

function packetFiles(packet) {
    if (!packet) {
        return [];
    }
    const files = packet.evidence_packet && packet.evidence_packet.files;
    if (Array.isArray(files) && files.length) {
        return files;
    }
    const snap = packet.last_packet || packet;
    if (Array.isArray(snap.file_paths)) {
        return snap.file_paths.map((path) => ({ path, why: '', tokens: 0, folded_symbols: [] }));
    }
    return [];
}

function packetFolds(packet) {
    const folds = [];
    for (const file of packetFiles(packet)) {
        const ids = Array.isArray(file.folds) ? file.folds : [];
        const symbols = Array.isArray(file.folded_symbols) ? file.folded_symbols : [];
        if (ids.length) {
            for (const id of ids) {
                folds.push({
                    fold_id: id,
                    file_path: file.path,
                    symbol_name: symbols.find((s) => id.includes(s)) || id,
                });
            }
        } else {
            for (const sym of symbols) {
                folds.push({
                    fold_id: `fold_${sym}`,
                    file_path: file.path,
                    symbol_name: sym,
                });
            }
        }
    }
    const seen = new Set();
    return folds.filter((f) => {
        const key = f.fold_id;
        if (seen.has(key)) {
            return false;
        }
        seen.add(key);
        return true;
    });
}

function reductionVsWorkspace(packet) {
    if (!packet) {
        return null;
    }
    const ep = packet.evidence_packet;
    if (ep) {
        return parsePct(ep.reduction_vs_workspace_pct);
    }
    const snap = packet.last_packet;
    if (snap && snap.workspace_tokens && snap.packet_tokens != null) {
        const ws = snap.workspace_tokens;
        if (ws > 0) {
            return ((ws - snap.packet_tokens) / ws) * 100;
        }
    }
    return null;
}

class MeshState {
    /**
     * @param {import('./api').MeshApi} api
     */
    constructor(api) {
        this.api = api;
        this._onDidChange = new vscode.EventEmitter();
        this.onDidChange = this._onDidChange.event;
        this.online = false;
        this.status = null;
        this.packet = null;
        this.lastError = null;
        this._timer = null;
        this._busy = false;
    }

    snapshot() {
        if (this.status && this.status.last_packet) {
            return this.status.last_packet;
        }
        if (this.packet && this.packet.evidence_packet) {
            const ep = this.packet.evidence_packet;
            return {
                coverage_claim: ep.coverage && ep.coverage.claim,
                file_count: (ep.files || []).length,
                fold_count: packetFolds(this.packet).length,
                physarum_used: ep.physarum_used,
                physarum_ms: ep.physarum_ms,
                workspace_tokens: ep.workspace_tokens,
                packet_tokens: ep.active_tokens,
                fill_used: ep.budget && ep.budget.fill_used,
                fill_cap: ep.budget && ep.budget.fill_cap,
                budget_mode: ep.budget && ep.budget.mode,
                grep_needed: ep.coverage && ep.coverage.claim === 'partial',
                file_paths: (ep.files || []).map((f) => f.path),
            };
        }
        return null;
    }

    files() {
        if (this.packet) {
            return packetFiles(this.packet);
        }
        const snap = this.snapshot();
        if (snap && Array.isArray(snap.file_paths)) {
            return snap.file_paths.map((path) => ({
                path,
                why: '',
                tokens: 0,
                folded_symbols: [],
            }));
        }
        return [];
    }

    folds() {
        return packetFolds(this.packet);
    }

    vsWorkspacePct() {
        const fromPacket = reductionVsWorkspace(this.packet);
        if (fromPacket != null) {
            return fromPacket;
        }
        const metrics = this.status && this.status.metrics;
        if (metrics && typeof metrics.overall_reduction_pct === 'number') {
            return metrics.overall_reduction_pct;
        }
        return null;
    }

    async refresh() {
        try {
            this.status = await this.api.getStatus();
            this.online = true;
            this.lastError = null;
        } catch (err) {
            this.online = false;
            this.status = null;
            this.lastError = err;
        }
        await this._syncContextKeys();
        this._onDidChange.fire(this);
        return this;
    }

    setPacket(packet) {
        this.packet = packet;
        this._syncContextKeys();
        this._onDidChange.fire(this);
    }

    start() {
        this.stop();
        const tick = () => {
            if (this._busy) {
                return;
            }
            this._busy = true;
            this.refresh().finally(() => {
                this._busy = false;
            });
        };
        tick();
        const cfg = vscode.workspace.getConfiguration('neuromesh');
        const ms = Math.max(1000, Number(cfg.get('pollIntervalMs')) || 4000);
        this._timer = setInterval(tick, ms);
    }

    stop() {
        if (this._timer) {
            clearInterval(this._timer);
            this._timer = null;
        }
    }

    dispose() {
        this.stop();
        this._onDidChange.dispose();
    }

    async _syncContextKeys() {
        const hasPacket = this.files().length > 0 || !!(this.snapshot() && this.snapshot().file_count);
        const hasFolds = this.folds().length > 0;
        await vscode.commands.executeCommand('setContext', 'neuromesh.online', this.online);
        await vscode.commands.executeCommand('setContext', 'neuromesh.hasPacket', hasPacket);
        await vscode.commands.executeCommand('setContext', 'neuromesh.hasFolds', hasFolds);
    }
}

module.exports = {
    MeshState,
    packetFiles,
    packetFolds,
    parsePct,
    reductionVsWorkspace,
};
