use std::collections::HashMap;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use rusqlite::{Connection, OptionalExtension, params_from_iter, types::Value as SqlValue};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{Shared, auth::UserId, db::row_to_json, notify, tz};

/// 数据可见范围
#[derive(Clone, Copy)]
enum Access {
    /// 只能访问自己创建的
    Owner,
    /// 待办：自己的 + 自己创建或加入的共享清单里的
    Todo,
}

/// 通用 CRUD 资源描述：新增一个模块 = 加一张表 + 在这里加一行
struct Res {
    name: &'static str,
    cols: &'static [&'static str], // 允许客户端写入的列（白名单）
    json_cols: &'static [&'static str],
    order: &'static str,
    access: Access,
}

const RESOURCES: &[Res] = &[
    Res {
        name: "countdowns",
        cols: &["title", "target_at", "color", "pinned", "remind_before", "repeat", "image_id", "channels"],
        json_cols: &["channels"],
        order: "pinned DESC, target_at",
        access: Access::Owner,
    },
    Res {
        name: "events",
        cols: &["title", "start_at", "end_at", "all_day", "repeat", "color", "note", "remind_before", "channels"],
        json_cols: &["channels"],
        order: "start_at",
        access: Access::Owner,
    },
    Res {
        name: "courses",
        cols: &[
            "name",
            "teacher",
            "room",
            "weekday",
            "start_time",
            "end_time",
            "weeks",
            "color",
            "remind_before",
            "period_from",
            "period_to",
            "channels",
        ],
        json_cols: &["channels"],
        order: "weekday, start_time",
        access: Access::Owner,
    },
    Res {
        name: "todos",
        cols: &[
            "title",
            "note",
            "due_at",
            "remind_at",
            "priority",
            "done",
            "done_at",
            "list_id",
            "parent_id",
            "repeat",
            "channels",
        ],
        json_cols: &["channels"],
        order: "done, priority DESC, due_at IS NULL, due_at, id DESC",
        access: Access::Todo,
    },
    Res {
        name: "habits",
        cols: &["name", "icon", "color", "target", "remind_time", "archived", "days", "goal", "unit", "channels"],
        json_cols: &["channels"],
        order: "archived, id",
        access: Access::Owner,
    },
    Res {
        name: "channels",
        cols: &["name", "kind", "config", "enabled", "kinds"],
        json_cols: &["config", "kinds"],
        order: "id",
        access: Access::Owner,
    },
    Res {
        name: "feeds",
        cols: &["name", "url", "color", "enabled"],
        json_cols: &[],
        order: "id",
        access: Access::Owner,
    },
    Res {
        name: "focus_sessions",
        cols: &["todo_id", "started_at", "minutes", "note"],
        json_cols: &[],
        order: "started_at DESC",
        access: Access::Owner,
    },
];

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/settings", get(get_settings).put(put_settings))
        .route("/habit_logs", get(habit_logs))
        .route("/habits/{id}/log", post(habit_log))
        .route("/habits/{id}/toggle", post(habit_toggle))
        .route("/channels/{id}/test", post(channel_test))
        .route("/push/key", get(push_key))
        .route("/{res}", get(list).post(create))
        .route("/{res}/{id}", patch(update).delete(remove))
}

// ---------- 错误处理 ----------

pub struct ApiErr(pub StatusCode, pub String);

impl IntoResponse for ApiErr {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

impl From<rusqlite::Error> for ApiErr {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::QueryReturnedNoRows => ApiErr(StatusCode::NOT_FOUND, "not found".into()),
            e => ApiErr(StatusCode::BAD_REQUEST, e.to_string()),
        }
    }
}

pub type ApiResult<T = Json<Value>> = Result<T, ApiErr>;

pub fn bad(msg: impl Into<String>) -> ApiErr {
    ApiErr(StatusCode::BAD_REQUEST, msg.into())
}

fn find(name: &str) -> Result<&'static Res, ApiErr> {
    RESOURCES
        .iter()
        .find(|r| r.name == name)
        .ok_or(ApiErr(StatusCode::NOT_FOUND, format!("unknown resource {name}")))
}

