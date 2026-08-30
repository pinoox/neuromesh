use crate::embeddings::{
    node_type_label, quantize::quantize_matrix, save_sidecar, symbol_sketch, EmbeddingIndex,
    EmbeddingSidecar, ModuleCentroid, DEFAULT_QUANT_SCALE, SIDECAR_VERSION,
};
use crate::NeuralProjectGraph;
use neuromesh_core::{EmbeddingConfig, EmbeddingModelId, NodeType};
use neuromesh_embed::{format_document_for_model, Embedder};
use std::collections::HashMap;
use std::path::Path;

fn index_embed_batch_size() -> usize {
    std::env::var("NEUROMESH_EMBED_INDEX_BATCH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(128)
        .clamp(32, 256)
}

fn symbol_passage(node: &neuromesh_core::ContextNode, model: EmbeddingModelId) -> Option<String> {
    if model == EmbeddingModelId::MiniLmMultilingualQ {
        let path = node.file_path.to_string_lossy().replace('\\', "/");
        let title = format!("{}::{}", path, node.name);
        let sig = node.signature.as_deref().unwrap_or("");
        Some(format_document_for_model(
            model,
            &title,
            node_type_label(node.node_type),
            sig,
            node.doc_summary.as_deref(),
        ))
    } else {
        symbol_sketch(node)
    }
}

fn copy_prior_vector(
    prior: &EmbeddingSidecar,
    prior_idx: usize,
    dim: usize,
    dst: &mut [f32],
) -> bool {
    let start = prior_idx * dim;
    let end = start + dim;
    if !prior.vectors_i8.is_empty() && end <= prior.vectors_i8.len() {
        let scale = prior
            .quant_scales
            .get(prior_idx)
            .copied()
            .filter(|s| *s > f32::EPSILON)
            .unwrap_or(prior.quant_scale);
        crate::embeddings::quantize::dequant_slice(&prior.vectors_i8[start..end], scale, dst);
        return true;
    }
    if !prior.vectors.is_empty() && end <= prior.vectors.len() {
        dst.copy_from_slice(&prior.vectors[start..end]);
        return true;
    }
    false
}

fn compute_module_centroids(
    graph: &NeuralProjectGraph,
    node_ids: &[neuromesh_core::NodeId],
    vectors: &[f32],
    dim: usize,
    enabled: bool,
) -> Vec<ModuleCentroid> {
    if !enabled || dim == 0 || node_ids.is_empty() {
        return Vec::new();
    }
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, node_id) in node_ids.iter().enumerate() {
        let Some(node) = graph.get_node(node_id) else {
            continue;
        };
        if node.node_type == NodeType::File {
            continue;
        }
        let start = i * dim;
        if start + dim > vectors.len() {
            continue;
        }
        let dir = node
            .file_path
            .parent()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| ".".into());
        groups.entry(dir).or_default().push(i);
    }
    groups
        .into_iter()
        .filter(|(_, indices)| indices.len() >= 2)
        .map(|(dir, indices)| {
            let mut sum = vec![0.0f32; dim];
            for &idx in &indices {
                let start = idx * dim;
                let slice = &vectors[start..start + dim];
                for (j, x) in slice.iter().enumerate() {
                    sum[j] += x;
                }
            }
            let n = indices.len() as f32;
            for x in &mut sum {
                *x /= n;
            }
            let norm: f32 = sum.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut sum {
                    *x /= norm;
                }
            }
            ModuleCentroid {
                dir,
                vector: sum,
                symbol_count: indices.len(),
            }
        })
        .collect()
}

pub fn graph_digest(graph: &NeuralProjectGraph) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(graph.generation().to_le_bytes());
    let hashes = graph.file_hashes();
    let mut paths: Vec<_> = hashes.keys().collect();
    paths.sort();
    for path in paths {
        hasher.update(path.as_bytes());
        if let Some(hash) = hashes.get(path) {
            hasher.update(hash.as_bytes());
        }
    }
    format!("{:x}", hasher.finalize())
}

