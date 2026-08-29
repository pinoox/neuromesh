use crate::packet_cache::{
    FileSelectionMeta, PacketBudgetSnapshot, PacketDetailCache, PacketDetails,
};
use neuromesh_context::{FoldDescriptor, ReversibleContextRegistry};
use neuromesh_core::{ContextView, NodeType, TaskSignature, TokenCounter};
use neuromesh_router::QualityGateDecision;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;

pub const MINIMAL_METADATA_BUDGET: usize = 256;
pub const STANDARD_METADATA_BUDGET: usize = 750;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseDetail {
    Minimal,
    Standard,
    Diagnostic,
}

impl ResponseDetail {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw.map(str::trim).unwrap_or("") {
            "standard" => Self::Standard,
            "diagnostic" => Self::Diagnostic,
            _ => Self::Minimal,
        }
    }

    fn metadata_budget(self) -> Option<usize> {
        match self {
            Self::Minimal => Some(MINIMAL_METADATA_BUDGET),
            Self::Standard => Some(STANDARD_METADATA_BUDGET),
            Self::Diagnostic => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub code: String,
    pub tokens: usize,
    pub why: Option<String>,
    pub sidecar: bool,
    pub line_range: Option<std::ops::Range<usize>>,
    pub folded_symbols: Vec<String>,
    pub folds: Vec<FoldDescriptor>,
}

#[derive(Serialize)]
struct TokenCounts {
    selected: usize,
    packet: usize,
}

#[derive(Serialize)]
struct MinimalNext {
    tool: String,
    queries: Vec<String>,
}

#[derive(Serialize)]
struct MinimalFile {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    why: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    sidecar: bool,
    code: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    folds: Vec<Value>,
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Serialize)]
struct MinimalContextResponse {
    packet_id: String,
    coverage: String,
    tokens: TokenCounts,
    files: Vec<MinimalFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<MinimalNext>,
}

pub fn collect_file_entries(
    view: &ContextView,
    registry: &ReversibleContextRegistry,
) -> Vec<FileEntry> {
    let mut entries: Vec<FileEntry> = view
        .active_nodes
        .iter()
        .filter(|n| n.node.node_type == NodeType::File)
        .map(|n| {
            let path = n.node.file_path.to_string_lossy().replace('\\', "/");
            let folds = folds_for_path(registry, &n.node.file_path, &view.fold_ids);
            FileEntry {
                path,
                code: n.node.content.clone().unwrap_or_default(),
                tokens: n.node.token_cost,
                why: n.expansion_reason.clone(),
                sidecar: n.sidecar,
                line_range: n.node.line_range.clone(),
                folded_symbols: n.folded_symbols.clone(),
                folds,
            }
        })
        .collect();
    if let Some(header) = view.packet_header.as_ref() {
        if let Some(first) = entries.first_mut() {
            if !first.code.starts_with("@nm:") {
                first.code = format!("{header}\n{}", first.code);
            }
        }
    }
    entries
}

fn folds_for_path(
    registry: &ReversibleContextRegistry,
    file_path: &Path,
    fold_ids: &[String],
) -> Vec<FoldDescriptor> {
    fold_ids
        .iter()
        .filter_map(|id| {
            let stored = registry.get_fold(id)?;
            if path_eq(&stored.file_path, file_path) {
                Some(FoldDescriptor::from(&stored.fold))
            } else {
                None
            }
        })
        .collect()
}

fn path_eq(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().replace('\\', "/") == b.to_string_lossy().replace('\\', "/")
}

pub fn collect_symbols(view: &ContextView) -> Vec<Value> {
    let mut nodes: Vec<_> = view
        .active_nodes
        .iter()
        .filter(|n| n.node.node_type != NodeType::File)
        .collect();
    nodes.sort_by(|a, b| {
        b.activation_score
            .total_cmp(&a.activation_score)
            .then_with(|| a.node.file_path.cmp(&b.node.file_path))
            .then_with(|| a.node.name.cmp(&b.node.name))
            .then_with(|| {
                a.node
                    .line_range
                    .as_ref()
                    .map(|r| r.start)
                    .cmp(&b.node.line_range.as_ref().map(|r| r.start))
            })
    });
    nodes
        .into_iter()
        .map(|n| {
            json!({
                "name": n.node.name,
                "path": n.node.file_path,
                "signature": n.node.signature,
                "why": n.expansion_reason,
                "kind": n.node.node_type,
                "id": n.node.id,
                "lines": n.node.line_range,
                "score": n.activation_score,
            })
        })
        .collect()
}

