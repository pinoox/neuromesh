/// Liveness probe used by the load balancer.
pub fn health_handler() -> &'static str {
    "ok"
}

/// Accept an order payload and hand it to the store.
pub fn submit_order_handler(payload: &str) -> usize {
    validate_order_payload(payload);
    payload.len()
}

fn validate_order_payload(payload: &str) -> bool {
    !payload.is_empty()
}
