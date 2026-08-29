pub mod confine;
pub mod hasher;
pub mod mcp_workspace;
pub mod tracker;
pub mod walker;
pub mod watcher;

pub use confine::{
    assert_safe_workspace, is_path_within, is_safe_workspace, path_escapes_workspace,
    read_workspace_file, resolve_workspace_file,
};
pub use hasher::ContentHasher;
pub use mcp_workspace::{
    parse_workspace_folder_paths, resolve_mcp_startup_workspace, same_workspace_path,
    workspace_from_ide_env,
};
pub use tracker::{FileFingerprint, IndexedFile, SourceLanguage};
pub use walker::{ProjectWalker, ScanReport};
pub use watcher::{FileChangeEvent, WorkspaceWatcher};
