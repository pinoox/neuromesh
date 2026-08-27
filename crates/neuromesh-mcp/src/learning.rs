use neuromesh_core::{NodeId, ProjectId, Result};
use neuromesh_graph::NeuralProjectGraph;
use neuromesh_memory::{replay_unapplied_episodes, LearningReplayTarget, MemoryDatabase};

struct GraphLearning<'a>(&'a NeuralProjectGraph);

impl LearningReplayTarget for GraphLearning<'_> {
    fn node_access_count(&self, id: &NodeId) -> Option<u64> {
        self.0.get_node(id).map(|node| node.access_count)
    }

    fn replay_learning_paths(&self, paths: &[(&[NodeId], bool)]) {
        NeuralProjectGraph::replay_learning_paths(self.0, paths);
    }
}

pub fn warmup_project_learning(
    memory_db: &MemoryDatabase,
    graph: &NeuralProjectGraph,
    project_id: &ProjectId,
) -> Result<usize> {
    replay_unapplied_episodes(memory_db, &GraphLearning(graph), project_id)
}
