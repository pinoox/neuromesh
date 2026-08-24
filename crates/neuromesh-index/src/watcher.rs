use crate::tracker::IndexedFile;
use crate::walker::ProjectWalker;
use neuromesh_core::ProjectId;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum FileChangeEvent {
    Modified(IndexedFile, String),
    Created(IndexedFile, String),
    Deleted(PathBuf),
}

pub struct WorkspaceWatcher {
    root_path: PathBuf,
    project_id: ProjectId,
    poll_interval: Duration,
    running: Arc<AtomicBool>,
    known_hashes: HashMap<PathBuf, String>,
}

impl WorkspaceWatcher {
    pub fn new(root_path: PathBuf, project_id: ProjectId) -> Self {
        Self {
            root_path,
            project_id,
            poll_interval: Duration::from_millis(150),
            running: Arc::new(AtomicBool::new(false)),
            known_hashes: HashMap::new(),
        }
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    pub fn start(&mut self) -> (mpsc::Receiver<FileChangeEvent>, Arc<AtomicBool>) {
        let (tx, rx) = mpsc::channel(100);
        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let root = self.root_path.clone();
        let project_id = self.project_id.clone();
        let interval = self.poll_interval;
        let thread_running = running.clone();

        // Initial scan to populate known hashes
        let walker = ProjectWalker::new(root.clone(), project_id.clone())
            .with_optional_max_files(neuromesh_core::Config::load().max_files);
        if let Ok(initial_files) = walker.scan() {
            for (file, _) in initial_files {
                self.known_hashes
                    .insert(file.full_path.clone(), file.blake3_hash.clone());
            }
        }

        let mut known = self.known_hashes.clone();

        tokio::spawn(async move {
            let mut last_scan = Instant::now();

            while thread_running.load(Ordering::SeqCst) {
                tokio::time::sleep(interval).await;

                if last_scan.elapsed() >= interval {
                    last_scan = Instant::now();

                    let walker = ProjectWalker::new(root.clone(), project_id.clone())
                        .with_optional_max_files(neuromesh_core::Config::load().max_files);
                    if let Ok(current_files) = walker.scan() {
                        let mut current_map = HashMap::new();

                        for (file, content) in current_files {
                            let path = file.full_path.clone();
                            let current_hash = file.blake3_hash.clone();
                            current_map.insert(path.clone(), (file.clone(), content.clone()));

                            match known.get(&path) {
                                Some(old_hash) if old_hash != &current_hash => {
                                    known.insert(path.clone(), current_hash);
                                    let _ = tx.send(FileChangeEvent::Modified(file, content)).await;
                                }
                                None => {
                                    known.insert(path.clone(), current_hash);
                                    let _ = tx.send(FileChangeEvent::Created(file, content)).await;
                                }
                                _ => {}
                            }
                        }

                        // Check for deletions
                        let deleted_paths: Vec<PathBuf> = known
                            .keys()
                            .filter(|k| !current_map.contains_key(*k))
                            .cloned()
                            .collect();

                        for del in deleted_paths {
                            known.remove(&del);
                            let _ = tx.send(FileChangeEvent::Deleted(del)).await;
                        }
                    }
                }
            }
        });

        (rx, running)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_creation() {
        let watcher = WorkspaceWatcher::new(PathBuf::from("."), ProjectId::new("test"));
        assert_eq!(watcher.poll_interval, Duration::from_millis(150));
    }
}