pub struct ContextBuild<'a> {
    pub packet_id: String,
    pub signature: &'a TaskSignature,
    pub gate: &'a QualityGateDecision,
    pub view: &'a ContextView,
    pub files: &'a [FileEntry],
    pub symbols: &'a [Value],
    pub workspace_tokens: usize,
    pub selected_raw: usize,
    pub packet_tokens: usize,
    pub vs_workspace: f32,
    pub vs_selected: f32,
    pub elapsed_ms: u64,
    pub index_meta: neuromesh_core::IndexMeta,
}

impl ContextBuild<'_> {
    fn seed_resolution_block(&self) -> Option<Value> {
        self.view.seed_resolution_telemetry.as_ref().map(|t| {
            json!({
                "engine": t.engine,
                "seeds_count": t.seeds_count,
                "monorepo_packages": t.monorepo_packages,
                "latency_ms": t.latency_ms,
            })
        })
    }

    fn task_metadata(&self) -> Value {
        let mut task = json!({
            "intent": self.signature.intent,
            "entity": self.signature.entity,
            "identifiers": self.signature.identifiers,
            "file_hints": self.signature.file_hints,
            "client_keywords": self.signature.client_keywords,
            "client_keywords_used": self.client_keywords_used(),
            "client_expansion": self.signature.client_expansion,
            "client_path_hints": self.signature.client_path_hints,
            "client_entity_types": self.signature.client_entity_types,
            "client_intent": self.signature.client_intent,
            "scenario": self.view.task_scenario,
            "confidence": self.signature.confidence,
        });
        if let Some(seed_resolution) = self.seed_resolution_block() {
            task["seed_resolution"] = seed_resolution;
        }
        if let Some(header) = self.view.packet_header.as_ref() {
            task["packet_header"] = json!(header);
        }
        task
    }

    fn client_keywords_used(&self) -> Vec<String> {
        if self.signature.client_keywords.is_empty() {
            return Vec::new();
        }
        self.view
            .seeds
            .iter()
            .filter(|s| {
                s.resolved_id.is_some()
                    && self
                        .signature
                        .client_keywords
                        .iter()
                        .any(|kw| kw.eq_ignore_ascii_case(&s.query))
            })
            .map(|s| s.query.clone())
            .collect()
    }

    pub fn to_details(&self) -> PacketDetails {
        PacketDetails {
            packet_id: self.packet_id.clone(),
            seeds: self.view.seeds.clone(),
            coverage: self.view.coverage.clone(),
            budget: PacketBudgetSnapshot {
                used: self.view.budget_used,
                cap: self.view.budget_cap,
                mode: self.view.budget_mode.clone(),
                seed_tokens: self.view.budget_seed_tokens,
                fill_used: self.view.budget_fill_used,
                fill_cap: self.view.budget_fill_cap,
                over_budget: self.view.over_budget,
            },
            membrane: self.gate.membrane_state.clone(),
            physarum_used: self.view.physarum_used,
            physarum_ms: self.view.physarum_ms,
            selection_method: self.view.selection_method.clone(),
            rank_candidates: self.view.rank_candidates.clone(),
            unresolved: self.view.unresolved.clone(),
            inactive_hints: self.view.inactive_descriptors.clone(),
            index: self.index_meta.clone(),
            files: self
                .files
                .iter()
                .map(|f| FileSelectionMeta {
                    path: f.path.clone(),
                    why: f.why.clone(),
                    tokens: f.tokens,
                    line_range: f.line_range.clone(),
                    folded_symbols: f.folded_symbols.clone(),
                    folds: f.folds.clone(),
                })
                .collect(),
            symbols: self.symbols.to_vec(),
            fold_ids: self.view.fold_ids.clone(),
            next_actions: self.view.next_actions.clone(),
            tokens_selected: self.selected_raw,
            tokens_packet: self.packet_tokens,
            workspace_tokens: self.workspace_tokens,
            seed_call_coverage: self.view.seed_call_coverage,
            effective_mode: format!("{:?}", self.gate.effective_mode),
            latency_ms: self.elapsed_ms,
            reduction_vs_workspace_pct: format!("{:.1}%", self.vs_workspace),
            reduction_vs_selected_pct: format!("{:.1}%", self.vs_selected),
        }
    }

    pub fn serialize(&self, detail: ResponseDetail) -> Value {
        let mut value = match detail {
            ResponseDetail::Minimal => self.minimal(),
            ResponseDetail::Standard => self.standard(),
            ResponseDetail::Diagnostic => self.diagnostic(),
        };
        if let Some(budget) = detail.metadata_budget() {
            enforce_metadata_budget(&mut value, budget);
        }
        value
    }

    fn coverage_claim(&self) -> &str {
        self.view
            .coverage
            .as_ref()
            .map(|c| c.claim.as_str())
            .unwrap_or("no_recorded_gap")
    }

    fn missing_seeds(&self) -> Vec<String> {
        self.view
            .coverage
            .as_ref()
            .map(|c| c.seeds_missed.clone())
            .unwrap_or_default()
    }

    fn needs_search(&self) -> bool {
        matches!(self.coverage_claim(), "partial" | "no_seed_resolved")
    }

    fn minimal(&self) -> Value {
        let missing = self.missing_seeds();
        let next = if self.needs_search() && !missing.is_empty() {
            Some(MinimalNext {
                tool: "neuromesh_search_symbols".into(),
                queries: missing.clone(),
            })
        } else {
            None
        };
        let files: Vec<MinimalFile> = self
            .files
            .iter()
            .map(|f| MinimalFile {
                path: f.path.clone(),
                why: f.why.clone().filter(|s| !s.is_empty()),
                sidecar: f.sidecar,
                code: f.code.clone(),
                folds: f
                    .folds
                    .iter()
                    .map(|d| serde_json::to_value(d).unwrap_or(Value::Null))
                    .collect(),
            })
            .collect();
        serde_json::to_value(MinimalContextResponse {
            packet_id: self.packet_id.clone(),
            coverage: self.coverage_claim().to_string(),
            tokens: TokenCounts {
                selected: self.selected_raw,
                packet: self.packet_tokens,
            },
            files,
            missing: if self.needs_search() && !missing.is_empty() {
                Some(missing)
            } else {
                None
            },
            next,
        })
        .unwrap_or(Value::Null)
    }

    fn standard(&self) -> Value {
        let mut packet = json!({
            "files": self.files.iter().map(|f| {
                let mut obj = json!({
                    "path": f.path,
                    "skeleton": f.code,
                    "tokens": f.tokens,
                });
                if let Some(why) = f.why.as_ref().filter(|s| !s.is_empty()) {
                    obj["why"] = json!(why);
                }
                if f.sidecar {
                    obj["sidecar"] = json!(true);
                }
                if let Some(range) = &f.line_range {
                    obj["line_range"] = json!(range);
                }
                if !f.folded_symbols.is_empty() {
                    obj["folded_symbols"] = json!(f.folded_symbols);
                }
                if !f.folds.is_empty() {
                    obj["folds"] = json!(f.folds);
                }
                obj
            }).collect::<Vec<_>>(),
            "coverage": self.view.coverage,
            "budget": {
                "used": self.view.budget_used,
                "cap": self.view.budget_cap,
                "mode": self.view.budget_mode,
                "seed_tokens": self.view.budget_seed_tokens,
                "fill_used": self.view.budget_fill_used,
                "fill_cap": self.view.budget_fill_cap,
                "over_budget": self.view.over_budget,
            },
            "workspace_tokens": self.workspace_tokens,
            "selected_raw_tokens": self.selected_raw,
            "active_tokens": self.packet_tokens,
            "reduction_vs_workspace_pct": format!("{:.1}%", self.vs_workspace),
            "reduction_vs_selected_pct": format!("{:.1}%", self.vs_selected),
            "seed_call_coverage": self.view.seed_call_coverage,
        });
        if !self.view.seeds.is_empty() {
            packet["seeds"] = json!(self.view.seeds);
        }
        if !self.symbols.is_empty() {
            packet["symbols"] = json!(self.symbols);
        }
        if !self.view.unresolved.is_empty() {
            packet["unresolved"] = json!(self.view.unresolved);
        }
        if !self.view.fold_ids.is_empty() {
            packet["fold_ids"] = json!(self.view.fold_ids);
        }
        if !self.view.next_actions.is_empty() {
            packet["next_actions"] = json!(self.view.next_actions);
        }
        if !self.view.structural_evidence.is_empty() {
            packet["structural_evidence"] = json!(self.view.structural_evidence);
        }
        if let Some(header) = self.view.packet_header.as_ref() {
            packet["packet_header"] = json!(header);
        }
        json!({
            "packet_id": self.packet_id,
            "task": self.task_metadata(),
            "effective_mode": format!("{:?}", self.gate.effective_mode),
            "latency_ms": self.elapsed_ms,
            "evidence_packet": packet,
        })
    }

    fn diagnostic(&self) -> Value {
        json!({
            "packet_id": self.packet_id,
            "task": self.task_metadata(),
            "membrane_state": self.gate.membrane_state,
            "effective_mode": format!("{:?}", self.gate.effective_mode),
            "latency_ms": self.elapsed_ms,
            "evidence_packet": {
                "index": self.index_meta,
                "seeds": self.view.seeds,
                "files": self.files.iter().map(|f| {
                    json!({
                        "path": f.path,
                        "skeleton": f.code,
                        "tokens": f.tokens,
                        "why": f.why,
                        "sidecar": f.sidecar,
                        "line_range": f.line_range,
                        "folded_symbols": f.folded_symbols,
                        "folds": f.folds.iter().map(|d| d.fold_id.clone()).collect::<Vec<_>>(),
                    })
                }).collect::<Vec<_>>(),
                "symbols": self.symbols,
                "unresolved": self.view.unresolved,
                "coverage": self.view.coverage,
                "fold_ids": self.view.fold_ids,
                "next_actions": self.view.next_actions,
                "budget": {
                    "used": self.view.budget_used,
                    "cap": self.view.budget_cap,
                    "mode": self.view.budget_mode,
                    "seed_tokens": self.view.budget_seed_tokens,
                    "fill_used": self.view.budget_fill_used,
                    "fill_cap": self.view.budget_fill_cap,
                    "over_budget": self.view.over_budget,
                },
                "inactive_hints": self.view.inactive_descriptors,
                "workspace_tokens": self.workspace_tokens,
                "selected_raw_tokens": self.selected_raw,
                "active_tokens": self.packet_tokens,
                "reduction_vs_workspace_pct": format!("{:.1}%", self.vs_workspace),
                "reduction_vs_selected_pct": format!("{:.1}%", self.vs_selected),
                "seed_call_coverage": self.view.seed_call_coverage,
                "physarum_used": self.view.physarum_used,
                "physarum_ms": self.view.physarum_ms,
                "selection_method": self.view.selection_method,
            }
        })
    }
}

