// avisafe-djilog-parser v0.2 — Rust + dji-log-parser
//
// Endpoints:
//   GET  /health     -> 200 ok
//   POST /parse      -> multipart: file=<DJI .txt|.zip>, [format=json|csv]
//                       Bearer-auth via AVISAFE_PARSER_TOKEN.
//                       Decrypts v13+ logs using DJI_API_KEY env.
//
// Response (json, default):
// {
//   "ok": true,
//   "version": 13,
//   "details": { ... },           // serde_json::Value of dji_log_parser::Details
//   "frames":  [ {custom,osd,gimbal,camera,rc,battery,home,recover,app}, ... ]
// }
//
// On unsupported / unrecoverable: 422 {error, reason}

use axum::{
    extract::{DefaultBodyLimit, Multipart},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::{io::Read, net::SocketAddr};
use tracing::{error, info, warn};

const MAX_BODY: usize = 50 * 1024 * 1024; // 50 MB

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    let app = Router::new()
        .route("/health", get(health))
        .route("/", get(health))
        .route("/parse", post(parse))
        .layer(DefaultBodyLimit::max(MAX_BODY));

    let addr: SocketAddr = "0.0.0.0:8080".parse().unwrap();
    info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "ok"
}


fn err(status: StatusCode, reason: impl Into<String>) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(json!({ "error": "parse_failed", "reason": reason.into() })),
    )
}

async fn parse(headers: HeaderMap, mut multipart: Multipart) -> impl IntoResponse {
    // --- auth ---
    let expected_token = std::env::var("AVISAFE_PARSER_TOKEN").unwrap_or_default();
    if expected_token.is_empty() {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "AVISAFE_PARSER_TOKEN not set");
    }
    let auth_ok = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.strip_prefix("Bearer ").unwrap_or(""))
        .map(|t| t == expected_token)
        .unwrap_or(false);
    if !auth_ok {
        return err(StatusCode::UNAUTHORIZED, "invalid bearer token");
    }

    // --- multipart fields ---
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut format = "json".to_string();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                match field.bytes().await {
                    Ok(b) => file_bytes = Some(b.to_vec()),
                    Err(e) => return err(StatusCode::BAD_REQUEST, format!("read file: {e}")),
                }
            }
            "format" => {
                if let Ok(b) = field.bytes().await {
                    format = String::from_utf8_lossy(&b).trim().to_string();
                }
            }
            _ => {
                let _ = field.bytes().await;
            }
        }
    }

    let mut bytes = match file_bytes {
        Some(b) => b,
        None => return err(StatusCode::BAD_REQUEST, "missing 'file' field"),
    };

    info!(
        "received {} bytes, filename={:?}, format={}",
        bytes.len(),
        filename,
        format
    );

    // --- unzip if needed ---
    if bytes.len() >= 4 && &bytes[0..2] == b"PK" {
        match extract_txt_from_zip(&bytes) {
            Ok(inner) => {
                info!("unzipped to {} bytes", inner.len());
                bytes = inner;
            }
            Err(e) => return err(StatusCode::UNPROCESSABLE_ENTITY, format!("unzip: {e}")),
        }
    }

    // --- parse ---
    let log = match dji_log_parser::DJILog::from_bytes(bytes) {
        Ok(l) => l,
        Err(e) => {
            warn!("from_bytes failed: {e}");
            return err(StatusCode::UNPROCESSABLE_ENTITY, format!("from_bytes: {e}"));
        }
    };

    info!("dji log version={}", log.version);

    // --- keychains for v13+ ---
    // fetch_keychains uses blocking ureq; wrap in block_in_place so we don't
    // stall the async runtime but still keep `&log` borrow.
    let keychains = if log.version >= 13 {
        let api_key = std::env::var("DJI_API_KEY").unwrap_or_default();
        if api_key.is_empty() {
            return err(
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("v{} log requires DJI_API_KEY (not configured)", log.version),
            );
        }
        let kc = tokio::task::block_in_place(|| log.fetch_keychains(&api_key));
        match kc {
            Ok(k) => Some(k),
            Err(e) => {
                error!("fetch_keychains failed: {e}");
                return err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("fetch_keychains: {e}"),
                );
            }
        }
    } else {
        None
    };

    // --- frames (CPU bound) ---
    let frames = match log.frames(keychains) {
        Ok(f) => f,
        Err(e) => {
            warn!("frames failed: {e}");
            return err(StatusCode::UNPROCESSABLE_ENTITY, format!("frames: {e}"));
        }
    };

    info!("parsed {} frames", frames.len());

    let details_value = serde_json::to_value(&log.details).unwrap_or(Value::Null);

    if format == "csv" {
        let csv = frames_to_csv(&frames);
        return (
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "version": log.version,
                "details": details_value,
                "csv": csv,
                "frame_count": frames.len(),
            })),
        );
    }

    // default json
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "version": log.version,
            "details": details_value,
            "frames": frames,
            "frame_count": frames.len(),
        })),
    )
}

fn extract_txt_from_zip(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_lowercase();
        if name.ends_with(".txt") {
            let mut out = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut out).map_err(|e| e.to_string())?;
            return Ok(out);
        }
    }
    Err("no .txt entry in zip".into())
}

fn frames_to_csv(frames: &[dji_log_parser::frame::Frame]) -> String {
    // Generic CSV: serialize each frame to flat JSON, collect union of keys.
    let mut rows: Vec<serde_json::Map<String, Value>> = Vec::with_capacity(frames.len());
    let mut keys: Vec<String> = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for f in frames {
        let v = serde_json::to_value(f).unwrap_or(Value::Null);
        let mut flat = serde_json::Map::new();
        flatten("", &v, &mut flat);
        for k in flat.keys() {
            if seen.insert(k.clone()) {
                keys.push(k.clone());
            }
        }
        rows.push(flat);
    }

    let mut out = String::new();
    out.push_str(&keys.iter().map(|k| csv_escape(k)).collect::<Vec<_>>().join(","));
    out.push('\n');
    for r in rows {
        let line = keys
            .iter()
            .map(|k| match r.get(k) {
                Some(Value::Null) | None => String::new(),
                Some(Value::String(s)) => csv_escape(s),
                Some(v) => csv_escape(&v.to_string()),
            })
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn flatten(prefix: &str, v: &Value, out: &mut serde_json::Map<String, Value>) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&key, val, out);
            }
        }
        _ => {
            out.insert(prefix.to_string(), v.clone());
        }
    }
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        let escaped = s.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}
