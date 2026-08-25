use neuromesh_core::{OptimizationMetadata, ProjectId};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

const HISTORY_CAP: usize = 1000;

pub fn telemetry_file_path() -> PathBuf {
    if let Ok(raw) = std::env::var("NEUROMESH_TELEMETRY_FILE") {
        let path = PathBuf::from(raw);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        return path;
    }
    let base = neuromesh_core::neuromesh_home();
    let _ = std::fs::create_dir_all(&base);
    base.join("telemetry_history.json")
}

pub fn load_persisted_history() -> Vec<OptimizationMetadata> {
    let path = telemetry_file_path();
    if path.exists() {
        if let Ok(data) = std::fs::read(&path) {
            if let Ok(records) = serde_json::from_slice::<Vec<OptimizationMetadata>>(&data) {
                return records;
            }
        }
    }
    Vec::new()
}

pub fn save_persisted_history(history: &[OptimizationMetadata]) {
    let path = telemetry_file_path();
    let to_save: Vec<_> = history.iter().rev().take(HISTORY_CAP).cloned().collect();
    let to_save_ordered: Vec<_> = to_save.into_iter().rev().collect();
    if let Ok(json_bytes) = serde_json::to_vec_pretty(&to_save_ordered) {
        let _ = std::fs::write(path, json_bytes);
    }
}

/// Append `meta` unless `request_id` is already present. Returns true when inserted.
pub fn append_unique(history: &mut Vec<OptimizationMetadata>, meta: OptimizationMetadata) -> bool {
    if history.iter().any(|h| h.request_id == meta.request_id) {
        return false;
    }
    history.push(meta);
    let overflow = history.len().saturating_sub(HISTORY_CAP);
    if overflow > 0 {
        history.drain(0..overflow);
    }
    true
}

pub fn record_global_telemetry(meta: OptimizationMetadata) {
    let mut history = load_persisted_history();
    if append_unique(&mut history, meta.clone()) {
        save_persisted_history(&history);
    }
    notify_monitor(meta);
}

fn notify_monitor(meta: OptimizationMetadata) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let payload = serde_json::to_vec(&meta).unwrap_or_default();
    let cfg = neuromesh_core::Config::load();
    let host = cfg.host;
    let port = cfg.port;
    std::mem::drop(handle.spawn(async move {
        use tokio::io::AsyncWriteExt;
        use tokio::net::TcpStream;
        let endpoint = format!("{host}:{port}");
        if let Ok(mut stream) = TcpStream::connect(&endpoint).await {
            let req = format!(
                "POST /api/telemetry/record HTTP/1.1\r\n\
                 Host: {endpoint}\r\n\
                 Content-Type: application/json\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                payload.len()
            );
            let _ = stream.write_all(req.as_bytes()).await;
            let _ = stream.write_all(&payload).await;
        }
    }));
}

pub fn filter_history(
    history: &[OptimizationMetadata],
    project_id: &ProjectId,
    workspace: &str,
    all_projects: bool,
) -> Vec<OptimizationMetadata> {
    let is_collective =
        all_projects || workspace == "__all__" || project_id.0.contains("collective");
    if is_collective {
        return history.to_vec();
    }
    let pid_lower = project_id.0.to_lowercase();
    let ws_lower = workspace.replace('\\', "/").to_lowercase();
    history
        .iter()
        .filter(|h| {
            let h_pid = h.project_id.0.to_lowercase();
            h_pid == pid_lower
                || h_pid.contains(&pid_lower)
                || pid_lower.contains(&h_pid)
                || ws_lower.ends_with(&format!("/{h_pid}"))
                || ws_lower.ends_with(&h_pid)
                || h_pid == "local"
                || h_pid == "project"
                || h_pid == "default"
        })
        .cloned()
        .collect()
}

