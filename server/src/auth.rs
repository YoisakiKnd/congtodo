//! 账号与会话：用户名 + 密码（argon2id），HttpOnly Cookie 会话。
//!
//! 注册策略由 CONGTODO_SIGNUP 控制：
//! - `first`（默认）：只有还没有任何用户时允许注册，第一个用户成为管理员
//! - `open`：任何人都可以注册
//! - `closed`：禁止注册（没有任何用户时仍允许创建第一个账号）

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use axum::{
    Extension, Json, Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rusqlite::{Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{
    Shared,
    api::{ApiErr, ApiResult},
    db::USER_TABLES,
};

const COOKIE: &str = "congtodo_session";
const SESSION_DAYS: i64 = 30;

/// 已登录用户，由中间件放进请求扩展
#[derive(Clone, Copy)]
pub struct UserId(pub i64);

#[derive(Clone, Copy, PartialEq)]
pub enum Signup {
    First,
    Open,
    Closed,
}

impl Signup {
    pub fn from_env() -> Self {
        match std::env::var("CONGTODO_SIGNUP").as_deref() {
            Ok("open") => Signup::Open,
            Ok("closed") => Signup::Closed,
            _ => Signup::First,
        }
    }
}

/// 不需要登录的接口
pub fn public_router() -> Router<Shared> {
    Router::new()
        .route("/auth/config", get(config))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
}

/// 需要登录的接口（挂在 require_user 中间件之后）
pub fn private_router() -> Router<Shared> {
    Router::new()
        .route("/auth/me", get(me))
        .route("/auth/password", post(change_password))
}

// ---------- 工具 ----------

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|kv| kv.trim().strip_prefix(&format!("{COOKIE}=")).map(str::to_string))
}

/// 反向代理声明了 https 时自动加 Secure；也可用 CONGTODO_SECURE_COOKIE=1 强制
fn cookie_header(headers: &HeaderMap, value: &str, max_age: i64) -> String {
    let https = headers.get("x-forwarded-proto").and_then(|v| v.to_str().ok()) == Some("https")
        || std::env::var("CONGTODO_SECURE_COOKIE").is_ok_and(|v| v == "1" || v == "true");
    format!(
        "{COOKIE}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{}",
        if https { "; Secure" } else { "" }
    )
}

fn bad(msg: &str) -> ApiErr {
    ApiErr(StatusCode::BAD_REQUEST, msg.into())
}

