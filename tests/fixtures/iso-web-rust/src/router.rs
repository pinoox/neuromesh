use crate::handlers::{health_handler, submit_order_handler};

pub struct Router {
    routes: Vec<(String, String)>,
}

impl Router {
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Register every HTTP route this service exposes.
    pub fn register_routes(&mut self) {
        self.routes.push(("GET /health".into(), "health".into()));
        self.routes.push(("POST /orders".into(), "orders".into()));
        health_handler();
        submit_order_handler("seed");
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
