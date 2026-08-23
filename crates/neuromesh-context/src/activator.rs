use crate::registry::ReversibleContextRegistry;
use crate::scoring::{ActivationScorer, ScoringWeights};
use crate::selector::{
    budget_mode_name, fill_budget, is_noise_path, seed_callee_exon_names, select,
};
use crate::skeleton::{CodeSkeletonizer, FunctionSpan};
use neuromesh_core::{
    ActivatedNodeView, ContextStatus, ContextView, CoverageReport, EdgeConfidence, EdgeType,
    NextAction, NodeId, NodeType, OptimizationMode, SeedResolution, TaskSignature,
};
use neuromesh_graph::{path_echoes_symbol, NeuralProjectGraph};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

const MAX_INACTIVE: usize = 12;
const PHYSARUM_SLA_MS: u64 = 20;

struct MaterializedNode {
    node: neuromesh_core::ContextNode,
    score: f32,
    reason: String,
    raw_tokens: usize,
    folds: Vec<String>,
    folded_symbols: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PhysarumTelemetry {
    pub used: bool,
    pub ms: u64,
}

/// Compact last-packet facts for the monitor dashboard (not the full evidence packet).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PacketSnapshot {
    pub coverage_claim: String,
    pub seeds_hit: usize,
    pub seeds_missed: usize,
    pub file_count: usize,
    pub fold_count: usize,
    pub physarum_used: bool,
    pub physarum_ms: u64,
    pub selection_method: String,
    pub workspace_tokens: usize,
    pub packet_tokens: usize,
    pub fill_used: usize,
    pub fill_cap: usize,
    pub budget_mode: String,
    pub seed_call_coverage: f32,
    pub next_action_count: usize,
    pub grep_needed: bool,
    pub file_paths: Vec<String>,
}

impl PacketSnapshot {
    fn from_view(view: &ContextView) -> Self {
        let coverage = view.coverage.as_ref();
        let claim = coverage
            .map(|c| c.claim.clone())
            .unwrap_or_else(|| "unknown".into());
        let files: Vec<String> = view
            .active_nodes
            .iter()
            .filter(|n| n.node.node_type == NodeType::File)
            .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
        Self {
            coverage_claim: claim.clone(),
            seeds_hit: coverage.map(|c| c.seeds_hit.len()).unwrap_or(0),
            seeds_missed: coverage.map(|c| c.seeds_missed.len()).unwrap_or(0),
            file_count: files.len(),
            fold_count: view.fold_ids.len(),
            physarum_used: view.physarum_used,
            physarum_ms: view.physarum_ms,
            selection_method: view.selection_method.clone(),
            workspace_tokens: view.workspace_tokens,
            packet_tokens: view.active_tokens,
            fill_used: view.budget_fill_used,
            fill_cap: view.budget_fill_cap,
            budget_mode: view.budget_mode.clone(),
            seed_call_coverage: view.seed_call_coverage,
            next_action_count: view.next_actions.len(),
            grep_needed: claim == "partial",
            file_paths: files.into_iter().take(12).collect(),
        }
    }
}

pub struct ContextActivator {
    scorer: ActivationScorer,
    registry: Arc<ReversibleContextRegistry>,
    last_physarum: Mutex<PhysarumTelemetry>,
    last_packet: Mutex<Option<PacketSnapshot>>,
}

impl ContextActivator {
    pub fn new(registry: Arc<ReversibleContextRegistry>) -> Self {
        Self {
            scorer: ActivationScorer::new(ScoringWeights::default()),
            registry,
            last_physarum: Mutex::new(PhysarumTelemetry::default()),
            last_packet: Mutex::new(None),
        }
    }

    pub fn registry(&self) -> &Arc<ReversibleContextRegistry> {
        &self.registry
    }

    pub fn last_physarum(&self) -> PhysarumTelemetry {
        *self.last_physarum.lock()
    }

    pub fn last_packet(&self) -> Option<PacketSnapshot> {
        self.last_packet.lock().clone()
    }