/// argon2 很耗 CPU，放到阻塞线程池，避免卡住单线程运行时
async fn hash_password(password: String) -> Result<String, ApiErr> {
    tokio::task::spawn_blocking(move || Argon2::default().hash_password(password.as_bytes()).map(|h| h.to_string()))
        .await
        .map_err(|e| ApiErr(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| ApiErr(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn verify_password(password: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
    })
    .await
    .unwrap_or(false)
}

fn create_session(conn: &Connection, user_id: i64) -> Result<String, ApiErr> {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).map_err(|e| ApiErr(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let token = hex::encode(buf);
    conn.execute("DELETE FROM sessions WHERE expires_at < ?", [now()])?;
    conn.execute(
        "INSERT INTO sessions (token_hash, user_id, expires_at) VALUES (?, ?, ?)",
        (hash_token(&token), user_id, now() + SESSION_DAYS * 86400),
    )?;
    Ok(token)
}

fn user_count(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT count(*) FROM users", [], |r| r.get(0))
}

fn signup_allowed(conn: &Connection, mode: Signup) -> rusqlite::Result<bool> {
    Ok(mode == Signup::Open || user_count(conn)? == 0)
}

fn validate(username: &str, password: &str) -> Result<(), ApiErr> {
    let len = username.chars().count();
    if !(3..=32).contains(&len) || !username.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err(bad("用户名需为 3-32 位字母、数字、下划线或横线"));
    }
    if password.chars().count() < 8 {
        return Err(bad("密码至少 8 位"));
    }
    Ok(())
}

// ---------- 中间件 ----------

pub async fn require_user(State(s): State<Shared>, mut req: Request, next: Next) -> Result<Response, ApiErr> {
    let token = session_cookie(req.headers()).ok_or(ApiErr(StatusCode::UNAUTHORIZED, "未登录".into()))?;
    let user_id: Option<i64> = {
        let conn = s.db.lock().unwrap();
        conn.query_row(
            "SELECT user_id FROM sessions WHERE token_hash = ? AND expires_at > ?",
            (hash_token(&token), now()),
            |r| r.get(0),
        )
        .optional()?
    };
    let user_id = user_id.ok_or(ApiErr(StatusCode::UNAUTHORIZED, "登录已过期".into()))?;
    req.extensions_mut().insert(UserId(user_id));
    Ok(next.run(req).await)
}

// ---------- 接口 ----------

#[derive(Deserialize)]
struct Credentials {
    username: String,
    password: String,
}

async fn config(State(s): State<Shared>) -> ApiResult {
    let conn = s.db.lock().unwrap();
    Ok(Json(json!({ "signup": signup_allowed(&conn, s.signup)?, "first": user_count(&conn)? == 0 })))
}

async fn register(State(s): State<Shared>, headers: HeaderMap, Json(c): Json<Credentials>) -> Result<Response, ApiErr> {
    let username = c.username.trim().to_string();
    validate(&username, &c.password)?;
    {
        let conn = s.db.lock().unwrap();
        if !signup_allowed(&conn, s.signup)? {
            return Err(ApiErr(StatusCode::FORBIDDEN, "当前不开放注册".into()));
        }
    }
    let hash = hash_password(c.password).await?;

    let mut conn = s.db.lock().unwrap();
    let tx = conn.transaction()?;
    // 在事务内再判断一次，防止并发注册绕过 first 策略
    if !signup_allowed(&tx, s.signup)? {
        return Err(ApiErr(StatusCode::FORBIDDEN, "当前不开放注册".into()));
    }
    let first = user_count(&tx)? == 0;
    let inserted = tx.execute(
        "INSERT INTO users (username, password_hash, is_admin) VALUES (?, ?, ?)",
        (&username, &hash, first as i64),
    );
    if let Err(rusqlite::Error::SqliteFailure(e, _)) = &inserted {
        if e.code == rusqlite::ErrorCode::ConstraintViolation {
            return Err(ApiErr(StatusCode::CONFLICT, "用户名已被占用".into()));
        }
    }
    inserted?;
    let user_id = tx.last_insert_rowid();
    if first {
        for t in USER_TABLES {
            tx.execute(&format!("UPDATE {t} SET user_id = ? WHERE user_id = 0"), [user_id])?;
        }
    }
    let token = create_session(&tx, user_id)?;
    tx.commit()?;

    Ok((
        [(header::SET_COOKIE, cookie_header(&headers, &token, SESSION_DAYS * 86400))],
        Json(json!({ "id": user_id, "username": username, "is_admin": first })),
    )
        .into_response())
}

async fn login(State(s): State<Shared>, headers: HeaderMap, Json(c): Json<Credentials>) -> Result<Response, ApiErr> {
    let row: Option<(i64, String, String, bool)> = {
        let conn = s.db.lock().unwrap();
        conn.query_row(
            "SELECT id, username, password_hash, is_admin FROM users WHERE username = ?",
            [c.username.trim()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?
    };
    // 用户不存在时也做一次哈希校验，避免通过响应时间探测用户名
    const DUMMY: &str = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$4mXpkOvJ1hqG8a5UX5MUMTvhE3cSB4tXuiUWWtSsI3M";
    let hash = row.as_ref().map_or(DUMMY.to_string(), |r| r.2.clone());
    let ok = verify_password(c.password, hash).await;
    let Some((id, username, _, is_admin)) = row.filter(|_| ok) else {
        return Err(ApiErr(StatusCode::UNAUTHORIZED, "用户名或密码错误".into()));
    };

    let token = {
        let conn = s.db.lock().unwrap();
        create_session(&conn, id)?
    };
    Ok((
        [(header::SET_COOKIE, cookie_header(&headers, &token, SESSION_DAYS * 86400))],
        Json(json!({ "id": id, "username": username, "is_admin": is_admin })),
    )
        .into_response())
}

async fn logout(State(s): State<Shared>, headers: HeaderMap) -> Result<Response, ApiErr> {
    if let Some(token) = session_cookie(&headers) {
        let conn = s.db.lock().unwrap();
        conn.execute("DELETE FROM sessions WHERE token_hash = ?", [hash_token(&token)])?;
    }
    Ok(([(header::SET_COOKIE, cookie_header(&headers, "", 0))], StatusCode::NO_CONTENT).into_response())
}

async fn me(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>) -> ApiResult {
    let conn = s.db.lock().unwrap();
    let (username, is_admin): (String, bool) =
        conn.query_row("SELECT username, is_admin FROM users WHERE id = ?", [uid], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(Json(json!({ "id": uid, "username": username, "is_admin": is_admin })))
}

#[derive(Deserialize)]
struct ChangePassword {
    old: String,
    new: String,
}

/// 修改密码后，本设备保持登录，其他设备的会话全部失效
async fn change_password(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    headers: HeaderMap,
    Json(p): Json<ChangePassword>,
) -> ApiResult<StatusCode> {
    if p.new.chars().count() < 8 {
        return Err(bad("新密码至少 8 位"));
    }
    let hash: String = {
        let conn = s.db.lock().unwrap();
        conn.query_row("SELECT password_hash FROM users WHERE id = ?", [uid], |r| r.get(0))?
    };
    if !verify_password(p.old, hash).await {
        return Err(bad("原密码错误"));
    }
    let new_hash = hash_password(p.new).await?;
    let current = session_cookie(&headers).map(|t| hash_token(&t)).unwrap_or_default();
    let conn = s.db.lock().unwrap();
    conn.execute("UPDATE users SET password_hash = ? WHERE id = ?", (&new_hash, uid))?;
    conn.execute("DELETE FROM sessions WHERE user_id = ? AND token_hash != ?", (uid, &current))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dummy_hash_is_valid_phc() {
        // 登录时用于等时比较的假哈希必须能被解析，否则会提前返回造成时间差
        assert!(PasswordHash::new(
            "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$4mXpkOvJ1hqG8a5UX5MUMTvhE3cSB4tXuiUWWtSsI3M"
        )
        .is_ok());
    }

    #[test]
    fn hash_and_verify() {
        let h = Argon2::default().hash_password(b"correct horse").unwrap().to_string();
        let parsed = PasswordHash::new(&h).unwrap();
        assert!(Argon2::default().verify_password(b"correct horse", &parsed).is_ok());
        assert!(Argon2::default().verify_password(b"wrong", &parsed).is_err());
    }

    #[test]
    fn username_rules() {
        assert!(validate("alice", "12345678").is_ok());
        assert!(validate("小明_01", "12345678").is_ok());
        assert!(validate("ab", "12345678").is_err());
        assert!(validate("a b c", "12345678").is_err());
        assert!(validate("alice", "short").is_err());
    }
}