pub fn to_sql(v: &Value) -> SqlValue {
    match v {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(*b as i64),
        Value::Number(n) => n
            .as_i64()
            .map(SqlValue::Integer)
            .unwrap_or_else(|| SqlValue::Real(n.as_f64().unwrap_or_default())),
        Value::String(s) => SqlValue::Text(s.clone()),
        other => SqlValue::Text(other.to_string()),
    }
}

/// 过滤出白名单内的字段
fn pick<'a>(res: &Res, body: &'a Map<String, Value>) -> Vec<(&'static str, &'a Value)> {
    res.cols
        .iter()
        .filter_map(|c| body.get(*c).map(|v| (*c, v)))
        .collect()
}

/// 当前用户可访问的行（uid 是整数，直接拼进 SQL 是安全的）
fn scope(res: &Res, uid: i64) -> String {
    match res.access {
        Access::Owner => format!("user_id = {uid}"),
        Access::Todo => format!(
            "(user_id = {uid} OR list_id IN (SELECT id FROM todo_lists WHERE user_id = {uid})
              OR list_id IN (SELECT list_id FROM list_members WHERE user_id = {uid}))"
        ),
    }
}

/// 是否能访问某个清单（创建者或成员）
pub fn can_use_list(conn: &Connection, uid: i64, list_id: i64) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM todo_lists WHERE id = ?1 AND user_id = ?2)
             OR EXISTS(SELECT 1 FROM list_members WHERE list_id = ?1 AND user_id = ?2)",
        [list_id, uid],
        |r| r.get(0),
    )
}

/// 写入前校验：引用的清单 / 父待办 / 渠道必须是自己能访问的
fn validate(conn: &Connection, res: &Res, uid: i64, body: &Map<String, Value>) -> Result<(), ApiErr> {
    if res.name == "todos" {
        if let Some(list_id) = body.get("list_id").and_then(Value::as_i64)
            && !can_use_list(conn, uid, list_id)? {
                return Err(bad("无权使用该清单"));
            }
        if let Some(parent) = body.get("parent_id").and_then(Value::as_i64) {
            let sql = format!("SELECT parent_id FROM todos WHERE id = ? AND {}", scope(res, uid));
            let grand: Option<i64> =
                conn.query_row(&sql, [parent], |r| r.get(0)).optional()?.ok_or_else(|| bad("父待办不存在"))?;
            if grand.is_some() {
                return Err(bad("子任务只支持一层"));
            }
        }
    }
    if res.name == "channels"
        && let Some(kind) = body.get("kind").and_then(Value::as_str)
            && !matches!(kind, "webhook" | "ntfy" | "bark" | "webpush") {
                return Err(bad("不支持的渠道类型"));
            }
    if let Some(Value::Array(ids)) = body.get("channels") {
        for id in ids {
            let id = id.as_i64().ok_or_else(|| bad("channels 应为渠道 id 数组"))?;
            conn.query_row("SELECT 1 FROM channels WHERE id = ? AND user_id = ?", [id, uid], |_| Ok(()))
                .map_err(|_| bad("渠道不存在"))?;
        }
    }
    Ok(())
}

fn fetch_one(conn: &Connection, res: &Res, uid: i64, id: i64) -> ApiResult {
    let sql = format!("SELECT * FROM {} WHERE id = ? AND {}", res.name, scope(res, uid));
    Ok(Json(conn.query_row(&sql, [id], |r| row_to_json(r, res.json_cols))?))
}

// ---------- 通用 CRUD ----------

async fn list(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>, Path(name): Path<String>) -> ApiResult {
    let res = find(&name)?;
    let conn = s.db.lock().unwrap();
    let mut stmt =
        conn.prepare(&format!("SELECT * FROM {} WHERE {} ORDER BY {}", res.name, scope(res, uid), res.order))?;
    let rows = stmt
        .query_map([], |r| row_to_json(r, res.json_cols))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Json(Value::Array(rows)))
}

