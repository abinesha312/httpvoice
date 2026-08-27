use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::call::{Call, CallStatus, CallStore};
use crate::sip;
use crate::twiml;

#[derive(Debug, Deserialize)]
pub struct CreateCallRequest {
    pub to: String,
    pub from: String,
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateCallResponse {
    pub id: Uuid,
    pub status: CallStatus,
}

#[derive(Debug, Serialize)]
pub struct CallStatusResponse {
    pub id: Uuid,
    pub to: String,
    pub from: String,
    pub status: CallStatus,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn create_call(
    State(store): State<Arc<CallStore>>,
    Json(payload): Json<CreateCallRequest>,
) -> Result<(StatusCode, Json<CreateCallResponse>), Response> {
    let sip_from = std::env::var("SIP_FROM").ok();

    if let Some(required_from) = sip_from {
        if payload.from != required_from {
            return Err((
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    error: format!(
                        "from field must match SIP_FROM env (got '{}', expected '{}')",
                        payload.from, required_from
                    ),
                }),
            )
                .into_response());
        }
    }

    if !is_valid_to(&payload.to) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid 'to' field: must be sip:user@host or E.164 format"),
            }),
        )
            .into_response());
    }

    if !is_valid_from(&payload.from) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid 'from' field: must be sip:user@host or E.164 format"),
            }),
        )
            .into_response());
    }

    let call = store.create(payload.to.clone(), payload.from.clone(), payload.url.clone());

    tokio::spawn(handle_call(store.clone(), call.clone()));

    Ok((
        StatusCode::CREATED,
        Json(CreateCallResponse {
            id: call.id,
            status: call.status,
        }),
    ))
}

pub async fn get_call(
    State(store): State<Arc<CallStore>>,
    Path(id): Path<Uuid>,
) -> Result<Json<CallStatusResponse>, StatusCode> {
    let call = store.get(&id).ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(CallStatusResponse {
        id: call.id,
        to: call.to,
        from: call.from,
        status: call.status,
    }))
}

pub async fn hangup_call(
    State(store): State<Arc<CallStore>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let call = store.get(&id).ok_or(StatusCode::NOT_FOUND)?;

    if call.status == CallStatus::Completed || call.status == CallStatus::Failed {
        return Err(StatusCode::GONE);
    }

    store.update_status(&id, CallStatus::Completed);
    Ok(StatusCode::NO_CONTENT)
}

async fn handle_call(store: Arc<CallStore>, call: Call) {
    store.update_status(&call.id, CallStatus::Ringing);

    if let Some(url) = &call.url {
        if let Err(e) = send_webhook(&call, url).await {
            tracing::warn!("Webhook failed for call {}: {}", call.id, e);
        }

        if let Ok(twiml_action) = fetch_twiml(url).await {
            if twiml_action.should_say() {
                tracing::info!("TwiML Say stub: would play tone/silence for call {}", call.id);
            }
        }
    }

    let result = sip::initiate_call(&call.to, &call.from).await;

    match result {
        Ok(_) => {
            store.update_status(&call.id, CallStatus::InProgress);
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            store.update_status(&call.id, CallStatus::Completed);
        }
        Err(e) => {
            tracing::error!("SIP call failed for {}: {}", call.id, e);
            store.update_status(&call.id, CallStatus::Failed);
        }
    }

    if let Some(url) = &call.url {
        let final_call = store.get(&call.id).unwrap();
        if let Err(e) = send_webhook(&final_call, url).await {
            tracing::warn!("Final webhook failed for call {}: {}", call.id, e);
        }
    }
}

async fn send_webhook(call: &Call, url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let params = [
        ("CallSid", call.id.to_string()),
        ("To", call.to.clone()),
        ("From", call.from.clone()),
        ("CallStatus", format!("{:?}", call.status).to_lowercase()),
    ];

    client
        .post(url)
        .form(&params)
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}

async fn fetch_twiml(url: &str) -> Result<twiml::TwimlAction, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await?;
    let xml = response.text().await?;
    twiml::parse_twiml(&xml)
}

fn is_valid_to(to: &str) -> bool {
    is_sip_uri(to) || is_e164(to)
}

fn is_valid_from(from: &str) -> bool {
    is_sip_uri(from) || is_e164(from)
}

fn is_sip_uri(s: &str) -> bool {
    s.starts_with("sip:") && s.contains('@')
}

fn is_e164(s: &str) -> bool {
    s.starts_with('+') && s[1..].chars().all(|c| c.is_ascii_digit()) && s.len() >= 8 && s.len() <= 15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sip_uri() {
        assert!(is_sip_uri("sip:user@example.com"));
        assert!(is_sip_uri("sip:alice@192.168.1.1"));
        assert!(!is_sip_uri("user@example.com"));
        assert!(!is_sip_uri("sip:noat"));
    }

    #[test]
    fn test_is_e164() {
        assert!(is_e164("+14155551234"));
        assert!(is_e164("+441234567890"));
        assert!(!is_e164("14155551234"));
        assert!(!is_e164("+123"));
        assert!(!is_e164("+12345678901234567"));
    }

    #[tokio::test]
    async fn test_create_call_validates_from() {
        std::env::set_var("SIP_FROM", "sip:legit@example.com");
        let store = Arc::new(CallStore::new());

        let req = CreateCallRequest {
            to: "sip:user@example.com".to_string(),
            from: "sip:spoof@evil.com".to_string(),
            url: None,
        };

        let result = create_call(State(store), Json(req)).await;
        assert!(result.is_err());
        std::env::remove_var("SIP_FROM");
    }

    #[tokio::test]
    async fn test_create_call_allows_matching_from() {
        std::env::set_var("SIP_FROM", "sip:legit@example.com");
        let store = Arc::new(CallStore::new());

        let req = CreateCallRequest {
            to: "sip:user@example.com".to_string(),
            from: "sip:legit@example.com".to_string(),
            url: None,
        };

        let result = create_call(State(store), Json(req)).await;
        assert!(result.is_ok());
        std::env::remove_var("SIP_FROM");
    }

    #[tokio::test]
    async fn test_hangup_call() {
        let store = Arc::new(CallStore::new());
        let call = store.create(
            "sip:user@example.com".to_string(),
            "sip:from@example.com".to_string(),
            None,
        );

        let result = hangup_call(State(store.clone()), Path(call.id)).await;
        assert_eq!(result, Ok(StatusCode::NO_CONTENT));

        let updated = store.get(&call.id).unwrap();
        assert_eq!(updated.status, CallStatus::Completed);
    }
}
