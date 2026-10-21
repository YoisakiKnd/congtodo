//! 中国法定节假日与调休，数据来自 NateScarlet/holiday-cn（根据国务院通知自动更新）。
//! 按年缓存到 holidays 表，7 天刷新一次；拉取失败时继续使用缓存。
//! 可用 CONGTODO_HOLIDAY_URL 指定镜像，`{year}` 会被替换成年份。

use std::time::Duration;

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::{Datelike, Utc};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{Shared, api::ApiResult, db::get_server_setting};

const DEFAULT_URL: &str = "https://cdn.jsdelivr.net/gh/NateScarlet/holiday-cn@master/{year}.json";
const REFRESH: i64 = 7 * 86400;
const RETRY: i64 = 3600;

pub fn router() -> Router<Shared> {
    Router::new().route("/holidays", get(list))
}

#[derive(Deserialize)]
struct Day {
    name: String,
    date: String,
    #[serde(rename = "isOffDay")]
    is_off_day: bool,
}

#[derive(Deserialize)]
struct YearData {
    days: Vec<Day>,
}

/// 拉取某年的数据并写入缓存；最近刷新过（或刚失败过）就跳过
pub async fn refresh(state: &Shared, year: i32) {
    let now = Utc::now().timestamp();
    let key = format!("holidays_{year}");
    let last: i64 = {
        let conn = state.db.lock().unwrap();
        get_server_setting(&conn, &key).and_then(|v| v.parse().ok()).unwrap_or(0)
    };
    // 正数 = 上次成功时间，负数 = 上次失败时间
    if (last > 0 && now - last < REFRESH) || (last < 0 && now + last < RETRY) {
        return;
    }
    let url = std::env::var("CONGTODO_HOLIDAY_URL").unwrap_or_else(|_| DEFAULT_URL.into()).replace("{year}", &year.to_string());
    let result = async {
        let resp = state.http.get(&url).timeout(Duration::from_secs(15)).send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        resp.json::<YearData>().await.map_err(|e| e.to_string())
    }
    .await;

    let mut conn = state.db.lock().unwrap();
    let stamp = match result {
        Ok(data) => {
            let mut save = || -> rusqlite::Result<()> {
                let tx = conn.transaction()?;
                tx.execute("DELETE FROM holidays WHERE date LIKE ?", [format!("{year}-%")])?;
                for d in &data.days {
                    tx.execute(
                        "INSERT OR REPLACE INTO holidays (date, name, off) VALUES (?, ?, ?)",
                        (&d.date, &d.name, d.is_off_day),
                    )?;
                }
                tx.commit()
            };
            match save() {
                Ok(()) => now,
                Err(e) => {
                    eprintln!("[holidays] {year}: {e}");
                    -now
                }
            }
        }
        Err(e) => {
            eprintln!("[holidays] {year}: {e}");
            -now
        }
    };
    let _ = conn.execute(
        "INSERT INTO server_settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (&key, stamp.to_string()),
    );
}

/// 后台：启动时和之后每天刷新今年与明年的数据（明年的通常在 11 月发布）
pub fn spawn(state: Shared) {
    tokio::spawn(async move {
        loop {
            let y = Utc::now().year();
            refresh(&state, y).await;
            refresh(&state, y + 1).await;
            tokio::time::sleep(Duration::from_secs(86400)).await;
        }
    });
}

#[derive(Deserialize)]
struct Q {
    year: i32,
}

async fn list(State(s): State<Shared>, Query(q): Query<Q>) -> ApiResult {
    if !(2000..=2100).contains(&q.year) {
        return Ok(Json(json!([])));
    }
    refresh(&s, q.year).await;
    let conn = s.db.lock().unwrap();
    let mut stmt = conn.prepare_cached("SELECT date, name, off FROM holidays WHERE date LIKE ? ORDER BY date")?;
    let rows = stmt
        .query_map([format!("{}-%", q.year)], |r| {
            Ok(json!({ "date": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "off": r.get::<_, bool>(2)? }))
        })?
        .collect::<rusqlite::Result<Vec<Value>>>()?;
    Ok(Json(Value::Array(rows)))
}
