use serde_json::json;

/// Register this instance with the Consul agent.
pub async fn register(
    consul_addr: &str,
    instance_id: &str,
    service_name: &str,
    health_url: &str,
    port: u16,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let body = json!({
        "ID":      instance_id,
        "Name":    service_name,
        "Port":    port,
        "Check": {
            "HTTP":                           health_url,
            "Interval":                       "5s",
            "Timeout":                        "2s",
            "DeregisterCriticalServiceAfter": "30s",
        }
    });

    client
        .put(format!("{consul_addr}/v1/agent/service/register"))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Write snapshot metadata to Consul KV.
/// Value: {"version": N, "path": "/snapshots/snapshot_N.txt"}
pub async fn put_snapshot(
    consul_addr: &str,
    version: u64,
    path: &str,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let value  = json!({ "version": version, "path": path }).to_string();

    client
        .put(format!("{consul_addr}/v1/kv/lexrs/snapshot"))
        .body(value)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Read the latest snapshot path from Consul KV (non-blocking, used at startup).
pub async fn latest_snapshot_path(consul_addr: &str) -> Result<Option<String>, String> {
    let client = reqwest::Client::new();

    let res = client
        .get(format!("{consul_addr}/v1/kv/lexrs/snapshot"))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    let body: serde_json::Value = res.json().await.map_err(|e| e.to_string())?;

    let encoded = body[0]["Value"]
        .as_str()
        .ok_or("missing Value field")?;

    // Consul base64-encodes KV values
    let decoded = base64_std_decode(encoded)?;
    let meta: serde_json::Value = serde_json::from_str(&decoded).map_err(|e| e.to_string())?;

    Ok(meta["path"].as_str().map(String::from))
}

// minimal base64 decode (no external dep)
fn base64_std_decode(s: &str) -> Result<String, String> {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let bytes: Vec<u8> = s.bytes().filter(|&c| c != b'\n' && c != b'\r').collect();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 { break; }
        let idx: Vec<u8> = chunk.iter().map(|&c| {
            if c == b'=' { 0 } else { T.iter().position(|&t| t == c).unwrap_or(0) as u8 }
        }).collect();
        out.push((idx[0] << 2) | (idx[1] >> 4));
        if chunk[2] != b'=' { out.push((idx[1] << 4) | (idx[2] >> 2)); }
        if chunk[3] != b'=' { out.push((idx[2] << 6) | idx[3]); }
    }
    String::from_utf8(out).map_err(|e| e.to_string())
}