fn passage_hash(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// Build or refresh symbol embeddings; reuses unchanged vectors from an existing sidecar (v4+).
pub fn rebuild_embeddings(
    graph: &NeuralProjectGraph,
    workspace: &Path,
    config: &EmbeddingConfig,
) -> neuromesh_core::Result<EmbeddingIndex> {
    let path = neuromesh_core::embeddings_path(workspace);
    let prior = crate::embeddings::load_sidecar(&path).ok().flatten();
    let digest = graph_digest(graph);

    let mut node_ids = Vec::new();
    let mut texts = Vec::new();
    let mut content_hashes = Vec::new();
    let mut reuse_indices: Vec<Option<usize>> = Vec::new();

    let prior_lookup: HashMap<_, _> = prior
        .as_ref()
        .filter(|p| {
            p.version >= crate::embeddings::MIN_SIDECAR_VERSION
                && p.model_id == config.model.as_str()
                && p.dim == config.matryoshka_dim
        })
        .map(|p| {
            p.node_ids
                .iter()
                .enumerate()
                .map(|(i, id)| (id.clone(), (i, p.content_hashes.get(i).cloned())))
                .collect()
        })
        .unwrap_or_default();

    for node in graph.get_all_nodes() {
        let Some(text) = symbol_passage(&node, config.model) else {
            continue;
        };
        let hash = passage_hash(&text);
        let reuse = prior_lookup.get(&node.id).and_then(|(idx, old_hash)| {
            if old_hash.as_deref() == Some(hash.as_str()) {
                Some(*idx)
            } else {
                None
            }
        });
        node_ids.push(node.id.clone());
        content_hashes.push(hash);
        texts.push(text);
        reuse_indices.push(reuse);
    }

    let dim = config.matryoshka_dim;
    let mut vectors_flat = vec![0.0f32; node_ids.len() * dim];
    let batch_size = index_embed_batch_size();

    let mut embed_batch: Vec<String> = Vec::new();
    let mut embed_batch_targets: Vec<usize> = Vec::new();

    for (out_idx, reuse) in reuse_indices.iter().enumerate() {
        if let Some(prior_idx) = reuse {
            if let Some(prior) = prior.as_ref() {
                let dst = out_idx * dim;
                if dst + dim <= vectors_flat.len()
                    && copy_prior_vector(prior, *prior_idx, dim, &mut vectors_flat[dst..dst + dim])
                {
                    continue;
                }
            }
        }
        embed_batch.push(texts[out_idx].clone());
        embed_batch_targets.push(out_idx);
    }

    if !embed_batch.is_empty() {
        let arc = Embedder::lazy_global(config.clone())
            .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
        let mut embedder = arc.lock();
        let mut batch_out = 0usize;
        while batch_out < embed_batch.len() {
            let end = (batch_out + batch_size).min(embed_batch.len());
            let chunk = &embed_batch[batch_out..end];
            let targets = &embed_batch_targets[batch_out..end];
            let embedded = embedder
                .embed_documents(chunk)
                .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
            for (vec, &target) in embedded.into_iter().zip(targets) {
                let dst = target * dim;
                if vec.len() == dim {
                    vectors_flat[dst..dst + dim].copy_from_slice(&vec);
                }
            }
            batch_out = end;
        }
    }

    let module_centroids = compute_module_centroids(
        graph,
        &node_ids,
        &vectors_flat,
        dim,
        config.module_cluster_enabled,
    );

    let quant_scale = DEFAULT_QUANT_SCALE;
    let (vectors_i8, quant_scales) = quantize_matrix(&vectors_flat, dim);

    let index = EmbeddingIndex {
        model_id: config.model.as_str().to_string(),
        dim,
        graph_generation: graph.generation(),
        graph_digest: digest.clone(),
        node_ids: node_ids.clone(),
        vectors: Vec::new(),
        module_centroids: module_centroids.clone(),
        content_hashes: content_hashes.clone(),
        vectors_i8: vectors_i8.clone(),
        quant_scales: quant_scales.clone(),
        quant_scale,
    };

    let sidecar = EmbeddingSidecar {
        version: SIDECAR_VERSION,
        model_id: index.model_id.clone(),
        dim: index.dim,
        graph_generation: index.graph_generation,
        graph_digest: digest,
        node_ids: index.node_ids.clone(),
        vectors: Vec::new(),
        module_centroids,
        content_hashes: index.content_hashes.clone(),
        vectors_i8,
        quant_scales,
        quant_scale,
    };
    save_sidecar(&path, &sidecar)?;

    Ok(index)
}

pub fn maybe_rebuild_embeddings(
    graph: &NeuralProjectGraph,
    workspace: &Path,
    config: &EmbeddingConfig,
) -> neuromesh_core::Result<()> {
    if !config.enabled || !config.index_on_build {
        return Ok(());
    }
    let path = neuromesh_core::embeddings_path(workspace);
    let digest = graph_digest(graph);
    if let Some(sidecar) = crate::embeddings::load_sidecar(&path)? {
        if sidecar.is_compatible(
            config.model.as_str(),
            config.matryoshka_dim,
            graph.generation(),
            &digest,
        ) {
            graph.install_embedding_index(EmbeddingIndex::from_sidecar(sidecar));
            return Ok(());
        }
    }
    let index = rebuild_embeddings(graph, workspace, config)?;
    graph.install_embedding_index(index);
    Ok(())
}

/// Force embedding sidecar rebuild (graph-first index uses this explicitly).
pub fn rebuild_embeddings_for_workspace(
    graph: &NeuralProjectGraph,
    workspace: &Path,
    config: &EmbeddingConfig,
) -> neuromesh_core::Result<()> {
    if !config.enabled {
        return Ok(());
    }
    let index = rebuild_embeddings(graph, workspace, config)?;
    graph.install_embedding_index(index);
    Ok(())
}

/// Load a compatible sidecar or incrementally rebuild after graph index (MCP background).
pub fn refresh_embeddings_after_index(
    graph: &NeuralProjectGraph,
    workspace: &Path,
    config: &EmbeddingConfig,
) -> neuromesh_core::Result<()> {
    if !config.enabled {
        return Ok(());
    }
    let path = neuromesh_core::embeddings_path(workspace);
    let digest = graph_digest(graph);
    if let Some(sidecar) = crate::embeddings::load_sidecar(&path)? {
        if sidecar.is_compatible(
            config.model.as_str(),
            config.matryoshka_dim,
            graph.generation(),
            &digest,
        ) {
            graph.install_embedding_index(EmbeddingIndex::from_sidecar(sidecar));
            return Ok(());
        }
    }
    rebuild_embeddings_for_workspace(graph, workspace, config)
}
