mod handlers;
mod router;

use router::Router;

fn main() {
    let mut router = Router::new();
    router.register_routes();
    println!("serving {} routes", router.route_count());
}
