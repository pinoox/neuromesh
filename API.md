# NeuroMesh V1 — API & Protocol Specification

## 1. OpenAI-Compatible Gateway Endpoints

NeuroMesh runs a local HTTP server (default: `http://127.0.0.1:8765`). Any AI agent configured with `OPENAI_BASE_URL=http://127.0.0.1:8765/v1` can connect transparently.

### 1.1 `POST /v1/chat/completions`
Standard OpenAI Chat Completions endpoint with transparent neural context optimization.

#### Request Headers
- `Content-Type: application/json`
- `Authorization: Bearer <API_KEY>` (Passed through to the configured upstream provider)
- `X-NeuroMesh-Project: <project_id>` (Optional: explicitly target a registered project)
- `X-NeuroMesh-Mode: max_quality | balanced | max_savings` (Optional: override global optimization mode)

#### Request Body
```json
{
  "model": "gpt-4o",
  "messages": [
    {"role": "system", "content": "You are a senior frontend developer."},
    {"role": "user", "content": "Make the shopping cart responsive."}
  ],
  "temperature": 0.2,
  "stream": true
}
```

#### Response (Streaming Server-Sent Events)
```text
data: {"id":"chatcmpl-neuromesh-102","object":"chat.completion.chunk","created":1740000000,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":"To"},"finish_reason":null}]}

data: {"id":"chatcmpl-neuromesh-102","object":"chat.completion.chunk","created":1740000000,"model":"gpt-4o","choices":[{"index":0,"delta":{"content":" make the Cart responsive..."},"finish_reason":null}]}

data: [DONE]
```

---

### 1.2 `GET /v1/models`
Returns list of available models from the active provider and local GGUF models.

#### Response
```json
{
  "object": "list",
  "data": [
    {"id": "gpt-4o", "object": "model", "owned_by": "openai"},
    {"id": "claude-3-7-sonnet-20250219", "object": "model", "owned_by": "anthropic"},
    {"id": "gemini-2.5-pro", "object": "model", "owned_by": "google"},
    {"id": "local-qwen-0.6b", "object": "model", "owned_by": "neuromesh-local"}
  ]
}
```

---

### 1.3 `POST /v1/responses`
Standard endpoint for newer client tool calling or structured responses.

---

## 2. NeuroMesh Internal Management REST Endpoints

### 2.1 `GET /api/v1/status`
Returns real-time runtime health, active project, memory state, cache hit rate, and token savings.

### 2.2 `POST /api/v1/projects/index`
Triggers an incremental or full re-indexing of a project workspace.

### 2.3 `POST /api/v1/context/activate`
Directly retrieves the activated Context View for a given Task Signature without invoking an external LLM.

### 2.4 `POST /api/v1/context/expand`
Reactivates inactive context nodes for a given session by node ID or file path.

---

## 3. Model Context Protocol (MCP) Server

NeuroMesh implements the Model Context Protocol (MCP) specification via stdio or HTTP/SSE.

### Registered MCP Tools
1. `search_context(query: string, limit?: number)`: Semantic and symbol search over the Project Graph.
2. `activate_context(task_description: string)`: Generates task signature and returns activated minimal context.
3. `expand_context(node_id: string, reason: string)`: Reversible expansion of a specific node or dependency subgraph.
4. `get_project_memory(category?: string)`: Retrieves persistent architectural decisions and coding conventions.
5. `get_task_state(task_id: string)`: Retrieves current working memory and active subtasks.
6. `search_memory(query: string, scope?: string)`: FTS5 search across episodic and project memories.
7. `get_symbol(name: string, file?: string)`: AST symbol definition, references, and related interfaces.
8. `get_dependency_graph(file_path: string, depth?: number)`: Subgraph of inward/outward dependencies with pheromone weights.
9. `get_previous_solution(task_similarity_query: string)`: Episodic memory lookup of previously successful paths.
