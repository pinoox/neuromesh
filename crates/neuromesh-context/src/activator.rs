use crate::fold::{FoldPolicy, OPTIONAL_EXON_BUDGET, SEED_EXON_BUDGET};
use crate::packet_analysis::{
    build_structural_evidence, compute_packet_gaps, enrich_coverage, inject_caller_context,
    semantic_style_coverage,
};
use crate::registry::ReversibleContextRegistry;
use crate::scoring::{ActivationScorer, ScoringWeights};
use crate::selector::{
    budget_mode_name, fill_budget, is_noise_path, packet_cap, seed_callee_exon_names, select,
};
use crate::skeleton::{CodeSkeletonizer, FoldedIntron, FunctionSpan};
use crate::style_routing::{
    inject_style_seeds, inject_view_component_seeds, is_style_task, style_noise_penalty,
    tighten_focused_view_selection,
};
use neuromesh_core::{
    decoy_allowed_for_prompt, hmvc_app_prefix, is_name_collision_decoy, is_schema_path,
    prompt_targets_database, prompt_targets_types, ActivatedNodeView, ContextStatus, ContextView,
    CoverageReport, EdgeConfidence, EdgeType, NextAction, NodeId, NodeType, OptimizationMode,
    SeedResolution, SkippedFile, TaskSignature,
};
use neuromesh_graph::{path_echoes_symbol, NeuralProjectGraph};
use neuromesh_task::{
    extract_cluster_nouns, is_route_query, split_task_clusters, stem_search_queries,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

const MAX_INACTIVE: usize = 12;
const PHYSARUM_SLA_MS: u64 = 20;

struct MaterializedNode {
    node: neuromesh_core::ContextNode,
    score: f32,
    reason: String,
    raw_tokens: usize,
    folds: Vec<FoldedIntron>,
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
            grep_needed: claim == "partial" || claim == "no_seed_resolved",
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
        for concept in &signature.related_concepts {
            if concept.len() < 4 {
                continue;
            }
            let lower = concept.to_lowercase();
            if lower == "layout" || lower == "breakpoints" || lower == "state" {
                continue;
            }
            queries.push((concept.clone(), 0.82, "concept"));
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

        let prompt = signature.raw_prompt.as_str();
        {
            let mut sink = SeedSink {
                resolutions: &mut seed_resolutions,
                energies: &mut seed_energies,
                reasons: &mut seed_reasons,
            };
            for (query, energy, reason) in queries {
                sink.push(graph, prompt, query, energy, reason);
            }

            if sink.energies.is_empty() && !is_style_task(signature) {
                for token in signature.raw_prompt.split_whitespace().take(8) {
                    let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
                    if clean.len() < 4 {
                        continue;
                    }
                    sink.push(graph, prompt, clean.to_string(), 0.55, "token");
                }
            }
        }

        seed_uncovered_clusters(
            graph,
            signature,
            &mut seed_resolutions,
            &mut seed_energies,
            &mut seed_reasons,
        );
        {
            let mut sink = SeedSink {
                resolutions: &mut seed_resolutions,
                energies: &mut seed_energies,
                reasons: &mut seed_reasons,
            };
            inject_style_seeds(graph, prompt, signature, &mut sink);
            inject_view_component_seeds(graph, prompt, signature, &mut sink);
        }
        mark_equivalent_file_hits(graph, &mut seed_resolutions, &mut seed_energies);
        cohere_ambiguous_seeds_to_app(graph, &mut seed_resolutions, &mut seed_energies, prompt);

        if is_style_task(signature) {
            let noise_ids: Vec<NodeId> = seed_energies
                .keys()
                .filter(|id| {
                    graph
                        .get_node(id)
                        .is_some_and(|n| style_noise_penalty(&n.file_path, signature) >= 20.0)
                })
                .cloned()
                .collect();
            for id in noise_ids {
                seed_energies.remove(&id);
                seed_reasons.remove(&id);
            }
            seed_resolutions.retain(|s| {
                s.resolved_id
                    .as_ref()
                    .is_none_or(|id| seed_energies.contains_key(id))
            });
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
        inject_caller_context(graph, &seed_set, prompt, &mut selection);
        tighten_focused_view_selection(graph, signature, &mut selection);
        let mut skipped_files: Vec<SkippedFile> = Vec::new();
        if is_style_task(signature) {
            selection.required.retain(|id| {
                let keep = graph
                    .get_node(id)
                    .map(|n| style_noise_penalty(&n.file_path, signature) < 20.0)
                    .unwrap_or(true);
                if !keep {
                    if let Some(node) = graph.get_node(id) {
                        skipped_files.push(SkippedFile {
                            path: node.file_path.to_string_lossy().replace('\\', "/"),
                            reason: "style task: filtered cart/promo noise (required)".into(),
                        });
                    }
                }
                keep
            });
            for id in selection.optional.clone() {
                let Some(node) = graph.get_node(&id) else {
                    continue;
                };
                if style_noise_penalty(&node.file_path, signature) >= 20.0 {
                    skipped_files.push(SkippedFile {
                        path: node.file_path.to_string_lossy().replace('\\', "/"),
                        reason: "style task: filtered cart/promo noise".into(),
                    });
                }
            }
            selection.optional.retain(|id| {
                graph
                    .get_node(id)
                    .map(|n| style_noise_penalty(&n.file_path, signature) < 20.0)
                    .unwrap_or(true)
            });
            for id in selection.optional.clone() {
                let Some(node) = graph.get_node(&id) else {
                    continue;
                };
                let penalty = style_noise_penalty(&node.file_path, signature);
                if penalty > 0.0 {
                    if let Some(score) = selection.scores.get_mut(&id) {
                        *score = (*score - penalty).max(0.0);
                    }
                }
            }
        }
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
        if let Some(lock) = locked_seed_hmvc_prefix(graph, &seed_set) {
            selection
                .required
                .retain(|id| keep_hmvc_packet_file(graph, &seed_set, &lock, id));
            selection
                .optional
                .retain(|id| keep_hmvc_packet_file(graph, &seed_set, &lock, id));
        }
        selection
            .required
            .retain(|id| keep_schema_packet_file(graph, &seed_set, prompt, id));
        selection
            .optional
            .retain(|id| keep_schema_packet_file(graph, &seed_set, prompt, id));
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

        let mut priority_symbols: HashSet<String> = HashSet::new();
        for seed in &seed_resolutions {
            if let Some(id) = &seed.resolved_id {
                if let Some(node) = graph.get_node(id) {
                    if node.node_type != NodeType::File {
                        priority_symbols.insert(node.name.to_lowercase());
                    }
                }
            }
            if !seed.query.contains(['/', '\\', '.']) {
                priority_symbols.insert(seed.query.to_lowercase());
            }
        }

        let fold_policy = FoldPolicy::from_task(&active_symbol_names, signature)
            .with_priority_symbols(priority_symbols);
        let packet_limit = packet_cap(effective_mode);

        let mut active_nodes = Vec::new();
        let mut included: HashSet<NodeId> = HashSet::new();
        let mut fill_used: usize = 0;
        let mut total_raw_tokens = 0;
        let mut fold_ids = Vec::new();
        let mut all_folds: Vec<FoldedIntron> = Vec::new();
        let registry = self.registry.clone();

        let materialize = |id: &NodeId,
                           scores: &HashMap<NodeId, f32>,
                           seed_energies: &HashMap<NodeId, f32>,
                           seed_reasons: &HashMap<NodeId, String>,
                           scorer: &crate::scoring::ActivationScorer,
                           exon_budget: usize|
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
            let hist_success = (node.base_relevance / 3.0).clamp(0.20, 1.0);
            let score = scores
                .get(id)
                .copied()
                .unwrap_or_else(|| scorer.score_node(&node, signature, rel_strength, hist_success));
            let reason = seed_reasons.get(id).cloned().unwrap_or_else(|| {
                scores
                    .get(id)
                    .map(|s| format!("utility:{s:.2}"))
                    .unwrap_or_else(|| "connector".into())
            });
            let mut folds = Vec::new();
            let mut folded_symbols = Vec::new();
            let policy = fold_policy.clone().with_exon_budget(exon_budget);
            let raw = if let Some(content) = graph.read_source(&node.file_path) {
                let raw = neuromesh_core::TokenCounter::count_tokens(&content);
                let spans = function_spans_for_file(graph, &node.file_path);
                let skeleton_res = CodeSkeletonizer::skeletonize_with_policy(
                    &node.file_path.to_string_lossy(),
                    &content,
                    &policy,
                    &spans,
                );
                for fold in skeleton_res.folds {
                    registry.register_fold(node.file_path.clone(), fold.clone());
                    folded_symbols.push(fold.symbol_name.clone());
                    folds.push(fold);
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

        let mut seed_items: Vec<MaterializedNode> = Vec::new();
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
                SEED_EXON_BUDGET,
            ) else {
                continue;
            };
            included.insert(id.clone());
            total_raw_tokens += item.raw_tokens;
            seed_items.push(item);
        }
        let mut seed_tokens = unique_file_tokens(seed_items.iter());

        let mut fill_items: Vec<MaterializedNode> = Vec::new();
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
                OPTIONAL_EXON_BUDGET,
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
            fill_items.push(item);
        }

        let packet_tokens = |seeds: &[MaterializedNode], fill: &[MaterializedNode]| -> usize {
            unique_file_tokens(seeds.iter().chain(fill.iter()))
        };
        while packet_tokens(&seed_items, &fill_items) > packet_limit && !fill_items.is_empty() {
            let dropped = fill_items.pop().expect("non-empty");
            fill_used = fill_used.saturating_sub(dropped.node.token_cost.max(1));
            total_raw_tokens = total_raw_tokens.saturating_sub(dropped.raw_tokens);
            included.remove(&dropped.node.id);
            self.registry.register_inactive(
                &dropped.node,
                0.2,
                signature.confidence,
                dropped.score,
                None,
            );
        }
        if packet_tokens(&seed_items, &fill_items) > packet_limit {
            for item in &mut seed_items {
                let Some(shrunk) = materialize(
                    &item.node.id,
                    &selection.scores,
                    &seed_energies,
                    &seed_reasons,
                    &self.scorer,
                    2,
                ) else {
                    continue;
                };
                *item = shrunk;
            }
            seed_tokens = unique_file_tokens(seed_items.iter());
        }

        let active_tokens = packet_tokens(&seed_items, &fill_items);
        for item in seed_items.into_iter().chain(fill_items) {
            fold_ids.extend(item.folds.iter().map(|f| f.fold_id.clone()));
            all_folds.extend(item.folds);
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

        let selected_paths: HashSet<String> = active_nodes
            .iter()
            .filter(|n| n.node.node_type == NodeType::File)
            .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
            .collect();
        let covered: Vec<String> = selected_paths.iter().cloned().collect();
        let (packet_gaps, unsure) =
            compute_packet_gaps(graph, &seed_set, &selected_paths, signature);
        let semantic_cov = semantic_style_coverage(&selected_paths, signature);
        let coverage = enrich_coverage(
            &seed_resolutions,
            packet_gaps,
            unsure,
            covered,
            skipped_files,
            semantic_cov,
        );
        let structural_evidence = build_structural_evidence(graph, &seed_set);
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
            &all_folds,
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
            over_budget: fill_used > fill_cap || active_tokens > packet_limit,
            fold_ids,
            seed_call_coverage: compute_seed_call_coverage(graph, &seed_set, &selected_paths),
            workspace_tokens,
            physarum_used,
            physarum_ms,
            selection_method: selection.method.to_string(),
            structural_evidence,
        };
        *self.last_packet.lock() = Some(PacketSnapshot::from_view(&view));
        view
    }
}

pub(crate) struct SeedSink<'a> {
    resolutions: &'a mut Vec<SeedResolution>,
    energies: &'a mut HashMap<NodeId, f32>,
    reasons: &'a mut HashMap<NodeId, String>,
}

