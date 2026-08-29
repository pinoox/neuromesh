use neuromesh_core::{NodeId, SeedResolution};
use neuromesh_graph::NeuralProjectGraph;
use std::collections::HashMap;

pub struct SeedBuffers<'res, 'eng, 'rsn> {
    pub resolutions: &'res mut Vec<SeedResolution>,
    pub energies: &'eng mut HashMap<NodeId, f32>,
    pub reasons: &'rsn mut HashMap<NodeId, String>,
}

pub type ResolveSeedFn = fn(&NeuralProjectGraph, &str, &str) -> Option<(NodeId, f32)>;

pub struct SeedSink<'res, 'eng, 'rsn> {
    resolutions: &'res mut Vec<SeedResolution>,
    energies: &'eng mut HashMap<NodeId, f32>,
    reasons: &'rsn mut HashMap<NodeId, String>,
    resolve: ResolveSeedFn,
}

impl<'res, 'eng, 'rsn> SeedSink<'res, 'eng, 'rsn> {
    pub fn new(
        resolutions: &'res mut Vec<SeedResolution>,
        energies: &'eng mut HashMap<NodeId, f32>,
        reasons: &'rsn mut HashMap<NodeId, String>,
        resolve: ResolveSeedFn,
    ) -> Self {
        Self {
            resolutions,
            energies,
            reasons,
            resolve,
        }
    }

    pub fn resolutions(&self) -> &[SeedResolution] {
        self.resolutions
    }

    pub fn resolved_count(&self) -> usize {
        self.resolutions
            .iter()
            .filter(|s| s.resolved_id.is_some())
            .count()
    }

    pub fn push(
        &mut self,
        graph: &NeuralProjectGraph,
        prompt: &str,
        query: String,
        energy: f32,
        reason: &str,
    ) {
        if self.resolutions.iter().any(|s| {
            s.resolved_id.is_some()
                && (s.query == query || s.query == format!("identifier:{query}"))
        }) {
            return;
        }
        if self.resolutions.iter().any(|s| s.query == query) {
            return;
        }
        if let Some((mut id, conf)) = (self.resolve)(graph, &query, prompt) {
            if graph
                .get_node(&id)
                .is_some_and(|n| n.node_type == neuromesh_core::NodeType::File)
            {
                if let Some(hit) = graph.search_symbols(&query, 6).into_iter().find(|hit| {
                    hit.name.eq_ignore_ascii_case(&query)
                        && hit.node_type != neuromesh_core::NodeType::File
                }) {
                    id = hit.id;
                }
            }
            self.insert(id, energy.max(conf), format!("{reason}:{query}"));
        } else {
            self.resolutions.push(SeedResolution {
                query,
                resolved_id: None,
                confidence: 0.0,
            });
        }
    }

    pub fn insert(&mut self, id: NodeId, energy: f32, reason: String) {
        self.energies
            .entry(id.clone())
            .and_modify(|e| *e = (*e).max(energy))
            .or_insert(energy);
        self.reasons.entry(id.clone()).or_insert(reason.clone());
        if !self
            .resolutions
            .iter()
            .any(|s| s.resolved_id.as_ref() == Some(&id))
        {
            self.resolutions.push(SeedResolution {
                query: reason,
                resolved_id: Some(id),
                confidence: energy,
            });
        }
    }

    pub fn buffers_mut(&mut self) -> SeedBuffers<'_, '_, '_> {
        SeedBuffers {
            resolutions: self.resolutions,
            energies: self.energies,
            reasons: self.reasons,
        }
    }
}
