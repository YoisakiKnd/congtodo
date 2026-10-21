//! 数据导出 / 导入、图片上传、推送记录。

use std::collections::HashMap;

use axum::{
    Extension, Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use rusqlite::{Connection, params_from_iter, types::Value as SqlValue};
use serde_json::{Map, Value, json};

use crate::{
    Shared,
    api::{ApiErr, ApiResult, bad, to_sql},
    auth::UserId,
    db::row_to_json,
};

const MAX_IMAGE: usize = 2 * 1024 * 1024;

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/export", get(export))
        .route("/import", post(import).layer(DefaultBodyLimit::max(32 * 1024 * 1024)))
        .route("/files", post(upload).layer(DefaultBodyLimit::max(MAX_IMAGE + 1024)))
        .route("/files/{id}", get(download).delete(delete_file))
        .route("/outbox", get(outbox))
}

/// 导出的表及其可导入的列（不含 id / user_id，id 在导入时重新分配）
const TABLES: &[(&str, &[&str])] = &[
    ("todo_lists", &["name", "color", "sort", "created_at"]),
    ("countdowns", &["title", "target_at", "color", "pinned", "remind_before", "repeat", "created_at"]),
    ("events", &["title", "start_at", "end_at", "all_day", "repeat", "color", "note", "remind_before", "created_at"]),
    (
        "courses",
        &["name", "teacher", "room", "weekday", "start_time", "end_time", "weeks", "color", "remind_before", "period_from", "period_to"],
    ),
    (
        "todos",
        &["title", "note", "due_at", "remind_at", "priority", "done", "done_at", "repeat", "created_at"],
    ),
    ("habits", &["name", "icon", "color", "target", "remind_time", "archived", "days", "goal", "unit", "created_at"]),
    ("channels", &["name", "kind", "config", "enabled", "kinds"]),
    ("feeds", &["name", "url", "color", "enabled"]),
    ("focus_sessions", &["started_at", "minutes", "note"]),
];

fn rows(conn: &Connection, sql: &str, uid: i64) -> rusqlite::Result<Vec<Value>> {
    let mut stmt = conn.prepare(sql)?;
    stmt.query_map([uid], |r| row_to_json(r, &[]))?.collect()
}

/// 导出当前用户的全部数据（JSON）。共享给我的清单不导出；图片不导出。
async fn export(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>) -> Result<Response, ApiErr> {
    let conn = s.db.lock().unwrap();
    let mut out = Map::new();
    out.insert("format".into(), json!("congtodo-export"));
    out.insert("version".into(), json!(1));
    out.insert("app_version".into(), json!(env!("CARGO_PKG_VERSION")));
    out.insert("exported_at".into(), json!(chrono::Utc::now().to_rfc3339()));
    for (table, _) in TABLES {
        // 待办只导出自己创建的；todo_id / list_id / parent_id 原样导出，导入时重新映射
        out.insert((*table).into(), Value::Array(rows(&conn, &format!("SELECT * FROM {table} WHERE user_id = ? ORDER BY id"), uid)?));
    }
    out.insert(
        "habit_logs".into(),
        Value::Array(rows(
            &conn,
            "SELECT l.* FROM habit_logs l JOIN habits h ON h.id = l.habit_id WHERE h.user_id = ? ORDER BY l.day",
            uid,
        )?),
    );
    let settings: Map<String, Value> = conn
        .prepare("SELECT key, value FROM user_settings WHERE user_id = ? AND key != 'ics_token'")?
        .query_map([uid], |r| Ok((r.get::<_, String>(0)?, Value::String(r.get(1)?))))?
        .collect::<rusqlite::Result<_>>()?;
    out.insert("settings".into(), Value::Object(settings));

    let name = format!("congtodo-{}.json", chrono::Utc::now().format("%Y%m%d"));
    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
        ],
        Value::Object(out).to_string(),
    )
        .into_response())
}

fn insert(conn: &Connection, table: &str, cols: &[&str], row: &Value, uid: i64, extra: &[(&str, SqlValue)]) -> rusqlite::Result<i64> {
    let mut names: Vec<&str> = cols.iter().copied().filter(|c| row.get(*c).is_some_and(|v| !v.is_null())).collect();
    let mut params: Vec<SqlValue> = names.iter().map(|c| to_sql(&row[*c])).collect();
    for (c, v) in extra {
        names.push(c);
        params.push(v.clone());
    }
    names.push("user_id");
    params.push(SqlValue::Integer(uid));
    let marks = vec!["?"; names.len()].join(",");
    conn.execute(&format!("INSERT INTO {table} ({}) VALUES ({marks})", names.join(",")), params_from_iter(params))?;
    Ok(conn.last_insert_rowid())
}

