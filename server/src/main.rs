mod api;
mod db;
mod notify;
mod scheduler;
mod web;

use std::sync::{Arc, Mutex};

use axum::{
    Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
};

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub http: reqwest::Client,
    pub token: Option<String>,
}

pub type Shared = Arc<AppState>;

fn env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

// 单线程运行时：个人应用并发很低，省掉多线程 worker 的栈和调度开销
#[tokio::main(flavor = "current_thread")]
async fn main() {
    let db_path = env("CONGTODO_DB", "congtodo.db");
    let addr = env("CONGTODO_ADDR", "0.0.0.0:8080");
    let token = std::env::var("CONGTODO_TOKEN").ok().filter(|t| !t.is_empty());

    let conn = db::open(&db_path).expect("failed to open database");
    let state: Shared = Arc::new(AppState {
        db: Mutex::new(conn),
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("congtodo/0.1")
            .build()
            .unwrap(),
        token,
    });

    scheduler::spawn(state.clone());

    let api = api::router()
        .layer(middleware::from_fn_with_state(state.clone(), auth))
        .with_state(state.clone());

    let app = Router::new().nest("/api", api).fallback(web::serve);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("congtodo listening on http://{addr}  (db: {db_path})");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .unwrap();
}

/// 设置了 CONGTODO_TOKEN 时，/api 需要 `Authorization: Bearer <token>`
async fn auth(State(s): State<Shared>, req: Request, next: Next) -> Result<Response, StatusCode> {
    if let Some(token) = &s.token {
        let ok = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .is_some_and(|v| v == token);
        if !ok {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }
    Ok(next.run(req).await)
}
