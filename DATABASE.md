# NeuroMesh V1 — Database & Persistence Specification

## 1. Storage Architecture

NeuroMesh uses an embedded SQLite database (`neuromesh.db`) stored within the project's `.neuromesh/` directory or the global user data directory.

### Performance Pragmas
Upon initialization, connection pools apply the following pragmas:
```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;
PRAGMA cache_size = -64000; -- 64MB memory cache
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 268435456; -- 256MB memory-mapped I/O
```

---

## 2. Relational Schema & Tables

### 2.1 Projects & File Index
```sql
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    framework TEXT,
    primary_language TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS indexed_files (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    blake3_hash TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    token_count INTEGER NOT NULL,
    language TEXT NOT NULL,
    last_modified INTEGER NOT NULL,
    ast_data BLOB,
    UNIQUE(project_id, relative_path)
);
CREATE INDEX IF NOT EXISTS idx_files_project_path ON indexed_files(project_id, relative_path);
```

---

### 2.2 Neural Project Graph (Nodes & Pheromone Edges)
```sql
CREATE TABLE IF NOT EXISTS graph_nodes (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    file_id TEXT REFERENCES indexed_files(id) ON DELETE CASCADE,
    node_type TEXT NOT NULL, -- 'file', 'component', 'function', 'class', 'symbol', 'api', 'style_token', etc.
    name TEXT NOT NULL,
    signature TEXT,
    start_line INTEGER,
    end_line INTEGER,
    token_cost INTEGER NOT NULL DEFAULT 0,
    base_relevance REAL NOT NULL DEFAULT 1.0,
    access_count INTEGER NOT NULL DEFAULT 0,
    last_accessed INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_graph_nodes_project ON graph_nodes(project_id, name);
CREATE INDEX IF NOT EXISTS idx_graph_nodes_file ON graph_nodes(file_id);

CREATE TABLE IF NOT EXISTS graph_edges (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_node_id TEXT NOT NULL REFERENCES graph_nodes(id) ON DELETE CASCADE,
    target_node_id TEXT NOT NULL REFERENCES graph_nodes(id) ON DELETE CASCADE,
    edge_type TEXT NOT NULL, -- 'imports', 'calls', 'references', 'contains', 'depends_on', 'modified_with', 'tested_by', 'related_to'
    pheromone_weight REAL NOT NULL DEFAULT 0.5,
    reinforcement_count INTEGER NOT NULL DEFAULT 0,
    failure_count INTEGER NOT NULL DEFAULT 0,
    last_reinforced INTEGER NOT NULL,
    UNIQUE(source_node_id, target_node_id, edge_type)
);
CREATE INDEX IF NOT EXISTS idx_edges_source ON graph_edges(source_node_id);
CREATE INDEX IF NOT EXISTS idx_edges_target ON graph_edges(target_node_id);
```

---

### 2.3 Multi-Tiered Memory Storage
```sql
-- Project Memory (Persistent architectural facts, conventions)
CREATE TABLE IF NOT EXISTS project_memory (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    category TEXT NOT NULL, -- 'architecture', 'convention', 'design_token', 'decision', 'constraint'
    key TEXT NOT NULL,
    content TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(project_id, category, key)
);

-- Episodic Memory (Experience traces from past tasks)
CREATE TABLE IF NOT EXISTS episodic_memory (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_signature_hash TEXT NOT NULL,
    intent TEXT NOT NULL,
    summary TEXT NOT NULL,
    activated_node_ids TEXT NOT NULL, -- JSON array of node IDs
    successful_path_edges TEXT NOT NULL, -- JSON array of edge IDs
    success INTEGER NOT NULL DEFAULT 1, -- 1 = success, 0 = failure
    tokens_saved INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_episodic_task ON episodic_memory(project_id, task_signature_hash);

-- Full Text Search for Memory & Documentation
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
    memory_id UNINDEXED,
    project_id,
    category,
    title,
    content,
    tokenize = 'porter unicode61'
);
```

---

### 2.4 Semantic & Tool Cache
```sql
CREATE TABLE IF NOT EXISTS tool_cache (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    tool_name TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    output_content TEXT NOT NULL,
    context_hash TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    hit_count INTEGER NOT NULL DEFAULT 0,
    UNIQUE(project_id, tool_name, input_hash, context_hash)
);

CREATE TABLE IF NOT EXISTS semantic_cache (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_signature_hash TEXT NOT NULL,
    context_hash TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_hash TEXT NOT NULL,
    response_content TEXT NOT NULL,
    token_usage_prompt INTEGER NOT NULL,
    token_usage_completion INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    hit_count INTEGER NOT NULL DEFAULT 0
);
```

---

### 2.5 Observability & Audit Logs
```sql
CREATE TABLE IF NOT EXISTS optimization_telemetry (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    task_intent TEXT,
    mode TEXT NOT NULL,
    tokens_before INTEGER NOT NULL,
    tokens_after INTEGER NOT NULL,
    token_reduction_pct REAL NOT NULL,
    nodes_before INTEGER NOT NULL,
    nodes_after INTEGER NOT NULL,
    expansions_count INTEGER NOT NULL DEFAULT 0,
    cache_hit INTEGER NOT NULL DEFAULT 0,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    latency_ms INTEGER NOT NULL,
    timestamp INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_telemetry_project ON optimization_telemetry(project_id, timestamp);
```
