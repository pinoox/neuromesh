use neuromesh_core::Result;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectivePattern {
    pub id: String,
    pub domain: String,
    pub intent: String,
    pub solution_template: String,
    pub success_count: usize,
}

pub struct CollectiveMemory {
    storage_path: Option<PathBuf>,
    patterns: Arc<RwLock<HashMap<String, CollectivePattern>>>,
}

impl Default for CollectiveMemory {
    fn default() -> Self {
        Self::open_global().unwrap_or_else(|_| Self::new_in_memory())
    }
}

impl CollectiveMemory {
    pub fn new_in_memory() -> Self {
        let mem = Self {
            storage_path: None,
            patterns: Arc::new(RwLock::new(HashMap::new())),
        };
        mem.seed_defaults();
        mem
    }

    pub fn open_global() -> Result<Self> {
        let global_dir = dirs::home_dir()
            .map(|h| h.join(".neuromesh"))
            .unwrap_or_else(|| PathBuf::from("."));

        let file_path = global_dir.join("collective_memory.json");
        let mut patterns = HashMap::new();

        if file_path.exists() {
            if let Ok(content) = fs::read_to_string(&file_path) {
                if let Ok(loaded) =
                    serde_json::from_str::<HashMap<String, CollectivePattern>>(&content)
                {
                    patterns = loaded;
                }
            }
        }

        let mem = Self {
            storage_path: Some(file_path),
            patterns: Arc::new(RwLock::new(patterns)),
        };

        if mem.patterns.read().is_empty() {
            mem.seed_defaults();
        }

        Ok(mem)
    }

    fn seed_defaults(&self) {
        let mut lock = self.patterns.write();
        lock.insert(
            "vue3-pinia-connect".to_string(),
            CollectivePattern {
                id: "vue3-pinia-connect".to_string(),
                domain: "Vue 3 + Pinia".to_string(),
                intent: "Connect store reactivity in component setup".to_string(),
                solution_template: "import { storeToRefs } from 'pinia'; const store = useCartStore(); const { items, total } = storeToRefs(store);".to_string(),
                success_count: 12,
            },
        );

        lock.insert(
            "rust-tokio-graceful-exit".to_string(),
            CollectivePattern {
                id: "rust-tokio-graceful-exit".to_string(),
                domain: "Rust Async".to_string(),
                intent: "Graceful cancellation with AtomicBool and mpsc".to_string(),
                solution_template: "let running = Arc::new(AtomicBool::new(true)); tokio::select! { _ = signal::ctrl_c() => { running.store(false, Ordering::SeqCst); } }".to_string(),
                success_count: 24,
            },
        );
    }

    pub fn record_pattern(&self, domain: &str, intent: &str, solution_template: &str) {
        let id = format!(
            "{}-{}",
            domain.to_lowercase().replace(' ', "-"),
            intent.to_lowercase().replace(' ', "-")
        );
        let mut lock = self.patterns.write();
        let entry = lock.entry(id.clone()).or_insert_with(|| CollectivePattern {
            id,
            domain: domain.to_string(),
            intent: intent.to_string(),
            solution_template: solution_template.to_string(),
            success_count: 0,
        });

        entry.success_count += 1;
        self.save();
    }

    pub fn find_matching_patterns(&self, query: &str) -> Vec<CollectivePattern> {
        let q = query.to_lowercase();
        let lock = self.patterns.read();
        lock.values()
            .filter(|p| {
                p.domain.to_lowercase().contains(&q) || p.intent.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    fn save(&self) {
        if let Some(path) = &self.storage_path {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let lock = self.patterns.read();
            if let Ok(content) = serde_json::to_string_pretty(&*lock) {
                let _ = fs::write(path, content);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collective_memory_seed_and_search() {
        let mem = CollectiveMemory::new_in_memory();
        let matches = mem.find_matching_patterns("Pinia");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].domain, "Vue 3 + Pinia");
    }
}
