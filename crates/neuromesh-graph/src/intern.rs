use neuromesh_core::{ContextEdge, ContextNode, EdgeId, NodeId};
use std::collections::{HashMap, HashSet};

/// Compact interned mesh: nodes and edges live in slot vectors, adjacency is `u32`.
#[derive(Default)]
pub(crate) struct MeshStore {
    node_of: HashMap<NodeId, u32>,
    nodes: Vec<Option<ContextNode>>,
    outgoing: Vec<Vec<u32>>,
    incoming: Vec<Vec<u32>>,
    free_nodes: Vec<u32>,
    edge_of: HashMap<EdgeId, u32>,
    edges: Vec<Option<ContextEdge>>,
    free_edges: Vec<u32>,
}

impl MeshStore {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn node_count(&self) -> usize {
        self.node_of.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edge_of.len()
    }

    pub fn node(&self, id: &NodeId) -> Option<&ContextNode> {
        let slot = *self.node_of.get(id)?;
        self.nodes.get(slot as usize)?.as_ref()
    }

    #[allow(dead_code)]
    pub fn node_mut(&mut self, id: &NodeId) -> Option<&mut ContextNode> {
        let slot = *self.node_of.get(id)?;
        self.nodes.get_mut(slot as usize)?.as_mut()
    }

    #[allow(dead_code)]
    pub fn edge(&self, id: &EdgeId) -> Option<&ContextEdge> {
        let slot = *self.edge_of.get(id)?;
        self.edges.get(slot as usize)?.as_ref()
    }

    pub fn edge_mut(&mut self, id: &EdgeId) -> Option<&mut ContextEdge> {
        let slot = *self.edge_of.get(id)?;
        self.edges.get_mut(slot as usize)?.as_mut()
    }

    pub fn insert_node(&mut self, node: ContextNode) -> u32 {
        if let Some(&slot) = self.node_of.get(&node.id) {
            if let Some(slot_node) = self.nodes.get_mut(slot as usize) {
                *slot_node = Some(node);
            }
            return slot;
        }
        let slot = if let Some(free) = self.free_nodes.pop() {
            free
        } else {
            let slot = self.nodes.len() as u32;
            self.nodes.push(None);
            self.outgoing.push(Vec::new());
            self.incoming.push(Vec::new());
            slot
        };
        self.node_of.insert(node.id.clone(), slot);
        self.nodes[slot as usize] = Some(node);
        slot
    }

    pub fn insert_edge(&mut self, edge: ContextEdge) -> Option<u32> {
        if self.edge_of.contains_key(&edge.id) {
            return self.edge_of.get(&edge.id).copied();
        }
        let source = *self.node_of.get(&edge.source)?;
        let target = *self.node_of.get(&edge.target)?;
        let slot = if let Some(free) = self.free_edges.pop() {
            free
        } else {
            let slot = self.edges.len() as u32;
            self.edges.push(None);
            slot
        };
        self.edge_of.insert(edge.id.clone(), slot);
        self.outgoing[source as usize].push(slot);
        self.incoming[target as usize].push(slot);
        self.edges[slot as usize] = Some(edge);
        Some(slot)
    }

    pub fn remove_nodes(&mut self, ids: &[NodeId]) {
        let drop_slots: HashSet<u32> = ids
            .iter()
            .filter_map(|id| self.node_of.get(id).copied())
            .collect();
        if drop_slots.is_empty() {
            return;
        }
        let mut drop_edges: HashSet<u32> = HashSet::new();
        for &slot in &drop_slots {
            drop_edges.extend(self.outgoing.get(slot as usize).into_iter().flatten());
            drop_edges.extend(self.incoming.get(slot as usize).into_iter().flatten());
        }
        for (edge_slot, edge) in self.edges.iter().enumerate() {
            if let Some(edge) = edge {
                let src = self.node_of.get(&edge.source).copied();
                let tgt = self.node_of.get(&edge.target).copied();
                if src.is_some_and(|s| drop_slots.contains(&s))
                    || tgt.is_some_and(|t| drop_slots.contains(&t))
                {
                    drop_edges.insert(edge_slot as u32);
                }
            }
        }
        for slot in drop_edges {
            self.remove_edge_slot(slot);
        }
        for id in ids {
            if let Some(slot) = self.node_of.remove(id) {
                self.nodes[slot as usize] = None;
                self.outgoing[slot as usize].clear();
                self.incoming[slot as usize].clear();
                self.free_nodes.push(slot);
            }
        }
    }

