'use strict';

/**
 * HTTP client for the local NeuroMesh monitor (`neuromesh monitor`).
 */
class MeshApi {
    /**
     * @param {() => { host: string, port: number }} configFn
     */
    constructor(configFn) {
        this._configFn = configFn;
    }

    origin() {
        const cfg = this._configFn();
        const host = (cfg.host || '127.0.0.1').trim();
        const port = Number(cfg.port) || 8765;
        return `http://${host}:${port}`;
    }

    /**
     * @param {string} method
     * @param {string} path
     * @param {object} [body]
     * @param {number} [timeoutMs]
     */
    async request(method, path, body, timeoutMs = 12000) {
        const url = `${this.origin()}${path}`;
        const ctrl = new AbortController();
        const timer = setTimeout(() => ctrl.abort(), timeoutMs);
        try {
            const res = await fetch(url, {
                method,
                headers: body ? { 'Content-Type': 'application/json' } : undefined,
                body: body ? JSON.stringify(body) : undefined,
                signal: ctrl.signal,
            });
            const text = await res.text();
            let data = null;
            try {
                data = text ? JSON.parse(text) : null;
            } catch {
                data = { raw: text };
            }
            if (!res.ok) {
                const err = new Error(
                    (data && (data.error || data.message)) || `${res.status} ${res.statusText}`
                );
                err.status = res.status;
                err.data = data;
                throw err;
            }
            return data;
        } finally {
            clearTimeout(timer);
        }
    }

    getStatus() {
        return this.request('GET', '/api/status', undefined, 5000);
    }

    reindex() {
        return this.request('POST', '/api/reindex', {}, 120000);
    }

    /**
     * Live `neuromesh_get_context` — same evidence packet the MCP agent receives.
     */
    simulate(prompt, mode) {
        return this.request('POST', '/api/simulate', { prompt, mode: mode || 'balanced' }, 30000);
    }

    expand(id, reason) {
        return this.request(
            'POST',
            '/api/expand',
            { fold_id: id, node_id: id, reason: reason || 'VS Code expand_fold' },
            15000
        );
    }

    setMode(mode) {
        return this.request('POST', '/api/config', { mode });
    }

    mcpCall(name, args) {
        return this.request('POST', '/api/mcp/call', { name, arguments: args || {} }, 30000);
    }

    getGraph() {
        return this.request('GET', '/api/graph', undefined, 8000);
    }
}

module.exports = { MeshApi };
