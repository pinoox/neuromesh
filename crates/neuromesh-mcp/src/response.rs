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
    /// Path + symbol + line range only — no skeleton bodies.
    Pointer,
    Minimal,
    Standard,
    Diagnostic,
}

impl ResponseDetail {
    pub fn parse(raw: Option<&str>) -> Self {
        if let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) {
            return match s {
                "pointer" | "lean" => Self::Pointer,
                "standard" => Self::Standard,
                "diagnostic" => Self::Diagnostic,
                _ => Self::Minimal,
            };
        }
        // Deployment default (e.g. lean IDE clients): NEUROMESH_RESPONSE_DETAIL=pointer
        match std::env::var("NEUROMESH_RESPONSE_DETAIL")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "pointer" | "lean" => Self::Pointer,
            "standard" => Self::Standard,
            "diagnostic" => Self::Diagnostic,
            _ => Self::Minimal,
        }
    }

    fn metadata_budget(self) -> Option<usize> {
        match self {
            Self::Pointer => Some(128),
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
    /// Ready-to-copy MCP arguments so the agent does not guess the shape.
    #[serde(skip_serializing_if = "Option::is_none")]
    example_args: Option<Value>,
}

#[derive(Serialize)]
struct MinimalFile {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    why: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    sidecar: bool,
    code: String,
    /// Fold ids only — full descriptors live in standard/diagnostic.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    folds: Vec<String>,
    /// How many fold ids were capped off (see `cap_minimal_folds`).
    #[serde(skip_serializing_if = "Option::is_none")]
    folds_omitted: Option<usize>,
}

/// Minimal packets cap fold ids per file: a 200-id list is ~7KB of
/// markers no agent expands one by one (pointer already caps at 6).
/// Full descriptors stay in standard/diagnostic, and `expand_fold`
/// also accepts query/node_id, so discovery is not blocked.
const MINIMAL_FOLDS_PER_FILE: usize = 8;

fn cap_minimal_folds(ids: &[String]) -> (Vec<String>, Option<usize>) {
    if ids.len() <= MINIMAL_FOLDS_PER_FILE {
        return (ids.to_vec(), None);
    }
    (
        ids[..MINIMAL_FOLDS_PER_FILE].to_vec(),
        Some(ids.len() - MINIMAL_FOLDS_PER_FILE),
    )
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// Prefer a real source file over connector/config noise for `agent_hint`.
fn pick_agent_hint_file<'a>(files: &'a [PointerFile], prompt: &str) -> Option<&'a PointerFile> {
    let mut best: Option<(i32, &PointerFile)> = None;
    for f in files {
        let p = f.path.to_ascii_lowercase();
        let mut score = 0i32;
        // Strongly prefer project source over scripts/fixtures/configs.
        if p.starts_with("crates/") || p.starts_with("src/") || p.starts_with("apps/") {
            score += 40;
        }
        if p.contains("/scripts/") || p.ends_with(".jsonc") || p.contains("/editors/") {
            score -= 30;
        }
        if p.contains("/tests/") || p.contains("/fixtures/") {
            score -= 15;
        }
        if f.excerpt.is_some() {
            score += 5;
        }
        if !f.fold_ids.is_empty() {
            score += 8;
        }
        if let Some(range) = &f.line_range {
            score += 3;
            let _ = range;
        }
        // Path stem / symbol overlap with the user prompt.
        let overlap = neuromesh_context::retrieval::path_stem_overlap(&f.path, prompt);
        if overlap > 0.0 {
            score += (overlap * 25.0) as i32;
        }
        for s in &f.symbols {
            if s.len() >= 4
                && prompt
                    .to_ascii_lowercase()
                    .contains(&s.to_ascii_lowercase())
            {
                score += 20;
                break;
            }
        }
        if best.map(|(bs, _)| score > bs).unwrap_or(true) {
            best = Some((score, f));
        }
    }
    best.map(|(_, f)| f)
}

