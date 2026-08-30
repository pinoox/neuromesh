use crate::detect::detect_proxy_launch_specs;
use neuromesh_core::{GraphBackendId, GraphProxyConfig, GraphProxyLaunchSpec, GraphProxyProvider};
use std::path::Path;

pub fn resolve_launch_spec(
    config: &GraphProxyConfig,
    workspace: &Path,
) -> Option<GraphProxyLaunchSpec> {
    match config.backend {
        GraphBackendId::Native => None,
        GraphBackendId::ProxyCbm => manual_or_detect(config, workspace, GraphProxyProvider::Cbm),
        GraphBackendId::ProxyGraphify => {
            manual_or_detect(config, workspace, GraphProxyProvider::Graphify)
        }
        GraphBackendId::Auto => detect_proxy_launch_specs(workspace)
            .candidates
            .into_iter()
            .next()
            .map(|c| c.spec),
    }
}

/// MCP stdio defaults to native graph. Only explicit proxy backends attach an external graph.
pub fn resolve_mcp_launch_spec(
    config: &GraphProxyConfig,
    workspace: &Path,
) -> Option<GraphProxyLaunchSpec> {
    match config.backend {
        GraphBackendId::Native | GraphBackendId::Auto => None,
        GraphBackendId::ProxyCbm | GraphBackendId::ProxyGraphify => {
            resolve_launch_spec(config, workspace)
        }
    }
}

fn manual_or_detect(
    config: &GraphProxyConfig,
    workspace: &Path,
    provider: GraphProxyProvider,
) -> Option<GraphProxyLaunchSpec> {
    if let Some(cmd) = config.command.as_ref().filter(|c| !c.is_empty()) {
        return Some(GraphProxyLaunchSpec {
            provider,
            server_name: config
                .server_name
                .clone()
                .unwrap_or_else(|| provider.as_str().into()),
            command: cmd.clone(),
            args: if config.args.is_empty() {
                vec!["mcp".into()]
            } else {
                config.args.clone()
            },
            env: config.env.clone(),
            config_path: None,
        });
    }
    detect_proxy_launch_specs(workspace)
        .candidates
        .into_iter()
        .find(|c| c.spec.provider == provider)
        .map(|c| c.spec)
}

#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::GraphProxyConfig;
    use std::path::Path;

    #[test]
    fn mcp_skips_auto_backend() {
        let cfg = GraphProxyConfig {
            backend: GraphBackendId::Auto,
            ..GraphProxyConfig::default()
        };
        assert!(resolve_mcp_launch_spec(&cfg, Path::new("/tmp/ws")).is_none());
    }
}
