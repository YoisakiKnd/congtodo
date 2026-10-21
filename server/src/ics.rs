//! ICS 日历：
//! - 导出：每个用户一个带密钥的订阅地址 /api/ics/<token>.ics，供手机 / 电脑日历订阅（只读）
//! - 导入：订阅外部 ICS（学校校历、节假日等），后台每 6 小时同步一次，重复规则展开成具体日期

use std::time::Duration;

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{Datelike, Months, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    Shared,
    api::{ApiErr, ApiResult, bad},
    auth::UserId,
    db::{get_setting, row_to_json},
    scheduler::{in_weeks, term_week},
    tz::{self, local},
};

/// 公开路由（凭令牌访问，不需要登录）
pub fn public_router() -> Router<Shared> {
    Router::new().route("/ics/{file}", get(feed))
}

pub fn private_router() -> Router<Shared> {
    Router::new()
        .route("/ics/token", get(get_token).post(new_token).delete(delete_token))
        .route("/feeds/events", get(feed_events))
        .route("/feeds/{id}/sync", post(sync_now))
}

// ---------- 订阅令牌 ----------

async fn get_token(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>) -> ApiResult {
    let conn = s.db.lock().unwrap();
    Ok(Json(json!({ "token": get_setting(&conn, uid, "ics_token") })))
}

