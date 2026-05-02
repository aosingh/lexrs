/// lexrs-writer
///
/// Accepts word ingestion and drives compaction.
/// Compacted snapshots are written to a shared volume and announced via Consul KV.
///
/// USAGE
///   writer [OPTIONS]
///
/// OPTIONS
///   --host <addr>             Bind address          (default: 0.0.0.0,  env: WRITER_HOST)
///   --port <port>             Listen port           (default: 3000,      env: WRITER_PORT)
///   --snapshot-dir <path>     Shared volume path    (default: /snapshots, env: SNAPSHOT_DIR)
///   --consul <url>            Consul address        (default: http://consul:8500, env: CONSUL_ADDR)
///   --compact-interval <secs> Auto-compact interval (default: 60, env: COMPACT_INTERVAL)
///
/// ROUTES
///   POST /words          {"words": ["foo", "bar"], "count": 1}
///   POST /compact        Trigger compaction immediately
///   GET  /snapshot/:ver  Download a snapshot file
///   GET  /health         Health check (for Consul)
///   GET  /stats          Trie word/node counts
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use lexrs::Trie;

mod consul;
mod snapshot;

// ── state ─────────────────────────────────────────────────────────────────────

struct WriterState {
    trie: RwLock<Trie>,
    snapshot_dir: String,
    consul_addr: String,
    version: std::sync::atomic::AtomicU64,
}

type Shared = Arc<WriterState>;

// ── request types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(untagged)]
enum WordEntry {
    Simple(String),
    WithCount { word: String, count: usize },
}

#[derive(Deserialize)]
struct IngestBody {
    words: Vec<WordEntry>,
    #[serde(default = "default_count")]
    count: usize,
}

fn default_count() -> usize {
    1
}

#[derive(Serialize)]
struct StatsResponse {
    words: usize,
    nodes: usize,
}

// ── handlers ──────────────────────────────────────────────────────────────────

async fn ingest(
    State(state): State<Shared>,
    Json(body): Json<IngestBody>,
) -> (StatusCode, Json<serde_json::Value>) {
    let mut trie = state.trie.write().unwrap();
    let mut inserted = 0usize;
    for entry in &body.words {
        let (word, count) = match entry {
            WordEntry::Simple(w) => (w.as_str(), body.count),
            WordEntry::WithCount { word, count } => (word.as_str(), *count),
        };
        if let Err(e) = trie.add(word, count) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": e.to_string(), "inserted": inserted })),
            );
        }
        inserted += 1;
    }
    (StatusCode::OK, Json(json!({ "inserted": inserted })))
}

async fn compact_handler(State(state): State<Shared>) -> (StatusCode, Json<serde_json::Value>) {
    match run_compact(&state).await {
        Ok(version) => (
            StatusCode::OK,
            Json(json!({ "status": "ok", "version": version })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        ),
    }
}

async fn get_snapshot(
    State(state): State<Shared>,
    Path(version): Path<u64>,
) -> impl axum::response::IntoResponse {
    let path = format!("{}/snapshot_{}.txt", state.snapshot_dir, version);
    match tokio::fs::read(&path).await {
        Ok(bytes) => (StatusCode::OK, bytes).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "snapshot not found").into_response(),
    }
}

async fn health() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

async fn stats(State(state): State<Shared>) -> Json<serde_json::Value> {
    let trie = state.trie.read().unwrap();
    Json(json!({ "words": trie.word_count(), "nodes": trie.node_count() }))
}

// ── compaction ────────────────────────────────────────────────────────────────

async fn run_compact(state: &WriterState) -> Result<u64, String> {
    // 1. Drain new words from Trie under a brief read lock
    let new_words: Vec<(String, usize)> = {
        let trie = state.trie.read().unwrap();
        trie.search_with_count("*").unwrap_or_default()
    };

    if new_words.is_empty() {
        return Ok(state.version.load(std::sync::atomic::Ordering::SeqCst));
    }

    // 2. Merge with existing snapshot (streaming — O(1) memory)
    let next_version = state
        .version
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        + 1;
    let current_version = next_version - 1;

    let existing_path = if current_version > 0 {
        Some(format!(
            "{}/snapshot_{}.txt",
            state.snapshot_dir, current_version
        ))
    } else {
        None
    };

    snapshot::merge_and_write(
        &state.snapshot_dir,
        next_version,
        existing_path.as_deref(),
        &new_words,
    )
    .await
    .map_err(|e| e.to_string())?;

    // 3. Announce to readers via Consul KV
    let path = format!("{}/snapshot_{}.txt", state.snapshot_dir, next_version);
    consul::put_snapshot(&state.consul_addr, next_version, &path)
        .await
        .map_err(|e| e.to_string())?;

    // 4. Clear Trie — it only holds the delta since last compact
    *state.trie.write().unwrap() = Trie::new();

    println!(
        "[compact] v{next_version}: merged {} new words",
        new_words.len()
    );
    Ok(next_version)
}

