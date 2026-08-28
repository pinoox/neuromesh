use crate::fold::normalize_fold_query;
use crate::skeleton::FoldedIntron;
use neuromesh_core::{ContextNode, InactiveContextDescriptor, NodeId, ProjectId};
use parking_lot::RwLock;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

/// Folds hold `original_body` source text, so a long MCP session would otherwise
/// grow without bound. Folds from the current activation are never evicted.
pub const MAX_RETAINED_FOLDS: usize = 2_000;

#[derive(Debug, Clone)]
pub struct StoredFold {
    pub fold: FoldedIntron,
    pub file_path: PathBuf,
}

#[derive(Default)]
struct FoldStore {
    by_id: BTreeMap<String, StoredFold>,
    by_symbol: HashMap<String, Vec<String>>,
    /// `(activation epoch, fold id)` in insertion order, used for LRU trimming.
    order: VecDeque<(u64, String)>,
    epoch: u64,
}

impl FoldStore {
    fn insert(&mut self, fold: StoredFold) {
        let id = fold.fold.fold_id.clone();
        let symbol = fold.fold.symbol_name.to_ascii_lowercase();
        if self.by_id.insert(id.clone(), fold).is_none() {
            self.by_symbol.entry(symbol).or_default().push(id.clone());
            self.order.push_back((self.epoch, id));
        }
        self.trim();
    }

    fn trim(&mut self) {
        while self.by_id.len() > MAX_RETAINED_FOLDS {
            let Some((epoch, _)) = self.order.front() else {
                break;
            };
            if *epoch == self.epoch {
                break;
            }
            let Some((_, id)) = self.order.pop_front() else {
                break;
            };
            self.remove(&id);
        }
    }

    fn remove(&mut self, id: &str) {
        let Some(stored) = self.by_id.remove(id) else {
            return;
        };
        let symbol = stored.fold.symbol_name.to_ascii_lowercase();
        if let Some(ids) = self.by_symbol.get_mut(&symbol) {
            ids.retain(|existing| existing != id);
            if ids.is_empty() {
                self.by_symbol.remove(&symbol);
            }
        }
    }

    fn clear(&mut self) {
        self.by_id.clear();
        self.by_symbol.clear();
        self.order.clear();
    }
}

#[derive(Clone, Default)]
pub struct ReversibleContextRegistry {
    inactive_nodes: Arc<RwLock<HashMap<NodeId, InactiveContextDescriptor>>>,
    node_store: Arc<RwLock<HashMap<NodeId, ContextNode>>>,
    folds: Arc<RwLock<FoldStore>>,
    session_project: Arc<RwLock<Option<ProjectId>>>,
}

impl ReversibleContextRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_inactive(
        &self,
        node: &ContextNode,
        relevance: f32,
        confidence: f32,
        activation_score: f32,
        parent_node: Option<NodeId>,
    ) {
        let descriptor = InactiveContextDescriptor {
            id: node.id.clone(),
            file_path: node.file_path.clone(),
            line_range: node.line_range.clone(),
            content_hash: node.content_hash.clone(),
            version: 1,
            token_cost: node.token_cost,
            relevance,
            confidence,
            activation_score,
            parent_node,
        };

        self.inactive_nodes
            .write()
            .insert(node.id.clone(), descriptor);
        self.node_store
            .write()
            .insert(node.id.clone(), node.clone());
    }

    pub fn register_fold(&self, file_path: PathBuf, fold: FoldedIntron) {
        self.folds.write().insert(StoredFold { fold, file_path });
    }

    pub fn get_fold(&self, fold_id: &str) -> Option<StoredFold> {
        let query = normalize_fold_query(fold_id);
        if query.is_empty() {
            return None;
        }
        let folds = self.folds.read();
        if let Some(hit) = folds.by_id.get(&query) {
            return Some(hit.clone());
        }
        let prefix = format!("{query}_");
        let prefixed = folds
            .by_id
            .range(prefix.clone()..)
            .take_while(|(id, _)| id.starts_with(&prefix))
            .map(|(_, stored)| stored);
        if let Some(best) = best_by_task_score(prefixed) {
            return Some(best.clone());
        }
        let symbol_ids = folds.by_symbol.get(&query.to_ascii_lowercase())?;
        let by_symbol = symbol_ids.iter().filter_map(|id| folds.by_id.get(id));
        best_by_task_score(by_symbol).cloned()
    }

    pub fn get_inactive_descriptors(&self) -> Vec<InactiveContextDescriptor> {
        self.inactive_nodes.read().values().cloned().collect()
    }

    pub fn retrieve_and_activate(&self, id: &NodeId) -> Option<ContextNode> {
        self.inactive_nodes.write().remove(id);
        self.node_store.read().get(id).cloned()
    }

    pub fn begin_activate(&self, project_id: &ProjectId) {
        let mut session = self.session_project.write();
        let mut folds = self.folds.write();
        if session.as_ref() != Some(project_id) {
            folds.clear();
            *session = Some(project_id.clone());
        }
        folds.epoch = folds.epoch.wrapping_add(1);
        folds.trim();
        drop(folds);
        self.inactive_nodes.write().clear();
        self.node_store.write().clear();
    }

    pub fn fold_count(&self) -> usize {
        self.folds.read().by_id.len()
    }

    pub fn clear(&self) {
        self.inactive_nodes.write().clear();
        self.node_store.write().clear();
        self.folds.write().clear();
        *self.session_project.write() = None;
    }
}

