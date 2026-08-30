use crate::{resolve_for_workspace, GraphProxySession, ProxySearchContext};
use neuromesh_core::{GraphProxyConfig, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeReport {
    pub connected: bool,
    pub provider: Option<String>,
    pub command: Option<String>,
    pub tools: Vec<String>,
    pub sample_files: usize,
    pub coverage: Option<String>,
    pub packet_tokens: usize,
    pub error: Option<String>,
}

pub async fn probe_graph_proxy(config: &GraphProxyConfig, workspace: &Path) -> Result<ProbeReport> {
    let Some(spec) = resolve_for_workspace(config, workspace) else {
        return Ok(ProbeReport {
            connected: false,
            provider: None,
            command: None,
            tools: Vec::new(),
            sample_files: 0,
            coverage: None,
            packet_tokens: 0,
            error: Some("no proxy launch spec resolved for current config".into()),
        });
    };

    let provider = spec.provider.as_str().to_string();
    let command = spec.command.clone();
    match GraphProxySession::connect(spec, workspace).await {
        Ok(mut session) => {
            let tools = session.probe_tools().await.unwrap_or_default();
            let ctx = ProxySearchContext {
                raw_prompt: "Router middleware app.use next pipeline".into(),
                client_keywords: vec![
                    "Router".into(),
                    "middleware".into(),
                    "app.use".into(),
                    "next".into(),
                ],
                client_expansion: vec!["pipeline".into()],
                ..Default::default()
            };
            match session.build_context_packet(&ctx, 5).await {
                Ok(packet) => Ok(ProbeReport {
                    connected: true,
                    provider: Some(provider),
                    command: Some(command),
                    tools,
                    sample_files: packet.files.len(),
                    coverage: Some(packet.coverage),
                    packet_tokens: packet.packet_tokens,
                    error: None,
                }),
                Err(e) => Ok(ProbeReport {
                    connected: true,
                    provider: Some(provider),
                    command: Some(command),
                    tools,
                    sample_files: 0,
                    coverage: None,
                    packet_tokens: 0,
                    error: Some(e.to_string()),
                }),
            }
        }
        Err(e) => Ok(ProbeReport {
            connected: false,
            provider: Some(provider),
            command: Some(command),
            tools: Vec::new(),
            sample_files: 0,
            coverage: None,
            packet_tokens: 0,
            error: Some(e.to_string()),
        }),
    }
}