async fn compact_task(state: Shared, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    ticker.tick().await;
    loop {
        ticker.tick().await;
        let word_count = state.trie.read().unwrap().word_count();
        if word_count == 0 {
            continue;
        }
        if let Err(e) = run_compact(&state).await {
            eprintln!("[compact] error: {e}");
        }
    }
}

// ── startup ───────────────────────────────────────────────────────────────────

fn flag(args: &[String], key: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == key).map(|w| w[1].clone())
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();

    let host = flag(&args, "--host").unwrap_or_else(|| env_or("WRITER_HOST", "0.0.0.0"));
    let port = flag(&args, "--port").unwrap_or_else(|| env_or("WRITER_PORT", "3000"));
    let snapshot_dir =
        flag(&args, "--snapshot-dir").unwrap_or_else(|| env_or("SNAPSHOT_DIR", "/snapshots"));
    let consul_addr =
        flag(&args, "--consul").unwrap_or_else(|| env_or("CONSUL_ADDR", "http://consul:8500"));
    let interval_s: u64 = flag(&args, "--compact-interval")
        .unwrap_or_else(|| env_or("COMPACT_INTERVAL", "60"))
        .parse()
        .unwrap_or(60);

    // Recover version counter from Consul — Trie starts empty (holds delta only)
    let start_version = match consul::latest_snapshot(&consul_addr).await {
        Ok(Some((version, _))) => {
            println!("[startup] resuming from snapshot v{version}, Trie empty");
            version
        }
        _ => {
            println!("[startup] no snapshot found, starting fresh");
            0
        }
    };

    // Register with Consul
    let instance_id = format!("lexrs-writer-{}", uuid::Uuid::new_v4());
    let health_url = format!("http://{}:{}/health", hostname(), port);
    if let Err(e) = consul::register(
        &consul_addr,
        &instance_id,
        "lexrs-writer",
        &health_url,
        port.parse().unwrap_or(3000),
    )
    .await
    {
        eprintln!("Consul registration failed: {e}");
    }

    let state: Shared = Arc::new(WriterState {
        trie: RwLock::new(Trie::new()),
        snapshot_dir: snapshot_dir.clone(),
        consul_addr: consul_addr.clone(),
        version: std::sync::atomic::AtomicU64::new(start_version),
    });

    // Spawn background compaction
    tokio::spawn(compact_task(
        Arc::clone(&state),
        Duration::from_secs(interval_s),
    ));

    let app = Router::new()
        .route("/words", post(ingest))
        .route("/compact", post(compact_handler))
        .route("/snapshot/{version}", get(get_snapshot))
        .route("/health", get(health))
        .route("/stats", get(stats))
        .with_state(state);

    let addr = format!("{host}:{port}");
    println!("lexrs-writer listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Failed to bind {addr}: {e}");
            std::process::exit(1);
        });
    axum::serve(listener, app).await.unwrap();
}

fn hostname() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "writer".to_string())
}

// bring axum::response::IntoResponse into scope for get_snapshot
use axum::response::IntoResponse;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_state(snapshot_dir: &str) -> Shared {
        Arc::new(WriterState {
            trie: RwLock::new(Trie::new()),
            snapshot_dir: snapshot_dir.to_string(),
            consul_addr: "http://127.0.0.1:1".to_string(),
            version: std::sync::atomic::AtomicU64::new(0),
        })
    }

    fn build_app(state: Shared) -> Router {
        Router::new()
            .route("/words", post(ingest))
            .route("/compact", post(compact_handler))
            .route("/snapshot/{version}", get(get_snapshot))
            .route("/health", get(health))
            .route("/stats", get(stats))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_health() {
        let res = build_app(test_state("/tmp"))
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn test_ingest_simple_words() {
        let res = build_app(test_state("/tmp"))
            .oneshot(
                Request::post("/words")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"words": ["apple", "banana", "cherry"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["inserted"], 3);
    }

    #[tokio::test]
    async fn test_ingest_with_per_word_counts() {
        let res = build_app(test_state("/tmp"))
            .oneshot(
                Request::post("/words")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"words": [{"word": "apple", "count": 5}, "banana"]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["inserted"], 2);
    }

    #[tokio::test]
    async fn test_stats_after_ingest() {
        let state = test_state("/tmp");
        build_app(Arc::clone(&state))
            .oneshot(
                Request::post("/words")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"words": ["apple", "apply", "apt"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        let res = build_app(Arc::clone(&state))
            .oneshot(Request::get("/stats").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["words"], 3);
    }

    #[tokio::test]
    async fn test_snapshot_not_found() {
        let res = build_app(test_state("/tmp"))
            .oneshot(Request::get("/snapshot/9999").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_compact_fails_without_consul() {
        let res = build_app(test_state("/tmp"))
            .oneshot(
                Request::post("/words")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"words": ["apple"]}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Compact will fail because Consul is unreachable — expect 500
        let state = test_state("/tmp");
        {
            state.trie.write().unwrap().add("apple", 1).unwrap();
        }
        let res = build_app(state)
            .oneshot(Request::post("/compact").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
