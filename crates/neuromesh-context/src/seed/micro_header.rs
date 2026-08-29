use neuromesh_core::{EdgeType, NodeId};
use neuromesh_core::{PacketHeaderConfig, SeedResolution};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::HashMap;

pub struct MicroHeaderGenerator;

impl MicroHeaderGenerator {
    pub fn generate(
        graph: &NeuralProjectGraph,
        config: &PacketHeaderConfig,
        stack_line: Option<&str>,
        seed_resolutions: &[SeedResolution],
        seed_energies: &HashMap<NodeId, f32>,
        max_flow_depth: usize,
    ) -> Option<String> {
        if !config.enabled {
            return None;
        }
        let mut lines = Vec::new();
        if config.include_stack {
            if let Some(stack) = stack_line.filter(|s| !s.is_empty()) {
                lines.push(format!("@nm:stack: {stack}"));
            }
        }
        if config.include_seeds {
            let seed_line = seed_resolutions
                .iter()
                .filter_map(|s| {
                    let id = s.resolved_id.as_ref()?;
                    if !seed_energies.contains_key(id) {
                        return None;
                    }
                    let node = graph.get_node(id)?;
                    let path = node.file_path.to_string_lossy().replace('\\', "/");
                    Some(format!("{path}:{}", node.name))
                })
                .take(4)
                .collect::<Vec<_>>()
                .join(", ");
            if !seed_line.is_empty() {
                lines.push(format!("@nm:seeds: {seed_line}"));
            }
        }
        if config.include_flow {
            if let Some(flow) = extract_call_flow(graph, seed_energies, max_flow_depth) {
                lines.push(format!("@nm:flow: {flow}"));
            }
        }
        if lines.is_empty() {
            return None;
        }
        lines.push("---".into());
        Some(lines.join("\n"))
    }

    pub fn token_estimate(header: &str) -> usize {
        header.split_whitespace().count()
    }
}

fn extract_call_flow(
    graph: &NeuralProjectGraph,
    seed_energies: &HashMap<NodeId, f32>,
    max_depth: usize,
) -> Option<String> {
    let start = seed_energies.keys().find_map(|id| graph.get_node(id))?;
    let mut chain = vec![start.name.clone()];
    let mut current = start.id.clone();
    for _ in 0..max_depth {
        let mut next_name = None;
        for (neighbor, edge) in graph.get_connected_neighbors(&current) {
            if edge.edge_type != EdgeType::Calls || edge.source != current {
                continue;
            }
            let Some(node) = graph.get_node(&neighbor) else {
                continue;
            };
            next_name = Some(node.name.clone());
            current = neighbor;
            break;
        }
        let Some(name) = next_name else {
            break;
        };
        chain.push(name);
    }
    if chain.len() < 2 {
        return None;
    }
    Some(chain.join(" -> "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_respects_token_budget() {
        let header = "@nm:stack: api:python/fastapi\n@nm:seeds: a.py:Foo\n---";
        assert!(MicroHeaderGenerator::token_estimate(header) <= 25);
    }
}