pub fn summarize_history(history: &[OptimizationMetadata]) -> AggregatedMetrics {
    if history.is_empty() {
        return AggregatedMetrics::default();
    }

    let total_requests = history.len() as u64;
    let mut total_before = 0u64;
    let mut total_after = 0u64;
    let mut cache_hits = 0u64;
    let mut total_expansions = 0u64;
    let mut total_latency = 0u64;
    let mut reduction_sum = 0.0f32;

    for r in history {
        total_before += r.tokens_before as u64;
        total_after += r.tokens_after as u64;
        if r.cache_hit {
            cache_hits += 1;
        }
        total_expansions += r.expansions_count as u64;
        total_latency += r.latency_ms;
        reduction_sum += r.token_reduction_pct;
    }

    let total_tokens_saved = total_before.saturating_sub(total_after);
    let overall_reduction_pct = if total_before > 0 {
        (total_tokens_saved as f32 / total_before as f32) * 100.0
    } else {
        0.0
    };

    AggregatedMetrics {
        total_requests,
        total_tokens_before: total_before,
        total_tokens_after: total_after,
        total_tokens_saved,
        overall_reduction_pct,
        mean_reduction_pct: reduction_sum / total_requests as f32,
        cache_hits,
        cache_hit_rate: (cache_hits as f32 / total_requests as f32) * 100.0,
        total_expansions,
        average_latency_ms: total_latency as f32 / total_requests as f32,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub total_requests: u64,
    pub total_tokens_before: u64,
    pub total_tokens_after: u64,
    pub total_tokens_saved: u64,
    pub overall_reduction_pct: f32,
    #[serde(default)]
    pub mean_reduction_pct: f32,
    pub cache_hits: u64,
    pub cache_hit_rate: f32,
    pub total_expansions: u64,
    pub average_latency_ms: f32,
}

#[derive(Clone)]
pub struct MetricsCollector {
    history: Arc<RwLock<Vec<OptimizationMetadata>>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        let initial = load_persisted_history();
        Self {
            history: Arc::new(RwLock::new(initial)),
        }
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, meta: OptimizationMetadata) {
        let mut history = load_persisted_history();
        if !append_unique(&mut history, meta) {
            *self.history.write() = history;
            return;
        }
        save_persisted_history(&history);
        *self.history.write() = history;
    }

    pub fn get_metrics(&self) -> AggregatedMetrics {
        summarize_history(&self.get_history())
    }

    pub fn get_history(&self) -> Vec<OptimizationMetadata> {
        let disk = load_persisted_history();
        *self.history.write() = disk.clone();
        disk
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use neuromesh_core::ProjectId;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn meta(id: &str, project: &str, before: usize, after: usize) -> OptimizationMetadata {
        OptimizationMetadata {
            request_id: id.into(),
            task_id: Some(format!("task {id}")),
            project_id: ProjectId::new(project),
            mode: "balanced".into(),
            tokens_before: before,
            tokens_after: after,
            token_reduction_pct: if before > 0 {
                (before.saturating_sub(after) as f32 / before as f32) * 100.0
            } else {
                0.0
            },
            nodes_before: 10,
            nodes_after: 2,
            expansions_count: 0,
            cache_hit: false,
            provider: "test".into(),
            model: "test".into(),
            latency_ms: 8,
            success: true,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn append_unique_skips_duplicate_request_ids() {
        let mut history = vec![meta("mcp-1", "neuromesh", 100, 40)];
        assert!(!append_unique(
            &mut history,
            meta("mcp-1", "neuromesh", 100, 40)
        ));
        assert_eq!(history.len(), 1);
        assert!(append_unique(
            &mut history,
            meta("mcp-2", "neuromesh", 80, 20)
        ));
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn filter_keeps_matching_project_and_generic_ids() {
        let rows = vec![
            meta("a", "neuromesh", 10, 4),
            meta("b", "other", 10, 4),
            meta("c", "local", 10, 4),
        ];
        let filtered = filter_history(
            &rows,
            &ProjectId::new("neuromesh"),
            r"c:\projects\neuromesh",
            false,
        );
        let ids: Vec<_> = filtered.iter().map(|r| r.request_id.as_str()).collect();
        assert_eq!(ids, vec!["a", "c"]);
    }

    #[test]
    fn filter_all_returns_every_project() {
        let rows = vec![meta("a", "neuromesh", 10, 4), meta("b", "other", 10, 4)];
        let filtered = filter_history(
            &rows,
            &ProjectId::new("neuromesh"),
            r"c:\projects\neuromesh",
            true,
        );
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn summarize_matches_saved_and_mean_reduction() {
        let rows = vec![
            meta("a", "neuromesh", 100, 40),
            meta("b", "neuromesh", 50, 25),
        ];
        let summary = summarize_history(&rows);
        assert_eq!(summary.total_requests, 2);
        assert_eq!(summary.total_tokens_before, 150);
        assert_eq!(summary.total_tokens_after, 65);
        assert_eq!(summary.total_tokens_saved, 85);
        assert!((summary.overall_reduction_pct - (85.0 / 150.0) * 100.0).abs() < 0.01);
        assert!((summary.mean_reduction_pct - 55.0).abs() < 0.01);
        assert!((summary.average_latency_ms - 8.0).abs() < f32::EPSILON);
    }

    #[test]
    fn record_without_tokio_runtime_still_persists() {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("nm-tel-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("telemetry_history.json");
        std::env::set_var("NEUROMESH_TELEMETRY_FILE", &path);
        record_global_telemetry(meta("cli-1", "neuromesh", 100, 10));
        record_global_telemetry(meta("cli-1", "neuromesh", 100, 10));
        let loaded = load_persisted_history();
        std::env::remove_var("NEUROMESH_TELEMETRY_FILE");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            loaded.len(),
            1,
            "duplicate request_id must not double-count"
        );
        assert_eq!(loaded[0].tokens_after, 10);
    }
}