/// 导入（追加，不覆盖已有数据）。清单、父子待办、习惯打卡、专注记录之间的引用会重新映射。
async fn import(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>, Json(data): Json<Value>) -> ApiResult {
    if data.get("format").and_then(Value::as_str) != Some("congtodo-export") {
        return Err(bad("不是 Congtodo 导出的文件"));
    }
    let arr = |k: &str| data.get(k).and_then(Value::as_array).cloned().unwrap_or_default();
    let id_of = |v: &Value| v.get("id").and_then(Value::as_i64);

    let mut conn = s.db.lock().unwrap();
    let tx = conn.transaction()?;
    let mut counts = Map::new();
    let mut lists: HashMap<i64, i64> = HashMap::new();
    let mut todos: HashMap<i64, i64> = HashMap::new();
    let mut habits: HashMap<i64, i64> = HashMap::new();

    for (table, cols) in TABLES {
        let items = arr(table);
        match *table {
            "todos" => {
                // 先导入父待办，再导入子任务
                let (parents, children): (Vec<_>, Vec<_>) = items.iter().partition(|t| t["parent_id"].is_null());
                for t in parents.into_iter().chain(children) {
                    let list = t["list_id"].as_i64().and_then(|l| lists.get(&l)).map(|&l| SqlValue::Integer(l));
                    let parent = t["parent_id"].as_i64().and_then(|p| todos.get(&p)).map(|&p| SqlValue::Integer(p));
                    let extra: Vec<(&str, SqlValue)> = [("list_id", list), ("parent_id", parent)]
                        .into_iter()
                        .filter_map(|(c, v)| Some((c, v?)))
                        .collect();
                    let new = insert(&tx, table, cols, t, uid, &extra)?;
                    if let Some(old) = id_of(t) {
                        todos.insert(old, new);
                    }
                }
            }
            "focus_sessions" => {
                for f in &items {
                    let todo = f["todo_id"].as_i64().and_then(|t| todos.get(&t)).map(|&t| ("todo_id", SqlValue::Integer(t)));
                    insert(&tx, table, cols, f, uid, &todo.into_iter().collect::<Vec<_>>())?;
                }
            }
            _ => {
                for item in &items {
                    let new = insert(&tx, table, cols, item, uid, &[])?;
                    if let Some(old) = id_of(item) {
                        match *table {
                            "todo_lists" => lists.insert(old, new),
                            "habits" => habits.insert(old, new),
                            _ => None,
                        };
                    }
                }
            }
        }
        counts.insert((*table).into(), json!(items.len()));
    }

    let mut logs = 0;
    for l in arr("habit_logs") {
        let (Some(h), Some(day)) = (l["habit_id"].as_i64().and_then(|h| habits.get(&h)), l["day"].as_str()) else {
            continue;
        };
        logs += tx.execute(
            "INSERT OR IGNORE INTO habit_logs (habit_id, day, count, note) VALUES (?, ?, ?, ?)",
            (h, day, l["count"].as_i64().unwrap_or(1), l["note"].as_str()),
        )?;
    }
    counts.insert("habit_logs".into(), json!(logs));

    // 设置：只补充本账号还没有的项，不覆盖
    if let Some(settings) = data.get("settings").and_then(Value::as_object) {
        for (k, v) in settings.iter().filter(|(k, _)| k.as_str() != "ics_token") {
            if let Some(v) = v.as_str() {
                tx.execute("INSERT OR IGNORE INTO user_settings (user_id, key, value) VALUES (?, ?, ?)", (uid, k, v))?;
            }
        }
    }
    tx.commit()?;
    Ok(Json(json!({ "imported": counts })))
}

// ---------- 图片 ----------

async fn upload(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult {
    let mime = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
    if !matches!(mime, "image/jpeg" | "image/png" | "image/webp") {
        return Err(bad("只支持 JPEG / PNG / WebP 图片"));
    }
    if body.is_empty() || body.len() > MAX_IMAGE {
        return Err(bad("图片不能超过 2MB"));
    }
    let conn = s.db.lock().unwrap();
    conn.execute("INSERT INTO files (user_id, mime, data) VALUES (?, ?, ?)", (uid, mime, body.as_ref()))?;
    Ok(Json(json!({ "id": conn.last_insert_rowid() })))
}

async fn download(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path(id): Path<i64>,
) -> Result<Response, ApiErr> {
    let conn = s.db.lock().unwrap();
    let (mime, data): (String, Vec<u8>) = conn.query_row(
        "SELECT mime, data FROM files WHERE id = ? AND user_id = ?",
        [id, uid],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    // 文件内容不可变（替换图片会生成新 id），可以长缓存
    Ok(([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "private, max-age=31536000, immutable".into())], data)
        .into_response())
}

async fn delete_file(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path(id): Path<i64>,
) -> ApiResult<StatusCode> {
    let conn = s.db.lock().unwrap();
    conn.execute("DELETE FROM files WHERE id = ? AND user_id = ?", [id, uid])?;
    conn.execute("UPDATE countdowns SET image_id = NULL WHERE image_id = ? AND user_id = ?", [id, uid])?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- 推送记录 ----------

/// 最近 100 条推送（含排队中 / 失败）
async fn outbox(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>) -> ApiResult {
    let conn = s.db.lock().unwrap();
    Ok(Json(Value::Array(rows(
        &conn,
        "SELECT o.id, o.kind, o.title, o.body, o.status, o.attempts, o.next_at, o.error, o.created_at, o.sent_at,
                o.channel_id, c.name AS channel
         FROM outbox o LEFT JOIN channels c ON c.id = o.channel_id
         WHERE o.user_id = ? ORDER BY o.id DESC LIMIT 100",
        uid,
    )?)))
}