pub fn explain_packet(details: &PacketDetails, include: &[String], graph: Option<Value>) -> Value {
    let want = |key: &str| {
        if key == "graph" {
            include.iter().any(|s| s == key)
        } else {
            include.is_empty() || include.iter().any(|s| s == key)
        }
    };
    let mut out = json!({ "packet_id": details.packet_id });
    if want("seeds") {
        out["seeds"] = json!(details.seeds);
        out["coverage"] = json!(details.coverage);
    }
    if want("selection") {
        out["selection"] = json!({
            "method": details.selection_method,
            "files": details.files,
            "symbols": details.symbols,
            "candidates": details.rank_candidates,
            "unresolved": details.unresolved,
            "inactive_hints": details.inactive_hints,
            "fold_ids": details.fold_ids,
            "next_actions": details.next_actions,
            "index": details.index,
        });
    }
    if want("budget") {
        out["budget"] = json!(details.budget);
        out["tokens"] = json!({
            "selected": details.tokens_selected,
            "packet": details.tokens_packet,
            "workspace": details.workspace_tokens,
        });
        out["seed_call_coverage"] = json!(details.seed_call_coverage);
        out["reduction_vs_workspace_pct"] = json!(details.reduction_vs_workspace_pct);
        out["reduction_vs_selected_pct"] = json!(details.reduction_vs_selected_pct);
    }
    if want("physarum") {
        out["physarum"] = json!({
            "used": details.physarum_used,
            "ms": details.physarum_ms,
            "selection_method": details.selection_method,
        });
    }
    if want("membrane") {
        out["membrane"] = json!(details.membrane);
        out["effective_mode"] = json!(details.effective_mode);
    }
    if want("graph") {
        if let Some(stats) = graph {
            out["graph"] = stats;
        }
    }
    out["latency_ms"] = json!(details.latency_ms);
    out
}

