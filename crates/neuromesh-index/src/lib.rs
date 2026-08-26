pub mod confine;
pub mod hasher;
pub mod tracker;
pub mod walker;
pub mod watcher;

pub use confine::{
    assert_safe_workspace, is_path_within, is_safe_workspace, path_escapes_workspace,
    read_workspace_file, resolve_workspace_file,
};
pub use hasher::ContentHasher;
pub use tracker::{FileFingerprint, IndexedFile, SourceLanguage};
pub use walker::{ProjectWalker, ScanReport};
pub use watcher::{FileChangeEvent, WorkspaceWatcher};
