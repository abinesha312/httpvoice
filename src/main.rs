use httpvoice::{api, auth, call};
use std::sync::Arc;

use call::CallStore;

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "httpvoice=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let call_store = Arc::new(CallStore::new());

    let app = Router::new()
        .route("/v1/calls", post(api::create_call))
        .route("/v1/calls/:id", get(api::get_call))
        .route("/v1/calls/:id/hangup", post(api::hangup_call))
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn(auth::auth_middleware))
        .with_state(call_store);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("httpvoice listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
