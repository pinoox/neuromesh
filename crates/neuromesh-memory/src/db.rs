use crate::episodic::EpisodicRecord;
use crate::project::ProjectFact;
use neuromesh_core::{ProjectId, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Default, Serialize, Deserialize)]
struct StorageData {
    project_facts: HashMap<String, ProjectFact>,
    episodes: Vec<EpisodicRecord>,
}

pub struct MemoryDatabase {
    path: Option<PathBuf>,
    data: Arc<RwLock<StorageData>>,
}

impl MemoryDatabase {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = if path.exists() {
            let content = fs::read_to_string(path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            let default_data = StorageData::default();
            let json = serde_json::to_string_pretty(&default_data)?;
            let _ = fs::write(path, json);
            default_data
        };

        Ok(Self {
            path: Some(path.to_path_buf()),
            data: Arc::new(RwLock::new(data)),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        Ok(Self {
            path: None,
            data: Arc::new(RwLock::new(StorageData::default())),
        })
    }

    fn persist(&self) -> Result<()> {
        if let Some(path) = &self.path {
            let data = self.data.read();
            let json = serde_json::to_string_pretty(&*data)?;
            fs::write(path, json)?;
        }
        Ok(())
    }

    pub fn save_project_fact(&self, fact: &ProjectFact) -> Result<()> {
        let key = format!("{}:{}:{}", fact.project_id.0, fact.category, fact.key);
        self.data.write().project_facts.insert(key, fact.clone());
        self.persist()?;
        Ok(())
    }

    pub fn get_project_facts(&self, project_id: &ProjectId) -> Result<Vec<ProjectFact>> {
        let data = self.data.read();
        let facts = data
            .project_facts
            .values()
            .filter(|f| f.project_id == *project_id)
            .cloned()
            .collect();
        Ok(facts)
    }

    pub fn save_episodic_record(&self, record: &EpisodicRecord) -> Result<()> {
        self.data.write().episodes.push(record.clone());
        self.persist()?;
        Ok(())
    }

    pub fn find_similar_episodes(
        &self,
        project_id: &ProjectId,
        query: &str,
    ) -> Result<Vec<EpisodicRecord>> {
        let data = self.data.read();
        let query_lower = query.to_lowercase();

        let mut matching: Vec<EpisodicRecord> = data
            .episodes
            .iter()
            .filter(|e| e.project_id == *project_id)
            .filter(|e| {
                if query_lower.is_empty() {
                    true
                } else {
                    e.summary.to_lowercase().contains(&query_lower)
                        || e.intent.to_lowercase().contains(&query_lower)
                }
            })
            .cloned()
            .collect();

        matching.sort_by_key(|b| std::cmp::Reverse(b.created_at));
        matching.truncate(10);
        Ok(matching)
    }
}