impl SeedSink<'_> {
    pub(crate) fn push(
        &mut self,
        graph: &NeuralProjectGraph,
        prompt: &str,
        query: String,
        energy: f32,
        reason: &str,
    ) {
        if self.resolutions.iter().any(|s| s.query == query) {
            return;
        }
        if let Some((id, conf)) = resolve_seed_query(graph, &query, prompt) {
            self.energies
                .entry(id.clone())
                .and_modify(|e| *e = (*e).max(energy))
                .or_insert(energy);
            self.reasons
                .entry(id.clone())
                .or_insert_with(|| format!("{reason}:{query}"));
            self.resolutions.push(SeedResolution {
                query,
                resolved_id: Some(id),
                confidence: conf,
            });
        } else {
            self.resolutions.push(SeedResolution {
                query,
                resolved_id: None,
                confidence: 0.0,
            });
        }
    }
}

fn resolve_seed_query(
    graph: &NeuralProjectGraph,
    query: &str,
    prompt: &str,
) -> Option<(NodeId, f32)> {
    if let Some(hit) = resolve_seed_query_once(graph, query, prompt) {
        return Some(hit);
    }
    for stem in stem_search_queries(query) {
        if let Some(hit) = resolve_seed_query_once(graph, &stem, prompt) {
            return Some(hit);
        }
    }
    None
}

