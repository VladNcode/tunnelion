//! Minimal local upstream for tunnelion: `GET /api/time`, `GET /api/slow?ms=…`.

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

#[derive(Clone)]
struct App {
    name: Arc<str>,
}

#[derive(Deserialize)]
struct SlowQuery {
    ms: Option<u64>,
}

fn usage() -> ! {
    eprintln!("usage: tunnelion-demo-server <PORT> <APP_NAME>");
    eprintln!("  GET /api/time");
    eprintln!("  GET /api/slow?ms=1000   (optional ms, default 1000, max 120000)");
    std::process::exit(1);
}

fn parse_args() -> (u16, Arc<str>) {
    let mut it = env::args().skip(1);
    let port_s = it.next().unwrap_or_else(|| usage());
    let name_s = it.next().unwrap_or_else(|| usage());
    if it.next().is_some() {
        usage();
    }
    let port: u16 = port_s.parse().unwrap_or_else(|_| usage());
    let name_s = name_s.trim();
    if port == 0 || name_s.is_empty() {
        usage();
    }
    (port, Arc::from(name_s.to_string().into_boxed_str()))
}

async fn api_time(State(s): State<App>) -> Json<serde_json::Value> {
    let unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    Json(json!({ "app": s.name.as_ref(), "unix_ms": unix_ms }))
}

async fn api_slow(State(s): State<App>, Query(q): Query<SlowQuery>) -> Json<serde_json::Value> {
    let ms = q.ms.unwrap_or(1000).clamp(0, 120_000);
    tokio::time::sleep(Duration::from_millis(ms)).await;
    Json(json!({ "app": s.name.as_ref(), "waited_ms": ms, "done": true }))
}

async fn not_found() -> (StatusCode, &'static str) {
    (StatusCode::NOT_FOUND, "not found\n")
}

#[tokio::main]
async fn main() {
    let (port, name) = parse_args();
    let app = App {
        name: Arc::clone(&name),
    };
    let router = Router::new()
        .route("/api/time", get(api_time))
        .route("/api/slow", get(api_slow))
        .fallback(not_found)
        .with_state(app);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    eprintln!("http://{addr}  app={name}  /api/time  /api/slow?ms=…");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, router).await.unwrap();
}
