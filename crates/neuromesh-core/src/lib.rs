pub mod config;
pub mod error;
pub mod paths;
pub mod source_path;
pub mod task;
pub mod token;
pub mod types;

pub use config::{
    parse_max_files, parse_port, Config, LocalAiConfig, OptimizationMode, ProviderConfig,
    ProviderType, Thresholds,
};
pub use error::{NeuroMeshError, Result};
pub use paths::{
    current_project_store, current_trust_list, ensure_project_data_dir, graph_path,
    leftover_workspace_dotdir, memory_db_path, neuromesh_home, normalize_workspace,
    project_config_path, project_data_dir, save_store_policy, trust_workspace_local,
    untrust_workspace_local, uses_local_dotdir, ProjectStore,
};
pub use source_path::{
    decoy_allowed_for_prompt, hmvc_app_prefix, is_alt_surface_path, is_bench_path,
    is_core_source_path, is_json_schema_path, is_legacy_path, is_locale_path,
    is_low_priority_source_path, is_name_collision_decoy, is_schema_path, name_match_specificity,
    prompt_targets_alt_surface, prompt_targets_bench, prompt_targets_database,
    prompt_targets_json_schema, prompt_targets_legacy, prompt_targets_locale, prompt_targets_types,
};
pub use task::{SubtaskNode, SubtaskStatus, TaskGraph, TaskIntent, TaskRisk, TaskSignature};
pub use token::TokenCounter;
pub use types::{
    ActivatedNodeView, ContextDiff, ContextEdge, ContextNode, ContextStatus, ContextView,
    CoverageReport, EdgeConfidence, EdgeId, EdgeType, InactiveContextDescriptor, IndexMeta,
    NextAction, NodeId, NodeType, OptimizationMetadata, PacketGap, ProjectId, RankCandidateView,
    SeedResolution, SkippedFile, StructuralEvidence, UnresolvedRef,
};
