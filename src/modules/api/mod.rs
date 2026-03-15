use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{IntoResponse, Response};
use axum::{routing::get, Json, Router};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use crate::modules::data::{
    BalanceFilter, BlocksFilter, DataError, DataService, Pagination, TransactionsFilter,
};
use crate::modules::jobs::{JobDetails, JobSummary, JobsError, JobsService};
use crate::modules::metrics::MetricsService;
use crate::modules::nodes::{NodeHealthDetails, NodeSummary, NodesError, NodesService};

#[derive(Debug, Clone)]
pub struct ApiAuth {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub jobs: JobsService,
    pub data: DataService,
    pub metrics: MetricsService,
    pub nodes: NodesService,
}

#[derive(Debug, Serialize)]
#[derive(ToSchema)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[derive(ToSchema)]
struct ApiError {
    code: &'static str,
    message: &'static str,
    details: serde_json::Value,
}

#[derive(Debug, Serialize)]
#[derive(ToSchema)]
struct JobsListResponse {
    items: Vec<JobSummary>,
}

#[derive(Debug, Serialize)]
#[derive(ToSchema)]
struct JobDetailsResponse {
    item: JobDetails,
}

#[derive(Debug, Serialize)]
#[derive(ToSchema)]
struct NodesListResponse {
    items: Vec<NodeSummary>,
}

#[derive(Debug, Serialize)]
#[derive(ToSchema)]
struct NodeDetailsResponse {
    item: NodeHealthDetails,
}

#[derive(Debug, Deserialize)]
#[derive(IntoParams)]
struct PaginationQuery {
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[derive(IntoParams)]
struct BalanceQuery {
    from_time: Option<i64>,
    to_time: Option<i64>,
    from_height: Option<i32>,
    to_height: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[derive(IntoParams)]
struct TransactionsQuery {
    from_height: Option<i32>,
    to_height: Option<i32>,
    from_time: Option<i64>,
    to_time: Option<i64>,
    address: Option<String>,
    txid: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[derive(IntoParams)]
struct MempoolQuery {
    address: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[derive(IntoParams)]
struct BlocksQuery {
    from_height: Option<i32>,
    to_height: Option<i32>,
    from_time: Option<i64>,
    to_time: Option<i64>,
    block_hash: Option<String>,
    has_txid: Option<String>,
    address: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        health,
        metrics,
        list_jobs,
        get_job,
        start_job,
        stop_job,
        pause_job,
        resume_job,
        retry_job,
        list_nodes,
        get_node_health,
        get_balance,
        get_utxos,
        list_transactions,
        list_mempool_transactions,
        list_blocks
    ),
    components(
        schemas(
            HealthResponse,
            ApiError,
            JobsListResponse,
            JobDetailsResponse,
            NodesListResponse,
            NodeDetailsResponse,
            JobSummary,
            JobDetails,
            NodeSummary,
            NodeHealthDetails,
            crate::modules::data::Pagination,
            crate::modules::data::BalanceResponse,
            crate::modules::data::BalanceAsOf,
            crate::modules::data::UtxoItem,
            crate::modules::data::UtxosResponse,
            crate::modules::data::TransactionIo,
            crate::modules::data::TransactionItem,
            crate::modules::data::TransactionsPage,
            crate::modules::data::BlockItem,
            crate::modules::data::BlocksPage
        )
    ),
    modifiers(&ApiSecurityAddon),
    tags(
        (name = "system", description = "Service health and metrics"),
        (name = "jobs", description = "Indexer jobs management"),
        (name = "nodes", description = "Bitcoin RPC node health"),
        (name = "data", description = "Indexed blockchain data queries")
    )
)]
struct ApiDoc;

struct ApiSecurityAddon;

impl utoipa::Modify for ApiSecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "basic_auth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Basic)
                    .description(Some("Basic auth credentials for indexer API"))
                    .build(),
            ),
        );
    }
}

pub fn router(auth: ApiAuth, state: AppState) -> Router {
    let openapi = ApiDoc::openapi();

    Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/v1/jobs", get(list_jobs))
        .route("/v1/jobs/{job_id}", get(get_job))
        .route("/v1/jobs/{job_id}/start", axum::routing::post(start_job))
        .route("/v1/jobs/{job_id}/stop", axum::routing::post(stop_job))
        .route("/v1/jobs/{job_id}/pause", axum::routing::post(pause_job))
        .route("/v1/jobs/{job_id}/resume", axum::routing::post(resume_job))
        .route("/v1/jobs/{job_id}/retry", axum::routing::post(retry_job))
        .route("/v1/nodes", get(list_nodes))
        .route("/v1/nodes/{node_id}/health", get(get_node_health))
        .route("/v1/data/addresses/{address}/balance", get(get_balance))
        .route("/v1/data/addresses/{address}/utxos", get(get_utxos))
        .route("/v1/data/transactions", get(list_transactions))
        .route("/v1/data/transactions/mempool", get(list_mempool_transactions))
        .route("/v1/data/blocks", get(list_blocks))
        .merge(SwaggerUi::new("/docs").url("/openapi.json", openapi))
        .with_state(state)
        .layer(from_fn_with_state(auth, basic_auth_middleware))
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Service health status", body = HealthResponse)
    )
)]
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

