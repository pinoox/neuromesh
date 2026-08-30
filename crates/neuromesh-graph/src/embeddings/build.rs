use crate::embeddings::file_passage::file_passage as compose_file_passage;
use crate::embeddings::{
    quantize::quantize_matrix, save_sidecar_atomic, sidecar_lock::with_sidecar_write,
    symbol_sketch, EmbeddingIndex, EmbeddingSidecar, ModuleCentroid, DEFAULT_QUANT_SCALE,
};
use crate::NeuralProjectGraph;
use neuromesh_core::{ContextNode, EmbeddingConfig, EmbeddingModelId, NodeType};
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

pub fn passage_hash(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

pub fn symbol_passage_for_node(node: &ContextNode, model: EmbeddingModelId) -> Option<String> {
    if model == EmbeddingModelId::MiniLmMultilingualQ {
        let path = node.file_path.to_string_lossy().replace('\\', "/");
        let title = format!("{}::{}", path, node.name);
        let sig = node.signature.as_deref().unwrap_or("");
        Some(format_document_for_model(
            model,
            &title,
            crate::embeddings::node_type_label(node.node_type),
            sig,
            node.doc_summary.as_deref(),
        ))
    } else {
        symbol_sketch(node)
    }
}

fn copy_prior_symbol_vector(
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

fn copy_prior_file_vector(
    prior: &EmbeddingSidecar,
    prior_idx: usize,
    dim: usize,
    dst: &mut [f32],
) -> bool {
    let start = prior_idx * dim;
    let end = start + dim;
    if end > prior.file_vectors_i8.len() {
        return false;
    }
    let scale = prior
        .file_quant_scales
        .get(prior_idx)
        .copied()
        .filter(|s| *s > f32::EPSILON)
        .unwrap_or(prior.quant_scale);
    crate::embeddings::quantize::dequant_slice(&prior.file_vectors_i8[start..end], scale, dst);
    true
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

fn embed_batch(
    config: &EmbeddingConfig,
    texts: &[String],
) -> neuromesh_core::Result<Vec<Vec<f32>>> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let arc = Embedder::lazy_global(config.clone())
        .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
    let mut embedder = arc.lock();
    let batch_size = index_embed_batch_size();
    let mut out = Vec::with_capacity(texts.len());
    let mut batch_out = 0usize;
    while batch_out < texts.len() {
        let end = (batch_out + batch_size).min(texts.len());
        let chunk = &texts[batch_out..end];
        let embedded = embedder
            .embed_documents(chunk)
            .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
        out.extend(embedded);
        batch_out = end;
    }
    Ok(out)
}

fn rebuild_hierarchical(
    graph: &NeuralProjectGraph,
    workspace: &Path,
    config: &EmbeddingConfig,
    prior: Option<&EmbeddingSidecar>,
    digest: &str,
) -> neuromesh_core::Result<EmbeddingIndex> {
    let dim = config.matryoshka_dim;

    // --- Tier-0: files ---
    let mut file_node_ids = Vec::new();
    let mut file_texts = Vec::new();
    let mut file_hashes = Vec::new();
    let mut file_reuse: Vec<Option<usize>> = Vec::new();

    let prior_file_lookup: HashMap<_, _> = prior
        .filter(|p| p.is_hierarchical() && p.model_id == config.model.as_str() && p.dim == dim)
        .map(|p| {
            p.file_node_ids
                .iter()
                .enumerate()
                .map(|(i, id)| (id.clone(), (i, p.file_content_hashes.get(i).cloned())))
                .collect()
        })
        .unwrap_or_default();

    for (file_id, _path) in graph.file_node_paths() {
        let Some(file_node) = graph.get_node(&file_id) else {
            continue;
        };
        let Some(text) = compose_file_passage(graph, &file_node, config.model) else {
            continue;
        };
        let hash = passage_hash(&text);
        let reuse = prior_file_lookup.get(&file_id).and_then(|(idx, old)| {
            if old.as_deref() == Some(hash.as_str()) {
                Some(*idx)
            } else {
                None
            }
        });
        file_node_ids.push(file_id);
        file_hashes.push(hash);
        file_texts.push(text);
        file_reuse.push(reuse);
    }

    let mut file_vectors_flat = vec![0.0f32; file_node_ids.len() * dim];
    let mut file_embed_batch = Vec::new();
    let mut file_embed_targets = Vec::new();
    for (out_idx, reuse) in file_reuse.iter().enumerate() {
        if let Some(prior_idx) = reuse {
            if let Some(prior) = prior {
                let dst = out_idx * dim;
                if dst + dim <= file_vectors_flat.len()
                    && copy_prior_file_vector(
                        prior,
                        *prior_idx,
                        dim,
                        &mut file_vectors_flat[dst..dst + dim],
                    )
                {
                    continue;
                }
            }
        }
        file_embed_batch.push(file_texts[out_idx].clone());
        file_embed_targets.push(out_idx);
    }

    if !file_embed_batch.is_empty() {
        let embedded = embed_batch(config, &file_embed_batch)?;
        for (vec, &target) in embedded.into_iter().zip(file_embed_targets.iter()) {
            let dst = target * dim;
            if vec.len() == dim {
                file_vectors_flat[dst..dst + dim].copy_from_slice(&vec);
            }
        }
    }

    let (file_vectors_i8, file_quant_scales) = quantize_matrix(&file_vectors_flat, dim);

    // --- Tier-1: symbols (lazy at cold rebuild — copy unchanged from prior v6) ---
    let mut symbol_node_ids = Vec::new();
    let mut symbol_hashes = Vec::new();
    let mut symbol_file_index = Vec::new();
    let mut symbol_vectors_flat = Vec::new();
    let mut symbol_quant_scales = Vec::new();

    if let Some(prior) = prior.filter(|p| p.is_hierarchical()) {
        let file_id_to_row: HashMap<_, _> = file_node_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i))
            .collect();

        for (sym_row, sym_id) in prior.node_ids.iter().enumerate() {
            let Some(node) = graph.get_node(sym_id) else {
                continue;
            };
            let Some(text) = symbol_passage_for_node(&node, config.model) else {
                continue;
            };
            let hash = passage_hash(&text);
            if prior.content_hashes.get(sym_row).map(String::as_str) != Some(hash.as_str()) {
                continue;
            }
            let file_id = graph.file_id_for_path(&node.file_path);
            let file_row = file_id
                .as_ref()
                .and_then(|fid| file_id_to_row.get(fid).copied())
                .unwrap_or_else(|| {
                    prior
                        .symbol_file_index
                        .get(sym_row)
                        .map(|&i| i as usize)
                        .unwrap_or(0)
                }) as u32;

            let mut vec_buf = vec![0.0f32; dim];
            if !copy_prior_symbol_vector(prior, sym_row, dim, &mut vec_buf) {
                continue;
            }
            symbol_node_ids.push(sym_id.clone());
            symbol_hashes.push(hash);
            symbol_file_index.push(file_row);
            symbol_vectors_flat.extend_from_slice(&vec_buf);
            symbol_quant_scales.push(
                prior
                    .quant_scales
                    .get(sym_row)
                    .copied()
                    .filter(|s| *s > f32::EPSILON)
                    .unwrap_or(prior.quant_scale),
            );
        }
    }

    let (symbol_vectors_i8, symbol_scales_out) = if symbol_vectors_flat.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        quantize_matrix(&symbol_vectors_flat, dim)
    };
    let symbol_quant_scales = if symbol_scales_out.is_empty() {
        symbol_quant_scales
    } else {
        symbol_scales_out
    };

    let module_centroids = compute_module_centroids(
        graph,
        &symbol_node_ids,
        &symbol_vectors_flat,
        dim,
        config.module_cluster_enabled && !symbol_node_ids.is_empty(),
    );

    let mut index = EmbeddingIndex {
        model_id: config.model.as_str().to_string(),
        dim,
        graph_generation: graph.generation(),
        graph_digest: digest.to_string(),
        node_ids: symbol_node_ids,
        vectors: Vec::new(),
        module_centroids,
        content_hashes: symbol_hashes,
        vectors_i8: symbol_vectors_i8,
        quant_scales: symbol_quant_scales,
        quant_scale: DEFAULT_QUANT_SCALE,
        file_node_ids,
        file_content_hashes: file_hashes,
        file_vectors_i8,
        file_quant_scales,
        symbol_file_index,
        ..Default::default()
    };
    index.rebuild_lookup_maps();

    let sidecar = index.to_sidecar();
    let path = neuromesh_core::embeddings_path(workspace);
    save_sidecar_atomic(&path, &sidecar)?;

    Ok(index)
}