/// 生成（或重新生成，旧地址立即失效）订阅令牌
async fn new_token(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>) -> ApiResult {
    let mut buf = [0u8; 16];
    getrandom::fill(&mut buf).map_err(|e| ApiErr(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let token = hex::encode(buf);
    let conn = s.db.lock().unwrap();
    conn.execute(
        "INSERT INTO user_settings (user_id, key, value) VALUES (?, 'ics_token', ?)
         ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value",
        (uid, &token),
    )?;
    Ok(Json(json!({ "token": token })))
}

async fn delete_token(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>) -> ApiResult<StatusCode> {
    let conn = s.db.lock().unwrap();
    conn.execute("DELETE FROM user_settings WHERE user_id = ? AND key = 'ics_token'", [uid])?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- 导出 ----------

/// RFC 5545 文本转义
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace('\n', "\\n").replace('\r', "")
}

/// 每行不超过 75 字节，续行以空格开头（不能切在 UTF-8 字符中间）
fn fold(line: &str) -> String {
    let mut out = String::new();
    let mut len = 0;
    for c in line.chars() {
        if len + c.len_utf8() > 75 {
            out.push_str("\r\n ");
            len = 1;
        }
        out.push(c);
        len += c.len_utf8();
    }
    out.push_str("\r\n");
    out
}

fn utc(ts: i64) -> String {
    Utc.timestamp_opt(ts, 0).single().unwrap_or_default().format("%Y%m%dT%H%M%SZ").to_string()
}

struct Cal(String);

impl Cal {
    fn line(&mut self, l: impl AsRef<str>) {
        self.0.push_str(&fold(l.as_ref()));
    }

    #[allow(clippy::too_many_arguments)]
    fn event(
        &mut self,
        uid: &str,
        title: &str,
        start: &str,
        end: Option<&str>,
        rrule: Option<&str>,
        desc: Option<&str>,
        location: Option<&str>,
        alarm: Option<i64>,
    ) {
        self.line("BEGIN:VEVENT");
        self.line(format!("UID:{uid}@congtodo"));
        self.line(format!("DTSTAMP:{}", utc(Utc::now().timestamp())));
        self.line(start);
        if let Some(e) = end {
            self.line(e);
        }
        if let Some(r) = rrule {
            self.line(format!("RRULE:{r}"));
        }
        self.line(format!("SUMMARY:{}", esc(title)));
        if let Some(d) = desc.filter(|d| !d.is_empty()) {
            self.line(format!("DESCRIPTION:{}", esc(d)));
        }
        if let Some(l) = location.filter(|l| !l.is_empty()) {
            self.line(format!("LOCATION:{}", esc(l)));
        }
        if let Some(m) = alarm {
            self.line("BEGIN:VALARM");
            self.line("ACTION:DISPLAY");
            self.line(format!("DESCRIPTION:{}", esc(title)));
            self.line(format!("TRIGGER:-PT{m}M"));
            self.line("END:VALARM");
        }
        self.line("END:VEVENT");
    }
}

fn date_prop(name: &str, d: NaiveDate) -> String {
    format!("{name};VALUE=DATE:{}", d.format("%Y%m%d"))
}

fn rrule(repeat: &str) -> Option<&'static str> {
    Some(match repeat {
        "daily" => "FREQ=DAILY",
        "weekly" => "FREQ=WEEKLY",
        "monthly" => "FREQ=MONTHLY",
        "yearly" => "FREQ=YEARLY",
        _ => return None,
    })
}

fn build_calendar(conn: &Connection, uid: i64) -> rusqlite::Result<String> {
    let tz = tz::user_tz(conn, uid);
    let mut cal = Cal(String::new());
    cal.line("BEGIN:VCALENDAR");
    cal.line("VERSION:2.0");
    cal.line("PRODID:-//Congtodo//Congtodo//ZH");
    cal.line("CALSCALE:GREGORIAN");
    cal.line("X-WR-CALNAME:Congtodo");
    cal.line(format!("X-WR-TIMEZONE:{}", tz.name()));
    cal.line("REFRESH-INTERVAL;VALUE=DURATION:PT1H");
    cal.line("X-PUBLISHED-TTL:PT1H");

    // 日程
    let mut stmt = conn.prepare(
        "SELECT id, title, start_at, end_at, all_day, repeat, note, remind_before FROM events WHERE user_id = ?",
    )?;
    let rows = stmt.query_map([uid], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, Option<i64>>(3)?,
            r.get::<_, bool>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<i64>>(7)?,
        ))
    })?;
    for r in rows {
        let (id, title, start, end, all_day, repeat, note, remind) = r?;
        let (s, e) = if all_day {
            let sd = local(start, tz).date_naive();
            // DTEND 对全天事件是「不包含」的那天
            let ed = end.map(|e| local(e, tz).date_naive()).unwrap_or(sd).succ_opt().unwrap();
            (date_prop("DTSTART", sd), Some(date_prop("DTEND", ed)))
        } else {
            (format!("DTSTART:{}", utc(start)), Some(format!("DTEND:{}", utc(end.unwrap_or(start + 3600)))))
        };
        cal.event(&format!("event-{id}"), &title, &s, e.as_deref(), rrule(&repeat), note.as_deref(), None, remind);
    }

    // 课程：按学期周次展开成具体日期（最多 30 周）
    if let Some(term) = get_setting(conn, uid, "term_start")
        && let Ok(start) = NaiveDate::parse_from_str(&term, "%Y-%m-%d") {
            let monday = start - chrono::Duration::days(start.weekday().num_days_from_monday() as i64);
            let mut stmt = conn.prepare(
                "SELECT id, name, room, teacher, weekday, start_time, end_time, weeks FROM courses WHERE user_id = ?",
            )?;
            let rows = stmt.query_map([uid], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                ))
            })?;
            for r in rows {
                let (id, name, room, teacher, weekday, st, et, weeks) = r?;
                for w in 1..=30 {
                    let d = monday + chrono::Duration::days((w - 1) * 7 + weekday - 1);
                    if !in_weeks(&weeks, term_week(&term, d).unwrap_or(0)) {
                        continue;
                    }
                    let (Some(s), Some(e)) = (tz::at(d, &st, tz), tz::at(d, &et, tz)) else { continue };
                    cal.event(
                        &format!("course-{id}-{d}"),
                        &name,
                        &format!("DTSTART:{}", utc(s)),
                        Some(&format!("DTEND:{}", utc(e))),
                        None,
                        teacher.as_deref(),
                        room.as_deref(),
                        None,
                    );
                }
            }
        }

    // 倒数日（全天）
    let mut stmt = conn.prepare("SELECT id, title, target_at, repeat FROM countdowns WHERE user_id = ?")?;
    let rows = stmt.query_map([uid], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?))
    })?;
    for r in rows {
        let (id, title, target, repeat) = r?;
        let d = local(target, tz).date_naive();
        let end = date_prop("DTEND", d.succ_opt().unwrap());
        cal.event(&format!("countdown-{id}"), &format!("⏳ {title}"), &date_prop("DTSTART", d), Some(&end), rrule(&repeat), None, None, None);
    }

    // 未完成且有截止时间的待办（全天）
    let mut stmt =
        conn.prepare("SELECT id, title, due_at, note FROM todos WHERE user_id = ? AND done = 0 AND due_at IS NOT NULL")?;
    let rows = stmt.query_map([uid], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, Option<String>>(3)?))
    })?;
    for r in rows {
        let (id, title, due, note) = r?;
        let d = local(due, tz).date_naive();
        let end = date_prop("DTEND", d.succ_opt().unwrap());
        cal.event(&format!("todo-{id}"), &format!("☐ {title}"), &date_prop("DTSTART", d), Some(&end), None, note.as_deref(), None, None);
    }

    cal.line("END:VCALENDAR");
    Ok(cal.0)
}

