//! 待办清单与共享：清单创建者可以把清单分享给其他用户（按用户名），成员可查看和编辑清单内的待办。
//! 提醒只发给待办的创建者。

use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, patch, post},
};
use rusqlite::{Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    Shared,
    api::{ApiErr, ApiResult, bad},
    auth::UserId,
};

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/lists", get(list).post(create))
        .route("/lists/{id}", patch(update).delete(remove))
        .route("/lists/{id}/members", post(add_member))
        .route("/lists/{id}/members/{user_id}", delete(remove_member))
}

fn owner_of(conn: &Connection, list_id: i64) -> Result<i64, ApiErr> {
    Ok(conn.query_row("SELECT user_id FROM todo_lists WHERE id = ?", [list_id], |r| r.get(0))?)
}

fn require_owner(conn: &Connection, uid: i64, list_id: i64) -> Result<(), ApiErr> {
    if owner_of(conn, list_id)? != uid {
        return Err(ApiErr(StatusCode::FORBIDDEN, "只有清单创建者可以这样做".into()));
    }
    Ok(())
}

/// 清单信息：owner 为创建者用户名，shared 表示当前用户是成员（不是创建者）
fn list_row(conn: &Connection, uid: i64, id: i64) -> rusqlite::Result<Value> {
    let (name, color, sort, owner_id, owner): (String, Option<String>, i64, i64, String) = conn.query_row(
        "SELECT l.name, l.color, l.sort, l.user_id, u.username FROM todo_lists l JOIN users u ON u.id = l.user_id
         WHERE l.id = ?",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    let mut stmt = conn.prepare_cached(
        "SELECT u.id, u.username FROM list_members m JOIN users u ON u.id = m.user_id WHERE m.list_id = ? ORDER BY u.username",
    )?;
    let members: Vec<Value> = stmt
        .query_map([id], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "username": r.get::<_, String>(1)? })))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(json!({
        "id": id, "name": name, "color": color, "sort": sort,
        "owner_id": owner_id, "owner": owner, "shared": owner_id != uid, "members": members,
    }))
}

async fn list(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>) -> ApiResult {
    let conn = s.db.lock().unwrap();
    let ids: Vec<i64> = conn
        .prepare_cached(
            "SELECT id FROM todo_lists WHERE user_id = ?1
             UNION SELECT list_id FROM list_members WHERE user_id = ?1",
        )?
        .query_map([uid], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let mut rows = ids.into_iter().map(|id| list_row(&conn, uid, id)).collect::<rusqlite::Result<Vec<_>>>()?;
    rows.sort_by_key(|r| (r["shared"].as_bool(), r["sort"].as_i64(), r["id"].as_i64()));
    Ok(Json(Value::Array(rows)))
}

#[derive(Deserialize)]
struct ListBody {
    name: Option<String>,
    color: Option<String>,
    sort: Option<i64>,
}

async fn create(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>, Json(b): Json<ListBody>) -> ApiResult {
    let name = b.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).ok_or_else(|| bad("清单名不能为空"))?;
    let conn = s.db.lock().unwrap();
    conn.execute(
        "INSERT INTO todo_lists (user_id, name, color, sort) VALUES (?, ?, ?, ?)",
        (uid, &name, &b.color, b.sort.unwrap_or(0)),
    )?;
    Ok(Json(list_row(&conn, uid, conn.last_insert_rowid())?))
}

async fn update(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path(id): Path<i64>,
    Json(b): Json<ListBody>,
) -> ApiResult {
    let conn = s.db.lock().unwrap();
    require_owner(&conn, uid, id)?;
    if let Some(name) = b.name.map(|n| n.trim().to_string()) {
        if name.is_empty() {
            return Err(bad("清单名不能为空"));
        }
        conn.execute("UPDATE todo_lists SET name = ? WHERE id = ?", (&name, id))?;
    }
    if let Some(color) = &b.color {
        conn.execute("UPDATE todo_lists SET color = ? WHERE id = ?", (color, id))?;
    }
    if let Some(sort) = b.sort {
        conn.execute("UPDATE todo_lists SET sort = ? WHERE id = ?", (sort, id))?;
    }
    Ok(Json(list_row(&conn, uid, id)?))
}

/// 删除清单；其中的待办保留但不再属于任何清单（回到各自创建者的「收件箱」）
async fn remove(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    let conn = s.db.lock().unwrap();
    require_owner(&conn, uid, id)?;
    conn.execute("DELETE FROM todo_lists WHERE id = ?", [id])?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct Member {
    username: String,
}

async fn add_member(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path(id): Path<i64>,
    Json(m): Json<Member>,
) -> ApiResult {
    let conn = s.db.lock().unwrap();
    require_owner(&conn, uid, id)?;
    let member: i64 = conn
        .query_row("SELECT id FROM users WHERE username = ? AND disabled = 0", [m.username.trim()], |r| r.get(0))
        .optional()?
        .ok_or_else(|| bad("找不到这个用户"))?;
    if member == uid {
        return Err(bad("不能把清单分享给自己"));
    }
    conn.execute("INSERT OR IGNORE INTO list_members (list_id, user_id) VALUES (?, ?)", [id, member])?;
    Ok(Json(list_row(&conn, uid, id)?))
}

/// 创建者移除成员，或成员自己退出
async fn remove_member(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path((id, member)): Path<(i64, i64)>,
) -> ApiResult<StatusCode> {
    let conn = s.db.lock().unwrap();
    if member != uid {
        require_owner(&conn, uid, id)?;
    }
    conn.execute("DELETE FROM list_members WHERE list_id = ? AND user_id = ?", [id, member])?;
    Ok(StatusCode::NO_CONTENT)
}
