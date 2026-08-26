use neuromesh_core::{Config, ProjectId, Result};
use neuromesh_observability::{filter_history, load_persisted_history, summarize_history};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const DEFAULT_LIMIT: usize = 20;

pub struct UsageArgs {
    pub all_projects: bool,
    pub limit: usize,
    pub help: bool,
}

pub fn parse_usage_args(args: &[String]) -> Result<UsageArgs> {
    let mut all_projects = false;
    let mut limit = DEFAULT_LIMIT;
    let mut help = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--all" | "-a" => all_projects = true,
            "help" | "-h" | "--help" => help = true,
            "--limit" => {
                let raw = args.get(i + 1).ok_or_else(|| {
                    neuromesh_core::NeuroMeshError::Config("--limit needs a number".into())
                })?;
                limit = parse_limit(raw)?;
                i += 1;
            }
            flag if flag.starts_with("--limit=") => {
                limit = parse_limit(&flag[8..])?;
            }
            other => {
                return Err(neuromesh_core::NeuroMeshError::Config(format!(
                    "unknown usage flag: {other}"
                )));
            }
        }
        i += 1;
    }
    Ok(UsageArgs {
        all_projects,
        limit: limit.max(1),
        help,
    })
}

fn parse_limit(raw: &str) -> Result<usize> {
    raw.parse::<usize>().map_err(|_| {
        neuromesh_core::NeuroMeshError::Config(format!(
            "--limit needs a positive number, got {raw}"
        ))
    })
}

fn print_usage_help() {
    println!("Usage: neuromesh usage [--all] [--limit N]");
    println!("  --all, -a     Include every project in ~/.neuromesh/telemetry_history.json");
    println!("  --limit N     Recent rows to print (default 20)");
}

pub fn execute(args: &[String]) -> Result<()> {
    let parsed = parse_usage_args(args)?;
    if parsed.help {
        print_usage_help();
        return Ok(());
    }

    let current_dir = std::env::current_dir()?;
    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    let project_id = ProjectId::new(&project_name);
    let workspace = current_dir.display().to_string();

    let file_path = neuromesh_observability::telemetry_file_path();
    let all = load_persisted_history();
    let filtered = filter_history(&all, &project_id, &workspace, parsed.all_projects);
    let summary = summarize_history(&filtered);

    let cfg = Config::load();
    let monitor = monitor_status(&cfg);

    println!("\nNeuroMesh usage");
    println!(
        "Project        : {}{}",
        if parsed.all_projects {
            "all".to_string()
        } else {
            project_name.clone()
        },
        if parsed.all_projects {
            String::new()
        } else {
            format!(" ({workspace})")
        }
    );
    println!("Telemetry file : {}", file_path.display());
    println!("Monitor        : {monitor}");
    println!(
        "Records        : {} this view / {} on disk",
        filtered.len(),
        all.len()
    );
    println!();
    println!("Summary");
    println!("  Requests         : {}", summary.total_requests);
    println!("  Tokens before    : {}", summary.total_tokens_before);
    println!("  Tokens after     : {}", summary.total_tokens_after);
    println!("  Tokens saved     : {}", summary.total_tokens_saved);
    println!("  Mean reduction   : {:.1}%", summary.mean_reduction_pct);
    println!("  Overall vs dump  : {:.1}%", summary.overall_reduction_pct);
    println!("  Avg latency      : {:.1} ms", summary.average_latency_ms);
    println!(
        "  Cache hits       : {} ({:.1}%)",
        summary.cache_hits, summary.cache_hit_rate
    );
    println!();

    if filtered.is_empty() {
        if all.is_empty() {
            println!("No MCP tool calls recorded yet.");
            println!(
                "Usage grows when an MCP client initializes (one session row) or calls a NeuroMesh tool."
            );
            println!("Editing files in the IDE does not add a row.\n");
        } else {
            println!("No rows for this project.");
            println!(
                "{} records exist on disk — run `neuromesh usage --all`.\n",
                all.len()
            );
        }
        return Ok(());
    }

    println!(
        "{:<20} {:<14} {:>8} {:>8} {:>8} {:>7} {:>5}  Task",
        "Time", "Mode", "Before", "After", "Saved", "%", "ms"
    );
    println!("{:-<96}", "");

    for row in filtered.iter().rev().take(parsed.limit) {
        let saved = row.tokens_before.saturating_sub(row.tokens_after);
        let task = row
            .task_id
            .as_deref()
            .unwrap_or("-")
            .chars()
            .take(36)
            .collect::<String>();
        println!(
            "{:<20} {:<14} {:>8} {:>8} {:>8} {:>6.1}% {:>5}  {}",
            row.timestamp.format("%Y-%m-%d %H:%M:%S"),
            truncate(&row.mode, 14),
            row.tokens_before,
            row.tokens_after,
            saved,
            row.token_reduction_pct,
            row.latency_ms,
            task
        );
    }
    if filtered.len() > parsed.limit {
        println!(
            "... {} older rows hidden (`neuromesh usage --limit {}`)",
            filtered.len() - parsed.limit,
            filtered.len()
        );
    }
    println!();
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

fn monitor_status(cfg: &Config) -> String {
    let endpoint = format!("{}:{}", cfg.host, cfg.port);
    let ok = endpoint
        .parse::<SocketAddr>()
        .ok()
        .and_then(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(200)).ok())
        .is_some();
    if ok {
        format!("listening on http://{endpoint}")
    } else {
        format!("not running ({endpoint}) — CLI still reads the telemetry file")
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_usage_args, DEFAULT_LIMIT};

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn parses_usage_flags() {
        let default = parse_usage_args(&args(&["neuromesh", "usage"])).unwrap();
        assert!(!default.all_projects);
        assert!(!default.help);
        assert_eq!(default.limit, DEFAULT_LIMIT);

        let all = parse_usage_args(&args(&["neuromesh", "usage", "--all"])).unwrap();
        assert!(all.all_projects);

        let limit = parse_usage_args(&args(&["neuromesh", "usage", "--limit", "5"])).unwrap();
        assert_eq!(limit.limit, 5);

        let eq = parse_usage_args(&args(&["neuromesh", "usage", "--limit=8", "-a"])).unwrap();
        assert_eq!(eq.limit, 8);
        assert!(eq.all_projects);
    }
}