async fn feed(State(s): State<Shared>, Path(file): Path<String>) -> Result<Response, ApiErr> {
    let token = file.strip_suffix(".ics").unwrap_or(&file);
    let conn = s.db.lock().unwrap();
    let uid: i64 = conn
        .query_row(
            "SELECT s.user_id FROM user_settings s JOIN users u ON u.id = s.user_id
             WHERE s.key = 'ics_token' AND s.value = ? AND u.disabled = 0",
            [token],
            |r| r.get(0),
        )
        .optional()?
        .ok_or(ApiErr(StatusCode::NOT_FOUND, "订阅地址无效".into()))?;
    let body = build_calendar(&conn, uid)?;
    Ok(([(header::CONTENT_TYPE, "text/calendar; charset=utf-8")], body).into_response())
}

// ---------- 导入：解析 ----------

#[derive(Debug, PartialEq)]
pub struct FeedEvent {
    pub uid: String,
    pub title: String,
    pub start: i64,
    pub end: Option<i64>,
    pub all_day: bool,
    pub location: Option<String>,
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(o) => out.push(o),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// DTSTART / DTEND / UNTIL 的取值：全天日期、UTC 时间或某时区的本地时间
#[derive(Clone, Copy)]
enum When {
    Date(NaiveDate),
    Local(NaiveDateTime, Tz),
}

impl When {
    fn parse(value: &str, tzid: Option<&str>, fallback: Tz) -> Option<When> {
        let v = value.trim();
        if v.len() == 8 {
            return NaiveDate::parse_from_str(v, "%Y%m%d").ok().map(When::Date);
        }
        if let Some(u) = v.strip_suffix('Z') {
            let dt = NaiveDateTime::parse_from_str(u, "%Y%m%dT%H%M%S").ok()?;
            return Some(When::Local(dt, Tz::UTC));
        }
        let dt = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
        Some(When::Local(dt, tzid.and_then(tz::parse).unwrap_or(fallback)))
    }

    fn ts(self, user_tz: Tz) -> Option<i64> {
        match self {
            When::Date(d) => Some(tz::day_start(d, user_tz)),
            When::Local(dt, tz) => tz.from_local_datetime(&dt).earliest().map(|d| d.timestamp()),
        }
    }

    fn date(self) -> NaiveDate {
        match self {
            When::Date(d) => d,
            When::Local(dt, _) => dt.date(),
        }
    }

    /// 同一时刻挪到另一天（重复规则展开用）
    fn on(self, d: NaiveDate) -> When {
        match self {
            When::Date(_) => When::Date(d),
            When::Local(dt, tz) => When::Local(NaiveDateTime::new(d, dt.time()), tz),
        }
    }
}

fn weekday_code(code: &str) -> Option<chrono::Weekday> {
    use chrono::Weekday::*;
    Some(match code.trim().trim_start_matches(|c: char| c == '+' || c == '-' || c.is_ascii_digit()) {
        "MO" => Mon,
        "TU" => Tue,
        "WE" => Wed,
        "TH" => Thu,
        "FR" => Fri,
        "SA" => Sat,
        "SU" => Sun,
        _ => return None,
    })
}

/// 展开重复规则：返回 [window_from, window_to] 内每次发生的日期（含首次）。
/// 支持 FREQ=DAILY/WEEKLY/MONTHLY/YEARLY、INTERVAL、COUNT、UNTIL、WEEKLY 的 BYDAY；其他规则只保留首次。
fn expand(start: When, rule: &str, window: (NaiveDate, NaiveDate), user_tz: Tz) -> Vec<NaiveDate> {
    let first = start.date();
    let parts: Vec<(&str, &str)> = rule.split(';').filter_map(|p| p.split_once('=')).collect();
    let get = |k: &str| parts.iter().find(|(key, _)| key.eq_ignore_ascii_case(k)).map(|(_, v)| *v);
    let supported = parts
        .iter()
        .all(|(k, _)| matches!(k.to_ascii_uppercase().as_str(), "FREQ" | "INTERVAL" | "COUNT" | "UNTIL" | "WKST" | "BYDAY"));
    let freq = get("FREQ").unwrap_or("").to_ascii_uppercase();
    let byday: Vec<chrono::Weekday> = get("BYDAY").map(|b| b.split(',').filter_map(weekday_code).collect()).unwrap_or_default();
    if !supported || (!byday.is_empty() && freq != "WEEKLY") {
        return vec![first];
    }
    let interval = get("INTERVAL").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1).max(1);
    let count = get("COUNT").and_then(|v| v.parse::<usize>().ok());
    let until = get("UNTIL")
        .and_then(|u| When::parse(u, None, user_tz))
        .map(|w| match w {
            When::Date(d) => d,
            When::Local(dt, tz) => tz.from_local_datetime(&dt).earliest().map_or(dt.date(), |d| d.with_timezone(&user_tz).date_naive()),
        });

