//! 管理员接口：用户管理 + 全站设置。挂在 require_user 之后，再经 require_admin 校验。

use axum::{
    Extension, Json, Router,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, patch},
};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    Shared,
    api::{ApiErr, ApiResult},
    auth::{self, Signup, UserId},
    db::{USER_TABLES, row_to_json},
};

pub fn router(state: Shared) -> Router<Shared> {
    Router::new()
        .route("/admin/users", get(list).post(create))
        .route("/admin/users/{id}", patch(update).delete(remove))
        .route("/admin/settings", get(get_settings).put(put_settings))
        .route("/admin/backup", get(backup))
        .route_layer(middleware::from_fn_with_state(state, require_admin))
}

async fn require_admin(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    req: Request,
    next: Next,
) -> Result<Response, ApiErr> {
    let is_admin: bool = {
        let conn = s.db.lock().unwrap();
        conn.query_row("SELECT is_admin FROM users WHERE id = ?", [uid], |r| r.get(0))?
    };
    if !is_admin {
        return Err(ApiErr(StatusCode::FORBIDDEN, "需要管理员权限".into()));
    }
    Ok(next.run(req).await)
}

fn bad(msg: &str) -> ApiErr {
    ApiErr(StatusCode::BAD_REQUEST, msg.into())
}

const USER_SQL: &str = "
    SELECT u.id, u.username, u.is_admin, u.disabled, u.created_at, u.last_login_at,
      (SELECT count(*) FROM sessions WHERE user_id = u.id AND expires_at > unixepoch()) AS sessions,
      (SELECT count(*) FROM todos WHERE user_id = u.id)
        + (SELECT count(*) FROM events WHERE user_id = u.id)
        + (SELECT count(*) FROM courses WHERE user_id = u.id)
        + (SELECT count(*) FROM habits WHERE user_id = u.id)
        + (SELECT count(*) FROM countdowns WHERE user_id = u.id) AS items
    FROM users u";

fn user_row(conn: &Connection, id: i64) -> ApiResult {
    Ok(Json(conn.query_row(&format!("{USER_SQL} WHERE u.id = ?"), [id], |r| row_to_json(r, &[]))?))
}

async fn list(State(s): State<Shared>) -> ApiResult {
    let conn = s.db.lock().unwrap();
    let mut stmt = conn.prepare_cached(&format!("{USER_SQL} ORDER BY u.id"))?;
    let rows = stmt.query_map([], |r| row_to_json(r, &[]))?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Json(Value::Array(rows)))
}

#[derive(Deserialize)]
struct NewUser {
    username: String,
    password: String,
    #[serde(default)]
    is_admin: bool,
}

/// 管理员创建账号不受注册策略限制
async fn create(State(s): State<Shared>, Json(u): Json<NewUser>) -> ApiResult {
    let username = u.username.trim().to_string();
    auth::validate_username(&username)?;
    auth::validate_password(&u.password)?;
    let hash = auth::hash(u.password).await?;
    let conn = s.db.lock().unwrap();
    let inserted = conn.execute(
        "INSERT INTO users (username, password_hash, is_admin) VALUES (?, ?, ?)",
        (&username, &hash, u.is_admin),
    );
    if let Err(rusqlite::Error::SqliteFailure(e, _)) = &inserted
        && e.code == rusqlite::ErrorCode::ConstraintViolation {
            return Err(ApiErr(StatusCode::CONFLICT, "用户名已被占用".into()));
        }
    inserted?;
    user_row(&conn, conn.last_insert_rowid())
}

#[derive(Deserialize)]
struct Patch {
    is_admin: Option<bool>,
    disabled: Option<bool>,
    password: Option<String>,
}

async fn update(
    State(s): State<Shared>,
    Extension(UserId(me)): Extension<UserId>,
    Path(id): Path<i64>,
    Json(p): Json<Patch>,
) -> ApiResult {
    // 不能降级或停用自己，保证始终至少有一个可用的管理员
    if id == me && (p.is_admin == Some(false) || p.disabled == Some(true)) {
        return Err(bad("不能取消自己的管理员身份或停用自己"));
    }
    let hash = match p.password {
        Some(pw) => {
            auth::validate_password(&pw)?;
            Some(auth::hash(pw).await?)
        }
        None => None,
    };

    let mut conn = s.db.lock().unwrap();
    let tx = conn.transaction()?;
    tx.query_row("SELECT 1 FROM users WHERE id = ?", [id], |_| Ok(()))?;
    if let Some(v) = p.is_admin {
        tx.execute("UPDATE users SET is_admin = ? WHERE id = ?", (v, id))?;
    }
    if let Some(v) = p.disabled {
        tx.execute("UPDATE users SET disabled = ? WHERE id = ?", (v, id))?;
    }
    if let Some(h) = &hash {
        tx.execute("UPDATE users SET password_hash = ? WHERE id = ?", (h, id))?;
    }
    // 重置密码或停用后，让该用户所有设备重新登录
    if hash.is_some() || p.disabled == Some(true) {
        tx.execute("DELETE FROM sessions WHERE user_id = ?", [id])?;
    }
    tx.commit()?;
    user_row(&conn, id)
}

/// 删除用户及其全部数据
async fn remove(
    State(s): State<Shared>,
    Extension(UserId(me)): Extension<UserId>,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    if id == me {
        return Err(bad("不能删除自己"));
    }
    let mut conn = s.db.lock().unwrap();
    let tx = conn.transaction()?;
    tx.query_row("SELECT 1 FROM users WHERE id = ?", [id], |_| Ok(()))?;
    // habit_logs 随 habits 级联删除，sessions 随 users 级联删除
    for t in USER_TABLES {
        tx.execute(&format!("DELETE FROM {t} WHERE user_id = ?"), [id])?;
    }
    tx.execute("DELETE FROM users WHERE id = ?", [id])?;
    tx.commit()?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_settings(State(s): State<Shared>) -> ApiResult {
    let conn = s.db.lock().unwrap();
    Ok(Json(json!({
        "signup": Signup::current(&conn, s.signup_default).as_str(),
        "signup_default": s.signup_default.as_str(),
    })))
}

#[derive(Deserialize)]
struct ServerSettings {
    signup: String,
}

async fn put_settings(State(s): State<Shared>, Json(b): Json<ServerSettings>) -> ApiResult<StatusCode> {
    let mode = Signup::parse(&b.signup).ok_or_else(|| bad("signup 只能是 first / open / closed"))?;
    let conn = s.db.lock().unwrap();
    conn.execute(
        "INSERT INTO server_settings (key, value) VALUES ('signup', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [mode.as_str()],
    )?;
    Ok(StatusCode::NO_CONTENT)
}

/// 下载整个数据库的一致性快照（VACUUM INTO，不影响正在运行的服务）
async fn backup(State(s): State<Shared>) -> Result<Response, ApiErr> {
    let path = std::env::temp_dir().join(format!("congtodo-backup-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    {
        let conn = s.db.lock().unwrap();
        conn.execute("VACUUM INTO ?", [path.to_string_lossy()])?;
    }
    let data = std::fs::read(&path).map_err(|e| ApiErr(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
    let _ = std::fs::remove_file(&path);
    let name = format!("congtodo-{}.db", chrono::Utc::now().format("%Y%m%d-%H%M%S"));
    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.sqlite3".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
        ],
        data?,
    )
        .into_response())
}
