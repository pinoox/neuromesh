use crate::tracker::IndexedFile;
use crate::walker::ProjectWalker;
use neuromesh_core::ProjectId;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
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
    debounce: Duration,
    running: Arc<AtomicBool>,
}

impl WorkspaceWatcher {
    pub fn new(root_path: PathBuf, project_id: ProjectId) -> Self {
        Self {
            root_path,
            project_id,
            debounce: Duration::from_millis(200),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn with_interval(mut self, interval: Duration) -> Self {
        self.debounce = interval;
        self
    }

    pub fn start(&mut self) -> (mpsc::Receiver<FileChangeEvent>, Arc<AtomicBool>) {
        let (tx, rx) = mpsc::channel(64);
        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        let root = self.root_path.clone();
        let project_id = self.project_id.clone();
        let debounce = self.debounce;
        let thread_running = running.clone();
        let (raw_tx, raw_rx) = std::sync::mpsc::channel();

        let mut watcher = match RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    let _ = raw_tx.send(event);
                }
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(_) => return (rx, running),
        };
        if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
            return (rx, running);
        }

        tokio::task::spawn_blocking(move || {
            let _watcher = watcher;
            let walker = ProjectWalker::new(root.clone(), project_id);
            let mut pending: HashSet<PathBuf> = HashSet::new();
            let mut deleted: HashSet<PathBuf> = HashSet::new();

            while thread_running.load(Ordering::SeqCst) {
                match raw_rx.recv_timeout(debounce) {
                    Ok(event) => collect_event(&root, event, &mut pending, &mut deleted),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        flush_pending(&walker, &root, &tx, &mut pending, &mut deleted);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });

        (rx, running)
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

fn collect_event(
    root: &Path,
    event: notify::Event,
    pending: &mut HashSet<PathBuf>,
    deleted: &mut HashSet<PathBuf>,
) {
    let is_delete = matches!(event.kind, EventKind::Remove(_));
    for path in event.paths {
        if ProjectWalker::is_ignored(&path) {
            continue;
        }
        if crate::confine::path_escapes_workspace(&path, root) {
            continue;
        }
        if !crate::confine::is_path_within(&path, root) {
            continue;
        }
        if is_delete {
            deleted.insert(path);
        } else {
            pending.insert(path);
        }
    }
}

fn flush_pending(
    walker: &ProjectWalker,
    root: &Path,
    tx: &mpsc::Sender<FileChangeEvent>,
    pending: &mut HashSet<PathBuf>,
    deleted: &mut HashSet<PathBuf>,
) {
    for path in deleted.drain() {
        let rel = path
            .strip_prefix(root)
            .map(|p| p.to_path_buf())
            .unwrap_or(path);
        let _ = tx.blocking_send(FileChangeEvent::Deleted(rel));
    }
    for path in pending.drain() {
        if let Some((file, content)) = walker.read_indexed(&path) {
            let _ = tx.blocking_send(FileChangeEvent::Modified(file, content));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_creation() {
        let watcher = WorkspaceWatcher::new(PathBuf::from("."), ProjectId::new("test"));
        assert_eq!(watcher.debounce, Duration::from_millis(200));
    }
}
