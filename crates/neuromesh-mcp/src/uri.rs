use serde_json::Value;
use std::path::PathBuf;

/// Turn an MCP/LSP file URI or a raw path into a filesystem path.
///
/// Handles `file:///C:/…`, `file://localhost/C:/…`, percent-encoding, and
/// the extra leading slash Windows clients put in front of the drive letter.
pub fn parse_workspace_uri(raw: &str) -> Option<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("null") {
        return None;
    }
    if !raw.to_ascii_lowercase().starts_with("file:") {
        return Some(normalize_os_path(raw));
    }
    let after_scheme = &raw[5..];
    let path_encoded = if let Some(rest) = after_scheme.strip_prefix("//") {
        if let Some(slash) = rest.find('/') {
            &rest[slash..]
        } else if rest.eq_ignore_ascii_case("localhost") {
            return None;
        } else {
            rest
        }
    } else {
        after_scheme
    };
    let decoded = percent_decode(path_encoded);
    if decoded.is_empty() {
        return None;
    }
    Some(normalize_os_path(&decoded))
}

/// Workspace root from MCP `initialize` params (folders, rootUri, cwd, env-style keys).
pub fn workspace_from_initialize(params: &Value) -> Option<PathBuf> {
    if let Some(folders) = params.get("workspaceFolders").and_then(Value::as_array) {
        for folder in folders {
            if let Some(uri) = folder.get("uri").and_then(Value::as_str) {
                if let Some(path) = parse_workspace_uri(uri) {
                    return Some(path);
                }
            }
            if let Some(path) = folder.get("path").and_then(Value::as_str) {
                if let Some(parsed) = parse_workspace_uri(path) {
                    return Some(parsed);
                }
            }
        }
    }
    for key in [
        "rootUri",
        "rootPath",
        "cwd",
        "workspaceRoot",
        "workspace",
        "projectRoot",
    ] {
        if let Some(raw) = params.get(key).and_then(Value::as_str) {
            if let Some(path) = parse_workspace_uri(raw) {
                return Some(path);
            }
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn normalize_os_path(s: &str) -> PathBuf {
    #[cfg(windows)]
    {
        let mut t = s.replace('/', "\\");
        if t.starts_with("\\\\localhost\\") {
            t = format!("\\{}", t.trim_start_matches("\\\\localhost"));
        }
        let bytes = t.as_bytes();
        if bytes.len() >= 3 && bytes[0] == b'\\' && bytes[2] == b':' {
            t.remove(0);
        } else if bytes.len() >= 3 && bytes[0] == b'\\' && bytes[2] == b'|' {
            t.remove(0);
            t.replace_range(1..2, ":");
        } else if bytes.len() >= 2 && bytes[1] == b'|' {
            t.replace_range(1..2, ":");
        }
        PathBuf::from(t)
    }
    #[cfg(not(windows))]
    {
        PathBuf::from(s.replace('\\', "/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn percent_decode_colon() {
        assert_eq!(percent_decode("c%3A/proj"), "c:/proj");
        assert_eq!(percent_decode("plain"), "plain");
        assert_eq!(percent_decode("%zz"), "%zz");
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_uris() {
        assert_eq!(
            parse_workspace_uri("file:///C:/projects/neuromesh").unwrap(),
            PathBuf::from(r"C:\projects\neuromesh")
        );
        assert_eq!(
            parse_workspace_uri("file://localhost/C:/projects/neuromesh").unwrap(),
            PathBuf::from(r"C:\projects\neuromesh")
        );
        assert_eq!(
            parse_workspace_uri("file:///c%3A/projects/neuromesh").unwrap(),
            PathBuf::from(r"c:\projects\neuromesh")
        );
        assert_eq!(
            parse_workspace_uri(r"C:\already\windows").unwrap(),
            PathBuf::from(r"C:\already\windows")
        );
        assert_eq!(
            parse_workspace_uri("file:///C|/projects/neuromesh").unwrap(),
            PathBuf::from(r"C:\projects\neuromesh")
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_file_uris() {
        assert_eq!(
            parse_workspace_uri("file:///home/user/proj").unwrap(),
            PathBuf::from("/home/user/proj")
        );
        assert_eq!(
            parse_workspace_uri("file://localhost/home/user/proj").unwrap(),
            PathBuf::from("/home/user/proj")
        );
        assert_eq!(
            parse_workspace_uri("/tmp/work").unwrap(),
            PathBuf::from("/tmp/work")
        );
    }

    #[test]
    fn initialize_folders_and_root_uri() {
        let params = json!({
            "workspaceFolders": [{ "uri": "file:///C:/projects/neuromesh" }]
        });
        let path = workspace_from_initialize(&params).unwrap();
        assert!(path.to_string_lossy().contains("neuromesh"));

        let params = json!({ "rootPath": "/tmp/work" });
        assert_eq!(
            workspace_from_initialize(&params).unwrap(),
            parse_workspace_uri("/tmp/work").unwrap()
        );

        assert!(workspace_from_initialize(&json!({})).is_none());
        assert!(parse_workspace_uri("").is_none());
        assert!(parse_workspace_uri("null").is_none());
    }
}