/// Highest `task_score`, ties broken by the `BTreeMap`/insertion order of the input.
fn best_by_task_score<'a, I>(items: I) -> Option<&'a StoredFold>
where
    I: Iterator<Item = &'a StoredFold>,
{
    items.reduce(|best, candidate| {
        if candidate.fold.task_score > best.fold.task_score {
            candidate
        } else {
            best
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fold(id: &str, symbol: &str, task_score: f32) -> FoldedIntron {
        FoldedIntron {
            fold_id: id.to_string(),
            symbol_name: symbol.to_string(),
            signature: format!("fn {symbol}()"),
            original_body: "x".repeat(256),
            start_line: 1,
            end_line: 9,
            saved_tokens: 32,
            owner: None,
            task_score,
        }
    }

    #[test]
    fn fold_store_stays_bounded_across_activations() {
        let registry = ReversibleContextRegistry::new();
        let project = ProjectId::new("bounded");
        let per_activation = 300;

        for activation in 0..40 {
            registry.begin_activate(&project);
            for i in 0..per_activation {
                registry.register_fold(
                    PathBuf::from("src/lib.rs"),
                    fold(&format!("a{activation}_f{i}"), &format!("sym{i}"), 1.0),
                );
            }
            assert!(
                registry.fold_count() <= MAX_RETAINED_FOLDS + per_activation,
                "activation {activation} left {} folds",
                registry.fold_count()
            );
        }

        assert!(registry.fold_count() <= MAX_RETAINED_FOLDS + per_activation);
    }

    #[test]
    fn current_activation_folds_survive_trimming() {
        let registry = ReversibleContextRegistry::new();
        let project = ProjectId::new("bounded");

        for activation in 0..12 {
            registry.begin_activate(&project);
            for i in 0..400 {
                registry.register_fold(
                    PathBuf::from("src/lib.rs"),
                    fold(&format!("a{activation}_f{i}"), &format!("sym{i}"), 1.0),
                );
            }
        }

        for i in 0..400 {
            assert!(
                registry.get_fold(&format!("a11_f{i}")).is_some(),
                "current-activation fold a11_f{i} was evicted"
            );
        }
        assert!(registry.get_fold("a0_f0").is_none());
    }

    #[test]
    fn get_fold_resolves_by_prefix_and_symbol() {
        let registry = ReversibleContextRegistry::new();
        registry.begin_activate(&ProjectId::new("lookup"));
        registry.register_fold(
            PathBuf::from("src/a.rs"),
            fold("handle_2f1a", "handle", 0.4),
        );
        registry.register_fold(
            PathBuf::from("src/b.rs"),
            fold("handle_9c3b", "handle", 0.9),
        );
        registry.register_fold(PathBuf::from("src/c.rs"), fold("render_1", "render", 0.5));

        let by_prefix = registry.get_fold("handle").expect("prefix match");
        assert_eq!(by_prefix.fold.fold_id, "handle_9c3b");

        let exact = registry.get_fold("render_1").expect("exact match");
        assert_eq!(exact.file_path, PathBuf::from("src/c.rs"));

        let by_symbol = registry.get_fold("RENDER").expect("symbol match");
        assert_eq!(by_symbol.fold.symbol_name, "render");

        assert!(registry.get_fold("nothing_here").is_none());
    }
}
