mod admin;
mod api;
mod auth;
mod data;
mod db;
mod holidays;
mod ics;
mod lists;
mod notify;
mod ratelimit;
mod scheduler;
mod tz;
mod web;
mod webpush;

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{Router, middleware, routing::get};

pub struct AppState {
    pub db: Mutex<rusqlite::Connection>,
    pub http: reqwest::Client,
    /// 环境变量给出的注册策略；管理员在界面上设置后以数据库为准
    pub signup_default: auth::Signup,
    pub limiter: ratelimit::Limiter,
    pub vapid: webpush::Vapid,
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
    let vapid = webpush::Vapid::load_or_create(&conn).expect("failed to init VAPID keys");
    let state: Shared = Arc::new(AppState {
        db: Mutex::new(conn),
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent(concat!("congtodo/", env!("CARGO_PKG_VERSION")))
            .build()
            .unwrap(),
        signup_default: auth::Signup::from_env(),
        limiter: ratelimit::Limiter::default(),
        vapid,
    });

    scheduler::spawn(state.clone());
    ics::spawn(state.clone());
    holidays::spawn(state.clone());

    let private = api::router()
        .merge(auth::private_router())
        .merge(lists::router())
        .merge(data::router())
        .merge(ics::private_router())
        .merge(holidays::router())
        .merge(admin::router(state.clone()))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_user));
    let api = auth::public_router()
        .merge(ics::public_router())
        .merge(private)
        .route("/health", get(|| async { concat!("ok ", env!("CARGO_PKG_VERSION")) }))
        .with_state(state.clone());

    let app = Router::new().nest("/api", api).fallback(web::serve);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("congtodo {} listening on http://{addr}  (db: {db_path})", env!("CARGO_PKG_VERSION"));
    // 带上对端地址，限流需要客户端 IP
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown())
        .await
        .unwrap();
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
