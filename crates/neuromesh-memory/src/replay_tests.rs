#[cfg(test)]
mod tests {
    use crate::db::MemoryDatabase;
    use crate::episodic::EpisodicRecord;
    use crate::replay::{replay_unapplied_episodes, LearningReplayTarget};
    use neuromesh_core::{NodeId, ProjectId, Result};
    use std::collections::HashSet;
    use std::sync::Mutex;

    struct FakeGraph {
        applied: Mutex<HashSet<String>>,
        replayed: Mutex<Vec<(Vec<NodeId>, bool)>>,
        persisted: Mutex<u32>,
    }

    impl FakeGraph {
        fn new() -> Self {
            Self {
                applied: Mutex::new(HashSet::new()),
                replayed: Mutex::new(Vec::new()),
                persisted: Mutex::new(0),
            }
        }
    }

    impl LearningReplayTarget for FakeGraph {
        fn learning_episode_applied(&self, episode_id: &str) -> bool {
            self.applied.lock().unwrap().contains(episode_id)
        }

        fn mark_learning_episode_applied(&self, episode_id: &str) {
            self.applied.lock().unwrap().insert(episode_id.to_string());
        }

        fn replay_learning_paths(&self, paths: &[(&[NodeId], bool)]) {
            let mut replayed = self.replayed.lock().unwrap();
            for (ids, success) in paths {
                replayed.push((ids.to_vec(), *success));
            }
        }

        fn persist_replayed_learning(&self) -> Result<()> {
            *self.persisted.lock().unwrap() += 1;
            Ok(())
        }
    }

    #[test]
    fn replay_skips_episodes_already_in_graph_checkpoint() {
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
        graph.mark_learning_episode_applied(&episode.id);
        db.save_episodic_record(&episode).unwrap();
        let replayed = replay_unapplied_episodes(&db, &graph, &pid).unwrap();
        assert_eq!(replayed, 1);
        assert!(
            graph.replayed.lock().unwrap().is_empty(),
            "checkpointed episodes must not replay"
        );
    }

    #[test]
    fn replay_applies_unreplayed_episode_and_persists() {
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
        assert!(graph.learning_episode_applied(&episode.id));
        assert_eq!(*graph.persisted.lock().unwrap(), 1);
    }
}
