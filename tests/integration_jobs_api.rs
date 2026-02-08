use std::time::Duration;

use reqwest::StatusCode;
use serde_json::Value;
use testcontainers::core::WaitFor;
use testcontainers::{clients::Cli, GenericImage};
use tokio::time::sleep;

use bitcoin_blockchain_indexer::modules::api::{self, ApiAuth, AppState};
use bitcoin_blockchain_indexer::modules::config::JobConfig;
use bitcoin_blockchain_indexer::modules::jobs::JobsService;
use bitcoin_blockchain_indexer::modules::storage::Storage;

async fn start_api(bind_addr: &str, auth: ApiAuth, state: AppState) {
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .expect("bind listener");

    tokio::spawn(async move {
        axum::serve(listener, api::router(auth, state))
            .await
            .expect("server");
    });
}

fn docker_available() -> bool {
    std::process::Command::new("docker")
        .arg("info")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

async fn setup() -> Option<(String, ApiAuth)> {
    if !docker_available() {
        eprintln!("Docker is not available, skipping integration test.");
        return None;
    }

    let docker = Cli::default();
    let image = GenericImage::new("postgres", "16")
        .with_env_var("POSTGRES_DB", "postgres")
        .with_env_var("POSTGRES_USER", "postgres")
        .with_env_var("POSTGRES_PASSWORD", "postgres")
        .with_exposed_port(5432)
        .with_wait_for(WaitFor::message_on_stdout(
            "database system is ready to accept connections",
        ));
    let node = docker.run(image);
    let port = node.get_host_port_ipv4(5432);

    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    std::env::set_var("DATABASE_URL", &database_url);
    std::env::set_var("MIGRATIONS_PATH", "migrations");

    let storage = Storage::connect().await.expect("connect storage");
    storage
        .apply_migrations()
        .await
        .expect("apply migrations");

    let jobs = vec![JobConfig {
        job_id: "full-sync".to_string(),
        mode: "all_addresses".to_string(),
        enabled: true,
        addresses: vec![],
    }];

    let jobs_service = JobsService::new(storage.pool().clone());
    jobs_service
        .sync_from_config(&jobs)
        .await
        .expect("sync jobs");

    let auth = ApiAuth {
        username: "admin".to_string(),
        password: "pass".to_string(),
    };

    let state = AppState { jobs: jobs_service };
    let bind_addr = "127.0.0.1:18080".to_string();
    start_api(&bind_addr, auth.clone(), state).await;
    sleep(Duration::from_millis(150)).await;

    Some((bind_addr, auth))
}

#[tokio::test]
#[ignore]
async fn jobs_lifecycle_api() {
    let Some((bind_addr, auth)) = setup().await else {
        return;
    };

    let client = reqwest::Client::new();

    let list_resp = client
        .get(format!("http://{bind_addr}/v1/jobs"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("list jobs");

    assert_eq!(list_resp.status(), StatusCode::OK);

    let list_body: Value = list_resp.json().await.expect("list body");
    assert_eq!(list_body["items"].as_array().unwrap().len(), 1);

    let start_resp = client
        .post(format!("http://{bind_addr}/v1/jobs/full-sync/start"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("start job");

    assert_eq!(start_resp.status(), StatusCode::OK);

    let start_body: Value = start_resp.json().await.expect("start body");
    assert_eq!(start_body["item"]["status"], "running");

    let pause_resp = client
        .post(format!("http://{bind_addr}/v1/jobs/full-sync/pause"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("pause job");

    assert_eq!(pause_resp.status(), StatusCode::OK);

    let pause_body: Value = pause_resp.json().await.expect("pause body");
    assert_eq!(pause_body["item"]["status"], "paused");

    let resume_resp = client
        .post(format!("http://{bind_addr}/v1/jobs/full-sync/resume"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("resume job");

    assert_eq!(resume_resp.status(), StatusCode::OK);

    let resume_body: Value = resume_resp.json().await.expect("resume body");
    assert_eq!(resume_body["item"]["status"], "running");

    let stop_resp = client
        .post(format!("http://{bind_addr}/v1/jobs/full-sync/stop"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("stop job");

    assert_eq!(stop_resp.status(), StatusCode::OK);

    let stop_body: Value = stop_resp.json().await.expect("stop body");
    assert_eq!(stop_body["item"]["status"], "created");
}

#[tokio::test]
#[ignore]
async fn jobs_requires_auth() {
    let Some((bind_addr, auth)) = setup().await else {
        return;
    };
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{bind_addr}/v1/jobs"))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let resp = client
        .get(format!("http://{bind_addr}/v1/jobs"))
        .basic_auth(&auth.username, Some("wrong"))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore]
async fn jobs_not_found() {
    let Some((bind_addr, auth)) = setup().await else {
        return;
    };
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://{bind_addr}/v1/jobs/missing"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore]
async fn jobs_invalid_transition() {
    let Some((bind_addr, auth)) = setup().await else {
        return;
    };
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://{bind_addr}/v1/jobs/full-sync/pause"))
        .basic_auth(&auth.username, Some(&auth.password))
        .send()
        .await
        .expect("request");

    assert_eq!(resp.status(), StatusCode::CONFLICT);
}
