//! Regression gate: `neuromesh mcp` must keep stdout pure JSON-RPC.
//!
//! The MCP command auto-starts the HTTP dashboard in-process, and its
//! startup banner used to go to stdout (`println!`). Strict MCP clients
//! (e.g. Antigravity spawning under `%LOCALAPPDATA%\Programs\…`, which has
//! no project marker) then failed the handshake with:
//! `invalid character '╔' looking for beginning of value`.
//! This test reproduces that exact scenario: spawn the real binary with a
//! marker-less directory, complete `initialize`, and assert stdout carries
//! JSON only while the banner still lands on stderr.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn spawn_mcp(junk_dir: &std::path::Path) -> std::process::Child {
    let bin = env!("CARGO_BIN_EXE_neuromesh");
    Command::new(bin)
        .arg("mcp")
        .arg(junk_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("NEUROMESH_WORKSPACE")
        .spawn()
        .expect("spawn neuromesh mcp")
}

fn read_lines<R: std::io::Read + Send + 'static>(stream: R, out: std::sync::mpsc::Sender<String>) {
    std::thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            let _ = out.send(line);
        }
    });
}

#[test]
fn mcp_stdout_stays_json_rpc_when_dashboard_autostarts() {
    let junk = std::env::temp_dir().join(format!("neuromesh-no-marker-{}", std::process::id()));
    std::fs::create_dir_all(&junk).expect("junk dir");
    // No project marker on purpose: .git / Cargo.toml / package.json / …
    for marker in [
        ".git",
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "go.mod",
    ] {
        assert!(
            !junk.join(marker).exists(),
            "junk dir must stay marker-less"
        );
    }

    let mut child = spawn_mcp(&junk);
    let (tx_out, rx_out) = std::sync::mpsc::channel();
    let (tx_err, rx_err) = std::sync::mpsc::channel();
    read_lines(child.stdout.take().expect("stdout"), tx_out);
    read_lines(child.stderr.take().expect("stderr"), tx_err);

    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "stdio-gate", "version": "1"}
        }
    });
    let mut stdin = child.stdin.take().expect("stdin");
    writeln!(stdin, "{}", init).expect("write initialize");
    writeln!(
        stdin,
        "{}",
        serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
    )
    .expect("write initialized");

    // Collect a few seconds: the dashboard bind races the handshake, so a
    // too-short window could miss a late banner on a slow machine.
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut stdout_lines: Vec<String> = Vec::new();
    let mut stderr_text = String::new();
    let mut saw_init_response = false;
    while Instant::now() < deadline {
        while let Ok(line) = rx_out.try_recv() {
            if line.trim().is_empty() {
                continue;
            }
            // Every stdout line must be JSON-RPC; anything else (a banner,
            // a log line) breaks strict clients.
            assert!(
                !line.contains('╔'),
                "stdout polluted with dashboard banner: {line:?}"
            );
            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line) {
                if msg.get("id") == Some(&serde_json::json!(1)) {
                    saw_init_response = true;
                }
            }
            stdout_lines.push(line);
        }
        while let Ok(line) = rx_err.try_recv() {
            stderr_text.push_str(&line);
            stderr_text.push('\n');
        }
        if saw_init_response && Instant::now() > deadline - Duration::from_secs(4) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&junk);

    assert!(
        saw_init_response,
        "initialize got no JSON response; stdout={stdout_lines:?} stderr={stderr_text:?}"
    );
    // The dashboard really started (banner fired) — just on the right
    // stream. Without this the stdout assertion could pass vacuously.
    assert!(
        stderr_text.contains('╔'),
        "dashboard banner never fired; gate proves nothing (stderr={stderr_text:?})"
    );
}
