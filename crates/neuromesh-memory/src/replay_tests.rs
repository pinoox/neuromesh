#[cfg(test)]
mod tests {
    use crate::db::MemoryDatabase;
    use crate::episodic::EpisodicRecord;
    use crate::replay::{replay_unapplied_episodes, LearningReplayTarget};
    use neuromesh_core::{NodeId, ProjectId};
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct FakeGraph {
        access: Mutex<HashMap<String, u64>>,
        replayed: Mutex<Vec<(Vec<NodeId>, bool)>>,
    }

    impl FakeGraph {
        fn new() -> Self {
            Self {
                access: Mutex::new(HashMap::new()),
                replayed: Mutex::new(Vec::new()),
            }
        }
    }

    impl LearningReplayTarget for FakeGraph {
        fn node_access_count(&self, id: &NodeId) -> Option<u64> {
            Some(*self.access.lock().unwrap().get(id.as_str()).unwrap_or(&0))
        }

        fn replay_learning_paths(&self, paths: &[(&[NodeId], bool)]) {
            let mut replayed = self.replayed.lock().unwrap();
            for (ids, success) in paths {
                replayed.push((ids.to_vec(), *success));
                let mut access = self.access.lock().unwrap();
                for id in *ids {
                    *access.entry(id.as_str().to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    #[test]
    fn replay_skips_episodes_already_reflected_in_graph() {
        let db = MemoryDatabase::open_in_memory().unwrap();
        let graph = FakeGraph::new();
        let pid = ProjectId::new("shop");
        let node = NodeId::new("src/CheckoutView.vue");
        graph
            .access
            .lock()
            .unwrap()
            .insert(node.as_str().to_string(), 3);
        let episode = EpisodicRecord::new(
            pid.clone(),
            "hash".into(),
            "checkout".into(),
            "ok".into(),
            vec![node.clone()],
            vec!["CheckoutView".into()],
            true,
            0,
        );
        db.save_episodic_record(&episode).unwrap();
        let replayed = replay_unapplied_episodes(&db, &graph, &pid).unwrap();
        assert_eq!(replayed, 1);
        assert!(
            graph.replayed.lock().unwrap().is_empty(),
            "already-learned nodes must not be replayed"
        );
    }

    #[test]
    fn replay_applies_unreplayed_episode_when_graph_is_cold() {
        let db = MemoryDatabase::open_in_memory().unwrap();
        let graph = FakeGraph::new();
        let pid = ProjectId::new("shop");
        let node = NodeId::new("src/CheckoutView.vue");
        let episode = EpisodicRecord::new(
            pid.clone(),
            "hash".into(),
            "checkout".into(),
            "ok".into(),
            vec![node.clone()],
            vec!["CheckoutView".into()],
            true,
            0,
        );
        db.save_episodic_record(&episode).unwrap();
        let replayed = replay_unapplied_episodes(&db, &graph, &pid).unwrap();
        assert_eq!(replayed, 1);
        assert_eq!(graph.replayed.lock().unwrap().len(), 1);
        assert_eq!(
            graph.access.lock().unwrap().get(node.as_str()).copied(),
            Some(1)
        );
    }
}
