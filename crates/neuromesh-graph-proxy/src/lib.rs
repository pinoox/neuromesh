use neuromesh_core::{GraphBackendId, GraphProxyConfig, GraphProxyLaunchSpec, GraphProxyProvider};
use std::path::Path;

mod cbm;
mod detect;
mod mcp_client;
mod packet;
mod probe;
mod resolve;

pub use cbm::CbmGraphProxy;
pub use detect::{detect_proxy_launch_specs, DetectReport, DetectedProxy};
pub use mcp_client::McpStdioClient;
pub use packet::{ProxyContextFile, ProxyContextPacket};
pub use probe::{probe_graph_proxy, ProbeReport};
pub use resolve::resolve_launch_spec;

/// Active external graph session (one MCP server child process).
pub struct GraphProxySession {
    pub spec: GraphProxyLaunchSpec,
    pub project: String,
    client: McpStdioClient,
    inner: ProxyBackend,
}

enum ProxyBackend {
    Cbm(CbmGraphProxy),
}

impl GraphProxySession {
    pub async fn connect(
        spec: GraphProxyLaunchSpec,
        workspace: &Path,
    ) -> neuromesh_core::Result<Self> {
        let mut client = McpStdioClient::spawn(&spec.command, &spec.args, &spec.env).await?;
        client.initialize().await?;
        let project = match spec.provider {
            GraphProxyProvider::Cbm => {
                let handle = client.clone_handle();
                CbmGraphProxy::resolve_project(&handle, workspace).await?
            }
            GraphProxyProvider::Graphify => workspace.to_string_lossy().replace('\\', "/"),
        };
        let inner = match spec.provider {
            GraphProxyProvider::Cbm => ProxyBackend::Cbm(CbmGraphProxy::new(client.clone_handle())),
            GraphProxyProvider::Graphify => {
                return Err(neuromesh_core::NeuroMeshError::Config(
                    "Graphify proxy adapter is not implemented yet; use proxy_cbm or native".into(),
                ));
            }
        };
        Ok(Self {
            spec,
            project,
            client,
            inner,
        })
    }

    pub fn provider(&self) -> GraphProxyProvider {
        self.spec.provider
    }

    pub async fn build_context_packet(
        &mut self,
        task: &str,
        limit: u32,
    ) -> neuromesh_core::Result<ProxyContextPacket> {
        match &mut self.inner {
            ProxyBackend::Cbm(cbm) => cbm.build_packet(&self.project, task, limit).await,
        }
    }

    pub async fn probe_tools(&mut self) -> neuromesh_core::Result<Vec<String>> {
        self.client.list_tool_names().await
    }
}

/// Resolve configured backend + optional auto-detect into a launch spec.
pub fn resolve_for_workspace(
    config: &GraphProxyConfig,
    workspace: &Path,
) -> Option<GraphProxyLaunchSpec> {
    resolve_launch_spec(config, workspace)
}

pub fn effective_backend_label(config: &GraphProxyConfig, workspace: &Path) -> GraphBackendId {
    match config.backend {
        GraphBackendId::Native => GraphBackendId::Native,
        other if resolve_launch_spec(config, workspace).is_some() => other,
        GraphBackendId::Auto => GraphBackendId::Native,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_by_default() {
        let cfg = GraphProxyConfig::default();
        assert_eq!(cfg.backend, GraphBackendId::Native);
        assert!(resolve_for_workspace(&cfg, Path::new("/tmp/ws")).is_none());
    }
}