    let mut out = Vec::new();
    let mut n = 0;
    'outer: for k in 0..2000u32 {
        let step = k * interval;
        let candidates: Vec<NaiveDate> = match freq.as_str() {
            "DAILY" => vec![first + chrono::Duration::days(step as i64)],
            "WEEKLY" => {
                let monday = first - chrono::Duration::days(first.weekday().num_days_from_monday() as i64)
                    + chrono::Duration::weeks(step as i64);
                let days = if byday.is_empty() { vec![first.weekday()] } else { byday.clone() };
                let mut v: Vec<NaiveDate> = days
                    .iter()
                    .map(|wd| monday + chrono::Duration::days(wd.num_days_from_monday() as i64))
                    .filter(|d| *d >= first)
                    .collect();
                v.sort();
                v
            }
            "MONTHLY" => first.checked_add_months(Months::new(step)).filter(|d| d.day() == first.day()).into_iter().collect(),
            "YEARLY" => first.checked_add_months(Months::new(step * 12)).filter(|d| d.day() == first.day()).into_iter().collect(),
            _ => return vec![first],
        };
        for d in candidates {
            if until.is_some_and(|u| d > u) || count.is_some_and(|c| n >= c) || d > window.1 {
                break 'outer;
            }
            n += 1;
            if d >= window.0 {
                out.push(d);
            }
        }
    }
    out
}

/// 解析 ICS 文本，重复事件展开到 [今天 - 1 年, 今天 + 2 年]
pub fn parse(text: &str, user_tz: Tz, today: NaiveDate) -> Vec<FeedEvent> {
    // 展开折行
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let raw = raw.trim_end_matches('\r');
        if let Some(cont) = raw.strip_prefix([' ', '\t']) {
            if let Some(last) = lines.last_mut() {
                last.push_str(cont);
            }
        } else {
            lines.push(raw.to_string());
        }
    }
    let window = (today - chrono::Duration::days(366), today + chrono::Duration::days(731));

    let mut out = Vec::new();
    let mut cur: Option<Vec<(String, String, String)>> = None; // (name, params, value)
    for line in lines {
        match line.as_str() {
            "BEGIN:VEVENT" => cur = Some(Vec::new()),
            "END:VEVENT" => {
                let Some(props) = cur.take() else { continue };
                let prop = |n: &str| props.iter().find(|(name, _, _)| name == n);
                // 单次修改的重复实例（RECURRENCE-ID）暂不单独处理
                if prop("RECURRENCE-ID").is_some() {
                    continue;
                }
                let tzid = |params: &str| {
                    params.split(';').find_map(|p| p.strip_prefix("TZID=")).map(|t| t.trim_matches('"').to_string())
                };
                let Some((_, sp, sv)) = prop("DTSTART") else { continue };
                let Some(start) = When::parse(sv, tzid(sp).as_deref(), user_tz) else { continue };
                let end = prop("DTEND").and_then(|(_, p, v)| When::parse(v, tzid(p).as_deref(), user_tz));
                let all_day = matches!(start, When::Date(_));
                let (Some(s0), e0) = (start.ts(user_tz), end.and_then(|e| e.ts(user_tz))) else { continue };
                let duration = e0.map(|e| e - s0);
                let title = prop("SUMMARY").map(|p| unescape(&p.2)).unwrap_or_else(|| "（无标题）".into());
                let uid = prop("UID").map(|p| p.2.clone()).unwrap_or_else(|| format!("{title}-{s0}"));
                let location = prop("LOCATION").map(|p| unescape(&p.2)).filter(|l| !l.is_empty());

                let dates = match prop("RRULE") {
                    Some((_, _, rule)) => expand(start, rule, window, user_tz),
                    None => vec![start.date()],
                };
                for d in dates {
                    let Some(s) = start.on(d).ts(user_tz) else { continue };
                    // 全天事件的 DTEND 是「不包含」的那天，存成包含的最后一天的 0 点
                    let e = duration.map(|du| if all_day { s + du - 86400 } else { s + du }).filter(|e| *e >= s);
                    out.push(FeedEvent { uid: uid.clone(), title: title.clone(), start: s, end: e, all_day, location: location.clone() });
                }
            }
            _ => {
                if let (Some(props), Some((head, value))) = (cur.as_mut(), line.split_once(':')) {
                    let (name, params) = head.split_once(';').unwrap_or((head, ""));
                    props.push((name.to_ascii_uppercase(), params.to_string(), value.to_string()));
                }
            }
        }
    }
    out
}