fn truncate_code(code: &str, max_bytes: usize) -> String {
    if code.len() <= max_bytes {
        return code.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !code.is_char_boundary(end) {
        end -= 1;
    }
    let mut out = code[..end].to_string();
    out.push_str("\n/* … truncated — expand_fold or Read for full skeleton */");
    out
}

/// Workspace-relative paths of confidently resolved seed nodes.
fn seed_file_paths(view: &ContextView) -> Vec<String> {
    let mut out = Vec::new();
    for s in &view.seeds {
        if s.confidence < 0.7 {
            continue;
        }
        let Some(id) = s.resolved_id.as_ref() else {
            continue;
        };
        let raw = id.0.as_ref();
        // file:crates/… or sym:crates/…:Type.method
        let rest = raw
            .strip_prefix("file:")
            .or_else(|| raw.strip_prefix("sym:"))
            .unwrap_or(raw);
        let path = rest.rsplit_once(':').map(|(p, _)| p).unwrap_or(rest);
        let path = if path.contains('/') { path } else { rest };
        if path.contains('/') && !out.iter().any(|p: &String| p == path) {
            out.push(path.to_string());
        }
    }
    out
}

/// Bare symbol names from seed resolved ids (`…:Type.method` → `method`).
fn seed_symbol_names(view: &ContextView) -> Vec<String> {
    let mut out = Vec::new();
    for s in &view.seeds {
        if s.confidence < 0.7 {
            continue;
        }
        let Some(id) = s.resolved_id.as_ref() else {
            continue;
        };
        let raw = id.0.as_ref();
        let Some((_, name)) = raw.rsplit_once(':') else {
            continue;
        };
        let sym = name.rsplit('.').next().unwrap_or(name);
        if !sym.is_empty() && !out.iter().any(|x: &String| x == sym) {
            out.push(sym.to_string());
        }
    }
    out
}

/// Keep a window around each named seed symbol so the packet contains the
/// requested function even when the file skeleton is huge.
/// Stops at the next top-level `fn`/`impl` and caps lines/bytes.
fn extract_seed_windows(code: &str, symbols: &[String]) -> String {
    if symbols.is_empty() || code.is_empty() {
        return String::new();
    }
    let lines: Vec<&str> = code.lines().collect();
    let mut keep = vec![false; lines.len()];
    const MAX_LINES: usize = 48;
    const MAX_BYTES: usize = 6_000;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        let is_def = t.starts_with("fn ")
            || t.starts_with("pub fn ")
            || t.starts_with("pub async fn ")
            || t.starts_with("async fn ")
            || t.starts_with("pub(crate) fn ")
            || t.starts_with("pub(super) fn ");
        if !is_def {
            continue;
        }
        if !symbols.iter().any(|sym| {
            t.contains(sym)
                && (t.contains(&format!("fn {sym}"))
                    || t.contains(&format!("fn {sym}("))
                    || t.ends_with(sym)
                    || t.contains(&format!(".{sym}")))
        }) {
            continue;
        }
        let start = i.saturating_sub(1);
        let mut end = start + 1;
        while end < lines.len() && end - start < MAX_LINES {
            let nt = lines[end].trim();
            // next top-level definition closes this function
            if end > i
                && (nt.starts_with("fn ")
                    || nt.starts_with("pub fn ")
                    || nt.starts_with("pub async fn ")
                    || nt.starts_with("async fn ")
                    || nt.starts_with("impl ")
                    || nt.starts_with("pub struct ")
                    || nt.starts_with("pub enum "))
                && !lines[end].starts_with(' ')
                && !lines[end].starts_with('\t')
            {
                break;
            }
            end += 1;
        }
        for slot in keep.iter_mut().take(end).skip(start) {
            *slot = true;
        }
    }
    let mut out = String::new();
    let mut in_gap = false;
    for (i, line) in lines.iter().enumerate() {
        if keep[i] {
            if out.len() + line.len() + 1 > MAX_BYTES {
                out.push_str("/* … truncated seed window */\n");
                break;
            }
            out.push_str(line);
            out.push('\n');
            in_gap = false;
        } else if !in_gap && !out.is_empty() {
            out.push_str("/* … */\n");
            in_gap = true;
        }
    }
    out
}

#[derive(Serialize)]
struct MinimalRetrieval {
    retrieval_level: String,
    claim: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sufficiency_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    critical_gaps: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_action: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    suggested_keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    cache_hit: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    embedding_used: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolution_tier: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    ort_session_active: bool,
}

#[derive(Serialize)]
struct PointerFile {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    why: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_range: Option<Vec<usize>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    symbols: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fold_ids: Vec<String>,
    /// 1–3 short lines so an agent can often start a fix without another Read.
    #[serde(skip_serializing_if = "Option::is_none")]
    excerpt: Option<String>,
}

#[derive(Serialize)]
struct PointerContextResponse {
    packet_id: String,
    coverage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolution_tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f32>,
    files: Vec<PointerFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<MinimalNext>,
    /// One-line loop: pointer → expand/read only what you need.
    #[serde(skip_serializing_if = "Option::is_none")]
    agent_hint: Option<String>,
}

