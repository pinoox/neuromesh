use super::build::{passage_hash, symbol_passage_for_node};
use crate::embeddings::{
    save_sidecar_atomic, sidecar_lock::with_sidecar_write, EmbeddingIndex, SIDECAR_VERSION,
};
use crate::NeuralProjectGraph;
use neuromesh_core::{ContextNode, EmbeddingConfig, NodeId, NodeType};
use neuromesh_embed::Embedder;
use std::collections::HashSet;
use std::path::Path;

const LAZY_SYMBOL_CAP_PER_FILE: usize = 32;
const LAZY_SYMBOL_TOTAL_CAP: usize = 128;

fn index_embed_batch_size() -> usize {
    std::env::var("NEUROMESH_EMBED_INDEX_BATCH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(128)
        .clamp(32, 256)
}

/// Embed missing symbol tier rows for files matched at query time; persists sidecar atomically.
pub fn lazy_embed_symbols_for_files(
    graph: &NeuralProjectGraph,
    workspace: &Path,
    config: &EmbeddingConfig,
    file_ids: &[NodeId],
    prompt: Option<&str>,
) -> neuromesh_core::Result<usize> {
    if !config.enabled || !config.hierarchical_index || file_ids.is_empty() {
        return Ok(0);
    }

    with_sidecar_write(workspace, || {
        lazy_embed_symbols_for_files_locked(graph, workspace, config, file_ids, prompt)
    })
}

fn symbol_overlap_score(node: &ContextNode, prompt: &str) -> u32 {
    let lower = prompt.to_lowercase();
    let name = node.name.to_lowercase();
    let mut score = 0u32;
    if name.len() >= 3 && lower.contains(&name) {
        score += 3;
    }
    for part in name.split(|c: char| !c.is_ascii_alphanumeric()) {
        if part.len() >= 4 && lower.contains(part) {
            score += 1;
        }
    }
    if let Some(sig) = node.signature.as_deref() {
        let sig_l = sig.to_lowercase();
        for token in lower.split(|c: char| !c.is_alphanumeric()) {
            if token.len() >= 4 && sig_l.contains(token) {
                score += 1;
            }
        }
    }
    score
}

fn lazy_embed_symbols_for_files_locked(
    graph: &NeuralProjectGraph,
    workspace: &Path,
    config: &EmbeddingConfig,
    file_ids: &[NodeId],
    prompt: Option<&str>,
) -> neuromesh_core::Result<usize> {
    let path = neuromesh_core::embeddings_path(workspace);
    let Some(mut sidecar) = crate::embeddings::load_sidecar(&path)? else {
        return Ok(0);
    };
    if !sidecar.is_hierarchical() {
        return Ok(0);
    }

    let dim = config.matryoshka_dim;
    let mut pending: Vec<(NodeId, u32, String, String, u32)> = Vec::new();
    let mut seen_files = HashSet::new();

    for file_id in file_ids {
        if pending.len() >= LAZY_SYMBOL_TOTAL_CAP {
            break;
        }
        if !seen_files.insert(file_id.clone()) {
            continue;
        }
        let Some(file_row) = sidecar.file_node_ids.iter().position(|id| id == file_id) else {
            continue;
        };
        let file_row = file_row as u32;
        let Some(file_node) = graph.get_node(file_id) else {
            continue;
        };
        let file_path = file_node.file_path.clone();

        let mut file_pending: Vec<(NodeId, u32, String, String, u32)> = Vec::new();
        for node in graph.nodes_in_file(&file_path) {
            if matches!(
                node.node_type,
                NodeType::File | NodeType::Directory | NodeType::Project
            ) {
                continue;
            }
            if sidecar.node_ids.iter().any(|id| id == &node.id) {
                continue;
            }
            let Some(text) = symbol_passage_for_node(&node, config.model) else {
                continue;
            };
            let overlap = prompt.map(|p| symbol_overlap_score(&node, p)).unwrap_or(0);
            file_pending.push((
                node.id.clone(),
                file_row,
                passage_hash(&text),
                text,
                overlap,
            ));
        }
        file_pending.sort_by_key(|b| std::cmp::Reverse(b.4));
        file_pending.truncate(LAZY_SYMBOL_CAP_PER_FILE);
        pending.extend(file_pending);
        pending.truncate(LAZY_SYMBOL_TOTAL_CAP);
    }

    if pending.is_empty() {
        return Ok(0);
    }

    let arc = Embedder::lazy_global(config.clone())
        .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
    let mut embedder = arc.lock();
    let batch_size = index_embed_batch_size();
    let mut added = 0usize;
    let mut batch_out = 0usize;

    while batch_out < pending.len() {
        let end = (batch_out + batch_size).min(pending.len());
        let chunk: Vec<String> = pending[batch_out..end]
            .iter()
            .map(|(_, _, _, t, _)| t.clone())
            .collect();
        let embedded = embedder
            .embed_documents(&chunk)
            .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;

        for (vec, (node_id, file_row, hash, _, _)) in
            embedded.into_iter().zip(pending[batch_out..end].iter())
        {
            if vec.len() != dim {
                continue;
            }
            let (qi8, scale) = {
                let mut qi8 = vec![0i8; dim];
                let scale = crate::embeddings::quantize::quantize_unit_vector(&vec, &mut qi8);
                (qi8, scale)
            };
            sidecar.node_ids.push(node_id.clone());
            sidecar.content_hashes.push(hash.clone());
            sidecar.symbol_file_index.push(*file_row);
            sidecar.vectors_i8.extend_from_slice(&qi8);
            sidecar.quant_scales.push(scale);
            added += 1;
        }
        batch_out = end;
    }

    if added > 0 {
        sidecar.version = SIDECAR_VERSION;
        save_sidecar_atomic(&path, &sidecar)?;
        let mut index = EmbeddingIndex::from_sidecar(sidecar);
        index.rebuild_lookup_maps();
        graph.install_embedding_index(index);
    }

    Ok(added)
}

/// Stats for CLI / doctor output.
pub fn sidecar_tier_stats(workspace: &Path) -> Option<(usize, usize, bool)> {
    let path = neuromesh_core::embeddings_path(workspace);
    let sidecar = crate::embeddings::load_sidecar(&path).ok()??;
    Some((
        sidecar.file_node_ids.len(),
        sidecar.node_ids.len(),
        sidecar.is_hierarchical(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lazy_caps_are_bounded() {
        assert_eq!(LAZY_SYMBOL_CAP_PER_FILE, 32);
        assert_eq!(LAZY_SYMBOL_TOTAL_CAP, 128);
    }
}