// ---------- 导入：同步 ----------

async fn fetch(http: &reqwest::Client, url: &str) -> Result<String, String> {
    let url = url.trim().replacen("webcal://", "https://", 1);
    let resp = http.get(&url).timeout(Duration::from_secs(20)).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("日历文件超过 8MB".into());
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if !text.contains("BEGIN:VCALENDAR") {
        return Err("不是有效的 ICS 日历".into());
    }
    Ok(text)
}

pub async fn sync(state: &Shared, feed_id: i64) -> Result<usize, String> {
    let (url, uid): (String, i64) = {
        let conn = state.db.lock().unwrap();
        conn.query_row("SELECT url, user_id FROM feeds WHERE id = ?", [feed_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| e.to_string())?
    };
    let result = fetch(&state.http, &url).await;
    let mut conn = state.db.lock().unwrap();
    let now = Utc::now().timestamp();
    match result {
        Ok(text) => {
            let tz = tz::user_tz(&conn, uid);
            let events = parse(&text, tz, local(now, tz).date_naive());
            let tx = conn.transaction().map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM feed_events WHERE feed_id = ?", [feed_id]).map_err(|e| e.to_string())?;
            for e in &events {
                tx.execute(
                    "INSERT OR REPLACE INTO feed_events (feed_id, uid, title, start_at, end_at, all_day, location)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                    (feed_id, &e.uid, &e.title, e.start, e.end, e.all_day, &e.location),
                )
                .map_err(|e| e.to_string())?;
            }
            tx.execute("UPDATE feeds SET last_sync = ?, last_error = NULL WHERE id = ?", (now, feed_id))
                .map_err(|e| e.to_string())?;
            tx.commit().map_err(|e| e.to_string())?;
            Ok(events.len())
        }
        Err(e) => {
            let _ = conn.execute("UPDATE feeds SET last_sync = ?, last_error = ? WHERE id = ?", (now, &e, feed_id));
            Err(e)
        }
    }
}

/// 后台：每 10 分钟检查一次，超过 6 小时没同步的订阅重新拉取
pub fn spawn(state: Shared) {
    tokio::spawn(async move {
        loop {
            let due: Vec<i64> = {
                let conn = state.db.lock().unwrap();
                conn.prepare(
                    "SELECT f.id FROM feeds f JOIN users u ON u.id = f.user_id
                     WHERE f.enabled = 1 AND u.disabled = 0 AND (f.last_sync IS NULL OR f.last_sync < ?) LIMIT 5",
                )
                .and_then(|mut s| s.query_map([Utc::now().timestamp() - 6 * 3600], |r| r.get(0))?.collect())
                .unwrap_or_default()
            };
            for id in due {
                if let Err(e) = sync(&state, id).await {
                    eprintln!("[feeds] feed {id}: {e}");
                }
            }
            tokio::time::sleep(Duration::from_secs(600)).await;
        }
    });
}

async fn sync_now(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>, Path(id): Path<i64>) -> ApiResult {
    {
        let conn = s.db.lock().unwrap();
        conn.query_row("SELECT 1 FROM feeds WHERE id = ? AND user_id = ?", [id, uid], |_| Ok(()))?;
    }
    let n = sync(&s, id).await.map_err(|e| ApiErr(StatusCode::BAD_GATEWAY, format!("同步失败：{e}")))?;
    Ok(Json(json!({ "events": n })))
}

#[derive(Deserialize)]
struct Range {
    from: i64,
    to: i64,
}

/// 某时间段内，已启用订阅的事件
async fn feed_events(State(s): State<Shared>, Extension(UserId(uid)): Extension<UserId>, Query(r): Query<Range>) -> ApiResult {
    if r.to - r.from > 400 * 86400 {
        return Err(bad("时间范围过大"));
    }
    let conn = s.db.lock().unwrap();
    let mut stmt = conn.prepare_cached(
        "SELECT e.feed_id, e.uid, e.title, e.start_at, e.end_at, e.all_day, e.location, f.color, f.name AS feed
         FROM feed_events e JOIN feeds f ON f.id = e.feed_id
         WHERE f.user_id = ? AND f.enabled = 1 AND e.start_at <= ? AND coalesce(e.end_at, e.start_at) >= ?
         ORDER BY e.start_at LIMIT 2000",
    )?;
    let rows = stmt.query_map((uid, r.to, r.from), |row| row_to_json(row, &[]))?.collect::<rusqlite::Result<Vec<Value>>>()?;
    Ok(Json(Value::Array(rows)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    const SH: Tz = chrono_tz::Asia::Shanghai;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn fold_and_escape() {
        let long = format!("SUMMARY:{}", "中".repeat(40));
        let folded = fold(&long);
        assert!(folded.split("\r\n").all(|l| l.len() <= 75));
        assert_eq!(folded.replace("\r\n ", "").trim_end(), long);
        assert_eq!(esc("a,b;c\nd"), "a\\,b\\;c\\nd");
        assert_eq!(unescape("a\\,b\\;c\\nd"), "a,b;c\nd");
    }

    #[test]
    fn parse_basic_events() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:1\r\nSUMMARY:国庆\r\n 节\r\nDTSTART;VALUE=DATE:20261001\r\nDTEND;VALUE=DATE:20261008\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:2\r\nSUMMARY:会议\\, 重要\r\nDTSTART:20261021T020000Z\r\nDTEND:20261021T030000Z\r\nLOCATION:A101\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:3\r\nSUMMARY:纽约\r\nDTSTART;TZID=America/New_York:20261021T090000\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let ev = parse(ics, SH, d("2026-10-21"));
        assert_eq!(ev.len(), 3);
        assert_eq!(ev[0].title, "国庆节");
        assert!(ev[0].all_day);
        // 全天 10/1-10/7（DTEND 不包含），end 存为 10/7 0 点
        assert_eq!(ev[0].end, Some(tz::day_start(d("2026-10-07"), SH)));
        assert_eq!(ev[1].title, "会议, 重要");
        assert_eq!(local(ev[1].start, SH).format("%H:%M").to_string(), "10:00");
        assert_eq!(ev[1].location.as_deref(), Some("A101"));
        assert_eq!(local(ev[2].start, SH).format("%m-%d %H:%M").to_string(), "10-21 21:00");
    }

    #[test]
    fn expand_rules() {
        let start = When::Local(NaiveDateTime::new(d("2026-09-07"), NaiveTime::from_hms_opt(8, 0, 0).unwrap()), SH);
        let w = (d("2026-01-01"), d("2027-12-31"));
        // 每两周一次，共 4 次
        let v = expand(start, "FREQ=WEEKLY;INTERVAL=2;COUNT=4", w, SH);
        assert_eq!(v, vec![d("2026-09-07"), d("2026-09-21"), d("2026-10-05"), d("2026-10-19")]);
        // 每周一三五直到 9/11（含）
        let v = expand(start, "FREQ=WEEKLY;BYDAY=MO,WE,FR;UNTIL=20260911T235959Z", w, SH);
        assert_eq!(v, vec![d("2026-09-07"), d("2026-09-09"), d("2026-09-11")]);
        // 无限重复只展开到窗口末尾
        assert_eq!(expand(start, "FREQ=YEARLY", w, SH), vec![d("2026-09-07"), d("2027-09-07")]);
        // 不支持的规则只保留首次
        assert_eq!(expand(start, "FREQ=MONTHLY;BYDAY=2MO", w, SH), vec![d("2026-09-07")]);
    }

    #[test]
    fn recurring_feed_event() {
        let ics = "BEGIN:VEVENT\nUID:r\nSUMMARY:周会\nDTSTART;TZID=Asia/Shanghai:20261019T100000\nDTEND;TZID=Asia/Shanghai:20261019T110000\nRRULE:FREQ=WEEKLY;COUNT=3\nEND:VEVENT\n";
        let ev = parse(ics, SH, d("2026-10-21"));
        assert_eq!(ev.len(), 3);
        assert!(ev.iter().all(|e| e.end == Some(e.start + 3600)));
        assert_eq!(local(ev[2].start, SH).format("%m-%d %H:%M").to_string(), "11-02 10:00");
    }
}