    pub fn activate(
        &self,
        graph: &NeuralProjectGraph,
        signature: &TaskSignature,
        mode: OptimizationMode,
    ) -> ContextView {
        self.registry.begin_activate(&graph.project_id());

        let is_critical = signature.requires_conservative_mode();
        let effective_mode = if is_critical {
            OptimizationMode::MaxQuality
        } else {
            mode
        };

        let hops = match effective_mode {
            OptimizationMode::MaxQuality => 3,
            OptimizationMode::Balanced => 2,
            OptimizationMode::MaxSavings => 1,
        };

        let mut seed_resolutions = Vec::new();
        let mut seed_energies: HashMap<NodeId, f32> = HashMap::new();
        let mut seed_reasons: HashMap<NodeId, String> = HashMap::new();

        let mut queries: Vec<(String, f32, &str)> = Vec::new();
        for ident in &signature.identifiers {
            queries.push((ident.clone(), 1.0, "identifier"));
        }
        if !signature.entity.is_empty()
            && signature.entity != "Workspace"
            && !signature
                .identifiers
                .iter()
                .any(|id| id == &signature.entity)
        {
            queries.push((signature.entity.clone(), 1.0, "entity"));
        }
        for hint in &signature.file_hints {
            queries.push((hint.clone(), 0.95, "file"));
        }

        if queries.is_empty() {
            for token in signature.raw_prompt.split_whitespace().take(8) {
                let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                if clean.len() < 4 {
                    continue;
                }
                queries.push((clean.to_string(), 0.55, "token"));
            }
        }

        for (query, energy, reason) in queries {
            if seed_resolutions
                .iter()
                .any(|s: &SeedResolution| s.query == query)
            {
                continue;
            }
            if let Some((ranked_id, confidence)) = graph.resolve_ranked(&query, None, None) {
                let id = prefer_search_seed(graph, &query, ranked_id, confidence);
                let conf = match confidence {
                    neuromesh_core::EdgeConfidence::Proven => 1.0,
                    neuromesh_core::EdgeConfidence::Likely => 0.62,
                    neuromesh_core::EdgeConfidence::Unresolved => 0.0,
                };
                seed_energies
                    .entry(id.clone())
                    .and_modify(|e| *e = (*e).max(energy))
                    .or_insert(energy);
                seed_reasons
                    .entry(id.clone())
                    .or_insert_with(|| format!("{reason}:{query}"));
                seed_resolutions.push(SeedResolution {
                    query,
                    resolved_id: Some(id),
                    confidence: conf,
                });
            } else if let Some(hit) =
                graph
                    .search_symbols(&query, 1)
                    .into_iter()
                    .next()
                    .filter(|hit| {
                        let q = query.to_lowercase();
                        let n = hit.name.to_lowercase();
                        hit.score >= 86.0 && (n == q || n.starts_with(&q) || q.starts_with(&n))
                    })
            {
                seed_energies
                    .entry(hit.id.clone())
                    .and_modify(|e| *e = (*e).max(energy * 0.8))
                    .or_insert(energy * 0.8);
                seed_reasons
                    .entry(hit.id.clone())
                    .or_insert_with(|| format!("{reason}:{query}"));
                seed_resolutions.push(SeedResolution {
                    query,
                    resolved_id: Some(hit.id),
                    confidence: (hit.score / 100.0).clamp(0.2, 0.75),
                });
            } else {
                seed_resolutions.push(SeedResolution {
                    query,
                    resolved_id: None,
                    confidence: 0.0,
                });
            }
        }

        let seed_set: HashSet<NodeId> = seed_energies.keys().cloned().collect();
        let neighborhood = if seed_set.is_empty() {
            HashSet::new()
        } else {
            graph.neighborhood(&seed_set, hops)
        };

        let mut focus_terms: HashSet<String> = HashSet::new();
        for ident in &signature.identifiers {
            focus_terms.insert(ident.to_lowercase());
        }
        for hint in &signature.file_hints {
            if let Some(stem) = std::path::Path::new(hint)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                focus_terms.insert(stem.to_lowercase());
            }
        }
        for token in signature
            .raw_prompt
            .split(|c: char| !c.is_alphanumeric() && c != '_')
        {
            let t = token.to_lowercase();
            if t.len() >= 5
                && !matches!(
                    t.as_str(),
                    "where" | "about" | "does" | "using" | "should" | "would" | "could"
                )
            {
                focus_terms.insert(t);
            }
        }

        let mut selection = select(
            graph,
            &neighborhood,
            &seed_set,
            &seed_energies,
            &focus_terms,
            effective_mode,
        );
        let fill_cap = fill_budget(effective_mode);

