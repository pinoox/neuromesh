mod sms_store;

use axum::{routing::post, Router};

fn app() -> Router {
    Router::new().route("/sms", post(store))
}

async fn store() {
    sms_store::save("inbox");
}
