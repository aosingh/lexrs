/// lexrs-reader
///
/// Serves searches from a DAWG loaded from a shared volume snapshot.
/// Watches Consul KV for new snapshot versions and reloads atomically.
///
/// USAGE
///   reader [OPTIONS]
///
/// OPTIONS
///   --host <addr>         Bind address    (default: 0.0.0.0,  env: READER_HOST)
///   --port <port>         Listen port     (default: 3001,      env: READER_PORT)
///   --snapshot-dir <path> Shared volume   (default: /snapshots, env: SNAPSHOT_DIR)
///   --consul <url>        Consul address  (default: http://consul:8500, env: CONSUL_ADDR)
///
/// ROUTES
///   GET /search?q=<pattern>[&dist=<n>][&with_count=true]
///   GET /prefix?q=<prefix>[&with_count=true]
///   GET /contains?q=<word>
///   GET /health
///   GET /stats
use std::sync::Arc;

use arc_swap::ArcSwap;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use lexrs::Dawg;

mod consul;
mod snapshot;

// ── state ─────────────────────────────────────────────────────────────────────

struct ReaderState {
    dawg: ArcSwap<Dawg>,
    snapshot_dir: String,
    consul_addr: String,
}

type Shared = Arc<ReaderState>;

// ── request / response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default)]
    with_count: bool,
    dist: Option<usize>,
}

#[derive(Deserialize)]
struct PrefixQuery {
    q: String,
    #[serde(default)]
    with_count: bool,
}

#[derive(Deserialize)]
struct ContainsQuery {
    q: String,
}

#[derive(Serialize)]
struct WordCount {
    word: String,
    count: usize,
}

// ── handlers ──────────────────────────────────────────────────────────────────

async fn search(
    State(state): State<Shared>,
    Query(params): Query<SearchQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let dawg = state.dawg.load();

    if let Some(dist) = params.dist {
        if params.with_count {
            let results: Vec<WordCount> = dawg
                .search_within_distance_count(&params.q, dist)
                .into_iter()
                .map(|(word, count)| WordCount { word, count })
                .collect();
            return (StatusCode::OK, Json(json!(results)));
        }
        return (
            StatusCode::OK,
            Json(json!(dawg.search_within_distance(&params.q, dist))),
        );
    }

    let pairs = match dawg.search_with_count(&params.q) {
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": e.to_string() })),
            );
        }
        Ok(p) => p,
    };

    if params.with_count {
        let results: Vec<WordCount> = pairs
            .into_iter()
            .map(|(word, count)| WordCount { word, count })
            .collect();
        (StatusCode::OK, Json(json!(results)))
    } else {
        let words: Vec<String> = pairs.into_iter().map(|(w, _)| w).collect();
        (StatusCode::OK, Json(json!(words)))
    }
}

async fn prefix_search(
    State(state): State<Shared>,
    Query(params): Query<PrefixQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let dawg = state.dawg.load();
    if params.with_count {
        let results: Vec<WordCount> = dawg
            .search_with_prefix_count(&params.q)
            .into_iter()
            .map(|(word, count)| WordCount { word, count })
            .collect();
        (StatusCode::OK, Json(json!(results)))
    } else {
        (
            StatusCode::OK,
            Json(json!(dawg.search_with_prefix(&params.q))),
        )
    }
}

async fn contains(
    State(state): State<Shared>,
    Query(params): Query<ContainsQuery>,
) -> (StatusCode, Json<serde_json::Value>) {
    let dawg = state.dawg.load();
    (
        StatusCode::OK,
        Json(json!({ "found": dawg.contains(&params.q) })),
    )
}

async fn health() -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

async fn stats(State(state): State<Shared>) -> Json<serde_json::Value> {
    let dawg = state.dawg.load();
    Json(json!({ "words": dawg.word_count(), "nodes": dawg.node_count() }))
}

// ── consul watch + reload ─────────────────────────────────────────────────────

