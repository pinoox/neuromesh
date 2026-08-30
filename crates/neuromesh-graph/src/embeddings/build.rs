use crate::embeddings::{
    node_type_label, save_sidecar, symbol_sketch, EmbeddingIndex, EmbeddingSidecar, SIDECAR_VERSION,
};
use crate::NeuralProjectGraph;
use neuromesh_core::{EmbeddingConfig, EmbeddingModelId};
use neuromesh_embed::{format_document, Embedder};
use std::path::Path;

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

pub fn rebuild_embeddings(
    graph: &NeuralProjectGraph,
    workspace: &Path,
    config: &EmbeddingConfig,
) -> neuromesh_core::Result<EmbeddingIndex> {
    let mut embedder = Embedder::try_new(config.clone())
        .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
    let mut node_ids = Vec::new();
    let mut texts = Vec::new();
    for node in graph.get_all_nodes() {
        let Some(mut sketch) = symbol_sketch(&node) else {
            continue;
        };
        if config.model == EmbeddingModelId::Gemma300mQ4 {
            let path = node.file_path.to_string_lossy().replace('\\', "/");
            let title = format!("{}::{}", path, node.name);
            let sig = node.signature.as_deref().unwrap_or("");
            sketch = format_document(&title, node_type_label(node.node_type), sig);
        }
        node_ids.push(node.id.clone());
        texts.push(sketch);
    }

    let vectors_flat = if texts.is_empty() {
        Vec::new()
    } else {
        let mut all = Vec::new();
        for chunk in texts.chunks(32) {
            let batch: Vec<String> = chunk.to_vec();
            let embedded = embedder
                .embed_documents(&batch)
                .map_err(|e| neuromesh_core::NeuroMeshError::Internal(e.to_string()))?;
            for mut vec in embedded {
                all.append(&mut vec);
            }
        }
        all
    };

    let digest = graph_digest(graph);
    let sidecar = EmbeddingSidecar {
        version: SIDECAR_VERSION,
        model_id: config.model.as_str().to_string(),
        dim: config.matryoshka_dim,
        graph_generation: graph.generation(),
        graph_digest: digest.clone(),
        node_ids: node_ids.clone(),
        vectors: vectors_flat.clone(),
    };
    let path = neuromesh_core::embeddings_path(workspace);
    save_sidecar(&path, &sidecar)?;

    Ok(EmbeddingIndex {
        model_id: config.model.as_str().to_string(),
        dim: config.matryoshka_dim,
        graph_generation: graph.generation(),
        graph_digest: digest,
        node_ids,
        vectors: vectors_flat,
    })
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
