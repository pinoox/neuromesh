pub mod benchmark;
pub mod connect;
pub mod doctor;
pub mod evaluate;
pub mod graph;
pub mod index;
pub mod init;
pub mod memory;
pub mod models;
pub mod monitor;
pub mod optimize;
pub mod port;
pub mod start;
pub mod status;
pub mod store;
pub mod usage;

use neuromesh_core::{parse_max_files, parse_port, Config, ProjectId, Result};
use neuromesh_index::ProjectWalker;

/// `--port 9000`, `-p 9000`, or `--port=9000` anywhere in argv.
pub fn port_from_args(args: &[String]) -> Result<Option<u16>> {
    for (i, a) in args.iter().enumerate() {
        if a == "--port" || a == "-p" {
            let raw = args.get(i + 1).ok_or_else(|| {
                neuromesh_core::NeuroMeshError::Config("--port needs a number".into())
            })?;
            return Ok(Some(parse_port(raw)?));
        }
        if let Some(raw) = a.strip_prefix("--port=") {
            return Ok(Some(parse_port(raw)?));
        }
    }
    Ok(None)
}

/// `--max-files` flag: omitted, `auto`, or a positive limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCapArg {
    Unspecified,
    Auto,
    Limit(usize),
}

/// `--max-files 20000`, `--max-files=20000`, or `--max-files auto`.
pub fn max_files_from_args(args: &[String]) -> Result<FileCapArg> {
    for (i, a) in args.iter().enumerate() {
        if a == "--max-files" {
            let raw = args.get(i + 1).ok_or_else(|| {
                neuromesh_core::NeuroMeshError::Config(
                    "--max-files needs a number or `auto`".into(),
                )
            })?;
            return Ok(match parse_max_files(raw)? {
                None => FileCapArg::Auto,
                Some(n) => FileCapArg::Limit(n),
            });
        }
        if let Some(raw) = a.strip_prefix("--max-files=") {
            return Ok(match parse_max_files(raw)? {
                None => FileCapArg::Auto,
                Some(n) => FileCapArg::Limit(n),
            });
        }
    }
    Ok(FileCapArg::Unspecified)
}

pub fn configured_walker(
    root: std::path::PathBuf,
    pid: ProjectId,
    cap: FileCapArg,
) -> ProjectWalker {
    let walker = ProjectWalker::new(root, pid);
    match cap {
        FileCapArg::Unspecified => walker.with_optional_max_files(Config::load().max_files),
        FileCapArg::Auto => walker,
        FileCapArg::Limit(n) => walker.with_max_files(n),
    }
}

pub fn apply_file_cap(config: Config, cap: FileCapArg) -> Config {
    match cap {
        FileCapArg::Unspecified => config,
        FileCapArg::Auto => config.with_max_files(None),
        FileCapArg::Limit(n) => config.with_max_files(Some(n)),
    }
}

pub fn persist_file_cap(cap: FileCapArg) -> Result<Option<std::path::PathBuf>> {
    let max_files = match cap {
        FileCapArg::Unspecified => return Ok(None),
        FileCapArg::Auto => None,
        FileCapArg::Limit(n) => Some(n),
    };
    let path = Config::from_files()
        .with_max_files(max_files)
        .save_local()?;
    Ok(Some(path))
}

pub fn print_file_cap(report: &neuromesh_index::ScanReport, indent: &str) {
    if report.auto_cap {
        println!(
            "{indent}File cap       : auto → {} (ceiling {})",
            report.file_cap, report.hard_cap
        );
    } else {
        println!("{indent}File cap       : {} (explicit)", report.file_cap);
    }
    if report.truncated {
        println!(
            "{indent}Truncated      : {} files omitted (test trees queued last)",
            report.omitted_over_cap
        );
    }
    if report.unchanged > 0 {
        println!(
            "{indent}Unchanged skip : {} files (mtime/size match)",
            report.unchanged
        );
    }
}

pub fn spawn_live_sync(
    graph: std::sync::Arc<neuromesh_graph::NeuralProjectGraph>,
    dir: std::path::PathBuf,
    pid: ProjectId,
    cap: FileCapArg,
) {
    if !ProjectWalker::is_safe_workspace(&dir) {
        graph.mark_index_ready();
        return;
    }
    let bg_graph = graph.clone();
    let bg_dir = dir.clone();
    let bg_pid = pid.clone();
    tokio::task::spawn_blocking(move || {
        let max_files = match cap {
            FileCapArg::Unspecified => Config::load().max_files,
            FileCapArg::Auto => None,
            FileCapArg::Limit(n) => Some(n),
        };
        bg_graph.reindex_incremental(&bg_dir, bg_pid, max_files);
    });
    tokio::spawn(async move {
        let mut watcher = neuromesh_index::WorkspaceWatcher::new(dir.clone(), pid);
        let (mut rx, _running) = watcher.start();
        while let Some(ev) = rx.recv().await {
            graph.apply_file_event(ev);
            let _ = graph.save_persisted(&dir);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{max_files_from_args, port_from_args, FileCapArg};

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn parses_port_flags() {
        assert_eq!(
            port_from_args(&args(&["monitor", "--port", "9000"])).unwrap(),
            Some(9000)
        );
        assert_eq!(
            port_from_args(&args(&["monitor", "-p", "9001"])).unwrap(),
            Some(9001)
        );
        assert_eq!(
            port_from_args(&args(&["monitor", "--port=9002"])).unwrap(),
            Some(9002)
        );
        assert_eq!(port_from_args(&args(&["monitor"])).unwrap(), None);
        assert!(port_from_args(&args(&["monitor", "--port"])).is_err());
        assert!(port_from_args(&args(&["monitor", "--port", "0"])).is_err());
    }

    #[test]
    fn parses_max_files_flags() {
        assert_eq!(
            max_files_from_args(&args(&["index", "--max-files", "20000"])).unwrap(),
            FileCapArg::Limit(20000)
        );
        assert_eq!(
            max_files_from_args(&args(&["index", "--max-files=auto"])).unwrap(),
            FileCapArg::Auto
        );
        assert_eq!(
            max_files_from_args(&args(&["index", "--max-files", "auto"])).unwrap(),
            FileCapArg::Auto
        );
        assert_eq!(
            max_files_from_args(&args(&["index"])).unwrap(),
            FileCapArg::Unspecified
        );
        assert!(max_files_from_args(&args(&["index", "--max-files"])).is_err());
    }
}