        let mut physarum_used = false;
        let mut physarum_ms = 0u64;
        if seed_set.len() >= 2 {
            let started = Instant::now();
            let tube = graph.solve_physarum_tube(&seed_set, hops.min(2));
            physarum_ms = started.elapsed().as_millis() as u64;
            let ran = tube.iterations_converged > 0;
            if ran && physarum_ms <= PHYSARUM_SLA_MS {
                physarum_used = true;
                for id in &tube.active_nodes {
                    let Some(node) = graph.get_node(id) else {
                        continue;
                    };
                    let Some(file_id) = graph.file_id_for_path(&node.file_path) else {
                        continue;
                    };
                    if selection.required.contains(&file_id) {
                        continue;
                    }
                    let entry = selection.scores.entry(file_id.clone()).or_insert(0.0);
                    if *entry < 8.0 {
                        *entry = 8.0;
                    }
                    if !selection.optional.contains(&file_id) {
                        selection.optional.push(file_id);
                    }
                    seed_reasons
                        .entry(id.clone())
                        .or_insert_with(|| "physarum_tube".into());
                }
                selection.method = "physarum_seed_fill";
            }
        }
        let scores = selection.scores.clone();
        selection.optional.sort_by(|a, b| {
            let sa = scores.get(a).copied().unwrap_or(0.0);
            let sb = scores.get(b).copied().unwrap_or(0.0);
            let score = sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal);
            if score != std::cmp::Ordering::Equal {
                return score;
            }
            let pa = graph
                .get_node(a)
                .map(|n| n.file_path.to_string_lossy().to_string())
                .unwrap_or_default();
            let pb = graph
                .get_node(b)
                .map(|n| n.file_path.to_string_lossy().to_string())
                .unwrap_or_default();
            pa.cmp(&pb)
        });
        let extra_cap = selection.optional_cap;
        selection.optional.truncate(extra_cap);
        *self.last_physarum.lock() = PhysarumTelemetry {
            used: physarum_used,
            ms: physarum_ms,
        };

        let mut active_symbol_names: HashSet<String> = HashSet::new();
        active_symbol_names.insert(signature.entity.to_lowercase());
        for ident in &signature.identifiers {
            active_symbol_names.insert(ident.to_lowercase());
        }
        for seed in &seed_resolutions {
            if let Some(id) = &seed.resolved_id {
                if let Some(node) = graph.get_node(id) {
                    active_symbol_names.insert(node.name.to_lowercase());
                }
            }
            active_symbol_names.insert(seed.query.to_lowercase());
        }
        for name in seed_callee_exon_names(graph, &seed_set) {
            active_symbol_names.insert(name);
        }

        let mut active_nodes = Vec::new();
        let mut included: HashSet<NodeId> = HashSet::new();
        let mut active_tokens = 0;
        let mut seed_tokens = 0;
        let mut fill_used: usize = 0;
        let mut total_raw_tokens = 0;
        let mut fold_ids = Vec::new();
        let registry = self.registry.clone();

        let materialize = |id: &NodeId,
                           scores: &HashMap<NodeId, f32>,
                           seed_energies: &HashMap<NodeId, f32>,
                           seed_reasons: &HashMap<NodeId, String>,
                           scorer: &crate::scoring::ActivationScorer|
         -> Option<MaterializedNode> {
            let mut node = graph.get_node(id)?;
            if is_noise_path(&node.file_path) && !seed_set.contains(id) {
                let seed_file = seed_set.iter().any(|s| {
                    graph
                        .get_node(s)
                        .is_some_and(|n| n.file_path == node.file_path)
                });
                if !seed_file {
                    return None;
                }
            }
            let rel_strength = *seed_energies.get(id).unwrap_or(&0.35);
            let score = scores
                .get(id)
                .copied()
                .unwrap_or_else(|| scorer.score_node(&node, signature, rel_strength, 1.0));
            let reason = seed_reasons.get(id).cloned().unwrap_or_else(|| {
                scores
                    .get(id)
                    .map(|s| format!("utility:{s:.2}"))
                    .unwrap_or_else(|| "connector".into())
            });
            let mut folds = Vec::new();
            let mut folded_symbols = Vec::new();
            let raw = if let Some(content) = node.content.clone() {
                let raw = neuromesh_core::TokenCounter::count_tokens(&content);
                let spans = function_spans_for_file(graph, &node.file_path);
                let skeleton_res = CodeSkeletonizer::skeletonize_with_spans(
                    &node.file_path.to_string_lossy(),
                    &content,
                    &active_symbol_names,
                    &spans,
                );
                for fold in &skeleton_res.folds {
                    registry.register_fold(node.file_path.clone(), fold.clone());
                    folded_symbols.push(fold.symbol_name.clone());
                    folds.push(fold.fold_id.clone());
                }
                node.content = Some(skeleton_res.skeleton_code);
                node.token_cost = skeleton_res.skeleton_tokens;
                raw
            } else {
                node.token_cost
            };
            Some(MaterializedNode {
                node,
                score,
                reason,
                raw_tokens: raw,
                folds,
                folded_symbols,
            })
        };

        for id in &selection.required {
            if included.contains(id) {
                continue;
            }
            let Some(item) = materialize(
                id,
                &selection.scores,
                &seed_energies,
                &seed_reasons,
                &self.scorer,
            ) else {
                continue;
            };
            included.insert(id.clone());
            total_raw_tokens += item.raw_tokens;
            seed_tokens += item.node.token_cost;
            active_tokens += item.node.token_cost;
            fold_ids.extend(item.folds);
            active_nodes.push(ActivatedNodeView {
                node: item.node,
                activation_score: item.score,
                status: ContextStatus::Active,
                expansion_reason: Some(item.reason),
                folded_symbols: item.folded_symbols,
            });
        }

        for id in &selection.optional {
            if included.contains(id) {
                continue;
            }
            let Some(item) = materialize(
                id,
                &selection.scores,
                &seed_energies,
                &seed_reasons,
                &self.scorer,
            ) else {
                continue;
            };
            let cost = item.node.token_cost.max(1);
            if fill_cap == 0 || fill_used.saturating_add(cost) > fill_cap {
                self.registry.register_inactive(
                    &item.node,
                    0.2,
                    signature.confidence,
                    item.score,
                    None,
                );
                continue;
            }
            included.insert(id.clone());
            total_raw_tokens += item.raw_tokens;
            fill_used += cost;
            active_tokens += cost;
            fold_ids.extend(item.folds);
            active_nodes.push(ActivatedNodeView {
                node: item.node,
                activation_score: item.score,
                status: ContextStatus::Active,
                expansion_reason: Some(item.reason),
                folded_symbols: item.folded_symbols,
            });
        }

        let mut inactive_count = 0usize;
        for id in &neighborhood {
            if included.contains(id) || inactive_count >= MAX_INACTIVE {
                continue;
            }
            if let Some(node) = graph.get_node(id) {
                let score = self.scorer.score_node(&node, signature, 0.2, 1.0);
                self.registry
                    .register_inactive(&node, 0.2, signature.confidence, score, None);
                inactive_count += 1;
            }
        }

        let mut inactive_descriptors = self.registry.get_inactive_descriptors();
        inactive_descriptors.sort_by(|a, b| {
            b.activation_score
                .partial_cmp(&a.activation_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        inactive_descriptors.truncate(MAX_INACTIVE);

        let workspace_tokens = graph.total_tokens().max(1);
        if total_raw_tokens == 0 {
            total_raw_tokens = workspace_tokens.max(active_tokens);
        }

        let reduction_percentage = if workspace_tokens > 0 {
            let saved = workspace_tokens.saturating_sub(active_tokens);
            (saved as f32 / workspace_tokens as f32) * 100.0
        } else {
            0.0
        };

        let coverage = CoverageReport::from_seeds(&seed_resolutions);
        let selected_paths: HashSet<String> = active_nodes
            .iter()
            .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
        let unresolved: Vec<_> = graph
            .unresolved_refs()
            .into_iter()
            .filter(|u| {
                selected_paths.contains(&u.from_file.to_string_lossy().replace('\\', "/"))
                    || seed_resolutions.iter().any(|s| s.query == u.name)
            })
            .take(40)
            .collect();

        let next_actions = build_next_actions(
            graph,
            &active_nodes,
            &included,
            &coverage,
            &fold_ids,
            &unresolved,
        );

        let view = ContextView {
            project_id: graph.project_id(),
            active_nodes,
            inactive_descriptors,
            total_raw_tokens,
            active_tokens,
            reduction_percentage,
            confidence_score: signature.confidence,
            bypass_applied: is_critical,
            seeds: seed_resolutions,
            unresolved,
            coverage: Some(coverage),
            next_actions,
            budget_used: active_tokens,
            budget_cap: seed_tokens.saturating_add(fill_cap),
            budget_mode: budget_mode_name(effective_mode).to_string(),
            budget_seed_tokens: seed_tokens,
            budget_fill_used: fill_used,
            budget_fill_cap: fill_cap,
            over_budget: fill_used > fill_cap,
            fold_ids,
            seed_call_coverage: compute_seed_call_coverage(graph, &seed_set, &selected_paths),
            workspace_tokens,
            physarum_used,
            physarum_ms,
            selection_method: selection.method.to_string(),
        };
        *self.last_packet.lock() = Some(PacketSnapshot::from_view(&view));
        view
    }
}

fn prefer_search_seed(
    graph: &NeuralProjectGraph,
    query: &str,
    ranked_id: NodeId,
    ranked_confidence: EdgeConfidence,
) -> NodeId {
    let Some(hit) = graph.search_symbols(query, 1).into_iter().next() else {
        return ranked_id;
    };
    if hit.score < 90.0 || hit.id == ranked_id {
        return ranked_id;
    }
    let exact_case = hit.name == query;
    let path_hit = path_echoes_symbol(&hit.file_path, query);
    if !exact_case && !path_hit {
        return ranked_id;
    }
    if ranked_confidence == EdgeConfidence::Proven && !exact_case && !path_hit {
        return ranked_id;
    }
    hit.id
}

fn function_spans_for_file(
    graph: &NeuralProjectGraph,
    path: &std::path::Path,
) -> Vec<FunctionSpan> {
    let norm = path.to_string_lossy().replace('\\', "/");
    graph
        .get_all_nodes()
        .into_iter()
        .filter(|n| {
            n.node_type == NodeType::Function
                && n.file_path.to_string_lossy().replace('\\', "/") == norm
        })
        .filter_map(|n| {
            let range = n.line_range?;
            Some(FunctionSpan {
                name: n.name,
                start_line: range.start,
                end_line: range.end.saturating_sub(1).max(range.start),
                signature: n.signature.unwrap_or_default(),
            })
        })
        .collect()
}

fn compute_seed_call_coverage(
    graph: &NeuralProjectGraph,
    seeds: &HashSet<NodeId>,
    selected_paths: &HashSet<String>,
) -> f32 {
    let mut total = 0usize;
    let mut hit = 0usize;
    for seed in seeds {
        for (neighbor, edge) in graph.get_connected_neighbors(seed) {
            if edge.edge_type != EdgeType::Calls || edge.source != *seed {
                continue;
            }
            let Some(node) = graph.get_node(&neighbor) else {
                continue;
            };
            total += 1;
            let path = node.file_path.to_string_lossy().replace('\\', "/");
            if selected_paths.contains(&path) {
                hit += 1;
            }
        }
    }
    if total == 0 {
        1.0
    } else {
        hit as f32 / total as f32
    }
}

fn build_next_actions(
    graph: &NeuralProjectGraph,
    active: &[ActivatedNodeView],
    selected: &HashSet<NodeId>,
    coverage: &CoverageReport,
    fold_ids: &[String],
    unresolved: &[neuromesh_core::UnresolvedRef],
) -> Vec<NextAction> {
    let mut actions = Vec::new();
    let partial = coverage.claim == "partial";
    if partial {
        for missed in &coverage.seeds_missed {
            actions.push(NextAction {
                tool: "neuromesh_search_symbols".into(),
                query: missed.clone(),
                why: "coverage is partial — Grep/search this missed seed".into(),
            });
        }
    }
    for fold_id in fold_ids.iter().take(3) {
        actions.push(NextAction {
            tool: "neuromesh_expand_fold".into(),
            query: fold_id.clone(),
            why: "wake this intron without reading the disk".into(),
        });
    }
    for node in active.iter().take(4) {
        for (neighbor, edge) in graph.get_connected_neighbors(&node.node.id) {
            if edge.edge_type == EdgeType::Calls && !selected.contains(&neighbor) {
                if let Some(outside) = graph.get_node(&neighbor) {
                    actions.push(NextAction {
                        tool: "neuromesh_trace".into(),
                        query: outside.name,
                        why: "caller or callee sits outside the packet".into(),
                    });
                    break;
                }
            }
        }
        if actions.len() >= 8 {
            break;
        }
    }
    if partial {
        if let Some(u) = unresolved.first() {
            if !actions.iter().any(|a| a.query == u.name) {
                actions.push(NextAction {
                    tool: "neuromesh_search_symbols".into(),
                    query: u.name.clone(),
                    why: format!(
                        "unresolved {:?} from {} — Grep only because coverage is partial",
                        u.relationship, u.from
                    ),
                });
            }
        }
    }
    actions.truncate(8);
    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;
    use neuromesh_index::{IndexedFile, SourceLanguage};
    use neuromesh_parser::CodeIntelligenceEngine;
    use neuromesh_task::TaskSignatureExtractor;
    use std::path::PathBuf;

    fn indexed(rel: &str) -> IndexedFile {
        IndexedFile {
            project_id: ProjectId::new("neuromesh"),
            relative_path: PathBuf::from(rel),
            full_path: PathBuf::from(rel),
            blake3_hash: "test".into(),
            byte_size: 200,
            token_count: 120,
            language: SourceLanguage::Rust,
            last_modified: chrono::Utc::now(),
        }
    }

    #[test]
    fn activates_identifier_neighborhood_not_whole_graph() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let tools = r#"
use neuromesh_task::TaskSignatureExtractor;
pub fn handle_tool_call() {
    let signature = TaskSignatureExtractor::extract("demo");
    activate(&signature);
}
pub fn unused_helper() { let x = 1; let y = 2; let z = 3; let w = 4; }
"#;
        let sig = r#"
pub struct TaskSignatureExtractor;
impl TaskSignatureExtractor {
    pub fn extract(prompt: &str) -> String { prompt.into() }
}
"#;
        graph.ingest_file(
            &indexed("crates/neuromesh-mcp/src/tools.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("tools.rs"),
                tools,
                SourceLanguage::Rust,
            ),
            Some(tools),
        );
        graph.ingest_file(
            &indexed("crates/neuromesh-task/src/signature.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("signature.rs"),
                sig,
                SourceLanguage::Rust,
            ),
            Some(sig),
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let signature =
            TaskSignatureExtractor::extract("How does handle_tool_call extract task intent?");
        let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);

        assert!(view
            .active_nodes
            .iter()
            .any(|n| n.node.name == "handle_tool_call"));
        assert!(view.coverage.is_some());
        assert!(view.budget_cap > 0);
        assert!(view.seeds.iter().any(|s| s.resolved_id.is_some()));
    }

    #[test]
    fn expand_fold_restores_body_without_disk() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let tools = r#"
use neuromesh_task::TaskSignatureExtractor;
pub fn handle_tool_call() {
    let signature = TaskSignatureExtractor::extract("demo");
    activate(&signature);
}
pub fn unused_helper() {
    let x = 1;
    let y = 2;
    let z = 3;
    let w = 4;
    let q = 5;
    x + y + z + w + q
}
"#;
        graph.ingest_file(
            &indexed("crates/neuromesh-mcp/src/tools.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("tools.rs"),
                tools,
                SourceLanguage::Rust,
            ),
            Some(tools),
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry.clone());
        let signature = TaskSignatureExtractor::extract("How does handle_tool_call work?");
        let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
        assert!(
            !view.fold_ids.is_empty(),
            "expected unused_helper to fold: {:?}",
            view.active_nodes
                .iter()
                .map(|n| n.node.content.clone())
                .collect::<Vec<_>>()
        );
        let fold_id = view.fold_ids[0].clone();
        let engine = crate::expansion::ExpansionEngine::new(registry);
        let expanded = engine
            .expand_fold(&fold_id)
            .expect("fold must be in registry");
        assert!(expanded.original_body.contains("let q = 5"));
        assert_eq!(expanded.fold_id, fold_id);
    }

    #[test]
    fn real_tools_rs_folds_siblings_and_roundtrips() {
        let tools_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("neuromesh-mcp")
            .join("src")
            .join("tools.rs");
        let tools = std::fs::read_to_string(&tools_path).expect("read tools.rs");
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        graph.ingest_file(
            &indexed("crates/neuromesh-mcp/src/tools.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("tools.rs"),
                &tools,
                SourceLanguage::Rust,
            ),
            Some(&tools),
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry.clone());
        let signature =
            TaskSignatureExtractor::extract("How does handle_tool_call extract intent?");
        let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
        let packet = view
            .active_nodes
            .iter()
            .find(|n| {
                n.node.node_type == NodeType::File
                    && n.node
                        .file_path
                        .to_string_lossy()
                        .replace('\\', "/")
                        .ends_with("tools.rs")
            })
            .expect("tools.rs in packet");
        let skeleton = packet.node.content.as_deref().unwrap_or("");
        assert!(
            !packet
                .folded_symbols
                .iter()
                .any(|s| s == "handle_tool_call"),
            "handle_tool_call must remain an exon, folded={:?}",
            packet.folded_symbols
        );
        assert!(
            skeleton.contains("handle_tool_call") || skeleton.contains("TaskSignatureExtractor"),
            "handle_tool_call exon must stay open; skeleton starts: {:?}",
            skeleton.chars().take(400).collect::<String>()
        );
        assert!(
            !packet.folded_symbols.is_empty() || !view.fold_ids.is_empty(),
            "sibling methods in tools.rs should fold"
        );
        let fold_id = view.fold_ids.first().cloned().expect("fold id");
        let engine = crate::expansion::ExpansionEngine::new(registry);
        let expanded = engine.expand_fold(&fold_id).expect("expand from registry");
        assert!(!expanded.original_body.is_empty());
        assert!(!expanded.original_body.contains("[neuromesh:fold:"));
    }

    #[test]
    fn physarum_tubes_connect_two_seeds_under_sla() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let a = r#"