#[utoipa::path(
    get,
    path = "/metrics",
    tag = "system",
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Prometheus metrics", content_type = "text/plain")
    )
)]
async fn metrics(State(state): State<AppState>) -> Result<Response, ApiResponse> {
    let body = state
        .metrics
        .render(state.jobs.pool())
        .await
        .map_err(|_| ApiResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "Storage failure"))?;

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
        .into_response())
}

#[utoipa::path(
    get,
    path = "/v1/jobs",
    tag = "jobs",
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Configured jobs with current status", body = JobsListResponse),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn list_jobs(State(state): State<AppState>) -> Result<Json<JobsListResponse>, ApiResponse> {
    let tip_height = state.nodes.tip_height().await.map_err(ApiResponse::from)?;
    let items = state
        .jobs
        .list()
        .await
        .map_err(ApiResponse::from)?
        .into_iter()
        .map(|mut item| {
            item.tip_height = tip_height;
            item
        })
        .collect();
    Ok(Json(JobsListResponse { items }))
}

#[utoipa::path(
    get,
    path = "/v1/jobs/{job_id}",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Job details", body = JobDetailsResponse),
        (status = 404, description = "Job not found", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn get_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.get(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
}

#[utoipa::path(
    get,
    path = "/v1/nodes",
    tag = "nodes",
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Configured nodes", body = NodesListResponse),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn list_nodes(State(state): State<AppState>) -> Result<Json<NodesListResponse>, ApiResponse> {
    let items = state.nodes.list().await.map_err(ApiResponse::from)?;
    Ok(Json(NodesListResponse { items }))
}