async fn create(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path(name): Path<String>,
    Json(body): Json<Map<String, Value>>,
) -> ApiResult {
    let res = find(&name)?;
    let fields = pick(res, &body);
    if fields.is_empty() {
        return Err(bad("empty body"));
    }
    let conn = s.db.lock().unwrap();
    validate(&conn, res, uid, &body)?;
    let cols = fields.iter().map(|(c, _)| *c).collect::<Vec<_>>().join(",");
    let marks = vec!["?"; fields.len()].join(",");
    let params = fields.iter().map(|(_, v)| to_sql(v)).chain([SqlValue::Integer(uid)]);
    conn.execute(
        &format!("INSERT INTO {} ({cols}, user_id) VALUES ({marks}, ?)", res.name),
        params_from_iter(params),
    )?;
    fetch_one(&conn, res, uid, conn.last_insert_rowid())
}

async fn update(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path((name, id)): Path<(String, i64)>,
    Json(body): Json<Map<String, Value>>,
) -> ApiResult {
    let res = find(&name)?;
    let fields = pick(res, &body);
    let conn = s.db.lock().unwrap();
    validate(&conn, res, uid, &body)?;
    if !fields.is_empty() {
        let set = fields.iter().map(|(c, _)| format!("{c} = ?")).collect::<Vec<_>>().join(",");
        let params = fields.iter().map(|(_, v)| to_sql(v)).chain([SqlValue::Integer(id)]);
        let n = conn.execute(
            &format!("UPDATE {} SET {set} WHERE id = ? AND {}", res.name, scope(res, uid)),
            params_from_iter(params),
        )?;
        if n == 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows.into());
        }
    }
    if res.name == "todos" && body.get("done").and_then(Value::as_i64) == Some(1)
        && let Some(row) = advance_repeat(&conn, uid, id)? {
            return Ok(Json(row));
        }
    fetch_one(&conn, res, uid, id)
}

