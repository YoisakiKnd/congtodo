mod api;
mod auth;
mod db;
mod notify;
mod scheduler;
mod web;

use std::sync::{Arc, Mutex};

use axum::{Router, middleware, routing::get};

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub http: reqwest::Client,
    pub signup: auth::Signup,
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

    let conn = db::open(&db_path).expect("failed to open database");
    let state: Shared = Arc::new(AppState {
        db: Mutex::new(conn),
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent(concat!("congtodo/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap(),
        signup: auth::Signup::from_env(),
    });

    scheduler::spawn(state.clone());

    let private = api::router()
        .merge(auth::private_router())
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_user));
    let api = auth::public_router()
        .merge(private)
        .route("/health", get(|| async { concat!("ok ", env!("CARGO_PKG_VERSION")) }))
        .with_state(state.clone());

    let app = Router::new().nest("/api", api).fallback(web::serve);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("congtodo {} listening on http://{addr}  (db: {db_path})", env!("CARGO_PKG_VERSION"));
    axum::serve(listener, app).with_graceful_shutdown(shutdown()).await.unwrap();
}

/// Ctrl-C 或 SIGTERM（docker stop）时优雅退出
async fn shutdown() {
    let term = async {
        #[cfg(unix)]
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
            return;
        }
        std::future::pending::<()>().await
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = term => {}
    }
}