    fn remove_edge_slot(&mut self, slot: u32) {
        let Some(edge) = self.edges.get_mut(slot as usize).and_then(Option::take) else {
            return;
        };
        self.edge_of.remove(&edge.id);
        if let Some(&src) = self.node_of.get(&edge.source) {
            if let Some(list) = self.outgoing.get_mut(src as usize) {
                list.retain(|existing| *existing != slot);
            }
        }
        if let Some(&tgt) = self.node_of.get(&edge.target) {
            if let Some(list) = self.incoming.get_mut(tgt as usize) {
                list.retain(|existing| *existing != slot);
            }
        }
        self.free_edges.push(slot);
    }

    pub fn inbound_edges(&self, ids: &[NodeId]) -> Vec<ContextEdge> {
        let drop_slots: HashSet<u32> = ids
            .iter()
            .filter_map(|id| self.node_of.get(id).copied())
            .collect();
        let mut out = Vec::new();
        for &slot in &drop_slots {
            for &edge_slot in self.incoming.get(slot as usize).into_iter().flatten() {
                if let Some(edge) = self.edges.get(edge_slot as usize).and_then(|e| e.as_ref()) {
                    if !drop_slots
                        .contains(&self.node_of.get(&edge.source).copied().unwrap_or(u32::MAX))
                    {
                        out.push(edge.clone());
                    }
                }
            }
        }
        out
    }

    pub fn neighbors(&self, id: &NodeId) -> Vec<(NodeId, ContextEdge)> {
        let Some(&slot) = self.node_of.get(id) else {
            return Vec::new();
        };
        let mut neighbors = Vec::new();
        for &edge_slot in self.outgoing.get(slot as usize).into_iter().flatten() {
            if let Some(edge) = self.edges.get(edge_slot as usize).and_then(|e| e.as_ref()) {
                neighbors.push((edge.target.clone(), edge.clone()));
            }
        }
        for &edge_slot in self.incoming.get(slot as usize).into_iter().flatten() {
            if let Some(edge) = self.edges.get(edge_slot as usize).and_then(|e| e.as_ref()) {
                neighbors.push((edge.source.clone(), edge.clone()));
            }
        }
        neighbors
    }

    pub fn outgoing_to(&self, source: &NodeId, target: &NodeId) -> Vec<EdgeId> {
        let Some(&slot) = self.node_of.get(source) else {
            return Vec::new();
        };
        self.outgoing
            .get(slot as usize)
            .into_iter()
            .flatten()
            .filter_map(|edge_slot| {
                self.edges
                    .get(*edge_slot as usize)
                    .and_then(|e| e.as_ref())
                    .filter(|edge| edge.target == *target)
                    .map(|edge| edge.id.clone())
            })
            .collect()
    }

    pub fn neighborhood(&self, seeds: &HashSet<NodeId>, hops: usize) -> HashSet<NodeId> {
        let mut visited = seeds.clone();
        let mut frontier: Vec<(u32, usize)> = seeds
            .iter()
            .filter_map(|id| self.node_of.get(id).map(|&slot| (slot, 0)))
            .collect();
        let mut i = 0;
        while i < frontier.len() {
            let (slot, depth) = frontier[i];
            i += 1;
            if depth >= hops {
                continue;
            }
            let mut next_slots = Vec::new();
            for &edge_slot in self.outgoing.get(slot as usize).into_iter().flatten() {
                if let Some(edge) = self.edges.get(edge_slot as usize).and_then(|e| e.as_ref()) {
                    if let Some(&nslot) = self.node_of.get(&edge.target) {
                        next_slots.push((nslot, edge.target.clone()));
                    }
                }
            }
            for &edge_slot in self.incoming.get(slot as usize).into_iter().flatten() {
                if let Some(edge) = self.edges.get(edge_slot as usize).and_then(|e| e.as_ref()) {
                    if let Some(&nslot) = self.node_of.get(&edge.source) {
                        next_slots.push((nslot, edge.source.clone()));
                    }
                }
            }
            for (nslot, nid) in next_slots {
                if visited.insert(nid) {
                    frontier.push((nslot, depth + 1));
                }
            }
        }
        visited
    }

