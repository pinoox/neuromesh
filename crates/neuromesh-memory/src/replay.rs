use crate::db::MemoryDatabase;
use neuromesh_core::{NodeId, ProjectId, Result};

/// Replay episodic feedback that was recorded but not yet reflected in the graph snapshot.
pub fn replay_unapplied_episodes<G>(
    memory_db: &MemoryDatabase,
    graph: &G,
    project_id: &ProjectId,
) -> Result<usize>
where
    G: LearningReplayTarget,
{
    let episodes = memory_db.unreplayed_episodes(project_id)?;
    if episodes.is_empty() {
        return Ok(0);
    }
    let mut replayed_ids = Vec::new();
    for episode in &episodes {
        let needs_replay = episode.activated_node_ids.iter().any(|id| {
            graph
                .node_access_count(id)
                .map(|count| count == 0)
                .unwrap_or(false)
        });
        if needs_replay {
            graph
                .replay_learning_paths(&[(episode.activated_node_ids.as_slice(), episode.success)]);
        }
        replayed_ids.push(episode.id.clone());
    }
    memory_db.mark_episodes_replayed(&replayed_ids)?;
    Ok(replayed_ids.len())
}

pub trait LearningReplayTarget {
    fn node_access_count(&self, id: &NodeId) -> Option<u64>;
    fn replay_learning_paths(&self, paths: &[(&[NodeId], bool)]);
}
