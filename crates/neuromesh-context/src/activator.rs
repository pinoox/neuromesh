use crate::registry::ReversibleContextRegistry;
use crate::scoring::{ActivationScorer, ScoringWeights};
use crate::skeleton::CodeSkeletonizer;
use neuromesh_core::{
    ActivatedNodeView, ContextStatus, ContextView, NodeId, OptimizationMode,
    TaskSignature,
};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

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

        let all_nodes = graph.get_all_nodes();
        let total_raw_tokens: usize = all_nodes.iter().map(|n| n.token_cost).sum();

        // 1. Check if bypass / conservative mode applies
        let is_critical = signature.requires_conservative_mode();
        let effective_mode = if is_critical {
            OptimizationMode::MaxQuality
        } else {
            mode
        };

        // 2. Identify Seed Nodes from Task Signature Entity & Concepts
        let mut seed_energies: HashMap<NodeId, f32> = HashMap::new();
        let matched_nodes = graph.find_nodes_by_name(&signature.entity);

        for node in &matched_nodes {
            seed_energies.insert(node.id.clone(), 1.0);
        }

        for concept in &signature.related_concepts {
            let concept_nodes = graph.find_nodes_by_name(concept);
            for node in concept_nodes {
                seed_energies.entry(node.id.clone()).or_insert(0.85);
            }
        }

        // If no direct seeds found, seed with top file nodes
        if seed_energies.is_empty() {
            for node in all_nodes.iter().take(5) {
                seed_energies.insert(node.id.clone(), 0.5);
            }
        }

        // 3. Spreading Activation (with Physarum Network Flow Optimization)
        let graph_energies = graph.spreading_activation(&seed_energies);

        // 4. Threshold determination based on mode
        let activation_threshold = match effective_mode {
            OptimizationMode::MaxQuality => 0.15,
            OptimizationMode::Balanced => 0.35,
            OptimizationMode::MaxSavings => 0.55,
        };

        let mut candidate_nodes = Vec::new();

        for node in all_nodes {
            let rel_strength = *graph_energies.get(&node.id).unwrap_or(&0.05);
            let score = self.scorer.score_node(&node, signature, rel_strength, 1.0);

            if score >= activation_threshold {
                candidate_nodes.push((node, score));
            } else {
                // Register into reversible context
                self.registry.register_inactive(
                    &node,
                    rel_strength,
                    signature.confidence,
                    score,
                    None,
                );
            }
        }

        // 5. Active Symbol Names for Bio-Genetic Skeletonization
        let mut active_symbol_names: HashSet<String> = HashSet::new();
        active_symbol_names.insert(signature.entity.to_lowercase());
        for c in &signature.related_concepts {
            active_symbol_names.insert(c.to_lowercase());
        }
        for (node, _) in &candidate_nodes {
            active_symbol_names.insert(node.name.to_lowercase());
        }

        // 6. Skeletonize and build ActivatedNodeViews
        let mut active_nodes = Vec::new();
        let mut active_tokens = 0;

        for (mut node, score) in candidate_nodes {
            // Apply Bio-Genetic Code Slicing if content is present
            if let Some(content) = &node.content {
                let skeleton_res = CodeSkeletonizer::skeletonize(
                    &node.file_path.to_string_lossy(),
                    content,
                    &active_symbol_names,
                );
                node.content = Some(skeleton_res.skeleton_code);
                node.token_cost = skeleton_res.skeleton_tokens;
            }

            active_tokens += node.token_cost;
            active_nodes.push(ActivatedNodeView {
                node,
                activation_score: score,
                status: ContextStatus::Active,
                expansion_reason: None,
            });
        }

        // Sort active nodes by activation score descending
        active_nodes.sort_by(|a, b| {
            b.activation_score
                .partial_cmp(&a.activation_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let inactive_descriptors = self.registry.get_inactive_descriptors();
        let reduction_percentage = if total_raw_tokens > 0 {
            let saved = total_raw_tokens.saturating_sub(active_tokens);
            (saved as f32 / total_raw_tokens as f32) * 100.0
        } else {
            0.0
        };

        ContextView {
            project_id: graph.project_id().clone(),
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