#[derive(Serialize)]
struct MinimalContextResponse {
    packet_id: String,
    coverage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolution_tier: Option<String>,
    tokens: TokenCounts,
    files: Vec<MinimalFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    missing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<MinimalNext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retrieval: Option<MinimalRetrieval>,
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

fn pointer_files(files: &[FileEntry]) -> Vec<PointerFile> {
    files
        .iter()
        .enumerate()
        .map(|(idx, f)| {
            let mut symbols: Vec<String> = f.folded_symbols.clone();
            let mut fold_ids: Vec<String> = Vec::new();
            for d in &f.folds {
                if !fold_ids.contains(&d.fold_id) {
                    fold_ids.push(d.fold_id.clone());
                }
                if let Some(sig) = Some(d.signature.as_str()).filter(|s| !s.is_empty()) {
                    if !symbols.iter().any(|s| s == sig) {
                        symbols.push(sig.to_string());
                    }
                }
            }
            symbols.truncate(12);
            fold_ids.truncate(6);
            let signature = f
                .code
                .lines()
                .map(str::trim)
                .find(|l| {
                    if l.is_empty()
                        || l.starts_with("//")
                        || l.starts_with("/*")
                        || l.starts_with("@nm:")
                        || *l == "---"
                        || *l == "```"
                        || l.starts_with("```")
                    {
                        return false;
                    }
                    true
                })
                .map(|s| s.chars().take(160).collect::<String>());
            // Top files get a tiny excerpt so agents can often skip an extra Read.
            let excerpt = if idx < 3 {
                let lines: Vec<String> = f
                    .code
                    .lines()
                    .map(str::trim)
                    .filter(|l| {
                        !l.is_empty()
                            && !l.starts_with("//")
                            && !l.starts_with("/*")
                            && !l.starts_with("@nm:")
                            && *l != "---"
                    })
                    .take(3)
                    .map(|s| s.chars().take(120).collect::<String>())
                    .collect();
                if lines.is_empty() {
                    None
                } else {
                    Some(lines.join("\n"))
                }
            } else {
                None
            };
            PointerFile {
                path: f.path.clone(),
                why: f.why.clone().filter(|s| !s.is_empty()),
                line_range: f.line_range.as_ref().map(|r| vec![r.start, r.end]),
                symbols,
                signature,
                fold_ids,
                excerpt,
            }
        })
        .collect()
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
    pub server_inferred_keywords: bool,
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
        if self.server_inferred_keywords {
            task["server_inferred_keywords"] = json!(true);
        }
        task
    }

    fn retrieval_block(&self) -> Option<Value> {
        self.view.retrieval.as_ref().map(|r| {
            json!({
                "retrieval_level": r.retrieval_level,
                "sufficiency_score": r.sufficiency_score,
                "confidence": r.confidence,
                "claim": r.claim,
                "levels_attempted": r.levels_attempted,
                "latency_ms": r.latency_ms,
                "full_workspace_fallback": r.full_workspace_fallback,
                "critical_gaps": r.critical_gaps,
                "non_critical_gaps": r.non_critical_gaps,
                "eligible_for_early_exit": r.eligible_for_early_exit,
                "next_action": r.next_action,
                "suggested_keywords": r.suggested_keywords,
                "embedding_used": r.embedding_used,
                "resolution_tier": r.resolution_tier,
                "max_embedding_score": r.max_embedding_score,
                "cache_hit": r.cache_hit,
                "ort_session_active": r.ort_session_active,
            })
        })
    }

    fn compact_retrieval(&self) -> Option<MinimalRetrieval> {
        self.view.retrieval.as_ref().map(|r| MinimalRetrieval {
            retrieval_level: r.retrieval_level.clone(),
            claim: r.claim.clone(),
            sufficiency_score: Some(r.sufficiency_score),
            confidence: Some(r.confidence),
            critical_gaps: r.critical_gaps.clone(),
            next_action: r.next_action.clone(),
            suggested_keywords: r.suggested_keywords.clone().unwrap_or_default(),
            cache_hit: r.cache_hit,
            embedding_used: r.embedding_used,
            resolution_tier: r.resolution_tier.clone(),
            ort_session_active: r.ort_session_active,
        })
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
            ResponseDetail::Pointer => self.pointer(),
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
        matches!(
            self.coverage_claim(),
            "partial" | "no_seed_resolved" | "no_confident_match"
        )
    }

