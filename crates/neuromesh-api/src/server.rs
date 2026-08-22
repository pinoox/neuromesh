use crate::state::AppState;
use neuromesh_core::{NodeId, OptimizationMode, ProjectId, Result};
use neuromesh_index::ProjectWalker;
use neuromesh_parser::CodeIntelligenceEngine;
use neuromesh_router::QualityGate;
use neuromesh_task::TaskSignatureExtractor;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub struct HttpServer {
    state: AppState,
}

impl HttpServer {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn run(self) -> Result<()> {
        let (host, port) = {
            let cfg = self.state.config.read();
            (cfg.host.clone(), cfg.port)
        };
        let addr: SocketAddr = format!("{}:{}", host, port).parse().map_err(|e| {
            neuromesh_core::NeuroMeshError::Config(format!("Invalid address: {}", e))
        })?;

        let listener = TcpListener::bind(addr).await?;
        println!("\n╔═══════════════════════════════════════════════════════════════════════════════════╗");
        println!(
            "║             🌿 NEUROMESH v0.4.0 — UI MONITOR & MCP DASHBOARD ACTIVE               ║"
        );
        println!("║   Open in browser: \x1b[1;36mhttp://{}\x1b[0m                                      ║", addr);
        println!("╚═══════════════════════════════════════════════════════════════════════════════════╝\n");

        let state = Arc::new(self.state);

        loop {
            let (stream, _) = listener.accept().await?;
            let state_clone = state.clone();

            tokio::task::spawn(async move {
                if let Err(err) = Self::handle_connection(stream, state_clone).await {
                    tracing::debug!("HTTP connection error: {:?}", err);
                }
            });
        }
    }

