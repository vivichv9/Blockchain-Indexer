use axum::body::Body;
use axum::extract::State;
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::{routing::get, Json, Router};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ApiAuth {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct ApiError {
    code: &'static str,
    message: &'static str,
    details: serde_json::Value,
}

pub fn router(auth: ApiAuth) -> Router {
    Router::new()
        .route("/health", get(health))
        .layer(from_fn_with_state(auth, basic_auth_middleware))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn basic_auth_middleware(
    State(auth): State<ApiAuth>,
    request: Request<Body>,
    next: Next,
) -> Response {
    match is_authorized(request.headers().get(AUTHORIZATION), &auth) {
        true => next.run(request).await,
        false => unauthorized_response(),
    }
}

fn is_authorized(header: Option<&HeaderValue>, auth: &ApiAuth) -> bool {
    let Some(header) = header else {
        return false;
    };

    let Ok(value) = header.to_str() else {
        return false;
    };

    let Some(encoded) = value.strip_prefix("Basic ") else {
        return false;
    };

    let Ok(decoded) = STANDARD.decode(encoded) else {
        return false;
    };

    let Ok(credentials) = String::from_utf8(decoded) else {
        return false;
    };

    let mut parts = credentials.splitn(2, ':');
    let username = parts.next().unwrap_or_default();
    let password = parts.next().unwrap_or_default();

    username == auth.username && password == auth.password
}

fn unauthorized_response() -> Response {
    let body = Json(ApiError {
        code: "AUTH_FAILED",
        message: "Authentication failed",
        details: serde_json::json!({}),
    });

    let mut response = (StatusCode::UNAUTHORIZED, body).into_response();
    response.headers_mut().insert(
        "www-authenticate",
        HeaderValue::from_static("Basic realm=\"indexer\""),
    );

    response
}