fn resolve_seed_query_once(
    graph: &NeuralProjectGraph,
    query: &str,
    prompt: &str,
) -> Option<(NodeId, f32)> {
    if query.contains("::") {
        if let Some(node) = graph.resolve_best(query) {
            if seed_path_allowed(graph, &node.id, prompt) {
                return Some((node.id, 1.0));
            }
        }
    }
    if query.contains(['/', '\\', '.']) && !is_route_query(query) {
        if let Some(id) = graph.resolve_file_hint(query) {
            if seed_path_allowed(graph, &id, prompt) {
                return Some((id, 0.95));
            }
        }
    }
    if let Some((ranked_id, confidence)) = graph.resolve_ranked(query, None, None) {
        let id = prefer_search_seed(graph, query, ranked_id, confidence, prompt);
        if seed_path_allowed(graph, &id, prompt) {
            let conf = match confidence {
                EdgeConfidence::Proven => 1.0,
                EdgeConfidence::Likely => 0.62,
                EdgeConfidence::Unresolved => 0.0,
            };
            return Some((id, conf));
        }
    }
    let hits = graph.search_symbols(query, 12);
    let q = query.to_lowercase();
    let hit = hits.into_iter().find(|hit| {
        if !seed_path_allowed(graph, &hit.id, prompt) {
            return false;
        }
        if hit.name.eq_ignore_ascii_case(query) {
            return true;
        }
        let n = hit.name.to_lowercase();
        hit.match_reason != "token" && hit.score >= 86.0 && (n.starts_with(&q) || q.starts_with(&n))
    })?;
    Some((hit.id, (hit.score / 100.0).clamp(0.2, 0.75)))
}

fn seed_path_allowed(graph: &NeuralProjectGraph, id: &NodeId, prompt: &str) -> bool {
    graph
        .get_node(id)
        .map(|node| {
            !is_name_collision_decoy(&node.file_path)
                || decoy_allowed_for_prompt(&node.file_path, prompt)
        })
        .unwrap_or(true)
}

fn mark_equivalent_file_hits(
    graph: &NeuralProjectGraph,
    seeds: &mut [SeedResolution],
    seed_energies: &mut HashMap<NodeId, f32>,
) {
    let hit_paths: Vec<(NodeId, String)> = seeds
        .iter()
        .filter_map(|s| {
            let id = s.resolved_id.as_ref()?;
            let node = graph.get_node(id)?;
            Some((
                id.clone(),
                node.file_path.to_string_lossy().replace('\\', "/"),
            ))
        })
        .collect();
    if hit_paths.is_empty() {
        return;
    }
    for seed in seeds.iter_mut() {
        if seed.resolved_id.is_some() {
            continue;
        }
        let query = seed.query.replace('\\', "/");
        let query_base = std::path::Path::new(&query)
            .file_name()
            .map(|s| s.to_string_lossy().replace('\\', "/"));
        if let Some((id, _)) = hit_paths.iter().find(|(_, path)| {
            path == &query
                || path.ends_with(&query)
                || query_base.as_ref().is_some_and(|base| {
                    path.ends_with(base) || path.rsplit('/').next() == Some(base)
                })
        }) {
            seed.resolved_id = Some(id.clone());
            seed.confidence = seed.confidence.max(0.95);
            seed_energies.entry(id.clone()).or_insert(0.95);
        }
    }
}

fn cohere_ambiguous_seeds_to_app(
    graph: &NeuralProjectGraph,
    seeds: &mut [SeedResolution],
    seed_energies: &mut HashMap<NodeId, f32>,
    prompt: &str,
) {
    let mut prefixes = HashSet::new();
    for seed in seeds.iter() {
        if seed.confidence < 0.9 {
            continue;
        }
        let Some(id) = seed.resolved_id.as_ref() else {
            continue;
        };
        if let Some(prefix) = graph
            .get_node(id)
            .and_then(|node| hmvc_app_prefix(&node.file_path))
        {
            prefixes.insert(prefix);
        }
    }
    if prefixes.len() != 1 {
        return;
    }
    let prefix = prefixes.into_iter().next().expect("checked len");
    let prefix_slash = format!("{prefix}/");
    for seed in seeds.iter_mut() {
        if seed.confidence >= 0.9 {
            continue;
        }
        let Some(id) = seed.resolved_id.clone() else {
            continue;
        };
        let Some(node) = graph.get_node(&id) else {
            continue;
        };
        let path = node.file_path.to_string_lossy().replace('\\', "/");
        if path.contains(&prefix_slash) {
            continue;
        }
        let hits = graph.search_symbols(&seed.query, 16);
        let Some(hit) = hits.into_iter().find(|hit| {
            if !seed_path_allowed(graph, &hit.id, prompt) {
                return false;
            }
            if !hit
                .file_path
                .to_string_lossy()
                .replace('\\', "/")
                .contains(&prefix_slash)
            {
                return false;
            }
            let name = hit
                .name
                .rsplit(['.', ':'])
                .next()
                .unwrap_or(hit.name.as_str());
            name.eq_ignore_ascii_case(&seed.query)
        }) else {
            continue;
        };
        seed_energies.remove(&id);
        seed.resolved_id = Some(hit.id.clone());
        seed.confidence = seed.confidence.max(0.9);
        seed_energies
            .entry(hit.id)
            .and_modify(|energy| *energy = (*energy).max(0.7))
            .or_insert(0.7);
    }
}

fn keep_hmvc_packet_file(
    graph: &NeuralProjectGraph,
    seeds: &HashSet<NodeId>,
    lock: &str,
    id: &NodeId,
) -> bool {
    let Some(node) = graph.get_node(id) else {
        return false;
    };
    if seeds.contains(id)
        || seeds.iter().any(|seed| {
            graph
                .get_node(seed)
                .is_some_and(|s| s.file_path == node.file_path)
        })
    {
        return true;
    }
    match hmvc_app_prefix(&node.file_path) {
        Some(prefix) => prefix == lock,
        None => true,
    }
}

fn keep_schema_packet_file(
    graph: &NeuralProjectGraph,
    seeds: &HashSet<NodeId>,
    prompt: &str,
    id: &NodeId,
) -> bool {
    let Some(node) = graph.get_node(id) else {
        return false;
    };
    if seeds.contains(id)
        || seeds.iter().any(|seed| {
            graph
                .get_node(seed)
                .is_some_and(|s| s.file_path == node.file_path)
        })
    {
        return true;
    }
    if !is_schema_path(&node.file_path) {
        return true;
    }
    prompt_targets_database(prompt)
}

fn locked_seed_hmvc_prefix(graph: &NeuralProjectGraph, seeds: &HashSet<NodeId>) -> Option<String> {
    let mut prefixes = HashSet::new();
    for id in seeds {
        if let Some(prefix) = graph
            .get_node(id)
            .and_then(|node| hmvc_app_prefix(&node.file_path))
        {
            prefixes.insert(prefix);
        }
    }
    if prefixes.len() == 1 {
        prefixes.into_iter().next()
    } else {
        None
    }
}