pub fn metadata_tokens(value: &Value) -> usize {
    let mut stripped = value.clone();
    strip_code_fields(&mut stripped);
    TokenCounter::count_tokens(&serde_json::to_string(&stripped).unwrap_or_default())
}

fn strip_code_fields(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("code");
            map.remove("skeleton");
            for v in map.values_mut() {
                strip_code_fields(v);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                strip_code_fields(v);
            }
        }
        _ => {}
    }
}

fn enforce_metadata_budget(value: &mut Value, budget: usize) {
    if metadata_tokens(value) <= budget {
        return;
    }
    shrink_folds_to_ids(value);
    if metadata_tokens(value) <= budget {
        return;
    }
    truncate_why(value, 40);
}

fn files_array_mut(value: &mut Value) -> Option<&mut Vec<Value>> {
    if value.get("files").is_some() {
        return value.get_mut("files").and_then(|f| f.as_array_mut());
    }
    value
        .get_mut("evidence_packet")
        .and_then(|ep| ep.get_mut("files"))
        .and_then(|f| f.as_array_mut())
}

fn shrink_folds_to_ids(value: &mut Value) {
    let Some(files) = files_array_mut(value) else {
        return;
    };
    for file in files {
        let Some(folds) = file.get_mut("folds").and_then(|f| f.as_array_mut()) else {
            continue;
        };
        let ids: Vec<Value> = folds
            .iter()
            .filter_map(|f| {
                f.get("fold_id")
                    .and_then(Value::as_str)
                    .map(|s| Value::String(s.to_string()))
                    .or_else(|| f.as_str().map(|s| Value::String(s.to_string())))
            })
            .collect();
        *folds = ids;
    }
}

