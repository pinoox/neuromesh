//! Serialize a proxy-built packet into NeuroMesh MCP evidence shape (minimal/standard compatible).
use neuromesh_core::TaskSignature;
use neuromesh_graph_proxy::ProxyContextPacket;
use neuromesh_router::QualityGateDecision;
use serde_json::{json, Value};

pub fn proxy_evidence_response(
    packet: &ProxyContextPacket,
    signature: &TaskSignature,
    gate: &QualityGateDecision,
    detail: crate::response::ResponseDetail,
    elapsed_ms: u64,
    backend_label: &str,
) -> Value {
    let packet_id = crate::packet_cache::PacketDetailCache::new_packet_id();
    let files: Vec<Value> = packet
        .files
        .iter()
        .map(|f| {
            json!({
                "path": f.path,
                "skeleton": f.code,
                "tokens": f.tokens,
                "why": f.why,
            })
        })
        .collect();

    let hints = &packet.retrieval;
    let seeds_missed: Vec<&str> = hints.missed_terms.iter().map(String::as_str).collect();

    let retrieval = json!({
        "retrieval_level": "proxy",
        "claim": packet.coverage,
        "confidence": hints.confidence,
        "sufficiency_score": hints.sufficiency_score,
        "critical_gaps": hints.critical_gaps,
        "suggested_keywords": hints.suggested_keywords,
        "levels_attempted": ["proxy"],
        "graph_backend": backend_label,
        "provider": packet.provider,
        "next_action": if packet.coverage == "no_seed_resolved"
            || !hints.suggested_keywords.is_empty()
        {
            Value::String("neuromesh_search_symbols".into())
        } else {
            Value::Null
        },
    });

    let coverage = json!({
        "claim": packet.coverage,
        "seeds_missed": seeds_missed,
    });

    let evidence = json!({
        "files": files,
        "coverage": coverage,
        "retrieval": retrieval,
        "active_tokens": packet.packet_tokens,
        "workspace_tokens": 0,
        "graph_backend": backend_label,
        "proxy_provider": packet.provider,
        "symbols_found": packet.symbols_found,
    });

    let base = json!({
        "packet_id": packet_id,
        "task": {
            "description": signature.raw_prompt,
            "effective_mode": format!("{:?}", gate.effective_mode),
        },
        "latency_ms": elapsed_ms,
        "evidence_packet": evidence,
    });

    match detail {
        crate::response::ResponseDetail::Diagnostic => base,
        crate::response::ResponseDetail::Standard => base,
        crate::response::ResponseDetail::Minimal => json!({
            "packet_id": packet_id,
            "coverage": packet.coverage,
            "tokens": { "selected": packet.packet_tokens, "packet": packet.packet_tokens },
            "files": files,
            "retrieval": retrieval,
            "graph_backend": backend_label,
        }),
    }
}