/// When a compound task names a second topic that identifier extraction skipped
/// (lowercase "router permission guard"), try those nouns as seeds. A cluster
/// with zero hits is recorded as a miss so coverage cannot claim no_recorded_gap.
fn seed_uncovered_clusters(
    graph: &NeuralProjectGraph,
    signature: &TaskSignature,
    seed_resolutions: &mut Vec<SeedResolution>,
    seed_energies: &mut HashMap<NodeId, f32>,
    seed_reasons: &mut HashMap<NodeId, String>,
) {
    for cluster in split_task_clusters(&signature.raw_prompt) {
        let cluster_lower = cluster.to_lowercase();
        let covered = seed_resolutions
            .iter()
            .any(|s| s.resolved_id.is_some() && cluster_lower.contains(&s.query.to_lowercase()))
            || signature
                .file_hints
                .iter()
                .any(|hint| cluster_lower.contains(&hint.to_lowercase()));
        if covered {
            continue;
        }
        let nouns = extract_cluster_nouns(&cluster);
        let mut cluster_hit = false;
        for noun in &nouns {
            if seed_resolutions.iter().any(|s| s.query == *noun) {
                if seed_resolutions
                    .iter()
                    .any(|s| s.query == *noun && s.resolved_id.is_some())
                {
                    cluster_hit = true;
                }
                continue;
            }
            let hits = resolve_cluster_noun_seeds(graph, noun, &nouns);
            if hits.is_empty() {
                continue;
            }
            cluster_hit = true;
            for (id, conf) in hits {
                let energy = 0.85;
                seed_energies
                    .entry(id.clone())
                    .and_modify(|e| *e = (*e).max(energy))
                    .or_insert(energy);
                seed_reasons
                    .entry(id.clone())
                    .or_insert_with(|| format!("cluster:{noun}"));
                seed_resolutions.push(SeedResolution {
                    query: noun.clone(),
                    resolved_id: Some(id),
                    confidence: conf,
                });
            }
        }
        if !cluster_hit {
            let miss = nouns
                .first()
                .cloned()
                .or_else(|| {
                    cluster
                        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                        .map(|t| t.to_string())
                        .find(|t| t.len() >= 5)
                })
                .unwrap_or_else(|| cluster.chars().take(32).collect());
            if !miss.is_empty() && !seed_resolutions.iter().any(|s| s.query == miss) {
                seed_resolutions.push(SeedResolution {
                    query: miss,
                    resolved_id: None,
                    confidence: 0.0,
                });
            }
        }
    }
}