fn truncate_why(value: &mut Value, max_chars: usize) {
    let Some(files) = files_array_mut(value) else {
        return;
    };
    for file in files {
        if let Some(Value::String(why)) = file.get_mut("why") {
            if why.chars().count() > max_chars {
                let cut: String = why.chars().take(max_chars).collect();
                *why = cut;
            }
        }
    }
}

pub fn fold_descriptors_from_skeleton(
    folds: &[neuromesh_context::FoldedIntron],
) -> Vec<FoldDescriptor> {
    folds.iter().map(FoldDescriptor::from).collect()
}

pub fn cache_and_build(
    cache: &PacketDetailCache,
    project_id: &str,
    build: &ContextBuild<'_>,
    detail: ResponseDetail,
) -> Value {
    cache.insert(project_id, build.to_details());
    build.serialize(detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_detail_defaults_to_minimal() {
        assert_eq!(ResponseDetail::parse(None), ResponseDetail::Minimal);
        assert_eq!(ResponseDetail::parse(Some("")), ResponseDetail::Minimal);
        assert_eq!(
            ResponseDetail::parse(Some("diagnostic")),
            ResponseDetail::Diagnostic
        );
    }

    #[test]
    fn metadata_budget_ignores_code_bodies() {
        let val = json!({
            "packet_id": "ctx_x",
            "coverage": "no_recorded_gap",
            "files": [{
                "path": "a.php",
                "why": "seed",
                "code": "fn huge() { /* ".to_string() + &"x".repeat(4000) + " */ }"
            }]
        });
        let tokens = metadata_tokens(&val);
        assert!(
            tokens < MINIMAL_METADATA_BUDGET,
            "code body must not count toward metadata: {tokens}"
        );
    }

    #[test]
    fn shrink_folds_replaces_descriptors_with_ids() {
        let mut val = json!({
            "files": [{
                "path": "a.rs",
                "code": "fn a() {}",
                "folds": [{
                    "fold_id": "fold_a_1",
                    "symbol": "a",
                    "signature": "fn a()",
                    "start_line": 1,
                    "end_line": 10,
                    "saved_tokens": 40
                }]
            }]
        });
        shrink_folds_to_ids(&mut val);
        assert_eq!(val["files"][0]["folds"][0], "fold_a_1");
    }
}