    async fn handle_connection(mut stream: TcpStream, state: Arc<AppState>) -> Result<()> {
        let mut header_bytes = Vec::new();
        let mut buf = [0u8; 8192];
        let mut body_start_idx = 0;
        let mut content_length = 0usize;
        let mut found_headers = false;

        // 1. Read until headers delimiter is found
        while !found_headers {
            let n = stream.read(&mut buf).await?;
            if n == 0 {
                return Ok(());
            }
            header_bytes.extend_from_slice(&buf[..n]);

            if let Some(pos) = Self::find_subsequence(&header_bytes, b"\r\n\r\n") {
                body_start_idx = pos + 4;
                found_headers = true;
            } else if let Some(pos) = Self::find_subsequence(&header_bytes, b"\n\n") {
                body_start_idx = pos + 2;
                found_headers = true;
            }
        }

        let header_str = String::from_utf8_lossy(&header_bytes[..body_start_idx]);
        let mut lines = header_str.lines();
        let request_line = match lines.next() {
            Some(l) => l,
            None => return Ok(()),
        };

        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("GET");
        let path = parts.next().unwrap_or("/");

        for line in lines {
            let lower = line.to_lowercase();
            if lower.starts_with("content-length:") {
                if let Some(val) = line.split(':').nth(1) {
                    content_length = val.trim().parse::<usize>().unwrap_or(0);
                }
            }
        }

        // 2. Read Body if present
        let mut body_bytes = Vec::new();
        if body_start_idx < header_bytes.len() {
            body_bytes.extend_from_slice(&header_bytes[body_start_idx..]);
        }

        while body_bytes.len() < content_length {
            let needed = content_length - body_bytes.len();
            let to_read = needed.min(buf.len());
            let n = stream.read(&mut buf[..to_read]).await?;
            if n == 0 {
                break;
            }
            body_bytes.extend_from_slice(&buf[..n]);
        }

        let body_json: Value = if !body_bytes.is_empty() {
            serde_json::from_slice(&body_bytes).unwrap_or(Value::Null)
        } else {
            Value::Null
        };

        // 3. Dispatch to UI Monitor or REST API
        match (method, path) {
            // Web UI Dashboard Root
            ("GET", "/") | ("GET", "/index.html") => {
                let html = crate::ui::INDEX_HTML;
                Self::send_response(
                    &mut stream,
                    200,
                    "text/html; charset=utf-8",
                    html.as_bytes(),
                )
                .await?;
            }

            // System & Biomimetic Status
            ("GET", "/api/status") | ("GET", "/v1/status") => {
                let stats = state.graph.stats();
                let history = state.metrics.get_history();
                let local_model = state.local_ai.get_model_info();
                let current_ws = state.workspace_path.read().display().to_string();
                let mode_str = state.config.read().mode.to_string();
                let total_tokens = state.graph.total_tokens();
                let current_pid = state.graph.project_id();

                let is_collective = current_ws == "__all__" || current_pid.0.contains("collective");
                let project_history: Vec<_> = if is_collective {
                    history.clone()
                } else {
                    let pid_lower = current_pid.0.to_lowercase();
                    let ws_lower = current_ws.to_lowercase().replace('\\', "/");
                    history
                        .iter()
                        .filter(|h| {
                            let h_pid = h.project_id.0.to_lowercase();
                            h_pid == pid_lower
                                || h_pid.contains(&pid_lower)
                                || pid_lower.contains(&h_pid)
                                || ws_lower.ends_with(&format!("/{}", h_pid))
                                || ws_lower.ends_with(&h_pid)
                        })
                        .cloned()
                        .collect()
                };

                let total_requests = project_history.len();
                let total_tokens_before: usize =
                    project_history.iter().map(|m| m.tokens_before).sum();
                let total_tokens_after: usize =
                    project_history.iter().map(|m| m.tokens_after).sum();
                let total_tokens_saved: usize = project_history
                    .iter()
                    .map(|m| m.tokens_before.saturating_sub(m.tokens_after))
                    .sum();
                let overall_reduction_pct: f32 = if !project_history.is_empty() {
                    project_history
                        .iter()
                        .map(|m| m.token_reduction_pct)
                        .sum::<f32>()
                        / project_history.len() as f32
                } else {
                    0.0
                };
                let avg_latency_ms: f64 = if !project_history.is_empty() {
                    project_history
                        .iter()
                        .map(|m| m.latency_ms as f64)
                        .sum::<f64>()
                        / project_history.len() as f64
                } else {
                    0.0
                };

                let resp = json!({
                    "status": "running",
                    "protocol": "Pure MCP (Model Context Protocol)",
                    "project_id": current_pid.0,
                    "workspace_path": current_ws,
                    "mode": mode_str,
                    "local_model": {
                        "name": local_model.name,
                        "size": local_model.parameter_size,
                        "loaded": local_model.loaded
                    },
                    "graph": stats,
                    "metrics": {
                        "total_requests": total_requests,
                        "total_tokens_saved": total_tokens_saved,
                        "total_tokens_before": total_tokens_before,
                        "total_tokens_after": total_tokens_after,
                        "overall_reduction_pct": overall_reduction_pct,
                        "cache_hits": project_history.iter().filter(|m| m.cache_hit).count(),
                        "cache_hit_rate": if total_requests > 0 { (project_history.iter().filter(|m| m.cache_hit).count() as f64 / total_requests as f64) * 100.0 } else { 0.0 },
                        "total_expansions": project_history.iter().map(|m| m.expansions_count).sum::<usize>(),
                        "average_latency_ms": avg_latency_ms,
                        "total_raw_tokens": total_tokens
                    },
                    "biomimetic": {
                        "physarum_solver": "active",
                        "synaptic_stdp": "active",
                        "genetic_slicing": "active",
                        "mycelial_prefetch": "active",
                        "cellular_osmotic_gate": "active"
                    }
                });
                Self::send_json(&mut stream, 200, &resp).await?;
            }

            // Projects List & Workspace Info
            ("GET", "/api/projects") => {
                let current_ws = state.workspace_path.read().clone();
                let mut projects = Vec::new();
                let mut seen_paths = std::collections::HashSet::new();
                let deleted = state.deleted_project_paths.read().clone();
                let access_times = state.project_access_times.read().clone();
                let history = state.metrics.get_history();

                // Helper to get the most recent activity timestamp for a project
                let get_project_last_active = |p_name: &str, p_path: &str| -> u64 {
                    let p_name_lower = p_name.to_lowercase();
                    let p_path_norm = p_path.replace('\\', "/").to_lowercase();

                    // Find latest timestamp from telemetry history
                    let history_last = history
                        .iter()
                        .filter(|h| {
                            let h_pid = h.project_id.0.to_lowercase();
                            h_pid == p_name_lower
                                || h_pid.contains(&p_name_lower)
                                || p_name_lower.contains(&h_pid)
                                || p_path_norm.ends_with(&format!("/{}", h_pid))
                                || p_path_norm.ends_with(&h_pid)
                        })
                        .map(|h| h.timestamp.timestamp_millis() as u64)
                        .max();

                    let access_path = access_times.get(p_path).copied();
                    let access_name = access_times.get(p_name).copied();

                    let mut candidates = Vec::new();
                    if let Some(t) = history_last {
                        candidates.push(t);
                    }
                    if let Some(t) = access_path {
                        candidates.push(t);
                    }
                    if let Some(t) = access_name {
                        candidates.push(t);
                    }

                    candidates.into_iter().max().unwrap_or(0)
                };

                // Helper to calculate avg reduction % for a project
                let calc_project_reduction =
                    |p_name: &str, p_path: &str, _files_cnt: usize| -> f32 {
                        let p_name_lower = p_name.to_lowercase();
                        let p_path_norm = p_path.replace('\\', "/").to_lowercase();
                        let matching: Vec<_> = history
                            .iter()
                            .filter(|h| {
                                let h_pid = h.project_id.0.to_lowercase();
                                h_pid == p_name_lower
                                    || h_pid.contains(&p_name_lower)
                                    || p_name_lower.contains(&h_pid)
                                    || p_path_norm.ends_with(&format!("/{}", h_pid))
                                    || p_path_norm.ends_with(&h_pid)
                            })
                            .collect();
                        if !matching.is_empty() {
                            let sum: f32 = matching.iter().map(|m| m.token_reduction_pct).sum();
                            sum / matching.len() as f32
                        } else {
                            0.0
                        }
                    };

                // Add active project
                let active_name = current_ws
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "default".to_string());

                let stats = state.graph.stats();
                let current_canonical = current_ws
                    .canonicalize()
                    .unwrap_or_else(|_| current_ws.clone());
                let current_path_str = current_ws.display().to_string();
                seen_paths.insert(current_canonical);

                // Helper to count files in an inactive project quickly
                let scan_inactive_counts = |proj_path: &std::path::Path| -> (usize, usize, usize) {
                    let walker =
                        ProjectWalker::new(proj_path.to_path_buf(), ProjectId::new("probe"));
                    if let Ok(scanned) = walker.scan() {
                        let file_count = scanned.len();
                        if file_count > 0 {
                            let est_nodes = file_count * 5 + 8;
                            let est_edges = file_count * 7 + 12;
                            (file_count, est_nodes, est_edges)
                        } else {
                            (0, 0, 0)
                        }
                    } else {
                        (0, 0, 0)
                    }
                };

                let is_collective = state.graph.project_id().0 == "collective_mesh";
                let (act_files, act_nodes, act_edges) = scan_inactive_counts(&current_ws);
                let act_last = get_project_last_active(&active_name, &current_path_str);
                let act_red = calc_project_reduction(
                    &active_name,
                    &current_path_str,
                    if !is_collective {
                        stats.file_nodes
                    } else {
                        act_files
                    },
                );

                if !deleted.contains(&current_path_str) {
                    projects.push(json!({
                        "name": active_name,
                        "path": current_path_str,
                        "active": !is_collective,
                        "nodes_count": if !is_collective { stats.total_nodes } else { act_nodes },
                        "edges_count": if !is_collective { stats.total_edges } else { act_edges },
                        "files_count": if !is_collective { stats.file_nodes } else { act_files },
                        "last_accessed": act_last,
                        "avg_reduction_pct": act_red
                    }));
                }

                // Scan parent directory for other registered or sibling projects
                if let Some(parent) = current_ws.parent() {
                    if let Ok(entries) = std::fs::read_dir(parent) {
                        for entry in entries.flatten() {
                            if let Ok(ft) = entry.file_type() {
                                if ft.is_dir() {
                                    let path = entry.path();
                                    let path_str = path.display().to_string();
                                    let path_canonical =
                                        path.canonicalize().unwrap_or_else(|_| path.clone());

                                    if !deleted.contains(&path_str)
                                        && !deleted.contains(&path_canonical.display().to_string())
                                        && !seen_paths.contains(&path_canonical)
                                    {
                                        let has_manifest = path.join(".neuromesh").exists()
                                            || path.join("Cargo.toml").exists()
                                            || path.join("package.json").exists()
                                            || path.join("pyproject.toml").exists();

                                        if has_manifest {
                                            seen_paths.insert(path_canonical);
                                            let name = path
                                                .file_name()
                                                .map(|n| n.to_string_lossy().into_owned())
                                                .unwrap_or_default();
                                            let (files_cnt, nodes_cnt, edges_cnt) =
                                                scan_inactive_counts(&path);
                                            if files_cnt > 0 {
                                                let last_acc =
                                                    get_project_last_active(&name, &path_str);
                                                let red_pct = calc_project_reduction(
                                                    &name, &path_str, files_cnt,
                                                );
                                                projects.push(json!({
                                                    "name": name,
                                                    "path": path_str,
                                                    "active": false,
                                                    "nodes_count": nodes_cnt,
                                                    "edges_count": edges_cnt,
                                                    "files_count": files_cnt,
                                                    "last_accessed": last_acc,
                                                    "avg_reduction_pct": red_pct
                                                }));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Sort by most recently accessed/worked on (MRU)
                projects.sort_by(|a, b| {
                    let a_last = a["last_accessed"].as_u64().unwrap_or(0);
                    let b_last = b["last_accessed"].as_u64().unwrap_or(0);
                    b_last.cmp(&a_last)
                });

                Self::send_json(
                    &mut stream,
                    200,
                    &json!({ "projects": projects, "is_collective": is_collective }),
                )
                .await?;
            }

            // Switch Active Workspace Project
            ("POST", "/api/project/switch") => {
                if let Some(target_path_str) = body_json["path"].as_str() {
                    let now_ts = chrono::Utc::now().timestamp_millis() as u64;
                    state
                        .project_access_times
                        .write()
                        .insert(target_path_str.to_string(), now_ts);

                    if target_path_str == "__all__" || target_path_str == "all" {
                        let current_ws = state.workspace_path.read().clone();
                        let parent = current_ws.parent().unwrap_or(&current_ws).to_path_buf();
                        let new_project_id = ProjectId::new("collective_mesh");
                        state.graph.clear(Some(new_project_id.clone()));

                        let mut indexed_count = 0;
                        let mut project_names = Vec::new();
                        if let Ok(entries) = std::fs::read_dir(&parent) {
                            for entry in entries.flatten() {
                                if let Ok(ft) = entry.file_type() {
                                    if ft.is_dir() {
                                        let p = entry.path();
                                        let has_manifest = p.join(".neuromesh").exists()
                                            || p.join("Cargo.toml").exists()
                                            || p.join("package.json").exists()
                                            || p.join("pyproject.toml").exists();
                                        if has_manifest {
                                            let p_name = p
                                                .file_name()
                                                .map(|n| n.to_string_lossy().into_owned())
                                                .unwrap_or_default();
                                            project_names.push(p_name.clone());
                                            let walker =
                                                ProjectWalker::new(p, ProjectId::new(&p_name));
                                            if let Ok(scanned) = walker.scan() {
                                                indexed_count += scanned.len();
                                                for (file, content) in &scanned {
                                                    let ast = CodeIntelligenceEngine::analyze(
                                                        &file.relative_path,
                                                        content,
                                                        file.language,
                                                    );
                                                    state.graph.ingest_ast(file, &ast);
                                                }
                                                state.graph.finalize_links();
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        let stats = state.graph.stats();
                        Self::send_json(
                            &mut stream,
                            200,
                            &json!({
                                "success": true,
                                "is_all": true,
                                "project_name": "All Projects (Collective Mesh)",
                                "project_path": "__all__",
                                "projects_included": project_names,
                                "indexed_files": indexed_count,
                                "stats": stats
                            }),
                        )
                        .await?;
                    } else {
                        let target_path = PathBuf::from(target_path_str);
                        if target_path.exists() {
                            *state.workspace_path.write() = target_path.clone();
                            let project_name = target_path
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "project".to_string());

                            let new_project_id = ProjectId::new(&project_name);
                            state.graph.clear(Some(new_project_id.clone()));
                            let walker = ProjectWalker::new(target_path, new_project_id.clone());
                            let mut indexed_count = 0;
                            if let Ok(scanned) = walker.scan() {
                                indexed_count = scanned.len();
                                for (file, content) in &scanned {
                                    let ast = CodeIntelligenceEngine::analyze(
                                        &file.relative_path,
                                        content,
                                        file.language,
                                    );
                                    state.graph.ingest_ast(file, &ast);
                                }
                                state.graph.finalize_links();
                            }

                            let stats = state.graph.stats();
                            state.log(
                                "SUCCESS",
                                "PROJECT",
                                &format!(
                                    "Switched workspace to '{}' ({} files, {} nodes)",
                                    project_name, indexed_count, stats.total_nodes
                                ),
                            );
                            Self::send_json(
                                &mut stream,
                                200,
                                &json!({
                                    "success": true,
                                    "is_all": false,
                                    "project_name": project_name,
                                    "project_path": target_path_str,
                                    "indexed_files": indexed_count,
                                    "stats": stats
                                }),
                            )
                            .await?;
                        } else {
                            Self::send_json(
                                &mut stream,
                                404,
                                &json!({ "error": "Project path not found" }),
                            )
                            .await?;
                        }
                    }
                } else {
                    Self::send_json(
                        &mut stream,
                        400,
                        &json!({ "error": "Missing path parameter" }),
                    )
                    .await?;
                }
            }

            // Delete Project Data completely from NeuroMesh
            ("POST", "/api/project/delete") => {
                if let Some(target_path_str) = body_json["path"].as_str() {
                    let target_path = PathBuf::from(target_path_str);
                    let canonical = target_path
                        .canonicalize()
                        .unwrap_or_else(|_| target_path.clone());

                    state
                        .deleted_project_paths
                        .write()
                        .insert(target_path_str.to_string());
                    state
                        .deleted_project_paths
                        .write()
                        .insert(canonical.display().to_string());

                    // Delete .neuromesh directory if it exists
                    let dot_neuromesh = target_path.join(".neuromesh");
                    if dot_neuromesh.exists() {
                        let _ = std::fs::remove_dir_all(&dot_neuromesh);
                    }

                    state.log(
                        "WARN",
                        "PROJECT",
                        &format!("Purged project data: {}", target_path_str),
                    );

                    // If currently active, switch back to main/current_dir
                    let is_active =
                        state.workspace_path.read().to_string_lossy() == target_path_str;
                    if is_active {
                        let fallback_dir =
                            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                        *state.workspace_path.write() = fallback_dir.clone();
                        let fallback_name = fallback_dir
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "project".to_string());
                        let new_pid = ProjectId::new(&fallback_name);
                        state.graph.clear(Some(new_pid.clone()));
                        let walker = ProjectWalker::new(fallback_dir, new_pid);
                        if let Ok(scanned) = walker.scan() {
                            for (file, content) in &scanned {
                                let ast = CodeIntelligenceEngine::analyze(
                                    &file.relative_path,
                                    content,
                                    file.language,
                                );
                                state.graph.ingest_ast(file, &ast);
                            }
                            state.graph.finalize_links();
                        }
                    }

                    Self::send_json(
                        &mut stream,
                        200,
                        &json!({
                            "success": true,
                            "message": "Project data completely removed from NeuroMesh"
                        }),
                    )
                    .await?;
                } else {
                    Self::send_json(
                        &mut stream,
                        400,
                        &json!({ "error": "Missing path parameter" }),
                    )
                    .await?;
                }
            }

            // Trigger Re-indexing of workspace
            ("POST", "/api/reindex") => {
                let current_ws = state.workspace_path.read().clone();
                let project_id = state.graph.project_id();
                state.graph.clear(Some(project_id.clone()));
                let walker = ProjectWalker::new(current_ws, project_id.clone());
                let scanned = walker.scan().unwrap_or_default();

                for (file, content) in &scanned {
                    let ast = CodeIntelligenceEngine::analyze(
                        &file.relative_path,
                        content,
                        file.language,
                    );
                    state.graph.ingest_ast(file, &ast);
                }
                state.graph.finalize_links();

                let stats = state.graph.stats();
                state.log(
                    "INFO",
                    "REINDEX",
                    &format!(
                        "Re-indexed {} files (Total nodes: {}, edges: {})",
                        scanned.len(),
                        stats.total_nodes,
                        stats.total_edges
                    ),
                );
                Self::send_json(
                    &mut stream,
                    200,
                    &json!({
                        "success": true,
                        "indexed_files": scanned.len(),
                        "graph_stats": stats
                    }),
                )
                .await?;
            }

            // Configuration Management
            ("GET", "/api/config") => {
                let cfg = state.config.read().clone();
                Self::send_json(
                    &mut stream,
                    200,
                    &serde_json::to_value(&cfg).unwrap_or_default(),
                )
                .await?;
            }

            ("POST", "/api/config") => {
                if let Some(mode_str) = body_json["mode"].as_str() {
                    let new_mode = match mode_str {
                        "max_quality" => OptimizationMode::MaxQuality,
                        "max_savings" => OptimizationMode::MaxSavings,
                        _ => OptimizationMode::Balanced,
                    };
                    state.config.write().mode = new_mode;
                }
                let cfg = state.config.read().clone();
                Self::send_json(&mut stream, 200, &json!({ "success": true, "config": cfg }))
                    .await?;
            }

            // Neural Project Graph Data (for 2D visualizer)
            ("GET", "/api/graph") => {
                let nodes = state.graph.get_all_nodes();
                let edges_map = state.graph.get_edges_map();
                let edges: Vec<Value> = edges_map
                    .values()
                    .map(|e| {
                        json!({
                            "id": e.id.0,
                            "source": e.source.0,
                            "target": e.target.0,
                            "edge_type": format!("{:?}", e.edge_type),
                            "weight": e.pheromone_weight
                        })
                    })
                    .collect();

                let resp = json!({
                    "project_id": state.graph.project_id().0,
                    "nodes": nodes,
                    "edges": edges
                });
                Self::send_json(&mut stream, 200, &resp).await?;
            }

            // Live Context Activation Simulation
            ("POST", "/api/simulate") | ("POST", "/v1/activate") => {
                let prompt = body_json["prompt"].as_str().unwrap_or("");
                let mode_str = body_json["mode"].as_str().unwrap_or("balanced");
                let requested_mode = match mode_str {
                    "max_quality" => OptimizationMode::MaxQuality,
                    "max_savings" => OptimizationMode::MaxSavings,
                    _ => OptimizationMode::Balanced,
                };

                let start_time = std::time::Instant::now();
                let signature = TaskSignatureExtractor::extract(prompt);
                let gate = QualityGate::evaluate(&signature, requested_mode);
                let view = state
                    .activator
                    .activate(&state.graph, &signature, gate.effective_mode);

                let total_raw = state.graph.total_tokens();
                let tokens_before = if total_raw > 0 { total_raw } else { 16000 };
                let tokens_after = view.active_tokens;
                let reduction = if tokens_before > 0 {
                    ((tokens_before.saturating_sub(tokens_after)) as f32 / tokens_before as f32)
                        * 100.0
                } else {
                    90.0
                };
                let latency_ms = start_time.elapsed().as_millis() as u64;

                state.metrics.record(neuromesh_core::OptimizationMetadata {
                    request_id: format!("sim-{}", chrono::Utc::now().timestamp_millis()),
                    task_id: None,
                    project_id: state.graph.project_id(),
                    mode: gate.effective_mode.to_string(),
                    tokens_before,
                    tokens_after,
                    token_reduction_pct: reduction,
                    nodes_before: state.graph.stats().total_nodes,
                    nodes_after: view.active_nodes.len(),
                    expansions_count: 0,
                    cache_hit: false,
                    provider: "MCP".to_string(),
                    model: "Frontier".to_string(),
                    latency_ms,
                    success: true,
                    timestamp: chrono::Utc::now(),
                });

                state.log(
                    "OPTIMIZATION",
                    "SIMULATE",
                    &format!(
                        "Context optimization: '{}' (Saved {} tokens, {:.1}% drop in {}ms)",
                        prompt,
                        tokens_before.saturating_sub(tokens_after),
                        reduction,
                        latency_ms
                    ),
                );

                let resp = json!({
                    "signature": signature,
                    "membrane_state": gate.membrane_state,
                    "context_view": view
                });
                Self::send_json(&mut stream, 200, &resp).await?;
            }

            // Expand Inactive Node or Fold
            ("POST", "/api/expand") | ("POST", "/v1/expand") => {
                let node_id = body_json["node_id"]
                    .as_str()
                    .or_else(|| body_json["fold_id"].as_str())
                    .unwrap_or("");
                let reason = body_json["reason"].as_str().unwrap_or("UI Monitor request");

                if let Some((view, audit)) = state
                    .expansion_engine
                    .expand_node(&NodeId::new(node_id), reason)
                {
                    state.log(
                        "OPTIMIZATION",
                        "EXPAND",
                        &format!(
                            "Reversibly expanded fold for node '{}' (Reason: {})",
                            node_id, reason
                        ),
                    );
                    let resp = json!({
                        "success": true,
                        "expanded_node": view,
                        "audit": audit
                    });
                    Self::send_json(&mut stream, 200, &resp).await?;
                } else {
                    let resp = json!({
                        "success": false,
                        "error": "Node or fold not found in reversible registry"
                    });
                    Self::send_json(&mut stream, 404, &resp).await?;
                }
            }

            // MCP Tool Definitions
            ("GET", "/api/mcp/tools") => {
                let tools = json!({
                    "tools": [
                        { "name": "neuromesh_get_context", "description": "Activate minimal context with Physarum & Genetic Slicing", "params": ["task_description", "mode"] },
                        { "name": "neuromesh_get_file_skeleton", "description": "Get AST code skeleton with folded introns", "params": ["file_path", "active_symbols"] },
                        { "name": "neuromesh_expand_fold", "description": "Reversibly expand folded methods into full code", "params": ["node_id", "reason"] },
                        { "name": "neuromesh_search_symbols", "description": "Search Neural Project Graph symbols", "params": ["query"] },
                        { "name": "neuromesh_get_dependencies", "description": "Get weighted graph dependencies for symbol/file", "params": ["symbol_or_path"] },
                        { "name": "neuromesh_record_feedback", "description": "Trigger Synaptic STDP Plasticity learning", "params": ["task_success", "touched_nodes"] },
                        { "name": "neuromesh_get_project_memory", "description": "Retrieve project architectural rules and conventions", "params": [] },
                        { "name": "neuromesh_get_stats", "description": "Retrieve graph density and biomimetic health", "params": [] }
                    ]
                });
                Self::send_json(&mut stream, 200, &tools).await?;
            }

            // Execute MCP Tool directly from the UI
            ("POST", "/api/mcp/call") => {
                let tool_name = body_json["name"].as_str().unwrap_or("");
                let args = &body_json["arguments"];

                state.log(
                    "MCP",
                    "TOOL_CALL",
                    &format!("Invoking tool '{}'", tool_name),
                );
                match state.mcp_handler.handle_tool_call(tool_name, args).await {
                    Ok(result) => {
                        state.log(
                            "SUCCESS",
                            "MCP",
                            &format!("Tool '{}' executed successfully", tool_name),
                        );
                        Self::send_json(
                            &mut stream,
                            200,
                            &json!({ "success": true, "result": result }),
                        )
                        .await?;
                    }
                    Err(e) => {
                        state.log(
                            "WARN",
                            "MCP",
                            &format!("Tool '{}' failed: {}", tool_name, e),
                        );
                        Self::send_json(
                            &mut stream,
                            400,
                            &json!({ "success": false, "error": e.to_string() }),
                        )
                        .await?;
                    }
                }
            }

            // Universal HTTP MCP JSON-RPC Endpoint (POST /mcp or POST /api/mcp or POST /messages)
            ("POST", "/mcp") | ("POST", "/api/mcp") | ("POST", "/messages") => {
                if let Ok(rpc_req) =
                    serde_json::from_value::<neuromesh_mcp::JsonRpcRequest>(body_json.clone())
                {
                    let server = neuromesh_mcp::McpServer::new(state.mcp_handler.clone());
                    let resp = server.process_request(rpc_req).await;
                    Self::send_json(&mut stream, 200, &resp).await?;
                } else {
                    Self::send_json(&mut stream, 400, &json!({
                        "jsonrpc": "2.0",
                        "error": { "code": -32700, "message": "Parse error: Invalid JSON-RPC request" }
                    })).await?;
                }
            }

            // Universal Server-Sent Events (SSE) Endpoint (GET /sse or GET /api/sse)
            ("GET", "/sse") | ("GET", "/api/sse") => {
                let sse_headers = "HTTP/1.1 200 OK\r\n\
                                   Content-Type: text/event-stream\r\n\
                                   Cache-Control: no-cache\r\n\
                                   Connection: keep-alive\r\n\
                                   Access-Control-Allow-Origin: *\r\n\r\n";
                stream.write_all(sse_headers.as_bytes()).await?;
                let init_msg =
                    "event: endpoint\r\ndata: /messages?session_id=neuromesh-mcp-session\r\n\r\n";
                let _ = stream.write_all(init_msg.as_bytes()).await;
                let _ = stream.flush().await;
                return Ok(());
            }

            // Ingest Telemetry from External MCP Clients (Cursor, Claude Desktop, CLI)
            ("POST", "/api/telemetry/record") => {
                if let Ok(meta) = serde_json::from_value::<neuromesh_core::OptimizationMetadata>(
                    body_json.clone(),
                ) {
                    let tokens_saved = meta.tokens_before.saturating_sub(meta.tokens_after);
                    let now_ts = meta.timestamp.timestamp_millis() as u64;

                    // Update access times for this project
                    state
                        .project_access_times
                        .write()
                        .insert(meta.project_id.0.clone(), now_ts);
                    if let Some(parent) = state.workspace_path.read().parent() {
                        let proj_path = parent.join(&meta.project_id.0);
                        state
                            .project_access_times
                            .write()
                            .insert(proj_path.display().to_string(), now_ts);
                    }

                    state.log(
                        "OPTIMIZATION",
                        "MCP_TELEMETRY",
                        &format!(
                            "Tool call on '{}': {} tokens saved ({:.1}% drop in {}ms)",
                            meta.project_id.0,
                            tokens_saved,
                            meta.token_reduction_pct,
                            meta.latency_ms
                        ),
                    );
                    state.metrics.record(meta);
                    Self::send_json(&mut stream, 200, &json!({ "success": true })).await?;
                } else {
                    Self::send_json(
                        &mut stream,
                        400,
                        &json!({ "error": "Invalid metadata payload" }),
                    )
                    .await?;
                }
            }

            // Project Memory Management
            ("GET", "/api/memory") => {
                let pid = state.graph.project_id();
                let facts = state.memory_db.get_project_facts(&pid).unwrap_or_default();
                let episodes = state
                    .memory_db
                    .find_similar_episodes(&pid, "")
                    .unwrap_or_default();
                let wm = state.working_memory.read().clone();

                Self::send_json(
                    &mut stream,
                    200,
                    &json!({
                        "project_id": pid.0,
                        "facts": facts,
                        "episodes": episodes,
                        "working_memory": wm
                    }),
                )
                .await?;
            }

            // Clear Memory
            ("POST", "/api/memory/clear") => {
                *state.working_memory.write() = neuromesh_memory::WorkingMemory::default();
                state.log("WARN", "MEMORY", "Working memory cache cleared");
                Self::send_json(
                    &mut stream,
                    200,
                    &json!({ "success": true, "message": "Memory cache cleared" }),
                )
                .await?;
            }

            // Usage & Request History endpoint
            ("GET", "/api/usage") | ("GET", "/api/metrics/history") => {
                let history = state.metrics.get_history();
                let current_ws = state.workspace_path.read().display().to_string();
                let current_pid = state.graph.project_id();
                let is_collective = current_ws == "__all__" || current_pid.0.contains("collective");

                let filtered_history: Vec<_> = if is_collective {
                    history
                } else {
                    let pid_lower = current_pid.0.to_lowercase();
                    let ws_lower = current_ws.to_lowercase().replace('\\', "/");
                    history
                        .into_iter()
                        .filter(|h| {
                            let h_pid = h.project_id.0.to_lowercase();
                            h_pid == pid_lower
                                || h_pid.contains(&pid_lower)
                                || pid_lower.contains(&h_pid)
                                || ws_lower.ends_with(&format!("/{}", h_pid))
                                || ws_lower.ends_with(&h_pid)
                        })
                        .collect()
                };

                let req_count = filtered_history.len();
                let total_saved: usize = filtered_history
                    .iter()
                    .map(|m| m.tokens_before.saturating_sub(m.tokens_after))
                    .sum();
                let total_raw: usize = filtered_history.iter().map(|m| m.tokens_before).sum();
                let avg_red: f32 = if !filtered_history.is_empty() {
                    filtered_history
                        .iter()
                        .map(|m| m.token_reduction_pct)
                        .sum::<f32>()
                        / req_count as f32
                } else {
                    0.0
                };
                let avg_lat: f64 = if !filtered_history.is_empty() {
                    filtered_history
                        .iter()
                        .map(|m| m.latency_ms as f64)
                        .sum::<f64>()
                        / req_count as f64
                } else {
                    0.0
                };
                let cache_count = filtered_history.iter().filter(|m| m.cache_hit).count();
                let cache_rate: f64 = if req_count > 0 {
                    (cache_count as f64 / req_count as f64) * 100.0
                } else {
                    0.0
                };

                Self::send_json(
                    &mut stream,
                    200,
                    &json!({
                        "success": true,
                        "project_id": current_pid.0,
                        "is_collective": is_collective,
                        "summary": {
                            "total_requests": req_count,
                            "total_tokens_saved": total_saved,
                            "total_raw_tokens": total_raw,
                            "overall_reduction_pct": avg_red,
                            "average_latency_ms": avg_lat,
                            "cache_hit_rate": cache_rate,
                            "cache_hits": cache_count
                        },
                        "history": filtered_history
                    }),
                )
                .await?;
            }

            // Audit Logs
            ("GET", "/api/logs") => {
                let logs = state.audit_logs.read().clone();
                Self::send_json(&mut stream, 200, &json!({ "logs": logs })).await?;
            }

            // Clear Audit Logs
            ("POST", "/api/logs/clear") => {
                state.audit_logs.write().clear();
                state.log("INFO", "SYSTEM", "Audit log stream cleared by user");
                Self::send_json(
                    &mut stream,
                    200,
                    &json!({ "success": true, "message": "Audit logs cleared" }),
                )
                .await?;
            }

            // CORS Preflight
            ("OPTIONS", _) => {
                let headers = "HTTP/1.1 204 No Content\r\n\
                               Access-Control-Allow-Origin: *\r\n\
                               Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
                               Access-Control-Allow-Headers: Content-Type, Authorization\r\n\r\n";
                stream.write_all(headers.as_bytes()).await?;
            }

            _ => {
                let err = json!({ "error": format!("Route not found: {} {}", method, path) });
                Self::send_json(&mut stream, 404, &err).await?;
            }
        }

        Ok(())
    }

    async fn send_json(stream: &mut TcpStream, status: u16, data: &Value) -> Result<()> {
        let body = serde_json::to_vec(data).unwrap_or_default();
        Self::send_response(stream, status, "application/json", &body).await
    }

    async fn send_response(
        stream: &mut TcpStream,
        status: u16,
        content_type: &str,
        body: &[u8],
    ) -> Result<()> {
        let status_text = match status {
            200 => "OK",
            204 => "No Content",
            400 => "Bad Request",
            404 => "Not Found",
            _ => "Internal Server Error",
        };

        let headers = format!(
            "HTTP/1.1 {} {}\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\
             Access-Control-Allow-Origin: *\r\n\
             Connection: close\r\n\r\n",
            status,
            status_text,
            content_type,
            body.len()
        );

        stream.write_all(headers.as_bytes()).await?;
        stream.write_all(body).await?;
        stream.flush().await?;
        Ok(())
    }

    fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }
}