pub fn start_job() {
    enqueue_job();
}
"#;
        let b = r#"
pub fn enqueue_job() {
    let x = 1;
    x
}
"#;
        graph.ingest_file(
            &indexed("src/worker.rs"),
            &CodeIntelligenceEngine::analyze(&PathBuf::from("worker.rs"), a, SourceLanguage::Rust),
            Some(a),
        );
        graph.ingest_file(
            &indexed("src/queue.rs"),
            &CodeIntelligenceEngine::analyze(&PathBuf::from("queue.rs"), b, SourceLanguage::Rust),
            Some(b),
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let signature = TaskSignatureExtractor::extract("How does start_job enqueue_job?");
        let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
        let seeds: Vec<_> = view
            .seeds
            .iter()
            .filter(|s| s.resolved_id.is_some())
            .collect();
        assert!(
            seeds.len() >= 2,
            "need two seeds for Physarum: {:?}",
            view.seeds
        );
        assert!(view.physarum_ms < 20, "tube latency {}ms", view.physarum_ms);
        assert!(
            view.physarum_used,
            "neighborhood Physarum must run for two seeds: method={}",
            view.selection_method
        );
        let tel = activator.last_physarum();
        assert!(tel.used);
        assert!(tel.ms < 20);
    }

    #[test]
    fn folds_survive_second_activate_in_same_session() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let tools = r#"
