use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};

pub async fn auth_middleware(
    headers: HeaderMap,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = std::env::var("HTTVOICE_TOKEN").ok();

    if let Some(required_token) = token {
        let auth_header = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));

        if auth_header != Some(&required_token) {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use std::env;

    #[test]
    fn test_auth_env_var() {
        env::remove_var("HTTVOICE_TOKEN");
        assert!(env::var("HTTVOICE_TOKEN").is_err());
        
        env::set_var("HTTVOICE_TOKEN", "test-token");
        assert_eq!(env::var("HTTVOICE_TOKEN").unwrap(), "test-token");
        env::remove_var("HTTVOICE_TOKEN");
    }
}
