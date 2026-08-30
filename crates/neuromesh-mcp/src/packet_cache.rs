use neuromesh_context::FoldDescriptor;
use neuromesh_core::{
    CoverageReport, InactiveContextDescriptor, IndexMeta, NextAction, SeedResolution, UnresolvedRef,
};
use neuromesh_router::OsmoticMembraneState;
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::ops::Range;
use std::time::{Duration, Instant};

const MAX_PACKETS: usize = 32;
const TTL: Duration = Duration::from_secs(600);
const MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct PacketBudgetSnapshot {
    pub used: usize,
    pub cap: usize,
    pub mode: String,
    pub seed_tokens: usize,
    pub fill_used: usize,
    pub fill_cap: usize,
    pub over_budget: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileSelectionMeta {
    pub path: String,
    pub why: Option<String>,
    pub tokens: usize,
    pub line_range: Option<Range<usize>>,
    pub folded_symbols: Vec<String>,
    pub folds: Vec<FoldDescriptor>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PacketDetails {
    pub packet_id: String,
    pub seeds: Vec<SeedResolution>,
    pub coverage: Option<CoverageReport>,
    pub budget: PacketBudgetSnapshot,
    pub membrane: OsmoticMembraneState,
    pub physarum_used: bool,
    pub physarum_ms: u64,
    pub selection_method: String,
    pub rank_candidates: Vec<neuromesh_core::RankCandidateView>,
    pub unresolved: Vec<UnresolvedRef>,
    pub inactive_hints: Vec<InactiveContextDescriptor>,
    pub index: IndexMeta,
    pub files: Vec<FileSelectionMeta>,
    pub symbols: Vec<Value>,
    pub fold_ids: Vec<String>,
    pub next_actions: Vec<NextAction>,
    pub tokens_selected: usize,
    pub tokens_packet: usize,
    pub workspace_tokens: usize,
    pub seed_call_coverage: f32,
    pub effective_mode: String,
    pub latency_ms: u64,
    pub reduction_vs_workspace_pct: String,
    pub reduction_vs_selected_pct: String,
}

struct CacheEntry {
    created_at: Instant,
    force_expired: bool,
    bytes: usize,
    details: PacketDetails,
}

struct CacheInner {
    project_id: Option<String>,
    order: VecDeque<String>,
    entries: HashMap<String, CacheEntry>,
    total_bytes: usize,
}

pub struct PacketDetailCache {
    inner: Mutex<CacheInner>,
    ttl: Duration,
    max_packets: usize,
    max_bytes: usize,
}

impl Default for PacketDetailCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PacketDetailCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(CacheInner {
                project_id: None,
                order: VecDeque::new(),
                entries: HashMap::new(),
                total_bytes: 0,
            }),
            ttl: TTL,
            max_packets: MAX_PACKETS,
            max_bytes: MAX_BYTES,
        }
    }

    pub fn new_packet_id() -> String {
        format!("ctx_{}", uuid::Uuid::new_v4().simple())
    }

    pub fn insert(&self, project_id: &str, details: PacketDetails) {
        let bytes = serde_json::to_vec(&details).map(|v| v.len()).unwrap_or(0);
        let mut inner = self.inner.lock();
        if inner.project_id.as_deref() != Some(project_id) {
            inner.order.clear();
            inner.entries.clear();
            inner.total_bytes = 0;
            inner.project_id = Some(project_id.to_string());
        }
        Self::evict_expired_locked(&mut inner, self.ttl);
        if inner.entries.contains_key(&details.packet_id) {
            Self::unlink(&mut inner, &details.packet_id);
        }
        while inner.order.len() >= self.max_packets
            || inner.total_bytes.saturating_add(bytes) > self.max_bytes
        {
            if !Self::evict_lru(&mut inner) {
                break;
            }
        }
        inner.order.push_back(details.packet_id.clone());
        inner.total_bytes = inner.total_bytes.saturating_add(bytes);
        inner.entries.insert(
            details.packet_id.clone(),
            CacheEntry {
                created_at: Instant::now(),
                force_expired: false,
                bytes,
                details,
            },
        );
    }

    pub fn get(&self, packet_id: &str) -> Result<PacketDetails, PacketCacheError> {
        let mut inner = self.inner.lock();
        Self::evict_expired_locked(&mut inner, self.ttl);
        let Some(entry) = inner.entries.get(packet_id) else {
            return Err(PacketCacheError::Unknown);
        };
        if entry.force_expired || entry.created_at.elapsed() > self.ttl {
            Self::unlink(&mut inner, packet_id);
            return Err(PacketCacheError::Expired);
        }
        let details = entry.details.clone();
        if let Some(pos) = inner.order.iter().position(|id| id == packet_id) {
            inner.order.remove(pos);
            inner.order.push_back(packet_id.to_string());
        }
        Ok(details)
    }

    #[cfg(test)]
    pub fn expire_for_test(&self, packet_id: &str) {
        let mut inner = self.inner.lock();
        if let Some(entry) = inner.entries.get_mut(packet_id) {
            entry.force_expired = true;
        }
    }

    fn evict_expired_locked(inner: &mut CacheInner, ttl: Duration) {
        let stale: Vec<String> = inner
            .entries
            .iter()
            .filter(|(_, e)| e.created_at.elapsed() > ttl)
            .map(|(id, _)| id.clone())
            .collect();
        for id in stale {
            Self::unlink(inner, &id);
        }
    }

    fn evict_lru(inner: &mut CacheInner) -> bool {
        let Some(id) = inner.order.pop_front() else {
            return false;
        };
        if let Some(entry) = inner.entries.remove(&id) {
            inner.total_bytes = inner.total_bytes.saturating_sub(entry.bytes);
        }
        true
    }

    fn unlink(inner: &mut CacheInner, packet_id: &str) {
        if let Some(pos) = inner.order.iter().position(|id| id == packet_id) {
            inner.order.remove(pos);
        }
        if let Some(entry) = inner.entries.remove(packet_id) {
            inner.total_bytes = inner.total_bytes.saturating_sub(entry.bytes);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketCacheError {
    Unknown,
    Expired,
}

impl PacketCacheError {
    pub fn message(self, packet_id: &str) -> String {
        match self {
            PacketCacheError::Unknown => {
                format!("packet_id unknown: {packet_id} — call get_context_packet again")
            }
            PacketCacheError::Expired => {
                format!("packet_id expired: {packet_id} — call get_context_packet again")
            }
        }
    }
}
