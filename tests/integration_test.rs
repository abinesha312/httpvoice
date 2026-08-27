use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

use httpvoice::{api, auth, call::CallStore};

#[tokio::test]
async fn test_localhost_sip_invite_integration() {
    use axum::{middleware, routing::post, Router};

    std::env::remove_var("HTTVOICE_TOKEN");
    std::env::remove_var("SIP_FROM");

    let store = Arc::new(CallStore::new());

    let app = Router::new()
        .route("/v1/calls", post(api::create_call))
        .layer(middleware::from_fn(auth::auth_middleware))
        .with_state(store.clone());

    let request_body = json!({
        "to": "sip:test@localhost",
        "from": "sip:caller@localhost",
        "url": null
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/calls")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_spoof_rejection_integration() {
    use axum::{middleware, routing::post, Router};

    let original_sip_from = std::env::var("SIP_FROM").ok();
    std::env::set_var("SIP_FROM", "sip:legit@example.com");

    let store = Arc::new(CallStore::new());

    let app = Router::new()
        .route("/v1/calls", post(api::create_call))
        .layer(middleware::from_fn(auth::auth_middleware))
        .with_state(store.clone());

    let request_body = json!({
        "to": "sip:victim@example.com",
        "from": "sip:spoof@evil.com",
        "url": null
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/calls")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    
    if let Some(val) = original_sip_from {
        std::env::set_var("SIP_FROM", val);
    } else {
        std::env::remove_var("SIP_FROM");
    }
}
