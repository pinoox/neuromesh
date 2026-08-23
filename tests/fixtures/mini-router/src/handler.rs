use crate::extract::{extract_route, Route};

pub fn handle_request(raw: &str) -> String {
    let route = extract_route(raw);
    dispatch(&route)
}

fn dispatch(route: &Route) -> String {
    format!("ok:{}", route.path)
}
