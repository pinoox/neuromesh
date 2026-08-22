use neuromesh_core::{ContextDiff, TokenCounter};
use std::collections::HashSet;
use std::path::PathBuf;

pub struct ContextDeduplicator;

impl ContextDeduplicator {
    /// Computes high-performance linear O(N + M) diff between two versions of a file
    pub fn compute_diff(
        file_path: PathBuf,
        old_content: &str,
        new_content: &str,
        old_hash: &str,
        new_hash: &str,
    ) -> Option<ContextDiff> {
        if old_hash == new_hash {
            return None;
        }

        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();

        let old_set: HashSet<&str> = old_lines.iter().copied().collect();
        let new_set: HashSet<&str> = new_lines.iter().copied().collect();

        let mut added_lines = Vec::new();
        let mut removed_lines = Vec::new();

        for (idx, line) in new_lines.iter().enumerate() {
            if !old_set.contains(line) {
                added_lines.push((idx + 1, line.to_string()));
            }
        }

        for (idx, line) in old_lines.iter().enumerate() {
            if !new_set.contains(line) {
                removed_lines.push((idx + 1, line.to_string()));
            }
        }

        let net_token_change = TokenCounter::diff_tokens(old_content, new_content);

        Some(ContextDiff {
            base_hash: old_hash.to_string(),
            new_hash: new_hash.to_string(),
            file_path,
            added_lines,
            removed_lines,
            net_token_change,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_diff_linear() {
        let old_code = "fn a() {}\nfn b() {}\nfn c() {}";
        let new_code = "fn a() {}\nfn b_updated() {}\nfn c() {}";

        let diff = ContextDeduplicator::compute_diff(
            PathBuf::from("test.rs"),
            old_code,
            new_code,
            "hash1",
            "hash2",
        )
        .unwrap();

        assert_eq!(diff.added_lines.len(), 1);
        assert_eq!(diff.removed_lines.len(), 1);
        assert_eq!(diff.added_lines[0].1, "fn b_updated() {}");
    }
}