    /// Next MCP call with copy-paste arguments (agent contract).
    fn build_next(&self, files: &[PointerFile]) -> Option<MinimalNext> {
        if !self.needs_search() {
            return None;
        }
        let retrieval = self.view.retrieval.as_ref();
        let missing = self.missing_seeds();
        let queries = if !missing.is_empty() {
            missing.clone()
        } else {
            retrieval
                .and_then(|r| r.suggested_keywords.clone())
                .unwrap_or_default()
        };
        let tool = retrieval
            .and_then(|r| r.next_action.clone())
            .unwrap_or_else(|| "neuromesh_search_symbols".into());
        let best = pick_agent_hint_file(files, self.signature.raw_prompt.as_str());
        let example_args = if tool.contains("search") {
            Some(json!({
                "query": queries.first().cloned().unwrap_or_else(|| self.signature.raw_prompt.clone()),
                "limit": 8
            }))
        } else if tool.contains("skeleton") {
            best.map(|f| json!({ "file_path": f.path }))
        } else if tool.contains("expand_gap") || tool.contains("expand_fold") {
            best.and_then(|f| f.fold_ids.first())
                .map(|fid| json!({ "fold_id": fid }))
        } else {
            None
        };
        Some(MinimalNext {
            tool,
            queries,
            example_args,
        })
    }

    /// Lean pointer packet: path + line range + symbols + signature + fold ids.
    /// Top files carry a 3-line excerpt so many agent turns need no extra Read.
    fn pointer(&self) -> Value {
        let files = pointer_files(self.files);
        let retrieval = self.view.retrieval.as_ref();
        let next = self.build_next(&files);
        let best = pick_agent_hint_file(&files, self.signature.raw_prompt.as_str());
        let agent_hint = best
            .map(|f| {
                let range = f
                    .line_range
                    .as_ref()
                    .map(|r| format!(" L{}-{}", r[0], r[1]))
                    .unwrap_or_default();
                if let Some(fid) = f.fold_ids.first() {
                    format!(
                        "Read {}{} or neuromesh_expand_fold({fid}) for folded body",
                        f.path, range
                    )
                } else {
                    format!("Read {}{range}", f.path)
                }
            })
            .or_else(|| {
                next.as_ref().map(|n| {
                    format!(
                        "Call {} with {}",
                        n.tool,
                        n.example_args
                            .as_ref()
                            .map(|a| a.to_string())
                            .unwrap_or_else(|| format!("{:?}", n.queries))
                    )
                })
            });
        serde_json::to_value(PointerContextResponse {
            packet_id: self.packet_id.clone(),
            coverage: self.coverage_claim().to_string(),
            resolution_tier: retrieval.and_then(|r| r.resolution_tier.clone()),
            confidence: retrieval.map(|r| r.confidence),
            files,
            next_action: next.as_ref().map(|n| n.tool.clone()),
            next,
            agent_hint,
        })
        .unwrap_or(Value::Null)
    }