fn rebuild_flat_symbols(
    graph: &NeuralProjectGraph,
    workspace: &Path,
    config: &EmbeddingConfig,
    prior: Option<&EmbeddingSidecar>,
    digest: &str,
) -> neuromesh_core::Result<EmbeddingIndex> {
    let mut node_ids = Vec::new();
    let mut texts = Vec::new();
    let mut content_hashes = Vec::new();
    let mut reuse_indices: Vec<Option<usize>> = Vec::new();

    let prior_lookup: HashMap<_, _> = prior
        .filter(|p| p.model_id == config.model.as_str() && p.dim == config.matryoshka_dim)
        .map(|p| {
            p.node_ids
                .iter()
                .enumerate()
                .map(|(i, id)| (id.clone(), (i, p.content_hashes.get(i).cloned())))
                .collect()
        })
        .unwrap_or_default();

    for node in graph.get_all_nodes() {
        let Some(text) = symbol_passage_for_node(&node, config.model) else {
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
    let mut embed_batch_texts: Vec<String> = Vec::new();
    let mut embed_batch_targets: Vec<usize> = Vec::new();

    for (out_idx, reuse) in reuse_indices.iter().enumerate() {
        if let Some(prior_idx) = reuse {
            if let Some(prior) = prior {
                let dst = out_idx * dim;
                if dst + dim <= vectors_flat.len()
                    && copy_prior_symbol_vector(
                        prior,
                        *prior_idx,
                        dim,
                        &mut vectors_flat[dst..dst + dim],
                    )
                {
                    continue;
                }
            }
        }
        embed_batch_texts.push(texts[out_idx].clone());
        embed_batch_targets.push(out_idx);
    }

    if !embed_batch_texts.is_empty() {
        let embedded = embed_batch(config, &embed_batch_texts)?;
        for (vec, &target) in embedded.into_iter().zip(embed_batch_targets.iter()) {
            let dst = target * dim;
            if vec.len() == dim {
                vectors_flat[dst..dst + dim].copy_from_slice(&vec);
            }
        }
    }

    let module_centroids = compute_module_centroids(
        graph,
        &node_ids,
        &vectors_flat,
        dim,
        config.module_cluster_enabled,
    );

    let (vectors_i8, quant_scales) = quantize_matrix(&vectors_flat, dim);

    let mut index = EmbeddingIndex {
        model_id: config.model.as_str().to_string(),
        dim,
        graph_generation: graph.generation(),
        graph_digest: digest.to_string(),
        node_ids: node_ids.clone(),
        vectors: Vec::new(),
        module_centroids,
        content_hashes: content_hashes.clone(),
        vectors_i8,
        quant_scales,
        quant_scale: DEFAULT_QUANT_SCALE,
        ..Default::default()
    };
    index.rebuild_lookup_maps();

    let sidecar = EmbeddingSidecar {
        version: 5,
        model_id: index.model_id.clone(),
        dim: index.dim,
        graph_generation: index.graph_generation,
        graph_digest: digest.to_string(),
        node_ids,
        vectors: Vec::new(),
        module_centroids: index.module_centroids.clone(),
        content_hashes,
        vectors_i8: index.vectors_i8.clone(),
        quant_scales: index.quant_scales.clone(),
        quant_scale: index.quant_scale,
        file_node_ids: Vec::new(),
        file_content_hashes: Vec::new(),
        file_vectors_i8: Vec::new(),
        file_quant_scales: Vec::new(),
        symbol_file_index: Vec::new(),
    };
    let path = neuromesh_core::embeddings_path(workspace);
    save_sidecar_atomic(&path, &sidecar)?;

    Ok(index)
}

/// Build or refresh embeddings; reuses unchanged vectors from an existing sidecar.
pub fn rebuild_embeddings(
    graph: &NeuralProjectGraph,
    workspace: &Path,
    config: &EmbeddingConfig,
) -> neuromesh_core::Result<EmbeddingIndex> {
    with_sidecar_write(workspace, || {
        rebuild_embeddings_locked(graph, workspace, config)
    })
}

fn rebuild_embeddings_locked(
    graph: &NeuralProjectGraph,
    workspace: &Path,
    config: &EmbeddingConfig,
) -> neuromesh_core::Result<EmbeddingIndex> {
    let path = neuromesh_core::embeddings_path(workspace);
    let prior = crate::embeddings::load_sidecar(&path).ok().flatten();
    let digest = graph_digest(graph);

    if config.hierarchical_index {
        rebuild_hierarchical(graph, workspace, config, prior.as_ref(), &digest)
    } else {
        rebuild_flat_symbols(graph, workspace, config, prior.as_ref(), &digest)
    }
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
        let ok = if config.hierarchical_index {
            sidecar.is_compatible_hierarchical(
                config.model.as_str(),
                config.matryoshka_dim,
                graph.generation(),
                &digest,
            )
        } else {
            sidecar.is_compatible(
                config.model.as_str(),
                config.matryoshka_dim,
                graph.generation(),
                &digest,
            )
        };
        if ok {
            graph.install_embedding_index(EmbeddingIndex::from_sidecar(sidecar));
            return Ok(());
        }
    }
    let index = rebuild_embeddings(graph, workspace, config)?;
    graph.install_embedding_index(index);
    Ok(())
}

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
        let ok = if config.hierarchical_index {
            sidecar.is_compatible_hierarchical(
                config.model.as_str(),
                config.matryoshka_dim,
                graph.generation(),
                &digest,
            )
        } else {
            sidecar.is_compatible(
                config.model.as_str(),
                config.matryoshka_dim,
                graph.generation(),
                &digest,
            )
        };
        if ok {
            graph.install_embedding_index(EmbeddingIndex::from_sidecar(sidecar));
            return Ok(());
        }
    }
    rebuild_embeddings_for_workspace(graph, workspace, config)
}
