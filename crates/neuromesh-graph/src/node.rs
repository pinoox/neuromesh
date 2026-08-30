use chrono::Utc;
use neuromesh_core::{ContextNode, NodeId, NodeType, ProjectId};
use std::ops::Range;
use std::path::PathBuf;

pub struct NodeFactory;

impl NodeFactory {
    pub fn create_file_node(
        project_id: ProjectId,
        file_path: PathBuf,
        token_cost: usize,
        content_hash: String,
        _content: Option<String>,
    ) -> ContextNode {
        let path_str = file_path.to_string_lossy();
        let name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();

        ContextNode {
            id: NodeId::from_file_path(&path_str),
            project_id,
            file_path,
            node_type: NodeType::File,
            name,
            signature: None,
            doc_summary: None,
            line_range: None,
            token_cost,
            content: None,
            content_hash,
            parent: None,
            base_relevance: 1.0,
            access_count: 0,
            last_accessed: Utc::now(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_symbol_node(
        project_id: ProjectId,
        file_path: PathBuf,
        node_type: NodeType,
        name: String,
        signature: Option<String>,
        doc_summary: Option<String>,
        line_range: Range<usize>,
        token_cost: usize,
        parent: Option<String>,
    ) -> ContextNode {
        let path_str = file_path.to_string_lossy();
        let id = NodeId::from_symbol_parts(&path_str, &name, parent.as_deref());

        ContextNode {
            id,
            project_id,
            file_path,
            node_type,
            name,
            signature,
            doc_summary,
            line_range: Some(line_range),
            token_cost,
            content: None,
            content_hash: String::new(),
            parent,
            base_relevance: 1.0,
            access_count: 0,
            last_accessed: Utc::now(),
        }
    }
}
