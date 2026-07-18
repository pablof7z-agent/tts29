use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use tokio::sync::Semaphore;

use crate::{AuthFailure, AuthValidator};

#[derive(Clone)]
pub struct IngressState {
    auth: AuthValidator,
    permits: Arc<Semaphore>,
    timeout: Duration,
    max_request_bytes: usize,
    max_response_bytes: usize,
    resource_metadata: Arc<str>,
    required_scope: Arc<str>,
}

impl IngressState {
    pub fn new(
        auth: AuthValidator,
        concurrency: usize,
        timeout: Duration,
        max_request_bytes: usize,
        max_response_bytes: usize,
        resource_metadata: impl Into<Arc<str>>,
        required_scope: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            auth,
            permits: Arc::new(Semaphore::new(concurrency)),
            timeout,
            max_request_bytes,
            max_response_bytes,
            resource_metadata: resource_metadata.into(),
            required_scope: required_scope.into(),
        }
    }
}

pub async fn protect(State(state): State<IngressState>, request: Request, next: Next) -> Response {
    if has_query_token(request.uri().query()) {
        return json_error(StatusCode::BAD_REQUEST, "invalid_request");
    }
    let token = match bearer_token(request.headers()) {
        Some(token) => token,
        None => return auth_error(&state, AuthFailure::InvalidToken, false),
    };
    if let Err(error) = state.auth.validate(token) {
        return auth_error(&state, error, true);
    }
    let permit = match state.permits.clone().try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => return json_error(StatusCode::SERVICE_UNAVAILABLE, "server_busy"),
    };
    let (parts, body) = request.into_parts();
    let bytes = match to_bytes(body, state.max_request_bytes).await {
        Ok(bytes) => bytes,
        Err(_) => return json_error(StatusCode::PAYLOAD_TOO_LARGE, "request_too_large"),
    };
    let request = Request::from_parts(parts, Body::from(bytes));
    let response = match tokio::time::timeout(state.timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => return json_error(StatusCode::GATEWAY_TIMEOUT, "request_timed_out"),
    };
    drop(permit);
    bound_response(response, state.max_response_bytes).await
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    let (scheme, token) = value.split_once(' ')?;
    (scheme.eq_ignore_ascii_case("Bearer")
        && !token.is_empty()
        && !token.bytes().any(|byte| byte.is_ascii_whitespace()))
    .then_some(token)
}

fn has_query_token(query: Option<&str>) -> bool {
    query.is_some_and(|value| {
        url::form_urlencoded::parse(value.as_bytes()).any(|(key, _)| key == "access_token")
    })
}

fn auth_error(state: &IngressState, failure: AuthFailure, include_error: bool) -> Response {
    let (status, error) = match failure {
        AuthFailure::InvalidToken => (StatusCode::UNAUTHORIZED, "invalid_token"),
        AuthFailure::InsufficientScope => (StatusCode::FORBIDDEN, "insufficient_scope"),
    };
    let mut challenge = format!(
        "Bearer resource_metadata=\"{}\", scope=\"{}\"",
        state.resource_metadata, state.required_scope
    );
    if include_error || failure == AuthFailure::InsufficientScope {
        challenge.push_str(&format!(", error=\"{error}\""));
    }
    let mut response = json_error(status, error);
    if let Ok(value) = HeaderValue::from_str(&challenge) {
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, value);
    }
    response
}

async fn bound_response(response: Response, limit: usize) -> Response {
    let (parts, body) = response.into_parts();
    match to_bytes(body, limit).await {
        Ok(bytes) => Response::from_parts(parts, Body::from(bytes)),
        Err(_) => json_error(StatusCode::INTERNAL_SERVER_ERROR, "response_too_large"),
    }
}

fn json_error(status: StatusCode, code: &'static str) -> Response {
    (status, Json(json!({ "error": code }))).into_response()
}
