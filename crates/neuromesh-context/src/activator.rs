use crate::registry::ReversibleContextRegistry;
use crate::scoring::{ActivationScorer, ScoringWeights};
use crate::selector::{budget_mode_name, select};
use crate::skeleton::CodeSkeletonizer;
use neuromesh_core::{
    ActivatedNodeView, ContextStatus, ContextView, CoverageReport, EdgeType, NextAction, NodeId,
    OptimizationMode, SeedResolution, TaskSignature,
};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const MAX_INACTIVE: usize = 12;

pub struct ContextActivator {
    scorer: ActivationScorer,
    registry: Arc<ReversibleContextRegistry>,
}

impl ContextActivator {
    pub fn new(registry: Arc<ReversibleContextRegistry>) -> Self {
        Self {
            scorer: ActivationScorer::new(ScoringWeights::default()),
            registry,
        }
    }

    pub fn activate(
        &self,
        graph: &NeuralProjectGraph,
        signature: &TaskSignature,
        mode: OptimizationMode,
    ) -> ContextView {
        self.registry.clear();

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
        if !signature.entity.is_empty() && signature.entity != "Workspace" {
            queries.push((signature.entity.clone(), 1.0, "entity"));
        }
        for ident in &signature.identifiers {
            queries.push((ident.clone(), 1.0, "identifier"));
        }
        for hint in &signature.file_hints {
            queries.push((hint.clone(), 0.95, "file"));
        }
        for concept in signature.related_concepts.iter().take(6) {
            queries.push((concept.clone(), 0.72, "concept"));
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
            if let Some((id, confidence)) = graph.resolve_ranked(&query, None, None) {
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

        if seed_set.len() >= 2 && neighborhood.len() <= 400 {
            let physarum = graph.solve_physarum_local(&seed_set, hops);
            for id in physarum.active_nodes {
                if neighborhood.contains(&id) {
                    let flux = physarum.node_flux.get(&id).copied().unwrap_or(0.0);
                    seed_energies
                        .entry(id)
                        .and_modify(|energy| *energy = (*energy).max(0.35 + 0.2 * flux));
                }
            }
        }

        let selection = select(
            graph,
            &neighborhood,
            &seed_set,
            &seed_energies,
            effective_mode,
        );
        let selected: HashSet<NodeId> = selection.node_ids.iter().cloned().collect();

        let mut kept = Vec::new();
        for id in &selection.node_ids {
            let Some(node) = graph.get_node(id) else {
                continue;
            };
            let rel_strength = *seed_energies.get(id).unwrap_or(&0.35);
            let score = selection
                .scores
                .get(id)
                .copied()
                .unwrap_or_else(|| self.scorer.score_node(&node, signature, rel_strength, 1.0));
            kept.push((node, score));
        }

        let mut inactive_count = 0usize;
        for id in &neighborhood {
            if selected.contains(id) || inactive_count >= MAX_INACTIVE {
                continue;
            }
            if let Some(node) = graph.get_node(id) {
                let score = self.scorer.score_node(&node, signature, 0.2, 1.0);
                self.registry
                    .register_inactive(&node, 0.2, signature.confidence, score, None);
                inactive_count += 1;
            }
        }

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

        let mut active_nodes = Vec::new();
        let mut active_tokens = 0;
        let mut total_raw_tokens = 0;
        let mut fold_ids = Vec::new();

        for (mut node, score) in kept {
            if let Some(content) = node.content.clone() {
                total_raw_tokens += neuromesh_core::TokenCounter::count_tokens(&content);
                let skeleton_res = CodeSkeletonizer::skeletonize(
                    &node.file_path.to_string_lossy(),
                    &content,
                    &active_symbol_names,
                );
                fold_ids.extend(skeleton_res.folds.iter().map(|f| f.fold_id.clone()));
                node.content = Some(skeleton_res.skeleton_code);
                node.token_cost = skeleton_res.skeleton_tokens;
            } else {
                total_raw_tokens += node.token_cost;
            }

            active_tokens += node.token_cost;
            let reason = seed_reasons.get(&node.id).cloned().or_else(|| {
                selection
                    .scores
                    .get(&node.id)
                    .map(|s| format!("utility:{s:.2}"))
            });
            active_nodes.push(ActivatedNodeView {
                node,
                activation_score: score,
                status: ContextStatus::Active,
                expansion_reason: reason,
            });
        }

        let mut inactive_descriptors = self.registry.get_inactive_descriptors();
        inactive_descriptors.sort_by(|a, b| {
            b.activation_score
                .partial_cmp(&a.activation_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        inactive_descriptors.truncate(MAX_INACTIVE);

        if total_raw_tokens == 0 {
            total_raw_tokens = graph.total_tokens().max(active_tokens);
        }

        let reduction_percentage = if total_raw_tokens > 0 {
            let saved = total_raw_tokens.saturating_sub(active_tokens);
            (saved as f32 / total_raw_tokens as f32) * 100.0
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
            &selected,
            &coverage,
            &fold_ids,
            &unresolved,
        );

        ContextView {
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
            budget_used: selection.budget_used.max(active_tokens),
            budget_cap: selection.budget_cap,
            budget_mode: budget_mode_name(effective_mode).to_string(),
            fold_ids,
        }
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
    for missed in &coverage.seeds_missed {
        actions.push(NextAction {
            tool: "neuromesh_search_symbols".into(),
            query: missed.clone(),
            why: "seed did not resolve in the index".into(),
        });
    }
    if let Some(fold_id) = fold_ids.first() {
        let on_call_path = active.iter().any(|n| {
            graph
                .get_connected_neighbors(&n.node.id)
                .iter()
                .any(|(_, e)| {
                    e.edge_type == EdgeType::Calls
                        && e.confidence != neuromesh_core::EdgeConfidence::Unresolved
                })
        });
        if on_call_path {
            actions.push(NextAction {
                tool: "neuromesh_expand_fold".into(),
                query: fold_id.clone(),
                why: "folded sibling sits on a Calls path".into(),
            });
        }
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
        if actions.len() >= 6 {
            break;
        }
    }
    if let Some(u) = unresolved.first() {
        if !actions.iter().any(|a| a.query == u.name) {
            actions.push(NextAction {
                tool: "neuromesh_search_symbols".into(),
                query: u.name.clone(),
                why: format!("unresolved {:?} from {}", u.relationship, u.from),
            });
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
}