/// Resolve a lowercase cluster noun to the files that actually answer it.
/// Sibling nouns in the same clause (`guard`, `router`) outrank a Vue
/// `directive/permission` UI helper that merely path-echoes `permission`.
fn resolve_cluster_noun_seeds(
    graph: &NeuralProjectGraph,
    noun: &str,
    sibling_nouns: &[String],
) -> Vec<(NodeId, f32)> {
    let noun_l = noun.to_lowercase();
    let cluster_l = sibling_nouns
        .iter()
        .map(|s| s.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    let hits = graph.search_symbols(noun, 16);
    let mut by_file: HashMap<String, (f32, NodeId)> = HashMap::new();
    for hit in hits {
        let Some(node) = graph.get_node(&hit.id) else {
            continue;
        };
        let path = node.file_path.to_string_lossy().replace('\\', "/");
        if is_noise_path(Path::new(&path)) {
            continue;
        }
        let path_l = path.to_lowercase();
        let stem = Path::new(&path_l)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let mut score = hit.score;
        if stem == noun_l {
            score += 24.0;
        }
        if is_template_path(&path_l) && stem == noun_l {
            score += 50.0;
        }
        if node.name.eq_ignore_ascii_case(noun) && stem != noun_l {
            score -= 36.0;
        }
        let hay = format!("{} {path_l}", node.name.to_lowercase());
        for sib in sibling_nouns {
            let sib_l = sib.to_lowercase();
            if sib_l != noun_l && hay.contains(&sib_l) {
                score += 20.0;
            }
        }
        if (path_l.contains("/directive/") || path_l.contains("/directives/"))
            && !cluster_l.contains("directive")
        {
            score -= 40.0;
        }
        if path_l.contains("/clipboard") && !cluster_l.contains("clipboard") {
            score -= 50.0;
        }
        if (path_l.contains("/profile/") || path_l.contains("usercard"))
            && !cluster_l.contains("profile")
            && !cluster_l.contains("card")
        {
            score -= 30.0;
        }
        let entry = by_file.entry(path_l).or_insert((f32::MIN, hit.id.clone()));
        if score > entry.0 {
            *entry = (score, hit.id);
        }
    }
    let mut ranked: Vec<(f32, NodeId)> = by_file.into_values().collect();
    ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let Some(&(best, _)) = ranked.first() else {
        return Vec::new();
    };
    if best < 50.0 {
        return Vec::new();
    }
    ranked
        .into_iter()
        .filter(|(score, _)| *score >= best - 28.0 && *score >= 50.0)
        .take(3)
        .map(|(score, id)| (id, (score / 200.0).clamp(0.55, 0.95)))
        .collect()
}

fn is_template_path(path_l: &str) -> bool {
    path_l.contains("/theme/")
        || path_l.contains("/templates/")
        || path_l.contains("/views/")
        || path_l.ends_with(".twig")
        || path_l.ends_with(".blade.php")
        || path_l.ends_with(".jinja")
        || path_l.ends_with(".hbs")
}

fn prefer_search_seed(
    graph: &NeuralProjectGraph,
    query: &str,
    ranked_id: NodeId,
    ranked_confidence: EdgeConfidence,
    prompt: &str,
) -> NodeId {
    let hits = graph.search_symbols(query, 8);
    if prompt_targets_types(prompt) {
        if let Some(hit) = hits.iter().find(|hit| {
            hit.node_type == NodeType::Symbol
                && hit.name.eq_ignore_ascii_case(query)
                && seed_path_allowed(graph, &hit.id, prompt)
        }) {
            return hit.id.clone();
        }
    }
    let Some(hit) = hits
        .into_iter()
        .find(|hit| hit.score >= 90.0 && seed_path_allowed(graph, &hit.id, prompt))
    else {
        return ranked_id;
    };
    if hit.id == ranked_id {
        return ranked_id;
    }
    if matches!(hit.node_type, NodeType::File) {
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

fn unique_file_tokens<'a, I>(items: I) -> usize
where
    I: Iterator<Item = &'a MaterializedNode>,
{
    let mut by_path: HashMap<String, (bool, usize)> = HashMap::new();
    for item in items {
        let path = item.node.file_path.to_string_lossy().replace('\\', "/");
        let is_file = item.node.node_type == NodeType::File;
        let cost = item.node.token_cost;
        match by_path.get_mut(&path) {
            Some((had_file, stored)) => {
                if is_file {
                    *had_file = true;
                    *stored = cost;
                } else if !*had_file {
                    *stored = (*stored).max(cost);
                }
            }
            None => {
                by_path.insert(path, (is_file, cost));
            }
        }
    }
    by_path.values().map(|(_, cost)| *cost).sum()
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
                owner: n.parent,
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
    fold_ids: &[FoldedIntron],
    unresolved: &[neuromesh_core::UnresolvedRef],
) -> Vec<NextAction> {
    let mut actions = Vec::new();
    let needs_search = coverage.claim == "partial" || coverage.claim == "no_seed_resolved";
    if needs_search {
        let why = if coverage.claim == "no_seed_resolved" {
            "no seed resolved — Grep this identifier; do not trust an empty or utility packet"
        } else {
            "coverage is partial — Grep/search this missed seed"
        };
        for missed in &coverage.seeds_missed {
            actions.push(NextAction {
                tool: "neuromesh_search_symbols".into(),
                query: missed.clone(),
                why: why.into(),
            });
        }
        for gap in &coverage.packet_gaps {
            actions.push(NextAction {
                tool: "neuromesh_search_symbols".into(),
                query: gap.path.clone(),
                why: format!("packet gap ({}): {}", gap.kind, gap.reason),
            });
        }
    }
    let mut ranked: Vec<&FoldedIntron> = fold_ids.iter().collect();
    ranked.sort_by(|a, b| {
        b.task_score
            .partial_cmp(&a.task_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.symbol_name.cmp(&b.symbol_name))
    });
    let relevant: Vec<&FoldedIntron> = ranked
        .iter()
        .copied()
        .filter(|f| f.task_score >= 8.0)
        .take(3)
        .collect();
    let expand: Vec<&FoldedIntron> = if !relevant.is_empty() {
        relevant
    } else {
        ranked.into_iter().take(1).collect()
    };
    for fold in expand {
        let owner = fold
            .owner
            .as_deref()
            .map(|o| format!("{o}."))
            .unwrap_or_default();
        actions.push(NextAction {
            tool: "neuromesh_expand_fold".into(),
            query: fold.fold_id.clone(),
            why: format!(
                "wake folded {owner}{} — closest intron to the task",
                fold.symbol_name
            ),
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
    if needs_search {
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
    fn null_safe_write_stays_open_and_fold_query_roundtrips() {
        let graph = NeuralProjectGraph::new(ProjectId::new("gson"));
        let adapter = r#"
package com.google.gson;
public class TypeAdapter<T> {
    public void write(JsonWriter out, T value) throws IOException {
        out.value("outer-write");
        out.value(String.valueOf(value));
    }
    public TypeAdapter<T> nullSafe() {
        return new NullSafeTypeAdapter();
    }
    public void unusedHelper() {
        int a = 1;
        int b = 2;
        int c = 3;
        int d = 4;
        int e = a + b + c + d;
    }
    public void writeValue(JsonWriter out, T value) throws IOException {
        out.value(String.valueOf(value));
    }
    private final class NullSafeTypeAdapter extends TypeAdapter<T> {
        @Override
        public void write(JsonWriter out, T value) throws IOException {
            if (value != null) {
                out.nullValue();
            } else {
                TypeAdapter.this.writeValue(out, value);
            }
        }
        @Override
        public T read(JsonReader in) throws IOException {
            if (in.peek() == null) {
                in.nextNull();
                return null;
            }
            return TypeAdapter.this.read(in);
        }
    }
}
"#;
        let array = r#"
package com.google.gson;
public final class JsonArray {
    public JsonArray deepCopy() {
        JsonArray result = new JsonArray();
        result.add("a");
        result.add("b");
        result.add("c");
        result.add("d");
        return result;
    }
    public void set(int index, JsonElement element) {
        int a = index;
        int b = a + 1;
        int c = b + 1;
        elements.set(c, element);
    }
}
"#;
        let mut adapter_file = indexed("gson/src/main/java/com/google/gson/TypeAdapter.java");
        adapter_file.language = SourceLanguage::Java;
        adapter_file.blake3_hash = "adapter".into();
        let mut array_file = indexed("gson/src/main/java/com/google/gson/JsonArray.java");
        array_file.language = SourceLanguage::Java;
        array_file.blake3_hash = "array".into();
        graph.ingest_file(
            &adapter_file,
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("TypeAdapter.java"),
                adapter,
                SourceLanguage::Java,
            ),
            Some(adapter),
        );
        graph.ingest_file(
            &array_file,
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("JsonArray.java"),
                array,
                SourceLanguage::Java,
            ),
            Some(array),
        );
        graph.finalize_links();

        let writes: Vec<_> = graph
            .find_nodes_by_name("write")
            .into_iter()
            .filter(|n| {
                n.node_type == NodeType::Function
                    && n.name == "write"
                    && n.file_path
                        .to_string_lossy()
                        .replace('\\', "/")
                        .ends_with("TypeAdapter.java")
            })
            .collect();
        assert_eq!(
            writes.len(),
            2,
            "TypeAdapter.write and NullSafeTypeAdapter.write must be distinct: {:?}",
            writes
                .iter()
                .map(|n| (n.id.as_str().to_string(), n.parent.clone()))
                .collect::<Vec<_>>()
        );
        assert_ne!(writes[0].id, writes[1].id);

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry.clone());
        let signature = TaskSignatureExtractor::extract(
            "I registered a custom TypeAdapter for my Point class using builder.registerTypeAdapter(Point.class, new PointAdapter().nullSafe()) exactly like the Gson javadoc example, but now every non-null Point field in my objects is being serialized as if it were null and dropped entirely from the JSON output. This started after I added .nullSafe(). Where does nullSafe() wrapping live and what could cause non-null values to be treated as null during serialization?",
        );
        let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
        let adapter_node = view
            .active_nodes
            .iter()
            .find(|n| {
                n.node.node_type == NodeType::File
                    && n.node
                        .file_path
                        .to_string_lossy()
                        .replace('\\', "/")
                        .ends_with("TypeAdapter.java")
            })
            .expect("TypeAdapter.java in packet");
        let skeleton = adapter_node.node.content.as_deref().unwrap_or("");
        assert!(
            skeleton.contains("out.nullValue()"),
            "buggy NullSafeTypeAdapter.write body must be visible: {skeleton}"
        );
        assert!(
            !skeleton.contains("int e = a + b + c + d"),
            "unrelated helper body must not ship: {skeleton}"
        );
        assert!(
            skeleton.len() < adapter.len(),
            "windowed skeleton {} should be thinner than the raw file {}",
            skeleton.len(),
            adapter.len()
        );
        if !view.fold_ids.is_empty() {
            let engine = crate::expansion::ExpansionEngine::new(registry);
            let printed = view.fold_ids[0].clone();
            let prefix = printed
                .rsplit_once('_')
                .map(|(head, _)| head.to_string())
                .unwrap_or_else(|| printed.clone());
            let expanded = engine
                .expand_fold(&printed)
                .expect("exact fold_id from the packet must resolve");
            assert!(!expanded.original_body.is_empty());
            let via_prefix = engine
                .expand_fold(&prefix)
                .expect("prefix of the printed fold_id must still resolve");
            assert_eq!(via_prefix.fold_id, expanded.fold_id);
            let via_query = engine.expand_fold(&format!(
                "/* [neuromesh:fold:{printed} | 6 lines folded | @Override] */"
            ));
            assert!(
                via_query.is_some(),
                "marker text from the packet must resolve"
            );
        }
    }

    fn bulky_fn(name: &str, marker: &str, lines: usize) -> String {
        let mut src = format!("pub fn {name}() {{\n    let {marker} = 1;\n");
        for _ in 0..lines {
            src.push_str("    let x = 1;\n");
        }
        src.push_str(&format!("    {marker}\n}}\n"));
        src
    }

    #[test]
    fn packet_cap_shrinks_seed_exons_not_the_top_method() {
        let graph = NeuralProjectGraph::new(ProjectId::new("cap"));
        let src = format!(
            "{}\n{}\n{}\n{}",
            bulky_fn("keepHot", "marker_keep_hot", 800),
            bulky_fn("otherA", "marker_other_a", 800),
            bulky_fn("otherB", "marker_other_b", 800),
            bulky_fn("otherC", "marker_other_c", 800),
        );
        graph.ingest_file(
            &indexed("src/keep_hot.rs"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("keep_hot.rs"),
                &src,
                SourceLanguage::Rust,
            ),
            Some(&src),
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let signature = TaskSignatureExtractor::extract(
            "How do keepHot, otherA, otherB, and otherC compute their values?",
        );
        let view = activator.activate(&graph, &signature, OptimizationMode::Balanced);
        let packet = view
            .active_nodes
            .iter()
            .find(|n| n.node.node_type == NodeType::File)
            .expect("seed file stays in the packet");
        let skeleton = packet.node.content.as_deref().unwrap_or("");
        assert!(
            skeleton.contains("marker_keep_hot"),
            "top-scored keepHot must stay open: {skeleton}"
        );
        assert!(
            !skeleton.contains("marker_other_c"),
            "reducing K must fold the lowest seed exon, not the top method: {skeleton}"
        );
        assert!(
            view.active_tokens <= packet_cap(OptimizationMode::Balanced),
            "packet {} exceeded balanced cap {}",
            view.active_tokens,
            packet_cap(OptimizationMode::Balanced)
        );
        assert!(!packet.folded_symbols.iter().any(|s| s == "keepHot"));
        assert!(packet.folded_symbols.iter().any(|s| s == "otherC"));
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
            Some("no_seed_resolved")
        );
        assert!(
            !miss
                .active_nodes
                .iter()
                .any(|n| n.node.node_type == neuromesh_core::NodeType::File),
            "missed seed must not ship a utility file: {:?}",
            miss.active_nodes
                .iter()
                .map(|n| n.node.file_path.clone())
                .collect::<Vec<_>>()
        );
        assert!(
            miss.next_actions
                .iter()
                .any(|a| a.tool == "neuromesh_search_symbols"),
            "Grep only when partial: {:?}",
            miss.next_actions
        );
    }

    #[test]
    fn exact_class_seed_beats_handle_utility_noise() {
        let graph = NeuralProjectGraph::new(ProjectId::new("symfony"));
        for i in 0..24 {
            let src = format!(
                "<?php\ninterface AccessDeniedHandlerInterface{i} {{\n    public function handle($request);\n}}\n"
            );
            let path = format!("src/AccessDeniedHandlerInterface{i}.php");
            graph.ingest_file(
                &indexed(&path),
                &CodeIntelligenceEngine::analyze(&PathBuf::from(&path), &src, SourceLanguage::PHP),
                Some(&src),
            );
        }
        let kernel = "<?php\nclass HttpKernel {\n    public function handle($request) { return $request; }\n}\n";
        graph.ingest_file(
            &indexed("src/HttpKernel.php"),
            &CodeIntelligenceEngine::analyze(
                &PathBuf::from("src/HttpKernel.php"),
                kernel,
                SourceLanguage::PHP,
            ),
            Some(kernel),
        );
        graph.finalize_links();
        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let view = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract(
                "how does HttpKernel handle a request and produce a response",
            ),
            OptimizationMode::Balanced,
        );
        let coverage = view.coverage.as_ref().expect("coverage");
        assert!(
            coverage.seeds_hit.iter().any(|s| s == "HttpKernel"),
            "HttpKernel must resolve, got {coverage:?}"
        );
        assert_ne!(coverage.claim, "no_seed_resolved");
        assert!(
            view.active_nodes.iter().any(|n| {
                n.node.file_path.to_string_lossy().replace('\\', "/") == "src/HttpKernel.php"
            }),
            "packet must include HttpKernel.php, got {:?}",
            view.active_nodes
                .iter()
                .map(|n| n.node.file_path.clone())
                .collect::<Vec<_>>()
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

    fn ingest_js(graph: &NeuralProjectGraph, rel: &str, src: &str) {
        ingest_lang(graph, rel, src, SourceLanguage::JavaScript);
    }

    fn ingest_vue(graph: &NeuralProjectGraph, rel: &str, src: &str) {
        ingest_lang(graph, rel, src, SourceLanguage::Vue);
    }

    fn ingest_lang(graph: &NeuralProjectGraph, rel: &str, src: &str, language: SourceLanguage) {
        graph.ingest_file(
            &IndexedFile {
                project_id: ProjectId::new("admin"),
                relative_path: PathBuf::from(rel),
                full_path: PathBuf::from(rel),
                blake3_hash: rel.to_string(),
                byte_size: src.len() as u64,
                token_count: 80,
                language,
                last_modified: chrono::Utc::now(),
            },
            &CodeIntelligenceEngine::analyze(&PathBuf::from(rel), src, language),
            Some(src),
        );
    }

    fn packet_paths(view: &ContextView) -> Vec<String> {
        view.active_nodes
            .iter()
            .filter(|n| n.node.node_type == NodeType::File)
            .map(|n| n.node.file_path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn compound_task_seeds_each_cluster_not_just_the_strongest() {
        let graph = NeuralProjectGraph::new(ProjectId::new("admin"));
        ingest_js(
            &graph,
            "src/store/user.js",
            r#"
export function login(userInfo) {
  return request({ url: "/login", method: "post", data: userInfo });
}
export function getInfo() {
  return request({ url: "/info", method: "get" });
}
export function logout() {
  return request({ url: "/logout", method: "post" });
}
"#,
        );
        ingest_js(
            &graph,
            "src/permission.js",
            r#"
export function registerPermissionGuard(router) {
  router.beforeEach(async (to, from, next) => {
    const roles = store.getters.roles;
    if (hasPermission(roles, to.meta.roles)) {
      next();
    }
  });
}
"#,
        );
        ingest_js(
            &graph,
            "src/store/modules/permission.js",
            r#"
export function hasPermission(roles, routeRoles) {
  if (!routeRoles || routeRoles.length === 0) {
    return true;
  }
  return roles.some((role) => routeRoles.includes(role));
}
export function generateRoutes(roles) {
  return filterAsyncRoutes(asyncRoutes, roles);
}
"#,
        );
        ingest_js(
            &graph,
            "src/directive/permission/permission.js",
            r#"
export default {
  inserted(el, binding) {
    checkPermission(el, binding);
  }
}
function checkPermission(el, binding) {
  const roles = store.getters.roles;
  const value = binding.value;
  return roles.some((role) => value.includes(role));
}
"#,
        );
        ingest_js(
            &graph,
            "src/directive/clipboard.js",
            r#"
export function clipboard(el, binding) {
  const text = String(binding.value);
  el.setAttribute("data-clipboard", text);
  return text;
}
"#,
        );
        ingest_vue(
            &graph,
            "src/views/profile/components/UserCard.vue",
            r#"
<template>
  <div class="user-card">{{ name }}</div>
</template>
<script>
export default {
  name: "UserCard",
  props: { name: { type: String, default: "" } }
}
</script>
"#,
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let view = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract(
                "how does the user login and logout flow work, including the login action, getInfo action, and how the router permission guard checks roles before each route",
            ),
            OptimizationMode::Balanced,
        );
        let files = packet_paths(&view);
        assert!(
            files.iter().any(|p| p.ends_with("src/store/user.js")),
            "login cluster must seed user.js, files={files:?} seeds={:?}",
            view.seeds
        );
        assert!(
            files.iter().any(|p| p.ends_with("src/permission.js")),
            "guard cluster must seed the router guard, files={files:?} seeds={:?}",
            view.seeds
        );
        assert!(
            files
                .iter()
                .any(|p| p.ends_with("src/store/modules/permission.js")),
            "guard cluster must seed hasPermission/generateRoutes, files={files:?} seeds={:?}",
            view.seeds
        );
        assert!(
            !files.iter().any(|p| p.contains("clipboard")),
            "clipboard decoy must stay out, files={files:?}"
        );
        assert!(
            !files.iter().any(|p| p.contains("UserCard")),
            "profile decoy must stay out, files={files:?}"
        );
        let coverage = view.coverage.as_ref().expect("coverage");
        assert!(
            coverage
                .seeds_hit
                .iter()
                .any(|s| s == "getInfo" || s == "user"),
            "login half must still hit, coverage={coverage:?}"
        );
        assert!(
            coverage
                .seeds_hit
                .iter()
                .any(|s| s.eq_ignore_ascii_case("permission")),
            "guard half must be a seed hit, coverage={coverage:?}"
        );
        assert_ne!(coverage.claim, "no_seed_resolved");
    }

    fn ingest_schema_collision(graph: &NeuralProjectGraph) {
        ingest_ts(
            graph,
            "packages/schema/src/core/parse.ts",
            r#"
export type Issue = { path: (string | number)[]; message: string };

export function parse(schema: object, data: unknown) {
  const result = _parse(schema, data, []);
  if (result.issues.length > 0) {
    throw result;
  }
  return result.value;
}

export function safeParse(schema: object, data: unknown) {
  return _parse(schema, data, []);
}

function _parse(schema: object, data: unknown, path: (string | number)[]) {
  const issues: Issue[] = [];
  if (typeof data !== "object" || data === null) {
    issues.push({ path, message: "invalid_type" });
  }
  return { value: data, issues };
}
"#,
        );
        ingest_ts(
            graph,
            "packages/schema/src/core/core.ts",
            r#"
export type ZodType<T = unknown> = { _output: T; _input: T };
export type output<T> = T extends { _output: infer Out } ? Out : T;
export type input<T> = T extends { _input: infer In } ? In : T;
"#,
        );
        ingest_ts(
            graph,
            "packages/schema/src/classic/schemas.ts",
            r#"
export function object(shape: Record<string, unknown>) {
  return { type: "object", shape };
}
"#,
        );
        ingest_ts(
            graph,
            "packages/bench/safeparse.ts",
            r#"
export function safeParse(schema: object, data: unknown) {
  return { success: true, data };
}
export function parseSimpleObject(data: unknown) {
  return typeof data === "object";
}
export function parseNestedObject(data: unknown) {
  return parseSimpleObject(data);
}
export function parseObjectArray(data: unknown) {
  return Array.isArray(data);
}
"#,
        );
        ingest_ts(
            graph,
            "packages/schema/src/locales/fa.ts",
            r#"
export function localeError(issue: { path: unknown[]; code: string }) {
  if (issue.code === "invalid_type") {
    return "validation error at path";
  }
  return "invalid";
}
export function invalidTypeError() {
  return "invalid type";
}
"#,
        );
        ingest_ts(
            graph,
            "packages/schema/src/v3/types.ts",
            r#"
export class ZodError extends Error {
  path: (string | number)[] = [];
}
export function parse(schema: object, data: unknown) {
  return data;
}
export function safeParse(schema: object, data: unknown) {
  return { success: true, data };
}
"#,
        );
        ingest_ts(
            graph,
            "packages/schema/src/v4/core/to-json-schema.ts",
            r#"
export function toJsonSchema(schema: object) {
  return { type: "object", schema };
}
export function parseJsonSchema(schema: object) {
  return toJsonSchema(schema);
}
"#,
        );
        graph.finalize_links();
    }

    #[test]
    fn seed_prefers_core_parse_over_bench_and_locale_decoys() {
        let graph = NeuralProjectGraph::new(ProjectId::new("shop"));
        ingest_schema_collision(&graph);
        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);

        let natural = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract(
                "how does z.object schema validate an object and how does parse() report validation errors with a path to the invalid field",
            ),
            OptimizationMode::Balanced,
        );
        let files = packet_paths(&natural);
        assert!(
            files
                .iter()
                .any(|p| p.ends_with("packages/schema/src/core/parse.ts")),
            "natural parse question must seed core/parse.ts, files={files:?} seeds={:?}",
            natural.seeds
        );
        assert!(
            !files.iter().any(|p| p.contains("/bench/")),
            "bench decoys must stay out, files={files:?}"
        );
        assert!(
            !files.iter().any(|p| p.contains("/locales/")),
            "locale catalogs must stay out, files={files:?}"
        );
        assert!(
            !files.iter().any(|p| p.contains("/v3/")),
            "v3 legacy must stay out, files={files:?}"
        );
        assert!(
            !files.iter().any(|p| p.contains("to-json-schema")),
            "json-schema conversion must stay out, files={files:?}"
        );

        let gerund = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract("how does parsing work in zod"),
            OptimizationMode::Balanced,
        );
        let gerund_files = packet_paths(&gerund);
        assert!(
            !gerund_files.is_empty(),
            "gerund phrasing must not return an empty packet, seeds={:?}",
            gerund.seeds
        );
        assert!(
            gerund_files
                .iter()
                .any(|p| p.ends_with("packages/schema/src/core/parse.ts")),
            "parsing → parse must seed core/parse.ts, files={gerund_files:?} seeds={:?}",
            gerund.seeds
        );

        let named = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract("where is the safeParse function implemented"),
            OptimizationMode::Balanced,
        );
        let named_files = packet_paths(&named);
        assert!(
            named_files
                .iter()
                .any(|p| p.ends_with("packages/schema/src/core/parse.ts")),
            "safeParse must resolve to core/parse.ts, files={named_files:?} seeds={:?}",
            named.seeds
        );
        assert!(
            !named_files
                .iter()
                .any(|p| p.ends_with("packages/bench/safeparse.ts")),
            "bench/safeparse.ts must not steal the safeParse seed, files={named_files:?}"
        );
    }

    #[test]
    fn seed_prefers_core_type_alias_for_z_infer() {
        let graph = NeuralProjectGraph::new(ProjectId::new("shop"));
        ingest_schema_collision(&graph);
        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let view = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract("how do ZodType generics flow through z.infer"),
            OptimizationMode::Balanced,
        );
        let files = packet_paths(&view);
        assert!(
            files
                .iter()
                .any(|p| p.ends_with("packages/schema/src/core/core.ts")),
            "z.infer must seed core.ts type aliases, files={files:?} seeds={:?}",
            view.seeds
        );
        assert!(
            !files.iter().any(|p| p.contains("classic/schemas")),
            "classic schemas must not steal the infer seed, files={files:?}"
        );
        assert!(
            !files.iter().any(|p| p.contains("/bench/")),
            "bench decoys must stay out, files={files:?}"
        );
    }

    #[test]
    fn uncovered_compound_cluster_is_partial_not_no_recorded_gap() {
        let graph = NeuralProjectGraph::new(ProjectId::new("admin"));
        ingest_js(
            &graph,
            "src/store/user.js",
            r#"
export function login(userInfo) {
  return request({ url: "/login", method: "post", data: userInfo });
}
export function getInfo() {
  return request({ url: "/info", method: "get" });
}
"#,
        );
        ingest_js(
            &graph,
            "src/directive/clipboard.js",
            r#"
export function clipboard(el, binding) {
  return String(binding.value);
}
"#,
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let view = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract(
                "how does the user login work, including getInfo, and how the router permission guard checks roles before each route",
            ),
            OptimizationMode::Balanced,
        );
        let coverage = view.coverage.as_ref().expect("coverage");
        assert!(
            !coverage.seeds_hit.is_empty(),
            "login half must still resolve, coverage={coverage:?}"
        );
        assert!(
            !coverage.seeds_missed.is_empty(),
            "named guard cluster with zero hits must be a miss, coverage={coverage:?}"
        );
        assert_eq!(
            coverage.claim, "partial",
            "half-resolved compound task must not claim no_recorded_gap, coverage={coverage:?}"
        );
        assert!(
            view.next_actions
                .iter()
                .any(|a| a.tool == "neuromesh_search_symbols"),
            "partial coverage must offer Grep, next={:?}",
            view.next_actions
        );
    }

    #[test]
    fn style_task_seeds_tokens_and_product_card() {
        let graph = NeuralProjectGraph::new(ProjectId::new("shop"));
        ingest_lang(
            &graph,
            "src/styles/_tokens.scss",
            "$radius-sm: 8px;\n$shadow-lift: 0 4px 12px rgba(0,0,0,.12);\n",
            SourceLanguage::SCSS,
        );
        ingest_lang(
            &graph,
            "src/styles/_mixins.scss",
            "@mixin card-base { border-radius: $radius-sm; }\n",
            SourceLanguage::SCSS,
        );
        ingest_vue(
            &graph,
            "src/components/ProductCard.vue",
            r#"<script setup>
defineProps({ product: Object })
</script>
<template><article class="product-card">{{ product.name }}</article></template>
<style lang="scss" scoped>
@use '../styles/tokens.scss' as *;
.product-card { border-radius: $radius-sm; }
</style>
"#,
        );
        ingest_js(
            &graph,
            "src/stores/cart.js",
            "export function applyPromo(code) { return code }\n",
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let view = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract(
                "Apply hover-lift and focus-within styles to ProductCard using SCSS tokens and mixins",
            ),
            OptimizationMode::Balanced,
        );
        let files = packet_paths(&view);
        assert!(
            files.iter().any(|p| p.contains("ProductCard")),
            "ProductCard must be in packet, files={files:?}"
        );
        assert!(
            files
                .iter()
                .any(|p| p.contains("tokens") || p.contains("mixins")),
            "style task must include tokens/mixins, files={files:?}"
        );
    }

    #[test]
    fn dead_code_task_flags_missing_callers_as_packet_gap() {
        let graph = NeuralProjectGraph::new(ProjectId::new("shop"));
        ingest_js(
            &graph,
            "src/stores/ui.js",
            r#"
export function goCart() { return 'cart' }
export function goCheckout() { return 'checkout' }
"#,
        );
        ingest_vue(
            &graph,
            "src/App.vue",
            r#"<script setup>
import { goCart } from './stores/ui.js'
goCart()
</script>
<template><div /></template>
"#,
        );
        graph.finalize_links();

        let registry = Arc::new(ReversibleContextRegistry::new());
        let activator = ContextActivator::new(registry);
        let view = activator.activate(
            &graph,
            &TaskSignatureExtractor::extract(
                "Find unused goCart in ui store and list all references across the project",
            ),
            OptimizationMode::Balanced,
        );
        let coverage = view.coverage.as_ref().expect("coverage");
        assert!(
            view.structural_evidence
                .iter()
                .any(|e| e.symbol == "goCart"),
            "structural evidence must include goCart, evidence={:?}",
            view.structural_evidence
        );
        if !coverage.packet_gaps.is_empty() {
            assert_eq!(
                coverage.claim, "partial",
                "missing caller files must downgrade coverage, coverage={coverage:?}"
            );
        }
    }

    #[test]
    fn feedback_increases_node_learning_bonus() {
        let graph = NeuralProjectGraph::new(ProjectId::new("learn"));
        ingest_vue(
            &graph,
            "src/views/CheckoutView.vue",
            r#"<script setup>
import { useCartStore } from '../stores/cart'
const cart = useCartStore()
function setQty(id, q) { cart.setQty(id, q) }
</script>
<template><div /></template>
"#,
        );
        ingest_js(&graph, "src/stores/cart.js", "export function setQty() {}");
        graph.finalize_links();

        let before = graph
            .node_learning_profile("CheckoutView")
            .map(|p| p.learning_bonus)
            .unwrap_or(0.0);
        for _ in 0..60 {
            if let Some(node) = graph.resolve_feedback_node("CheckoutView") {
                graph.reinforce_node_access(&node.id, true);
            }
        }
        let after = graph
            .node_learning_profile("CheckoutView")
            .expect("profile")
            .learning_bonus;
        assert!(
            after > before,
            "learning_bonus should increase after feedback: before={before} after={after}"
        );
    }
}
