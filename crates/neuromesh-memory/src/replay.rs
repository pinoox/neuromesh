use crate::db::MemoryDatabase;
use neuromesh_core::{NodeId, ProjectId, Result};

/// Replay episodic feedback not yet reflected in the graph snapshot.
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
        if graph.learning_episode_applied(&episode.id) {
            replayed_ids.push(episode.id.clone());
            continue;
        }
        if !episode.activated_node_ids.is_empty() {
            graph
                .replay_learning_paths(&[(episode.activated_node_ids.as_slice(), episode.success)]);
            graph.mark_learning_episode_applied(&episode.id);
            graph.persist_replayed_learning()?;
        }
        replayed_ids.push(episode.id.clone());
    }
    memory_db.mark_episodes_replayed(&replayed_ids)?;
    Ok(replayed_ids.len())
}

pub trait LearningReplayTarget {
    fn learning_episode_applied(&self, episode_id: &str) -> bool;
    fn mark_learning_episode_applied(&self, episode_id: &str);
    fn replay_learning_paths(&self, paths: &[(&[NodeId], bool)]);
    fn persist_replayed_learning(&self) -> Result<()>;
}
