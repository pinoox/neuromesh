use crate::project::ProjectFact;
use neuromesh_core::ProjectId;
use std::fs;
use std::path::Path;

/// Seed project memory from the repository itself — manifests, docs, crate layout.
pub fn extract_project_facts(root: &Path, project_id: &ProjectId) -> Vec<ProjectFact> {
    let mut facts = Vec::new();

    if let Ok(cargo) = fs::read_to_string(root.join("Cargo.toml")) {
        if cargo.contains("[workspace]") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "architecture",
                "build_system",
                "Rust cargo workspace",
            ));
        }
        if let Some(members) = extract_workspace_members(&cargo) {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "architecture",
                "workspace_crates",
                members,
            ));
        }
        if cargo.contains("neuromesh") {
            facts.push(ProjectFact::new(
                project_id.clone(),
                "framework",
                "product",
                "NeuroMesh biomimetic MCP context engine",
            ));
        }
    }

    if root.join("package.json").exists() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "javascript_toolchain",
            "Node/TypeScript project with package.json",
        ));
    }

    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "framework",
            "python_toolchain",
            "Python project",
        ));
    }

    for doc in ["ARCHITECTURE.md", "DESIGN.md", "README.md", "API.md"] {
        let path = root.join(doc);
        if let Ok(content) = fs::read_to_string(&path) {
            let summary = first_meaningful_lines(&content, 8);
            if !summary.is_empty() {
                facts.push(ProjectFact::new(
                    project_id.clone(),
                    "documentation",
                    doc.to_lowercase(),
                    summary,
                ));
            }
        }
    }

    if root.join("crates/neuromesh-mcp").exists() {
        facts.push(ProjectFact::new(
            project_id.clone(),
            "convention",
            "agent_interface",
            "Primary agent interface is MCP (stdio JSON-RPC). Prefer neuromesh_get_context for task-conditioned evidence packets.",
        ));
    }

    facts
}

fn extract_workspace_members(cargo: &str) -> Option<String> {
    let start = cargo.find("members")?;
    let slice = &cargo[start..];
    let open = slice.find('[')?;
    let close = slice.find(']')?;
    let inner = &slice[open + 1..close];
    let members: Vec<&str> = inner
        .split(',')
        .filter_map(|part| {
            let trimmed = part.trim().trim_matches('"').trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .collect();
    if members.is_empty() {
        None
    } else {
        Some(members.join(", "))
    }
}

fn first_meaningful_lines(content: &str, max_lines: usize) -> String {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("<") && *line != "---")
        .take(max_lines)
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(600)
        .collect()
}
