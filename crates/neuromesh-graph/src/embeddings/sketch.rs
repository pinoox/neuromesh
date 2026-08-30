use neuromesh_core::{ContextNode, NodeType};

pub fn node_type_label(node_type: NodeType) -> &'static str {
    match node_type {
        NodeType::Project => "project",
        NodeType::Directory => "directory",
        NodeType::File => "file",
        NodeType::Component => "component",
        NodeType::Class => "class",
        NodeType::Function => "function",
        NodeType::Symbol => "symbol",
        NodeType::Import => "import",
        NodeType::Dependency => "dependency",
        NodeType::Api => "api",
        NodeType::DbModel => "db_model",
        NodeType::Test => "test",
        NodeType::Config => "config",
        NodeType::Doc => "doc",
        NodeType::Task => "task",
        NodeType::Decision => "decision",
        NodeType::Memory => "memory",
        NodeType::StyleToken => "style_token",
    }
}

pub fn symbol_sketch(node: &ContextNode) -> Option<String> {
    if matches!(
        node.node_type,
        NodeType::File | NodeType::Directory | NodeType::Project | NodeType::Api
    ) {
        return None;
    }
    let path = node.file_path.to_string_lossy().replace('\\', "/");
    let title = if path.is_empty() {
        node.name.clone()
    } else {
        format!("{}::{}", path, node.name)
    };
    let kind = node_type_label(node.node_type);
    let signature = node.signature.as_deref().unwrap_or("").trim();
    let doc = node.doc_summary.as_deref().unwrap_or("").trim();
    let sig = if signature.is_empty() {
        String::new()
    } else {
        signature.chars().take(180).collect::<String>()
    };
    let doc_part = if doc.is_empty() {
        String::new()
    } else {
        format!(" - {}", doc.chars().take(120).collect::<String>())
    };
    let sketch = if sig.is_empty() {
        format!("title: {title} | text: {kind}{doc_part}")
    } else {
        format!("title: {title} | text: {kind} {sig}{doc_part}")
    };
    Some(sketch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::{NodeId, ProjectId};
    use std::path::PathBuf;

    #[test]
    fn skips_file_nodes() {
        let node = ContextNode {
            id: NodeId::new("f1"),
            project_id: ProjectId::new("p"),
            file_path: PathBuf::from("src/a.rs"),
            node_type: NodeType::File,
            name: "a.rs".into(),
            signature: None,
            doc_summary: None,
            line_range: None,
            token_cost: 0,
            content: None,
            content_hash: String::new(),
            parent: None,
            base_relevance: 0.0,
            access_count: 0,
            last_accessed: chrono::Utc::now(),
        };
        assert!(symbol_sketch(&node).is_none());
    }

    #[test]
    fn includes_symbol_signature_and_doc() {
        let node = ContextNode {
            id: NodeId::new("fn1"),
            project_id: ProjectId::new("p"),
            file_path: PathBuf::from("src/routes.ts"),
            node_type: NodeType::Function,
            name: "handleAuth".into(),
            signature: Some("export function handleAuth(req, res)".into()),
            doc_summary: Some("Validates JWT and attaches user to request".into()),
            line_range: None,
            token_cost: 0,
            content: None,
            content_hash: String::new(),
            parent: None,
            base_relevance: 0.0,
            access_count: 0,
            last_accessed: chrono::Utc::now(),
        };
        let sketch = symbol_sketch(&node).expect("sketch");
        assert!(sketch.contains("handleAuth"));
        assert!(sketch.contains("function"));
        assert!(sketch.contains("Validates JWT"));
    }
}