async fn watch_and_reload(state: Shared) {
    let client = reqwest::Client::new();
    let mut index = 0u64;

    loop {
        let url = format!(
            "{}/v1/kv/lexrs/snapshot?wait=30s&index={}",
            state.consul_addr, index
        );

        let res = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[watch] consul error: {e}");
                continue;
            }
        };

        let new_index: u64 = res
            .headers()
            .get("X-Consul-Index")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(index);

        if new_index <= index {
            // timeout with no change — loop
            continue;
        }

        index = new_index;

        let body: serde_json::Value = match res.json().await {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[watch] parse error: {e}");
                continue;
            }
        };

        // Consul returns base64-encoded value
        let encoded = match body[0]["Value"].as_str() {
            Some(v) => v,
            None => continue,
        };
        let decoded = match base64_decode(encoded) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[watch] base64 error: {e}");
                continue;
            }
        };
        let meta: serde_json::Value = match serde_json::from_str(&decoded) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[watch] json error: {e}");
                continue;
            }
        };

        let path = match meta["path"].as_str() {
            Some(p) => p.to_string(),
            None => continue,
        };
        let version = meta["version"].as_u64().unwrap_or(0);

        println!("[watch] new snapshot v{version} at {path}");

        match snapshot::load(&path).await {
            Ok(dawg) => {
                state.dawg.store(Arc::new(dawg));
                println!("[watch] reloaded DAWG from v{version}");
            }
            Err(e) => eprintln!("[watch] reload failed: {e}"),
        }
    }
}

fn base64_decode(s: &str) -> Result<String, String> {
    // standard base64 without external crate — use built-in via a simple decode
    let bytes = (0..s.len())
        .step_by(4)
        .flat_map(|i| {
            let chunk = &s[i..(i + 4).min(s.len())];
            decode_chunk(chunk)
        })
        .collect::<Vec<u8>>();
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

fn decode_chunk(chunk: &str) -> Vec<u8> {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let idx: Vec<u8> = chunk
        .bytes()
        .map(|c| {
            if c == b'=' {
                0
            } else {
                TABLE.iter().position(|&t| t == c).unwrap_or(0) as u8
            }
        })
        .collect();
    if idx.len() < 4 {
        return vec![];
    }
    let b0 = (idx[0] << 2) | (idx[1] >> 4);
    let b1 = (idx[1] << 4) | (idx[2] >> 2);
    let b2 = (idx[2] << 6) | idx[3];
    match chunk.contains('=') {
        true if chunk.ends_with("==") => vec![b0],
        true => vec![b0, b1],
        false => vec![b0, b1, b2],
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

    let host = flag(&args, "--host").unwrap_or_else(|| env_or("READER_HOST", "0.0.0.0"));
    let port = flag(&args, "--port").unwrap_or_else(|| env_or("READER_PORT", "3001"));
    let snapshot_dir =
        flag(&args, "--snapshot-dir").unwrap_or_else(|| env_or("SNAPSHOT_DIR", "/snapshots"));
    let consul_addr =
        flag(&args, "--consul").unwrap_or_else(|| env_or("CONSUL_ADDR", "http://consul:8500"));

    // Load latest snapshot from shared volume on startup
    let initial_dawg = match consul::latest_snapshot_path(&consul_addr).await {
        Ok(Some(path)) => {
            println!("Loading initial snapshot from {path}");
            snapshot::load(&path).await.unwrap_or_else(|e| {
                eprintln!("Failed to load snapshot: {e}");
                Dawg::new()
            })
        }
        _ => {
            println!("No snapshot found, starting with empty DAWG");
            Dawg::new()
        }
    };

    // Register with Consul
    let instance_id = format!("lexrs-reader-{}", uuid::Uuid::new_v4());
    let health_url = format!("http://{}:{}/health", hostname(), port);
    if let Err(e) = consul::register(
        &consul_addr,
        &instance_id,
        "lexrs-reader",
        &health_url,
        port.parse().unwrap_or(3001),
    )
    .await
    {
        eprintln!("Consul registration failed: {e}");
    }

    let state: Shared = Arc::new(ReaderState {
        dawg: ArcSwap::new(Arc::new(initial_dawg)),
        snapshot_dir,
        consul_addr,
    });

    // Spawn Consul watch background task
    tokio::spawn(watch_and_reload(Arc::clone(&state)));

    let app = Router::new()
        .route("/search", get(search))
        .route("/prefix", get(prefix_search))
        .route("/contains", get(contains))
        .route("/health", get(health))
        .route("/stats", get(stats))
        .with_state(state);

    let addr = format!("{host}:{port}");
    println!("lexrs-reader listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Failed to bind {addr}: {e}");
            std::process::exit(1);
        });
    axum::serve(listener, app).await.unwrap();
}