#[utoipa::path(
    get,
    path = "/v1/nodes/{node_id}/health",
    tag = "nodes",
    params(
        ("node_id" = String, Path, description = "Node identifier")
    ),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Node health details", body = NodeDetailsResponse),
        (status = 404, description = "Node not found", body = ApiError),
        (status = 503, description = "Node is unavailable", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn get_node_health(
    Path(node_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<NodeDetailsResponse>, ApiResponse> {
    let item = state.nodes.get(&node_id).await.map_err(ApiResponse::from)?;
    Ok(Json(NodeDetailsResponse { item }))
}

#[utoipa::path(
    post,
    path = "/v1/jobs/{job_id}/start",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Started job", body = JobDetailsResponse),
        (status = 404, description = "Job not found", body = ApiError),
        (status = 409, description = "Invalid state transition", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn start_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.start(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
}

#[utoipa::path(
    post,
    path = "/v1/jobs/{job_id}/stop",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Stopped job", body = JobDetailsResponse),
        (status = 404, description = "Job not found", body = ApiError),
        (status = 409, description = "Invalid state transition", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn stop_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.stop(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
}

#[utoipa::path(
    post,
    path = "/v1/jobs/{job_id}/pause",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Paused job", body = JobDetailsResponse),
        (status = 404, description = "Job not found", body = ApiError),
        (status = 409, description = "Invalid state transition", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn pause_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.pause(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
}

#[utoipa::path(
    post,
    path = "/v1/jobs/{job_id}/resume",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Resumed job", body = JobDetailsResponse),
        (status = 404, description = "Job not found", body = ApiError),
        (status = 409, description = "Invalid state transition", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn resume_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.resume(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
}

#[utoipa::path(
    post,
    path = "/v1/jobs/{job_id}/retry",
    tag = "jobs",
    params(
        ("job_id" = String, Path, description = "Job identifier")
    ),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Retried job", body = JobDetailsResponse),
        (status = 404, description = "Job not found", body = ApiError),
        (status = 409, description = "Invalid state transition", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn retry_job(
    Path(job_id): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<JobDetailsResponse>, ApiResponse> {
    let item = state.jobs.retry(&job_id).await.map_err(ApiResponse::from)?;
    Ok(Json(JobDetailsResponse { item }))
}

#[utoipa::path(
    get,
    path = "/v1/data/addresses/{address}/balance",
    tag = "data",
    params(
        ("address" = String, Path, description = "Bitcoin address"),
        BalanceQuery
    ),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Current or historical address balance", body = crate::modules::data::BalanceResponse),
        (status = 404, description = "Address is not indexed", body = ApiError),
        (status = 422, description = "Validation failed", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn get_balance(
    Path(address): Path<String>,
    Query(query): Query<BalanceQuery>,
    State(state): State<AppState>,
) -> Result<Json<crate::modules::data::BalanceResponse>, ApiResponse> {
    let item = state
        .data
        .get_balance(
            &address,
            BalanceFilter {
                from_time: query.from_time,
                to_time: query.to_time,
                from_height: query.from_height,
                to_height: query.to_height,
            },
        )
        .await
        .map_err(ApiResponse::from)?;
    Ok(Json(item))
}

#[utoipa::path(
    get,
    path = "/v1/data/addresses/{address}/utxos",
    tag = "data",
    params(
        ("address" = String, Path, description = "Bitcoin address")
    ),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Current UTXO set for address", body = crate::modules::data::UtxosResponse),
        (status = 404, description = "Address is not indexed", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn get_utxos(
    Path(address): Path<String>,
    State(state): State<AppState>,
) -> Result<Json<crate::modules::data::UtxosResponse>, ApiResponse> {
    let item = state.data.get_utxos(&address).await.map_err(ApiResponse::from)?;
    Ok(Json(item))
}

#[utoipa::path(
    get,
    path = "/v1/data/transactions",
    tag = "data",
    params(TransactionsQuery),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Confirmed transactions page", body = crate::modules::data::TransactionsPage),
        (status = 404, description = "Address is not indexed", body = ApiError),
        (status = 422, description = "Validation failed", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn list_transactions(
    Query(query): Query<TransactionsQuery>,
    State(state): State<AppState>,
) -> Result<Json<crate::modules::data::TransactionsPage>, ApiResponse> {
    let pagination = parse_pagination(&state.data, query.offset, query.limit)?;
    let page = state
        .data
        .list_transactions(
            TransactionsFilter {
                from_height: query.from_height,
                to_height: query.to_height,
                from_time: query.from_time,
                to_time: query.to_time,
                address: query.address,
                txid: query.txid,
            },
            pagination,
        )
        .await
        .map_err(ApiResponse::from)?;
    Ok(Json(page))
}

#[utoipa::path(
    get,
    path = "/v1/data/transactions/mempool",
    tag = "data",
    params(MempoolQuery),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Mempool transactions page", body = crate::modules::data::TransactionsPage),
        (status = 404, description = "Address is not indexed", body = ApiError),
        (status = 422, description = "Validation failed", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn list_mempool_transactions(
    Query(query): Query<MempoolQuery>,
    State(state): State<AppState>,
) -> Result<Json<crate::modules::data::TransactionsPage>, ApiResponse> {
    let pagination = parse_pagination(&state.data, query.offset, query.limit)?;
    let page = state
        .data
        .list_mempool_transactions(query.address.as_deref(), pagination)
        .await
        .map_err(ApiResponse::from)?;
    Ok(Json(page))
}

#[utoipa::path(
    get,
    path = "/v1/data/blocks",
    tag = "data",
    params(BlocksQuery),
    security(
        ("basic_auth" = [])
    ),
    responses(
        (status = 200, description = "Canonical blocks page", body = crate::modules::data::BlocksPage),
        (status = 404, description = "Address is not indexed", body = ApiError),
        (status = 422, description = "Validation failed", body = ApiError),
        (status = 500, description = "Storage failure", body = ApiError)
    )
)]
async fn list_blocks(
    Query(query): Query<BlocksQuery>,
    State(state): State<AppState>,
) -> Result<Json<crate::modules::data::BlocksPage>, ApiResponse> {
    let pagination = parse_pagination(&state.data, query.offset, query.limit)?;
    let page = state
        .data
        .list_blocks(
            BlocksFilter {
                from_height: query.from_height,
                to_height: query.to_height,
                from_time: query.from_time,
                to_time: query.to_time,
                block_hash: query.block_hash,
                has_txid: query.has_txid,
                address: query.address,
            },
            pagination,
        )
        .await
        .map_err(ApiResponse::from)?;
    Ok(Json(page))
}

fn parse_pagination(
    _data: &DataService,
    offset: Option<i64>,
    limit: Option<i64>,
) -> Result<Pagination, ApiResponse> {
    DataService::validate_pagination(offset, limit).map_err(ApiResponse::from)
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

impl From<DataError> for ApiResponse {
    fn from(err: DataError) -> Self {
        match err {
            DataError::AddressNotIndexed => ApiResponse::with_details(
                StatusCode::NOT_FOUND,
                "ADDRESS_NOT_INDEXED",
                "Address is not indexed",
                serde_json::json!({}),
            ),
            DataError::Validation(message) => ApiResponse::with_details(
                StatusCode::UNPROCESSABLE_ENTITY,
                "VALIDATION_ERROR",
                "Validation failed",
                serde_json::json!({ "reason": message }),
            ),
            DataError::Storage(_) => ApiResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "Storage failure",
            ),
        }
    }
}

impl From<NodesError> for ApiResponse {
    fn from(err: NodesError) -> Self {
        match err {
            NodesError::NotFound => ApiResponse::new(StatusCode::NOT_FOUND, "NOT_FOUND", "Not found"),
            NodesError::Rpc(_) => ApiResponse::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "NODE_UNAVAILABLE",
                "Node is unavailable",
            ),
            NodesError::Storage(_) => ApiResponse::new(
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

    fn with_details(
        status: StatusCode,
        code: &'static str,
        message: &'static str,
        details: serde_json::Value,
    ) -> Self {
        Self {
            status,
            body: Json(ApiError {
                code,
                message,
                details,
            }),
        }
    }
}

impl IntoResponse for ApiResponse {
    fn into_response(self) -> Response {
        (self.status, self.body).into_response()
    }
}
