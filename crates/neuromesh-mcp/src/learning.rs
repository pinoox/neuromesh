use neuromesh_core::{NodeId, ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_memory::{replay_unapplied_episodes, LearningReplayTarget, MemoryDatabase};

struct GraphLearning<'a>(&'a NeuralProjectGraph);

impl LearningReplayTarget for GraphLearning<'_> {
    fn learning_episode_applied(&self, episode_id: &str) -> bool {
        self.0.learning_episode_applied(episode_id)
    }

    fn mark_learning_episode_applied(&self, episode_id: &str) {
        self.0.mark_learning_episode_applied(episode_id);
    }

    fn replay_learning_paths(&self, paths: &[(&[NodeId], bool)]) {
        NeuralProjectGraph::replay_learning_paths(self.0, paths);
    }

    fn persist_replayed_learning(&self) -> Result<()> {
        self.0.save_persisted_if_ready()
    }
}

pub fn warmup_project_learning(
    memory_db: &MemoryDatabase,
    graph: &NeuralProjectGraph,
    project_id: &ProjectId,
) -> Result<usize> {
    replay_unapplied_episodes(memory_db, &GraphLearning(graph), project_id)
}
