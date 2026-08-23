pub struct Route {
    pub path: String,
}

pub fn extract_route(raw: &str) -> Route {
    let path = raw.trim().to_string();
    Route { path }
}
