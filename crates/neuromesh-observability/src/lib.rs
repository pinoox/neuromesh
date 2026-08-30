pub mod metrics;
pub mod record;

pub use metrics::record_global_telemetry;
pub use metrics::{
    append_unique, filter_history, load_persisted_history, save_persisted_history,
    summarize_history, telemetry_file_path, AggregatedMetrics, MetricsCollector,
};
pub use record::{
    cli_request_id, record_activity, record_metadata, ActivityRecord, TelemetrySurface,
};
