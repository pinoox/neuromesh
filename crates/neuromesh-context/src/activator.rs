use crate::registry::ReversibleContextRegistry;
use crate::scoring::{ActivationScorer, ScoringWeights};
use crate::skeleton::CodeSkeletonizer;
use neuromesh_core::{
    ActivatedNodeView, ContextStatus, ContextView, NodeId, NodeType, OptimizationMode, TaskSignature,
};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const MAX_ACTIVE_FILES: usize = 10;
const MAX_ACTIVE_SYMBOLS: usize = 24;
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

        for (query, energy, reason) in queries {
            for hit in graph.search_symbols(&query, 6) {
                seed_energies
                    .entry(hit.id.clone())
                    .and_modify(|e| *e = (*e).max(energy))
                    .or_insert(energy);
                seed_reasons
                    .entry(hit.id)
                    .or_insert_with(|| format!("{reason}:{query}"));
            }
        }

        if seed_energies.is_empty() {
            for token in signature.raw_prompt.split_whitespace().take(8) {
                let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                if clean.len() < 4 {
                    continue;
                }
                for hit in graph.search_symbols(clean, 3) {
                    seed_energies.entry(hit.id.clone()).or_insert(0.55);
                    seed_reasons
                        .entry(hit.id)
                        .or_insert_with(|| format!("token:{clean}"));
                }
            }
        }

        let seed_set: HashSet<NodeId> = seed_energies.keys().cloned().collect();
        let neighborhood = if seed_set.is_empty() {
            HashSet::new()
        } else {
            graph.neighborhood(&seed_set, hops)
        };

        let mut graph_energies = HashMap::new();
        for (id, energy) in &seed_energies {
            graph_energies.insert(id.clone(), *energy);
        }
        if neighborhood.len() <= 400 && seed_set.len() > 1 {
            let physarum = graph.solve_physarum_local(&seed_set, hops);
            for id in physarum.active_nodes {
                if neighborhood.contains(&id) {
                    let flux = physarum.node_flux.get(&id).copied().unwrap_or(0.55);
                    graph_energies
                        .entry(id)
                        .and_modify(|e| *e = (*e).max(0.55 + 0.4 * flux))
                        .or_insert(0.45 + 0.3 * flux);
                }
            }
        } else {
            for id in &neighborhood {
                graph_energies.entry(id.clone()).or_insert(0.28);
            }
        }

        let activation_threshold = match effective_mode {
            OptimizationMode::MaxQuality => 0.12,
            OptimizationMode::Balanced => 0.22,
            OptimizationMode::MaxSavings => 0.38,
        };

        let mut candidate_nodes = Vec::new();
        for id in &neighborhood {
            let Some(node) = graph.get_node(id) else {
                continue;
            };
            let rel_strength = *graph_energies.get(id).unwrap_or(&0.2);
            let score = self.scorer.score_node(&node, signature, rel_strength, 1.0);
            if score >= activation_threshold || seed_set.contains(id) {
                candidate_nodes.push((node, score));
            } else if candidate_nodes.len() < 48 {
                self.registry.register_inactive(
                    &node,
                    rel_strength,
                    signature.confidence,
                    score,
                    None,
                );
            }
        }

        candidate_nodes.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut kept = Vec::new();
        let mut file_count = 0usize;
        let mut symbol_count = 0usize;
        for (node, score) in candidate_nodes {
            let is_file = node.node_type == NodeType::File;
            if is_file {
                if file_count >= MAX_ACTIVE_FILES {
                    self.registry
                        .register_inactive(&node, score, signature.confidence, score, None);
                    continue;
                }
                file_count += 1;
            } else {
                if symbol_count >= MAX_ACTIVE_SYMBOLS {
                    self.registry
                        .register_inactive(&node, score, signature.confidence, score, None);
                    continue;
                }
                symbol_count += 1;
            }
            kept.push((node, score));
        }

        let mut active_symbol_names: HashSet<String> = HashSet::new();
        active_symbol_names.insert(signature.entity.to_lowercase());
        for ident in &signature.identifiers {
            active_symbol_names.insert(ident.to_lowercase());
        }
        for (node, _) in &kept {
            active_symbol_names.insert(node.name.to_lowercase());
        }

        let mut active_nodes = Vec::new();
        let mut active_tokens = 0;
        let mut total_raw_tokens = 0;

        for (mut node, score) in kept {
            if let Some(content) = node.content.clone() {
                total_raw_tokens += neuromesh_core::TokenCounter::count_tokens(&content);
                let skeleton_res = CodeSkeletonizer::skeletonize(
                    &node.file_path.to_string_lossy(),
                    &content,
                    &active_symbol_names,
                );
                node.content = Some(skeleton_res.skeleton_code);
                node.token_cost = skeleton_res.skeleton_tokens;
            } else {
                total_raw_tokens += node.token_cost;
            }

            active_tokens += node.token_cost;
            let reason = seed_reasons.get(&node.id).cloned();
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

        ContextView {
            project_id: graph.project_id(),
            active_nodes,
            inactive_descriptors,
            total_raw_tokens,
            active_tokens,
            reduction_percentage,
            confidence_score: signature.confidence,
            bypass_applied: is_critical,
        }
    }
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
            &CodeIntelligenceEngine::analyze(&PathBuf::from("tools.rs"), tools, SourceLanguage::Rust),
            Some(tools),
        );
        graph.ingest_file(
            &indexed("crates/neuromesh-task/src/signature.rs"),
            &CodeIntelligenceEngine::analyze(&PathBuf::from("signature.rs"), sig, SourceLanguage::Rust),
            Some(sig),
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let signature = TaskSignatureExtractor::extract(
            "How does handle_tool_call extract task intent?",
        );
        let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);

        assert!(view.active_nodes.iter().any(|n| n.node.name == "handle_tool_call"));
        assert!(view.active_nodes.len() < 12);
        assert!(view.inactive_descriptors.len() <= 12);
    }
}