    pub fn subgraph(
        &self,
        nodes: &HashSet<NodeId>,
    ) -> (HashMap<NodeId, ContextNode>, HashMap<EdgeId, ContextEdge>) {
        let node_map: HashMap<NodeId, ContextNode> = nodes
            .iter()
            .filter_map(|id| self.node(id).cloned().map(|n| (id.clone(), n)))
            .collect();
        let mut edge_map = HashMap::new();
        for id in nodes {
            let Some(&slot) = self.node_of.get(id) else {
                continue;
            };
            for &edge_slot in self.outgoing.get(slot as usize).into_iter().flatten() {
                if let Some(edge) = self.edges.get(edge_slot as usize).and_then(|e| e.as_ref()) {
                    if nodes.contains(&edge.target) {
                        edge_map.insert(edge.id.clone(), edge.clone());
                    }
                }
            }
        }
        (node_map, edge_map)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &ContextNode> {
        self.nodes.iter().filter_map(|n| n.as_ref())
    }

    pub fn edges(&self) -> impl Iterator<Item = &ContextEdge> {
        self.edges.iter().filter_map(|e| e.as_ref())
    }

    pub fn edges_mut(&mut self) -> impl Iterator<Item = &mut ContextEdge> {
        self.edges.iter_mut().filter_map(|e| e.as_mut())
    }

    #[allow(dead_code)]
    pub fn node_ids(&self) -> impl Iterator<Item = &NodeId> {
        self.node_of.keys()
    }

    pub fn nodes_map(&self) -> HashMap<NodeId, ContextNode> {
        self.nodes().map(|n| (n.id.clone(), n.clone())).collect()
    }

    pub fn edges_map(&self) -> HashMap<EdgeId, ContextEdge> {
        self.edges().map(|e| (e.id.clone(), e.clone())).collect()
    }

    pub fn snapshot_nodes(&self) -> Vec<ContextNode> {
        self.nodes().cloned().collect()
    }

    pub fn snapshot_edges(&self) -> Vec<ContextEdge> {
        self.edges().cloned().collect()
    }

    pub fn load_lists(&mut self, nodes: Vec<ContextNode>, edges: Vec<ContextEdge>) {
        self.clear();
        for node in nodes {
            let mut node = node;
            node.content = None;
            self.insert_node(node);
        }
        for edge in edges {
            let _ = self.insert_edge(edge);
        }
    }
}

use crate::query::tokenize;
use neuromesh_core::{EdgeType, UnresolvedRef};
use neuromesh_index::{FileFingerprint, IndexedFile};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(crate) struct GraphData {
    pub mesh: MeshStore,
    pub name_to_nodes: BTreeMap<String, Vec<NodeId>>,
    pub file_to_nodes: HashMap<PathBuf, Vec<NodeId>>,
    pub token_to_nodes: HashMap<String, Vec<NodeId>>,
    pub pending: Vec<PendingRel>,
    pub unresolved: Vec<UnresolvedRef>,
    pub impl_index: HashMap<String, Vec<NodeId>>,
    pub export_index: HashMap<String, Vec<NodeId>>,
    pub file_hashes: HashMap<String, String>,
    pub file_fingerprints: HashMap<String, FileFingerprint>,
    pub generation: u64,
    pub indexed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub stale_files: Vec<String>,
    pub workspace_root: Option<PathBuf>,
    pub source_overlay: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct GraphSnapshot {
    pub version: u32,
    pub nodes: Vec<ContextNode>,
    pub edges: Vec<ContextEdge>,
    pub pending: Vec<PendingRel>,
    pub unresolved: Vec<UnresolvedRef>,
    pub file_hashes: HashMap<String, String>,
    #[serde(default)]
    pub file_fingerprints: HashMap<String, FileFingerprint>,
    #[serde(default)]
    pub export_index: HashMap<String, Vec<NodeId>>,
    pub generation: u64,
    pub indexed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub stale_files: Vec<String>,
    #[serde(default)]
    pub workspace_root: Option<PathBuf>,
}

#[derive(Deserialize)]
pub(crate) struct LegacyGraphData {
    pub nodes: HashMap<NodeId, ContextNode>,
    pub edges: HashMap<EdgeId, ContextEdge>,
    pub pending: Vec<PendingRel>,
    pub unresolved: Vec<UnresolvedRef>,
    #[serde(default)]
    pub export_index: HashMap<String, Vec<NodeId>>,
    pub file_hashes: HashMap<String, String>,
    pub generation: u64,
    pub indexed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub stale_files: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct PendingRel {
    pub source_file: PathBuf,
    pub source_symbol: String,
    pub target_symbol: String,
    pub relationship: EdgeType,
    pub target_file_hint: Option<String>,
    #[serde(default)]
    pub receiver_hint: Option<String>,
}

pub(crate) fn insert_indexed_node(data: &mut GraphData, node: ContextNode) {
    let id = node.id.clone();
    let path = node.file_path.clone();
    let name = node.name.clone();
    let parent = node.parent.clone();
    data.mesh.insert_node(node);
    data.file_to_nodes.entry(path).or_default().push(id.clone());
    data.name_to_nodes
        .entry(name.to_lowercase())
        .or_default()
        .push(id.clone());
    index_tokens(data, &id, &name);
    if let Some(parent) = parent {
        let key = format!("{}::{}", parent.to_lowercase(), name.to_lowercase());
        data.impl_index.entry(key).or_default().push(id);
    }
}

pub(crate) fn capture_inbound_pending(data: &GraphData, path: &Path) -> Vec<PendingRel> {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let ids: Vec<NodeId> = data
        .file_to_nodes
        .iter()
        .filter(|(stored, _)| stored.to_string_lossy().replace('\\', "/") == normalized)
        .flat_map(|(_, ids)| ids.iter().cloned())
        .collect();
    let mut pending = Vec::new();
    for edge in data.mesh.inbound_edges(&ids) {
        if !matches!(
            edge.edge_type,
            EdgeType::Calls
                | EdgeType::Imports
                | EdgeType::DependsOn
                | EdgeType::References
                | EdgeType::UsedBy
        ) {
            continue;
        }
        let Some(source) = data.mesh.node(&edge.source) else {
            continue;
        };
        let Some(target) = data.mesh.node(&edge.target) else {
            continue;
        };
        pending.push(PendingRel {
            source_file: source.file_path.clone(),
            source_symbol: source.name.clone(),
            target_symbol: target.name.clone(),
            relationship: edge.edge_type,
            target_file_hint: Some(target.file_path.to_string_lossy().replace('\\', "/")),
            receiver_hint: None,
        });
    }
    pending
}

pub(crate) fn remove_file_nodes_locked(data: &mut GraphData, path: &Path) {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let keys: Vec<PathBuf> = data
        .file_to_nodes
        .keys()
        .filter(|p| p.to_string_lossy().replace('\\', "/") == normalized)
        .cloned()
        .collect();
    if keys.is_empty() {
        data.file_hashes.remove(&normalized);
        data.file_fingerprints.remove(&normalized);
        data.source_overlay.remove(&normalized);
        return;
    }
    let mut ids = Vec::new();
    for key in keys {
        if let Some(list) = data.file_to_nodes.remove(&key) {
            ids.extend(list);
        }
    }
    for id in &ids {
        if let Some(node) = data.mesh.node(id).cloned() {
            if let Some(list) = data.name_to_nodes.get_mut(&node.name.to_lowercase()) {
                list.retain(|existing| existing != id);
            }
            if let Some(parent) = &node.parent {
                let key = format!("{}::{}", parent.to_lowercase(), node.name.to_lowercase());
                if let Some(list) = data.impl_index.get_mut(&key) {
                    list.retain(|existing| existing != id);
                }
            }
            if let Some(list) = data.export_index.get_mut(&node.name.to_lowercase()) {
                list.retain(|existing| existing != id);
            }
        }
    }
    data.mesh.remove_nodes(&ids);
    data.file_hashes.remove(&normalized);
    data.file_fingerprints.remove(&normalized);
    data.source_overlay.remove(&normalized);
}

pub(crate) fn rebuild_indexes(data: &mut GraphData) {
    data.name_to_nodes.clear();
    data.file_to_nodes.clear();
    data.token_to_nodes.clear();
    data.impl_index.clear();
    let nodes = data.mesh.snapshot_nodes();
    for node in nodes {
        let id = node.id.clone();
        data.file_to_nodes
            .entry(node.file_path.clone())
            .or_default()
            .push(id.clone());
        data.name_to_nodes
            .entry(node.name.to_lowercase())
            .or_default()
            .push(id.clone());
        index_tokens(data, &id, &node.name);
        if let Some(parent) = &node.parent {
            let key = format!("{}::{}", parent.to_lowercase(), node.name.to_lowercase());
            data.impl_index.entry(key).or_default().push(id);
        }
    }
}

pub(crate) fn infer_workspace_root(file: &IndexedFile) -> Option<PathBuf> {
    let full = file.full_path.to_string_lossy().replace('\\', "/");
    let rel = file.relative_path.to_string_lossy().replace('\\', "/");
    let trimmed = full.strip_suffix(rel.as_str())?.trim_end_matches('/');
    if trimmed.is_empty() {
        return file.full_path.parent().map(Path::to_path_buf);
    }
    Some(PathBuf::from(trimmed))
}

pub(crate) fn index_tokens(data: &mut GraphData, id: &NodeId, name: &str) {
    for token in tokenize(name) {
        let ids = data.token_to_nodes.entry(token).or_default();
        if !ids.iter().any(|existing| existing == id) {
            ids.push(id.clone());
        }
    }
}
