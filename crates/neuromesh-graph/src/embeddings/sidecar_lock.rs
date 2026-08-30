//! Per-workspace locks so concurrent MCP queries cannot corrupt `embeddings.bin`.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

static SIDECAR_WRITE_LOCKS: OnceLock<parking_lot::Mutex<HashMap<PathBuf, Arc<RwLock<()>>>>> =
    OnceLock::new();

fn lock_map() -> &'static parking_lot::Mutex<HashMap<PathBuf, Arc<RwLock<()>>>> {
    SIDECAR_WRITE_LOCKS.get_or_init(|| parking_lot::Mutex::new(HashMap::new()))
}

/// Shared exclusive lock keyed by sidecar path.
pub fn sidecar_write_lock(workspace: &Path) -> Arc<RwLock<()>> {
    let path = neuromesh_core::embeddings_path(workspace);
    let mut map = lock_map().lock();
    map.entry(path)
        .or_insert_with(|| Arc::new(RwLock::new(())))
        .clone()
}

/// Run `f` while holding the workspace sidecar write lock (lazy embed + rebuild).
pub fn with_sidecar_write<R>(workspace: &Path, f: impl FnOnce() -> R) -> R {
    let lock = sidecar_write_lock(workspace);
    let _guard = lock.write();
    f()
}