pub fn handle_tool_call() {
    let signature = 1;
    signature
}
pub fn unused_helper() {
    let x = 1;
    let y = 2;
    let z = 3;
    let w = 4;
    x + y + z + w
}
"#;
        graph.ingest_file(
            &indexed("crates/neuromesh-mcp/src/tools.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("tools.rs"),
                tools,
                SourceLanguage::Rust,
            ),
            Some(tools),
        );
        graph.finalize_links();
        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry.clone());
        let first = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract("How does handle_tool_call work?"),
            OptimizationMode::Balanced,
        );
        let fold_id = first.fold_ids.first().cloned().expect("fold");
        let second = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract("How does handle_tool_call work?"),
            OptimizationMode::Balanced,
        );
        assert!(
            registry.get_fold(&fold_id).is_some()
                || second
                    .fold_ids
                    .iter()
                    .any(|id| registry.get_fold(id).is_some()),
            "folds must persist across get_context in one session"
        );
        assert!(registry.fold_count() > 0);
    }

    #[test]
    fn synaptic_feedback_pulls_coedited_file_into_second_packet() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let worker = r#"
pub fn process_job() {
    let n = 1;
    n
}
"#;
        let sig = r#"
pub fn extract(prompt: &str) -> String {
    prompt.to_string()
}
"#;
        graph.ingest_file(
            &indexed("src/worker.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("worker.rs"),
                worker,
                SourceLanguage::Rust,
            ),
            Some(worker),
        );
        graph.ingest_file(
            &indexed("src/signature.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("signature.rs"),
                sig,
                SourceLanguage::Rust,
            ),
            Some(sig),
        );
        graph.finalize_links();
        let worker_file = graph
            .file_id_for_path(&PathBuf::from("src/worker.rs"))
            .expect("worker file");
        let sig_file = graph
            .file_id_for_path(&PathBuf::from("src/signature.rs"))
            .expect("signature file");
        graph.add_edge(
            worker_file.clone(),
            sig_file.clone(),
            neuromesh_core::EdgeType::ModifiedWith,
        );

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let signature = TaskSignatureExtractor::extract("How does process_job work?");
        let first = activator.activate(&graph, &signature, OptimizationMode::Balanced);
        let first_has_sig = first.active_nodes.iter().any(|n| {
            n.node
                .file_path
                .to_string_lossy()
                .replace('\\', "/")
                .contains("signature.rs")
        });
        graph.record_neural_spike(worker_file.clone(), true, true);
        graph.record_neural_spike(sig_file.clone(), true, true);
        graph.apply_stdp_on_path(&[worker_file.clone(), sig_file.clone()]);
        graph.reinforce_path(&[worker_file, sig_file], true);
        graph.reinforce_path(
            &[
                graph
                    .file_id_for_path(&PathBuf::from("src/worker.rs"))
                    .unwrap(),
                graph
                    .file_id_for_path(&PathBuf::from("src/signature.rs"))
                    .unwrap(),
            ],
            true,
        );

        let second = activator.activate(&graph, &signature, OptimizationMode::Balanced);
        let second_has_sig = second.active_nodes.iter().any(|n| {
            n.node
                .file_path
                .to_string_lossy()
                .replace('\\', "/")
                .contains("signature.rs")
        });
        assert!(
            second_has_sig,
            "pheromone fill should pull signature.rs after feedback; first={first_has_sig}"
        );
    }

    #[test]
    fn next_actions_expand_folds_and_grep_only_when_partial() {
        let graph = NeuralProjectGraph::new(ProjectId::new("neuromesh"));
        let tools = r#"
pub fn handle_tool_call() {
    let signature = 1;
    signature
}
pub fn unused_helper() {
    let x = 1;
    let y = 2;
    let z = 3;
    x + y + z
}
"#;
        graph.ingest_file(
            &indexed("src/tools.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("tools.rs"),
                tools,
                SourceLanguage::Rust,
            ),
            Some(tools),
        );
        graph.finalize_links();
        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let hit = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract("How does handle_tool_call work?"),
            OptimizationMode::Balanced,
        );
        assert!(
            hit.next_actions
                .iter()
                .any(|a| a.tool == "neuromesh_expand_fold")
                || hit.fold_ids.is_empty(),
            "expand_fold should be offered when folds exist: {:?}",
            hit.next_actions
        );
        assert!(
            !hit.next_actions
                .iter()
                .any(|a| a.tool == "neuromesh_search_symbols"),
            "Grep is not next when coverage is complete: {:?}",
            hit.next_actions
        );
        let miss = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract("What does __no_such_symbol_xyz__ do?"),
            OptimizationMode::Balanced,
        );
        assert_eq!(
            miss.coverage.as_ref().map(|c| c.claim.as_str()),
            Some("partial")
        );
        assert!(
            miss.next_actions
                .iter()
                .any(|a| a.tool == "neuromesh_search_symbols"),
            "Grep only when partial: {:?}",
            miss.next_actions
        );
    }

    fn indexed_ts(rel: &str) -> IndexedFile {
        IndexedFile {
            project_id: ProjectId::new("shop"),
            relative_path: PathBuf::from(rel),
            full_path: PathBuf::from(rel),
            blake3_hash: rel.to_string(),
            byte_size: 400,
            token_count: 80,
            language: SourceLanguage::TypeScript,
            last_modified: chrono::Utc::now(),
        }
    }

    fn ingest_ts(graph: &NeuralProjectGraph, rel: &str, src: &str) {
        graph.ingest_file(
            &indexed_ts(rel),
            &CodeIntelligenceEngine::analyze(&PathBuf::from(rel), src, SourceLanguage::TypeScript),
            Some(src),
        );
    }

    #[test]
    fn seed_callees_stay_open_siblings_still_fold() {
        let graph = NeuralProjectGraph::new(ProjectId::new("shop"));
        ingest_ts(
            &graph,
            "src/orders/checkout.ts",
            r#"
import { applyLoyaltyDiscount } from "./loyalty.ts";
import { authorizePaymentIntent } from "../payments/stripe.ts";

export function calculateCheckoutTotal(amount: number): number {
  const discounted = applyLoyaltyDiscount(amount);
  authorizePaymentIntent(discounted);
  return discounted;
}

export function unusedCheckoutDebugDump(amount: number): string {
  const a = amount;
  const b = a + 1;
  const c = b + 2;
  const d = c + 3;
  const e = d + 4;
  return String(a + b + c + d + e);
}
"#,
        );
        ingest_ts(
            &graph,
            "src/orders/loyalty.ts",
            r#"
export function applyLoyaltyDiscount(amount: number): number {
  const points = Math.floor(amount / 100);
  const boost = points * 5;
  return Math.max(0, amount - boost);
}

export function unusedExpireStalePoints(points: number): number {
  const a = points;
  const b = a / 2;
  const c = b / 2;
  const d = c / 2;
  return Math.floor(a + b + c + d);
}
"#,
        );
        ingest_ts(
            &graph,
            "src/payments/stripe.ts",
            r#"
export function authorizePaymentIntent(amount: number): string {
  if (amount <= 0) {
    throw new Error("invalid");
  }
  return "pi_" + String(amount);
}

export function unusedListTestCards(): string[] {
  const a = "4242";
  const b = "4000";
  const c = a + b;
  const d = c + "12";
  return [a, b, c, d];
}
"#,
        );
        ingest_ts(
            &graph,
            "src/lib/logger.ts",
            r#"
export function writeShopLog(event: string): void {
  console.log(event);
}

export function unusedRotateBuffers(rows: string[]): string[] {
  const out: string[] = [];
  for (const row of rows) {
    if (row.length > 0) {
      out.push(row);
    }
  }
  return out.slice(0, 10);
}
"#,
        );
        ingest_ts(
            &graph,
            "src/inventory/warehouse.ts",
            r#"
export function unusedRebalanceBins(): number {
  const a = 1;
  const b = 2;
  const c = 3;
  const d = 4;
  return a + b + c + d;
}
"#,
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let view = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract(
                "How does calculateCheckoutTotal apply loyalty before authorizePaymentIntent?",
            ),
            OptimizationMode::Balanced,
        );

        let files: Vec<String> = view
            .active_nodes
            .iter()
            .filter(|n| n.node.node_type == NodeType::File)
            .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
        assert!(
            files.iter().any(|p| p.ends_with("loyalty.ts")),
            "callee file loyalty.ts must be in the packet: {files:?}"
        );
        let loyalty = view
            .active_nodes
            .iter()
            .find(|n| {
                n.node.node_type == NodeType::File
                    && n.node
                        .file_path
                        .to_string_lossy()
                        .replace('\\', "/")
                        .ends_with("loyalty.ts")
            })
            .expect("loyalty.ts");
        assert!(
            !loyalty
                .folded_symbols
                .iter()
                .any(|s| s == "applyLoyaltyDiscount"),
            "applyLoyaltyDiscount is a seed callee and must stay an exon, folded={:?}",
            loyalty.folded_symbols
        );
        let checkout = view
            .active_nodes
            .iter()
            .find(|n| {
                n.node.node_type == NodeType::File
                    && n.node
                        .file_path
                        .to_string_lossy()
                        .replace('\\', "/")
                        .ends_with("checkout.ts")
            })
            .expect("checkout.ts");
        assert!(
            checkout
                .folded_symbols
                .iter()
                .any(|s| s == "unusedCheckoutDebugDump"),
            "unused sibling must still fold, folded={:?}",
            checkout.folded_symbols
        );
    }

    #[test]
    fn searcher_seed_ships_module_file() {
        let graph = NeuralProjectGraph::new(ProjectId::new("shop"));
        let searcher_mod = r#"
pub struct Searcher {
    needle: String,
}

impl Searcher {
    pub fn search(&self, haystack: &str) -> bool {
        let extra = haystack.len();
        haystack.contains(&self.needle) && extra > 0
    }
}
"#;
        let query_fn = r#"
pub fn searcher(haystack: &str, needle: &str) -> bool {
    let a = haystack.len();
    let b = needle.len();
    let c = a.saturating_sub(b);
    haystack.contains(needle) && c < 10_000
}
"#;
        graph.ingest_file(
            &indexed("src/searcher/mod.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("src/searcher/mod.rs"),
                searcher_mod,
                SourceLanguage::Rust,
            ),
            Some(searcher_mod),
        );
        graph.ingest_file(
            &indexed("src/query.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("src/query.rs"),
                query_fn,
                SourceLanguage::Rust,
            ),
            Some(query_fn),
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let view = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract("How does Searcher scan a haystack?"),
            OptimizationMode::Balanced,
        );
        let files: Vec<String> = view
            .active_nodes
            .iter()
            .filter(|n| n.node.node_type == NodeType::File)
            .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
        assert!(
            files.iter().any(|p| p.ends_with("searcher/mod.rs")),
            "packet must include searcher/mod.rs, files={files:?}"
        );
        assert!(
            view.seeds.iter().any(|s| {
                s.query == "Searcher"
                    && s.resolved_id
                        .as_ref()
                        .and_then(|id| {
                            graph.get_node(id).map(|n| {
                                n.name == "Searcher"
                                    && n.file_path
                                        .to_string_lossy()
                                        .replace('\\', "/")
                                        .ends_with("searcher/mod.rs")
                            })
                        })
                        .unwrap_or(false)
            }),
            "seed Searcher must resolve to searcher/mod.rs, seeds={:?}",
            view.seeds
        );
    }
}
