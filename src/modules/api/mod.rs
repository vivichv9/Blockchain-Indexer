use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::{routing::get, Json, Router};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;

use crate::modules::jobs::{JobDetails, JobSummary, JobsError, JobsService};

#[derive(Debug, Clone)]
pub struct ApiAuth {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub jobs: JobsService,
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

#[derive(Debug, Serialize)]
struct JobsListResponse {
    items: Vec<JobSummary>,
}

#[derive(Debug, Serialize)]
struct JobDetailsResponse {
    item: JobDetails,
}

pub fn router(auth: ApiAuth, state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/jobs", get(list_jobs))
        .route("/v1/jobs/{job_id}", get(get_job))
        .route("/v1/jobs/{job_id}/start", axum::routing::post(start_job))
        .route("/v1/jobs/{job_id}/stop", axum::routing::post(stop_job))
        .route("/v1/jobs/{job_id}/pause", axum::routing::post(pause_job))
        .route("/v1/jobs/{job_id}/resume", axum::routing::post(resume_job))
        .route("/v1/jobs/{job_id}/retry", axum::routing::post(retry_job))
        .with_state(state)
        .layer(from_fn_with_state(auth, basic_auth_middleware))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn list_jobs(State(state): State<AppState>) -> Result<Json<JobsListResponse>, ApiResponse> {
    let items = state.jobs.list().await.map_err(ApiResponse::from)?;
    Ok(Json(JobsListResponse { items }))
}

async fn get_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.get(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
}

async fn start_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.start(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
}

async fn stop_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.stop(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
}

async fn pause_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.pause(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
}

async fn resume_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.resume(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
}

async fn retry_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.retry(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
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

#[derive(Debug)]
struct ApiResponse {
    status: StatusCode,
    body: Json<ApiError>,
}

impl From<JobsError> for ApiResponse {
    fn from(err: JobsError) -> Self {
        match err {
            JobsError::NotFound => ApiResponse::new(StatusCode::NOT_FOUND, "NOT_FOUND", "Not found"),
            JobsError::InvalidTransition(_) => ApiResponse::new(
                StatusCode::CONFLICT,
                "CONFLICT",
                "Invalid job state transition",
            ),
            JobsError::Serialization(_) => ApiResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Serialization failure",
            ),
            JobsError::Storage(_) => ApiResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Storage failure",
            ),
        }
    }
}

impl ApiResponse {
    fn new(status: StatusCode, code: &'static str, message: &'static str) -> Self {
        Self {
            status,
            body: Json(ApiError {
                code,
                message,
                details: serde_json::json!({}),
            }),
        }
    }
}

impl IntoResponse for ApiResponse {
    fn into_response(self) -> Response {
        (self.status, self.body).into_response()
    }
}
