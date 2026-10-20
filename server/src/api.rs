use std::collections::HashMap;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use rusqlite::{params_from_iter, types::Value as SqlValue};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{Shared, db::row_to_json, notify};

/// 通用 CRUD 资源描述：新增一个模块 = 加一张表 + 在这里加一行
struct Res {
    name: &'static str,
    cols: &'static [&'static str], // 允许客户端写入的列（白名单）
    json_cols: &'static [&'static str],
    order: &'static str,
}

const RESOURCES: &[Res] = &[
    Res {
        name: "countdowns",
        cols: &["title", "target_at", "color", "pinned", "remind_before"],
        json_cols: &[],
        order: "pinned DESC, target_at",
    },
    Res {
        name: "events",
        cols: &["title", "start_at", "end_at", "all_day", "repeat", "color", "note", "remind_before"],
        json_cols: &[],
        order: "start_at",
    },
    Res {
        name: "courses",
        cols: &["name", "teacher", "room", "weekday", "start_time", "end_time", "weeks", "color", "remind_before"],
        json_cols: &[],
        order: "weekday, start_time",
    },
    Res {
        name: "todos",
        cols: &["title", "note", "due_at", "remind_at", "priority", "done", "done_at"],
        json_cols: &[],
        order: "done, priority DESC, due_at IS NULL, due_at, id DESC",
    },
    Res {
        name: "habits",
        cols: &["name", "icon", "color", "target", "remind_time", "archived"],
        json_cols: &[],
        order: "archived, id",
    },
    Res {
        name: "channels",
        cols: &["name", "kind", "config", "enabled"],
        json_cols: &["config"],
        order: "id",
    },
];

pub fn router() -> Router<Shared> {
    Router::new()
        .route("/settings", get(get_settings).put(put_settings))
        .route("/habit_logs", get(habit_logs))
        .route("/habits/{id}/toggle", post(habit_toggle))
        .route("/channels/{id}/test", post(channel_test))
        .route("/{res}", get(list).post(create))
        .route("/{res}/{id}", patch(update).delete(remove))
}

// ---------- 错误处理 ----------

pub struct ApiErr(StatusCode, String);

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

type ApiResult<T = Json<Value>> = Result<T, ApiErr>;

fn find(name: &str) -> Result<&'static Res, ApiErr> {
    RESOURCES
        .iter()
        .find(|r| r.name == name)
        .ok_or(ApiErr(StatusCode::NOT_FOUND, format!("unknown resource {name}")))
}

fn to_sql(v: &Value) -> SqlValue {
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

fn fetch_one(conn: &rusqlite::Connection, res: &Res, id: i64) -> ApiResult {
    let sql = format!("SELECT * FROM {} WHERE id = ?", res.name);
    Ok(Json(conn.query_row(&sql, [id], |r| row_to_json(r, res.json_cols))?))
}

// ---------- 通用 CRUD ----------

async fn list(State(s): State<Shared>, Path(name): Path<String>) -> ApiResult {
    let res = find(&name)?;
    let conn = s.db.lock().unwrap();
    let mut stmt = conn.prepare_cached(&format!("SELECT * FROM {} ORDER BY {}", res.name, res.order))?;
    let rows = stmt
        .query_map([], |r| row_to_json(r, res.json_cols))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Json(Value::Array(rows)))
}

async fn create(State(s): State<Shared>, Path(name): Path<String>, Json(body): Json<Map<String, Value>>) -> ApiResult {
    let res = find(&name)?;
    let fields = pick(res, &body);
    if fields.is_empty() {
        return Err(ApiErr(StatusCode::BAD_REQUEST, "empty body".into()));
    }
    let cols = fields.iter().map(|(c, _)| *c).collect::<Vec<_>>().join(",");
    let marks = vec!["?"; fields.len()].join(",");
    let conn = s.db.lock().unwrap();
    conn.execute(
        &format!("INSERT INTO {} ({cols}) VALUES ({marks})", res.name),
        params_from_iter(fields.iter().map(|(_, v)| to_sql(v))),
    )?;
    fetch_one(&conn, res, conn.last_insert_rowid())
}

async fn update(
    State(s): State<Shared>,
    Path((name, id)): Path<(String, i64)>,
    Json(body): Json<Map<String, Value>>,
) -> ApiResult {
    let res = find(&name)?;
    let fields = pick(res, &body);
    let conn = s.db.lock().unwrap();
    if !fields.is_empty() {
        let set = fields.iter().map(|(c, _)| format!("{c} = ?")).collect::<Vec<_>>().join(",");
        let params = fields.iter().map(|(_, v)| to_sql(v)).chain([SqlValue::Integer(id)]);
        conn.execute(&format!("UPDATE {} SET {set} WHERE id = ?", res.name), params_from_iter(params))?;
    }
    fetch_one(&conn, res, id)
}

async fn remove(State(s): State<Shared>, Path((name, id)): Path<(String, i64)>) -> ApiResult<StatusCode> {
    let res = find(&name)?;
    let conn = s.db.lock().unwrap();
    conn.execute(&format!("DELETE FROM {} WHERE id = ?", res.name), [id])?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- 设置 ----------

async fn get_settings(State(s): State<Shared>) -> ApiResult<Json<HashMap<String, String>>> {
    let conn = s.db.lock().unwrap();
    let mut stmt = conn.prepare_cached("SELECT key, value FROM settings")?;
    let map = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Json(map))
}

async fn put_settings(
    State(s): State<Shared>,
    Json(body): Json<HashMap<String, String>>,
) -> ApiResult<StatusCode> {
    let mut conn = s.db.lock().unwrap();
    let tx = conn.transaction()?;
    for (k, v) in &body {
        tx.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [k, v],
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

async fn habit_logs(State(s): State<Shared>, Query(q): Query<Range>) -> ApiResult {
    let conn = s.db.lock().unwrap();
    let mut stmt = conn.prepare_cached("SELECT habit_id, day FROM habit_logs WHERE day >= ? ORDER BY day")?;
    let rows = stmt
        .query_map([q.from.unwrap_or_default()], |r| row_to_json(r, &[]))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Json(Value::Array(rows)))
}

#[derive(Deserialize)]
struct Toggle {
    day: String,
}

/// 切换某天的打卡状态，返回切换后的状态
async fn habit_toggle(State(s): State<Shared>, Path(id): Path<i64>, Json(t): Json<Toggle>) -> ApiResult {
    let conn = s.db.lock().unwrap();
    let removed = conn.execute("DELETE FROM habit_logs WHERE habit_id = ? AND day = ?", (id, &t.day))?;
    if removed == 0 {
        conn.execute("INSERT INTO habit_logs (habit_id, day) VALUES (?, ?)", (id, &t.day))?;
    }
    Ok(Json(json!({ "done": removed == 0 })))
}

// ---------- 通知渠道测试 ----------

async fn channel_test(State(s): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    let (kind, config): (String, String) = {
        let conn = s.db.lock().unwrap();
        conn.query_row("SELECT kind, config FROM channels WHERE id = ?", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
    };
    let cfg: Value = serde_json::from_str(&config).unwrap_or_default();
    let msg = notify::Msg {
        title: "Congtodo 测试通知".into(),
        body: "如果你看到这条消息，说明通知渠道配置成功 🎉".into(),
        kind: "test",
    };
    notify::send(&s.http, &kind, &cfg, &msg)
        .await
        .map_err(|e| ApiErr(StatusCode::BAD_GATEWAY, e))?;
    Ok(Json(json!({ "ok": true })))
}
