use neuromesh_index::IndexedFile;
use serde_json::Value;

/// Import prefix → directory maps from manifests (composer PSR-4, go.mod).
#[derive(Clone, Debug, Default)]
pub struct ManifestHints {
    prefixes: Vec<(String, String)>,
}

impl ManifestHints {
    pub fn from_scanned(scanned: &[(IndexedFile, String)]) -> Self {
        let mut prefixes = Vec::new();
        for (file, content) in scanned {
            let name = file
                .relative_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            match name {
                "composer.json" => prefixes.extend(parse_composer_psr4(content)),
                "go.mod" => {
                    if let Some(module) = parse_go_module(content) {
                        prefixes.push((module, String::new()));
                    }
                }
                _ => {}
            }
        }
        Self { prefixes }
    }

    pub fn is_empty(&self) -> bool {
        self.prefixes.is_empty()
    }

    /// Rewrite a normalized import hint (`App/Foo` or `github.com/acme/app/sms`) to a
    /// workspace-relative path when a manifest prefix matches.
    pub fn rewrite(&self, hint: &str) -> Option<String> {
        let hint = hint.replace('\\', "/");
        let mut best: Option<(usize, String)> = None;
        for (prefix, dir) in &self.prefixes {
            let prefix = prefix.trim_end_matches('/');
            if prefix.is_empty() {
                continue;
            }
            if hint == prefix || hint.starts_with(&format!("{prefix}/")) {
                let rest = hint[prefix.len()..].trim_start_matches('/');
                let dir = dir.trim_end_matches('/');
                let rewritten = if dir.is_empty() {
                    rest.to_string()
                } else if rest.is_empty() {
                    dir.to_string()
                } else {
                    format!("{dir}/{rest}")
                };
                if rewritten.is_empty() {
                    continue;
                }
                if best
                    .as_ref()
                    .map(|(len, _)| prefix.len() > *len)
                    .unwrap_or(true)
                {
                    best = Some((prefix.len(), rewritten));
                }
            }
        }
        best.map(|(_, path)| path)
    }
}

fn parse_go_module(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("module ") {
            let module = rest.split_whitespace().next()?.trim();
            if !module.is_empty() {
                return Some(module.to_string());
            }
        }
    }
    None
}

fn parse_composer_psr4(json: &str) -> Vec<(String, String)> {
    let Ok(v) = serde_json::from_str::<Value>(json) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for pointer in ["/autoload/psr-4", "/autoload/psr-0", "/autoload-dev/psr-4"] {
        let Some(map) = v.pointer(pointer).and_then(|x| x.as_object()) else {
            continue;
        };
        for (ns, dir) in map {
            let ns = ns.replace('\\', "/").trim_end_matches('/').to_string();
            let dir = dir
                .as_str()
                .unwrap_or("")
                .replace('\\', "/")
                .trim_end_matches('/')
                .to_string();
            if !ns.is_empty() {
                out.push((ns, dir));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::ProjectId;
    use neuromesh_index::{IndexedFile, SourceLanguage};
    use std::path::PathBuf;

    fn indexed(rel: &str) -> IndexedFile {
        IndexedFile {
            project_id: ProjectId::new("demo"),
            relative_path: PathBuf::from(rel),
            full_path: PathBuf::from(rel),
            blake3_hash: "test".into(),
            byte_size: 10,
            token_count: 8,
            language: SourceLanguage::JSON,
            last_modified: chrono::Utc::now(),
        }
    }

    #[test]
    fn composer_psr4_rewrites_namespace_to_src() {
        let composer = r#"{
            "autoload": { "psr-4": { "App\\": "src/" } }
        }"#;
        let hints = ManifestHints::from_scanned(&[(indexed("composer.json"), composer.into())]);
        assert_eq!(
            hints.rewrite("App/Installer/InstallPlatformException"),
            Some("src/Installer/InstallPlatformException".into())
        );
    }

    #[test]
    fn go_mod_strips_module_prefix() {
        let gomod = "module github.com/acme/app\n\ngo 1.22\n";
        let hints = ManifestHints::from_scanned(&[(indexed("go.mod"), gomod.into())]);
        assert_eq!(
            hints.rewrite("github.com/acme/app/smsstore"),
            Some("smsstore".into())
        );
    }
}