fn hostname() -> String {
    std::env::var("HOSTNAME").unwrap_or_else(|_| "reader".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_swap::ArcSwap;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use lexrs::Dawg;
    use tower::ServiceExt;

    fn test_state() -> Shared {
        let mut dawg = Dawg::new();
        dawg.add("apple", 3).unwrap();
        dawg.add("apply", 1).unwrap();
        dawg.add("apt", 2).unwrap();
        dawg.add("banana", 5).unwrap();
        dawg.reduce();
        Arc::new(ReaderState {
            dawg: ArcSwap::new(Arc::new(dawg)),
            snapshot_dir: "/tmp".to_string(),
            consul_addr: "http://127.0.0.1:1".to_string(),
        })
    }

    fn build_app(state: Shared) -> Router {
        Router::new()
            .route("/search", get(search))
            .route("/prefix", get(prefix_search))
            .route("/contains", get(contains))
            .route("/health", get(health))
            .route("/stats", get(stats))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_health() {
        let res = build_app(test_state())
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn test_contains_found() {
        let res = build_app(test_state())
            .oneshot(
                Request::get("/contains?q=apple")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["found"], true);
    }

    #[tokio::test]
    async fn test_contains_not_found() {
        let res = build_app(test_state())
            .oneshot(
                Request::get("/contains?q=cherry")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["found"], false);
    }

    #[tokio::test]
    async fn test_prefix_search() {
        let res = build_app(test_state())
            .oneshot(Request::get("/prefix?q=ap").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let mut words: Vec<String> = serde_json::from_slice(&body).unwrap();
        words.sort();
        assert_eq!(words, vec!["apple", "apply", "apt"]);
    }

    #[tokio::test]
    async fn test_wildcard_search() {
        let res = build_app(test_state())
            .oneshot(Request::get("/search?q=b*").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let words: Vec<String> = serde_json::from_slice(&body).unwrap();
        assert_eq!(words, vec!["banana"]);
    }

    #[tokio::test]
    async fn test_fuzzy_search() {
        let res = build_app(test_state())
            .oneshot(
                Request::get("/search?q=aple&dist=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let words: Vec<String> = serde_json::from_slice(&body).unwrap();
        assert!(words.contains(&"apple".to_string()));
    }

    #[tokio::test]
    async fn test_search_with_count() {
        let res = build_app(test_state())
            .oneshot(
                Request::get("/search?q=apple&with_count=true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let results: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["word"], "apple");
        assert_eq!(results[0]["count"], 3);
    }

    #[tokio::test]
    async fn test_stats() {
        let res = build_app(test_state())
            .oneshot(Request::get("/stats").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        // word_count() returns sum of all frequencies: 3+1+2+5 = 11
        assert_eq!(json["words"], 11);
    }

    #[test]
    fn test_base64_decode() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), "hello");
        assert_eq!(base64_decode("d29ybGQ=").unwrap(), "world");
        assert_eq!(base64_decode("Zm9v").unwrap(), "foo");
    }
}
