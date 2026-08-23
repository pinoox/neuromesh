use std::io::BufRead;

/// Read one JSON-RPC payload from an MCP stdio stream.
///
/// Clients speak either newline-delimited JSON (MCP spec) or LSP-style
/// `Content-Length` framing (Cursor / older SDKs). Mixing the two used to hang:
/// `read_line` waited forever for a newline that a Content-Length body never
/// sends while the client waited for our reply.
pub fn read_message<R: BufRead>(reader: &mut R) -> std::io::Result<Option<String>> {
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 {
            return Ok(None);
        }
        if line.starts_with('\u{feff}') {
            line = line.trim_start_matches('\u{feff}').to_string();
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return Ok(Some(trimmed.to_string()));
        }

        let mut content_length: Option<usize> = None;
        loop {
            let lower = line.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("content-length:") {
                content_length = rest.trim().parse().ok();
            }
            if line.trim().is_empty() {
                break;
            }
            line.clear();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
        }

        if let Some(len) = content_length {
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u8; len];
            std::io::Read::read_exact(reader, &mut buf)?;
            let body = String::from_utf8(buf)
                .unwrap_or_else(|e| String::from_utf8_lossy(&e.into_bytes()).into_owned());
            let body = body.trim();
            if body.is_empty() {
                continue;
            }
            return Ok(Some(body.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::read_message;
    use std::io::Cursor;

    #[test]
    fn ndjson_initialize() {
        let raw = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}\n";
        let mut cur = Cursor::new(raw.as_bytes());
        let msg = read_message(&mut cur).unwrap().unwrap();
        assert!(msg.contains("initialize"));
        assert!(read_message(&mut cur).unwrap().is_none());
    }

    #[test]
    fn content_length_without_trailing_newline_does_not_hang() {
        let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let framed = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let mut cur = Cursor::new(framed.into_bytes());
        let msg = read_message(&mut cur).unwrap().expect("body");
        assert_eq!(msg, body);
        assert!(read_message(&mut cur).unwrap().is_none());
    }

    #[test]
    fn content_length_with_content_type_header() {
        let body = r#"{"jsonrpc":"2.0","id":0,"method":"initialize"}"#;
        let framed = format!(
            "Content-Length: {}\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{}",
            body.len(),
            body
        );
        let mut cur = Cursor::new(framed.into_bytes());
        let msg = read_message(&mut cur).unwrap().unwrap();
        assert!(msg.contains("initialize"));
    }
}