    fn minimal(&self) -> Value {
        let missing = self.missing_seeds();
        let pf = pointer_files(self.files);
        let next = self.build_next(&pf);
        let retrieval = self.view.retrieval.as_ref();
        let conf = retrieval.map(|r| r.confidence);
        // Cap bodies, but always keep the seed file's skeleton so the named
        // symbol (e.g. handle_tool_call) is actually in the packet.
        const MAX_MINIMAL_BODY: usize = 4800;
        let seed_paths = seed_file_paths(self.view);
        let seed_syms = seed_symbol_names(self.view);
        let mut kept_bodies = 0usize;
        let files: Vec<MinimalFile> = self
            .files
            .iter()
            .map(|f| {
                let all_fold_ids: Vec<String> = f.folds.iter().map(|d| d.fold_id.clone()).collect();
                let (fold_ids, folds_omitted) = cap_minimal_folds(&all_fold_ids);
                let is_seed_file = seed_paths.iter().any(|sp| {
                    f.path.eq_ignore_ascii_case(sp) || f.path.ends_with(sp) || sp.ends_with(&f.path)
                });
                let code = if f.sidecar && !is_seed_file {
                    String::new()
                } else if is_seed_file {
                    kept_bodies += 1;
                    let win = extract_seed_windows(&f.code, &seed_syms);
                    if win.is_empty() {
                        truncate_code(&f.code, MAX_MINIMAL_BODY)
                    } else {
                        win
                    }
                } else if kept_bodies >= 2 {
                    String::new()
                } else {
                    kept_bodies += 1;
                    truncate_code(&f.code, MAX_MINIMAL_BODY)
                };
                MinimalFile {
                    path: f.path.clone(),
                    why: f.why.clone().filter(|s| !s.is_empty()),
                    sidecar: f.sidecar,
                    code,
                    folds: fold_ids,
                    folds_omitted,
                }
            })
            .collect();
        serde_json::to_value(MinimalContextResponse {
            packet_id: self.packet_id.clone(),
            coverage: self.coverage_claim().to_string(),
            confidence: conf,
            resolution_tier: retrieval.and_then(|r| r.resolution_tier.clone()),
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
            retrieval: self.compact_retrieval(),
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
        if let Some(retrieval) = self.retrieval_block() {
            packet["retrieval"] = retrieval;
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
        let mut evidence = json!({
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
        });
        if let Some(retrieval) = self.retrieval_block() {
            evidence["retrieval"] = retrieval;
        }
        json!({
            "packet_id": self.packet_id,
            "task": self.task_metadata(),
            "membrane_state": self.gate.membrane_state,
            "effective_mode": format!("{:?}", self.gate.effective_mode),
            "latency_ms": self.elapsed_ms,
            "evidence_packet": evidence,
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
    let details = build.to_details();
    cache.insert(project_id, details);
    build.serialize(detail)
}

/// Rehydrate a semantic cache hit with a fresh packet_id and `retrieval.cache_hit`.
pub fn apply_semantic_cache_hit(
    packet_cache: &PacketDetailCache,
    project_id: &str,
    mut response: Value,
    mut details: PacketDetails,
    new_packet_id: String,
) -> Value {
    details.packet_id = new_packet_id.clone();
    packet_cache.insert(project_id, details);
    if let Some(obj) = response.as_object_mut() {
        obj.insert("packet_id".into(), json!(new_packet_id));
        if let Some(retrieval) = obj.get_mut("retrieval").and_then(|r| r.as_object_mut()) {
            retrieval.insert("cache_hit".into(), json!(true));
        } else {
            obj.insert("retrieval".into(), json!({ "cache_hit": true }));
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_detail_defaults_to_minimal() {
        std::env::remove_var("NEUROMESH_RESPONSE_DETAIL");
        assert_eq!(ResponseDetail::parse(None), ResponseDetail::Minimal);
        assert_eq!(ResponseDetail::parse(Some("")), ResponseDetail::Minimal);
        assert_eq!(
            ResponseDetail::parse(Some("diagnostic")),
            ResponseDetail::Diagnostic
        );
        assert_eq!(
            ResponseDetail::parse(Some("pointer")),
            ResponseDetail::Pointer
        );
        assert_eq!(ResponseDetail::parse(Some("lean")), ResponseDetail::Pointer);
        // Explicit argument always wins over any deployment default.
        std::env::set_var("NEUROMESH_RESPONSE_DETAIL", "pointer");
        assert_eq!(
            ResponseDetail::parse(Some("minimal")),
            ResponseDetail::Minimal
        );
        std::env::remove_var("NEUROMESH_RESPONSE_DETAIL");
    }

    #[test]
    fn pointer_omits_code_bodies() {
        let files = vec![FileEntry {
            path: "src/lib.rs".into(),
            code: "// comment\npub fn hello() {\n    let secret = 1;\n}\n".repeat(20),
            tokens: 400,
            why: Some("seed".into()),
            sidecar: false,
            line_range: Some(10..40),
            folded_symbols: vec!["hello".into()],
            folds: vec![],
        }];
        let pointers = pointer_files(&files);
        let dumped = serde_json::to_string(&pointers).unwrap();
        // Excerpt may include a short signature line but not a large body dump.
        assert!(dumped.contains("src/lib.rs"));
        assert!(dumped.contains("hello"));
        assert!(
            dumped.len() * 2 < files[0].code.len(),
            "pointer should be much smaller than code ({} vs {})",
            dumped.len(),
            files[0].code.len()
        );
    }

    #[test]
    fn agent_hint_prefers_source_over_connector() {
        let files = vec![
            PointerFile {
                path: "scripts/mcp_comprehensive_test.py".into(),
                why: None,
                line_range: None,
                symbols: vec![],
                signature: None,
                fold_ids: vec![],
                excerpt: Some("import json".into()),
            },
            PointerFile {
                path: "crates/neuromesh-core/src/token.rs".into(),
                why: None,
                line_range: Some(vec![1, 40]),
                symbols: vec!["TokenCounter".into()],
                signature: Some("pub struct TokenCounter".into()),
                fold_ids: vec!["fold_token_1".into()],
                excerpt: Some("pub struct TokenCounter".into()),
            },
        ];
        let best =
            pick_agent_hint_file(&files, "How does the system estimate the number of tokens?");
        assert_eq!(best.unwrap().path, "crates/neuromesh-core/src/token.rs");
    }
    #[test]
    fn minimal_caps_fold_ids_per_file() {
        let ids: Vec<String> = (0..20).map(|i| format!("fold_{i}")).collect();
        let (kept, omitted) = cap_minimal_folds(&ids);
        assert_eq!(kept.len(), MINIMAL_FOLDS_PER_FILE);
        assert_eq!(kept[0], "fold_0");
        assert_eq!(omitted, Some(20 - MINIMAL_FOLDS_PER_FILE));
        let short = ids[..3].to_vec();
        let (kept_short, omitted_short) = cap_minimal_folds(&short);
        assert_eq!(kept_short, short);
        assert_eq!(omitted_short, None);
        // Serialized shape: capped list + honest count, nothing else.
        let file = MinimalFile {
            path: "src/lib.rs".into(),
            why: None,
            sidecar: false,
            code: String::new(),
            folds: kept,
            folds_omitted: omitted,
        };
        let dumped = serde_json::to_value(&file).unwrap();
        assert_eq!(
            dumped["folds"].as_array().unwrap().len(),
            MINIMAL_FOLDS_PER_FILE
        );
        assert_eq!(dumped["folds_omitted"], 20 - MINIMAL_FOLDS_PER_FILE);
        assert!(dumped.get("why").is_none());
    }

    #[test]
    fn pointer_includes_fold_ids_and_excerpt() {
        let files = vec![FileEntry {
            path: "src/lib.rs".into(),
            code: "pub fn alpha() {\n    1\n}\npub fn beta() {\n    2\n}\n".into(),
            tokens: 80,
            why: None,
            sidecar: false,
            line_range: Some(1..10),
            folded_symbols: vec!["beta".into()],
            folds: vec![FoldDescriptor {
                fold_id: "fold_beta_1".into(),
                symbol: "beta".into(),
                signature: "pub fn beta()".into(),
                start_line: 4,
                end_line: 6,
                saved_tokens: 10,
            }],
        }];
        let pointers = pointer_files(&files);
        assert_eq!(pointers[0].fold_ids, vec!["fold_beta_1".to_string()]);
        assert!(pointers[0]
            .excerpt
            .as_deref()
            .unwrap_or("")
            .contains("alpha"));
        assert_eq!(pointers[0].line_range, Some(vec![1, 10]));
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
    fn extract_seed_windows_finds_later_function() {
        let mut code = String::new();
        for i in 0..200 {
            code.push_str(&format!("fn filler_{i}() {{\n    let _ = {i};\n}}\n"));
        }
        code.push_str(
            "pub fn reinforce_path(&self, ids: &[NodeId]) {\n    self.reinforce(ids);\n}\n",
        );
        for i in 0..50 {
            code.push_str(&format!("fn tail_{i}() {{\n}}\n"));
        }
        let win = extract_seed_windows(&code, &["reinforce_path".to_string()]);
        assert!(
            win.contains("fn reinforce_path"),
            "window missing target: {win}"
        );
        assert!(!win.contains("filler_0"), "should not keep early fillers");
    }

    #[test]
    fn extract_seed_windows_stops_at_next_fn() {
        let mut code = String::from("pub fn reinforce_path(&self) {\n    self.reinforce();\n}\n");
        for i in 0..80 {
            code.push_str(&format!("pub fn other_{i}() {{\n    let _ = {i};\n}}\n"));
        }
        let win = extract_seed_windows(&code, &["reinforce_path".to_string()]);
        assert!(win.contains("fn reinforce_path"));
        assert!(!win.contains("other_0"), "must stop at next top-level fn");
        assert!(win.len() < 2000, "window too large: {}", win.len());
    }

    #[test]
    fn extract_seed_windows_caps_bytes() {
        let mut code = String::from("pub fn huge_target(&self) {\n");
        for i in 0..400 {
            code.push_str(&format!("    let x{i} = {};\n", "y".repeat(80)));
        }
        code.push_str("}\n");
        let win = extract_seed_windows(&code, &["huge_target".to_string()]);
        assert!(win.contains("fn huge_target"));
        assert!(win.len() <= 6_200, "unbounded window: {}", win.len());
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
