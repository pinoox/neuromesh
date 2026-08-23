use neuromesh_core::OptimizationMetadata;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

fn get_telemetry_file_path() -> PathBuf {
    let base = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".neuromesh");
    let _ = std::fs::create_dir_all(&base);
    base.join("telemetry_history.json")
}

pub fn load_persisted_history() -> Vec<OptimizationMetadata> {
    let path = get_telemetry_file_path();
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
    let path = get_telemetry_file_path();
    let to_save: Vec<_> = history.iter().rev().take(1000).cloned().collect();
    let to_save_ordered: Vec<_> = to_save.into_iter().rev().collect();
    if let Ok(json_bytes) = serde_json::to_vec_pretty(&to_save_ordered) {
        let _ = std::fs::write(path, json_bytes);
    }
}

pub fn record_global_telemetry(meta: OptimizationMetadata) {
    // 1. Append to local file
    let mut history = load_persisted_history();
    history.push(meta.clone());
    save_persisted_history(&history);

    // 2. Notify local monitor if it is listening on the configured port
    let payload = serde_json::to_vec(&meta).unwrap_or_default();
    let cfg = neuromesh_core::Config::load();
    let host = cfg.host;
    let port = cfg.port;
    tokio::spawn(async move {
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
    });
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub total_requests: u64,
    pub total_tokens_before: u64,
    pub total_tokens_after: u64,
    pub total_tokens_saved: u64,
    pub overall_reduction_pct: f32,
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
        let mut history = self.history.write();
        history.push(meta);
        if history.len() > 1000 {
            history.remove(0);
        }
        save_persisted_history(&history);
    }

    pub fn get_metrics(&self) -> AggregatedMetrics {
        let history = self.history.read();
        if history.is_empty() {
            return AggregatedMetrics::default();
        }

        let total_requests = history.len() as u64;
        let mut total_before = 0u64;
        let mut total_after = 0u64;
        let mut cache_hits = 0u64;
        let mut total_expansions = 0u64;
        let mut total_latency = 0u64;

        for r in history.iter() {
            total_before += r.tokens_before as u64;
            total_after += r.tokens_after as u64;
            if r.cache_hit {
                cache_hits += 1;
            }
            total_expansions += r.expansions_count as u64;
            total_latency += r.latency_ms;
        }

        let total_tokens_saved = total_before.saturating_sub(total_after);
        let overall_reduction_pct = if total_before > 0 {
            (total_tokens_saved as f32 / total_before as f32) * 100.0
        } else {
            0.0
        };

        let cache_hit_rate = (cache_hits as f32 / total_requests as f32) * 100.0;
        let average_latency_ms = total_latency as f32 / total_requests as f32;

        AggregatedMetrics {
            total_requests,
            total_tokens_before: total_before,
            total_tokens_after: total_after,
            total_tokens_saved,
            overall_reduction_pct,
            cache_hits,
            cache_hit_rate,
            total_expansions,
            average_latency_ms,
        }
    }

    pub fn get_history(&self) -> Vec<OptimizationMetadata> {
        let history = self.history.read();
        history.clone()
    }
}