/// 重复待办被勾选完成后：推进到下一次（截止 / 提醒时间同步平移），子任务重置；返回 `_next: true`
fn advance_repeat(conn: &Connection, uid: i64, id: i64) -> Result<Option<Value>, ApiErr> {
    let (repeat, due, remind, owner): (String, Option<i64>, Option<i64>, i64) =
        conn.query_row("SELECT repeat, due_at, remind_at, user_id FROM todos WHERE id = ?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?;
    let Some(due) = due.filter(|_| repeat != "none") else { return Ok(None) };
    let tz = tz::user_tz(conn, owner);
    let Some(next) = tz::next_occurrence(due, &repeat, chrono::Utc::now().timestamp(), tz) else { return Ok(None) };
    conn.execute(
        "UPDATE todos SET done = 0, done_at = NULL, due_at = ?, remind_at = ? WHERE id = ?",
        (next, remind.map(|r| r + (next - due)), id),
    )?;
    conn.execute("UPDATE todos SET done = 0, done_at = NULL WHERE parent_id = ?", [id])?;
    let mut row = fetch_one(conn, find("todos")?, uid, id)?.0;
    row["_next"] = json!(true);
    Ok(Some(row))
}

async fn remove(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path((name, id)): Path<(String, i64)>,
) -> ApiResult<StatusCode> {
    let res = find(&name)?;
    let conn = s.db.lock().unwrap();
    conn.execute(&format!("DELETE FROM {} WHERE id = ? AND {}", res.name, scope(res, uid)), [id])?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- 设置 ----------

async fn get_settings(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
) -> ApiResult<Json<HashMap<String, String>>> {
    let conn = s.db.lock().unwrap();
    let mut stmt = conn.prepare_cached("SELECT key, value FROM user_settings WHERE user_id = ?")?;
    let mut map: HashMap<String, String> = stmt
        .query_map([uid], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    // ICS 订阅令牌只通过专门的接口读取
    map.remove("ics_token");
    Ok(Json(map))
}

/// 前端不能直接写的设置项
const PROTECTED_SETTINGS: &[&str] = &["ics_token"];

async fn put_settings(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Json(body): Json<HashMap<String, String>>,
) -> ApiResult<StatusCode> {
    if let Some(tz) = body.get("timezone").filter(|v| !v.is_empty())
        && tz::parse(tz).is_none() {
            return Err(bad(format!("未知时区：{tz}")));
        }
    let mut conn = s.db.lock().unwrap();
    let tx = conn.transaction()?;
    for (k, v) in body.iter().filter(|(k, _)| !PROTECTED_SETTINGS.contains(&k.as_str())) {
        tx.execute(
            "INSERT INTO user_settings (user_id, key, value) VALUES (?1, ?2, ?3)
             ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value",
            (uid, k, v),
        )?;
    }
    tx.commit()?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- 习惯打卡 ----------

#[derive(Deserialize)]
struct Range {
    from: Option<String>,
}

async fn habit_logs(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Query(q): Query<Range>,
) -> ApiResult {
    let conn = s.db.lock().unwrap();
    let mut stmt = conn.prepare_cached(
        "SELECT l.habit_id, l.day, l.count, l.note FROM habit_logs l JOIN habits h ON h.id = l.habit_id
         WHERE h.user_id = ? AND l.day >= ? ORDER BY l.day",
    )?;
    let rows = stmt
        .query_map((uid, q.from.unwrap_or_default()), |r| row_to_json(r, &[]))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Json(Value::Array(rows)))
}

fn own_habit(conn: &Connection, uid: i64, id: i64) -> rusqlite::Result<()> {
    conn.query_row("SELECT 1 FROM habits WHERE id = ? AND user_id = ?", [id, uid], |_| Ok(()))
}

#[derive(Deserialize)]
struct Toggle {
    day: String,
}

/// 切换某天的打卡状态，返回切换后的状态
async fn habit_toggle(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path(id): Path<i64>,
    Json(t): Json<Toggle>,
) -> ApiResult {
    let conn = s.db.lock().unwrap();
    own_habit(&conn, uid, id)?;
    let removed = conn.execute("DELETE FROM habit_logs WHERE habit_id = ? AND day = ?", (id, &t.day))?;
    if removed == 0 {
        conn.execute("INSERT INTO habit_logs (habit_id, day) VALUES (?, ?)", (id, &t.day))?;
    }
    Ok(Json(json!({ "done": removed == 0 })))
}

#[derive(Deserialize)]
struct LogBody {
    day: String,
    count: i64,
    note: Option<String>,
}

/// 设置某天的完成次数和备注；次数为 0 且没有备注时删除记录
async fn habit_log(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path(id): Path<i64>,
    Json(b): Json<LogBody>,
) -> ApiResult {
    if chrono::NaiveDate::parse_from_str(&b.day, "%Y-%m-%d").is_err() {
        return Err(bad("day 格式应为 YYYY-MM-DD"));
    }
    let note = b.note.filter(|n| !n.trim().is_empty());
    let count = b.count.clamp(0, 999);
    let conn = s.db.lock().unwrap();
    own_habit(&conn, uid, id)?;
    if count == 0 && note.is_none() {
        conn.execute("DELETE FROM habit_logs WHERE habit_id = ? AND day = ?", (id, &b.day))?;
    } else {
        conn.execute(
            "INSERT INTO habit_logs (habit_id, day, count, note) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(habit_id, day) DO UPDATE SET count = excluded.count, note = excluded.note",
            (id, &b.day, count, &note),
        )?;
    }
    Ok(Json(json!({ "habit_id": id, "day": b.day, "count": count, "note": note })))
}

// ---------- 通知渠道 ----------

async fn channel_test(
    State(s): State<Shared>,
    Extension(UserId(uid)): Extension<UserId>,
    Path(id): Path<i64>,
) -> ApiResult {
    let (kind, config): (String, String) = {
        let conn = s.db.lock().unwrap();
        conn.query_row("SELECT kind, config FROM channels WHERE id = ? AND user_id = ?", [id, uid], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
    };
    let cfg: Value = serde_json::from_str(&config).unwrap_or_default();
    let msg = notify::Msg {
        title: "Congtodo 测试通知".into(),
        body: "如果你看到这条消息，说明通知渠道配置成功 🎉".into(),
        kind: "test".into(),
    };
    notify::send(&s.http, &s.vapid, &kind, &cfg, &msg)
        .await
        .map_err(|e| ApiErr(StatusCode::BAD_GATEWAY, e.message().to_string()))?;
    Ok(Json(json!({ "ok": true })))
}

/// 浏览器订阅推送时需要的 VAPID 公钥
async fn push_key(State(s): State<Shared>) -> Json<Value> {
    Json(json!({ "key": s.vapid.public_key() }))
}
