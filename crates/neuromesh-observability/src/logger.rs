use neuromesh_core::{OptimizationMetadata, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Default, Serialize, Deserialize)]
struct TelemetryData {
    records: Vec<OptimizationMetadata>,
}

pub struct TelemetryDatabase {
    path: Option<PathBuf>,
    data: Arc<RwLock<TelemetryData>>,
}

impl TelemetryDatabase {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = if path.exists() {
            let content = fs::read_to_string(path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            TelemetryData::default()
        };

        Ok(Self {
            path: Some(path.to_path_buf()),
            data: Arc::new(RwLock::new(data)),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        Ok(Self {
            path: None,
            data: Arc::new(RwLock::new(TelemetryData::default())),
        })
    }

    pub fn record_telemetry(&self, meta: &OptimizationMetadata) -> Result<()> {
        let mut data = self.data.write();
        data.records.push(meta.clone());
        if data.records.len() > 2000 {
            data.records.remove(0);
        }

        if let Some(path) = &self.path {
            let json = serde_json::to_string_pretty(&*data)?;
            fs::write(path, json)?;
        }

        Ok(())
    }
}
