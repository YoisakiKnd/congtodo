//! 提醒调度，每分钟一轮：
//! 1. collect  计算 (last, now] 窗口内到期的提醒（按各用户时区），sent_log 去重
//! 2. enqueue  按条目 / 渠道类型路由到渠道，免打扰时段内的推迟到结束时刻，写入 outbox
//! 3. dispatch 发送 outbox 中到点的消息，失败按 1 / 5 / 30 分钟退避重试，最多 3 次

use std::{collections::HashMap, time::Duration};

use chrono::{Datelike, NaiveDate, Timelike};
use chrono_tz::Tz;
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;

use crate::{
    Shared, notify,
    tz::{self, at, dates_in, day_start, local},
};

pub fn spawn(state: Shared) {
    tokio::spawn(async move {
        let mut last = chrono::Utc::now().timestamp() - 60;
        loop {
            let now = chrono::Utc::now().timestamp();
            tokio::time::sleep(Duration::from_secs((60 - now % 60 + 1) as u64)).await;
            let now = chrono::Utc::now().timestamp();
            {
                let conn = state.db.lock().unwrap();
                if let Err(e) = collect(&conn, last, now).and_then(|due| enqueue(&conn, due, now)) {
                    eprintln!("[scheduler] {e}");
                }
            }
            last = now;
            dispatch(&state, now).await;
        }
    });
}

// ---------- 工具 ----------

fn humanize(mins: i64) -> String {
    match mins {
        m if m >= 1440 && m % 1440 == 0 => format!("{} 天", m / 1440),
        m if m >= 60 && m % 60 == 0 => format!("{} 小时", m / 60),
        m => format!("{m} 分钟"),
    }
}

/// "1-8,10,12-16" 是否包含第 week 周
pub fn in_weeks(spec: &str, week: i64) -> bool {
    spec.split([',', '，']).any(|part| {
        let mut it = part.trim().splitn(2, '-').map(|n| n.trim().parse::<i64>().ok());
        match (it.next().flatten(), it.next()) {
            (Some(a), None) => a == week,
            (Some(a), Some(Some(b))) => (a..=b).contains(&week),
            _ => false,
        }
    })
}

/// 学期第几周（term_start 所在周的周一算第 1 周的开始）
pub fn term_week(term_start: &str, day: NaiveDate) -> Option<i64> {
    let start = NaiveDate::parse_from_str(term_start, "%Y-%m-%d").ok()?;
    let monday = start - chrono::Duration::days(start.weekday().num_days_from_monday() as i64);
    Some((day - monday).num_days().div_euclid(7) + 1)
}

pub fn repeat_hits(rule: &str, origin: NaiveDate, d: NaiveDate) -> bool {
    d >= origin
        && match rule {
            "daily" => true,
            "weekly" => d.weekday() == origin.weekday(),
            "monthly" => d.day() == origin.day(),
            "yearly" => d.month() == origin.month() && d.day() == origin.day(),
            _ => false,
        }
}

/// habits.days："1,3,5" → 是否包含该日（NULL / 空 = 每天）
fn habit_day(days: Option<&str>, d: NaiveDate) -> bool {
    match days.filter(|s| !s.trim().is_empty()) {
        None => true,
        Some(s) => s.split(',').any(|x| x.trim().parse() == Ok(d.weekday().number_from_monday())),
    }
}

fn parse_channels(v: Option<String>) -> Option<Vec<i64>> {
    serde_json::from_str::<Vec<i64>>(&v?).ok().filter(|v| !v.is_empty())
}

// ---------- 用户上下文 ----------

/// 每轮加载一次的用户设置
struct Ctx {
    tz: Tz,
    settings: HashMap<String, String>,
}

impl Ctx {
    fn get(&self, key: &str) -> Option<&str> {
        self.settings.get(key).map(String::as_str).filter(|v| !v.is_empty())
    }
}

/// 所有启用中的用户
fn load_users(conn: &Connection) -> rusqlite::Result<HashMap<i64, Ctx>> {
    let tzs = tz::all_user_tz(conn)?;
    let mut users: HashMap<i64, Ctx> = conn
        .prepare_cached("SELECT id FROM users WHERE disabled = 0")?
        .query_map([], |r| r.get::<_, i64>(0))?
        .flatten()
        .map(|id| (id, Ctx { tz: tzs.get(&id).copied().unwrap_or_else(tz::default_tz), settings: HashMap::new() }))
        .collect();
    let mut stmt = conn.prepare_cached("SELECT user_id, key, value FROM user_settings")?;
    for r in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))? {
        let (uid, k, v) = r?;
        if let Some(u) = users.get_mut(&uid) {
            u.settings.insert(k, v);
        }
    }
    Ok(users)
}

fn holidays(conn: &Connection) -> rusqlite::Result<HashMap<String, (String, bool)>> {
    let mut stmt = conn.prepare_cached("SELECT date, name, off FROM holidays")?;
    stmt.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?.collect()
}

// ---------- 1. 收集到期提醒 ----------

pub struct Due {
    key: String,
    user_id: i64,
    kind: &'static str,
    title: String,
    body: String,
    /// 条目指定的渠道；None 表示按渠道的类型设置
    channels: Option<Vec<i64>>,
}

fn collect(conn: &Connection, from: i64, to: i64) -> rusqlite::Result<Vec<Due>> {
    let users = load_users(conn)?;
    let holidays = holidays(conn)?;
    let mut due: Vec<Due> = Vec::new();
    let inwin = |t: i64| t > from && t <= to;
    let fmt = |ts: i64, tz: Tz| local(ts, tz).format("%m-%d %H:%M").to_string();

    // 待办
    let mut stmt = conn.prepare_cached(
        "SELECT id, title, due_at, remind_at, user_id, channels FROM todos
         WHERE done = 0 AND remind_at > ?1 AND remind_at <= ?2",
    )?;
    for r in stmt.query_map((from, to), |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })? {
        let (id, title, due_at, remind_at, user_id, channels) = r?;
        let Some(u) = users.get(&user_id) else { continue };
        let body = match due_at {
            Some(d) => format!("{title}\n截止：{}", fmt(d, u.tz)),
            None => title,
        };
        due.push(Due {
            key: format!("todo:{id}:{remind_at}"),
            user_id,
            kind: "todo",
            title: "📝 待办提醒".into(),
            body,
            channels: parse_channels(channels),
        });
    }

    // 倒数日（含每年重复，如生日）
    let mut stmt = conn.prepare_cached(
        "SELECT id, title, target_at, remind_before, user_id, repeat, channels FROM countdowns
         WHERE remind_before IS NOT NULL",
    )?;
    for r in stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, Option<String>>(6)?,
        ))
    })? {
        let (id, title, target, before, user_id, repeat, channels) = r?;
        let Some(u) = users.get(&user_id) else { continue };
        let lead = before * 60;
        let origin = local(target, u.tz);
        let occurrences: Vec<i64> = if repeat == "yearly" {
            dates_in(from, to, lead, u.tz)
                .into_iter()
                .filter(|d| repeat_hits("yearly", origin.date_naive(), *d))
                .filter_map(|d| tz::at_time(d, origin.time(), u.tz))
                .collect()
        } else {
            vec![target]
        };
        for occ in occurrences.into_iter().filter(|o| inwin(o - lead)) {
            let years = local(occ, u.tz).year() - origin.year();
            let title = if years > 0 { format!("⏳ {title}（{years} 周年）") } else { format!("⏳ {title}") };
            due.push(Due {
                key: format!("countdown:{id}:{occ}:{before}"),
                user_id,
                kind: "countdown",
                title,
                body: format!("还有 {}（{}）", humanize(before), fmt(occ, u.tz)),
                channels: parse_channels(channels.clone()),
            });
        }
    }

    // 日程（含重复）
    let mut stmt = conn.prepare_cached(
        "SELECT id, title, start_at, repeat, remind_before, note, user_id, channels FROM events
         WHERE remind_before IS NOT NULL",
    )?;
    for r in stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, Option<String>>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, Option<String>>(7)?,
        ))
    })? {
        let (id, title, start, repeat, before, note, user_id, channels) = r?;
        let Some(u) = users.get(&user_id) else { continue };
        let lead = before * 60;
        let origin = local(start, u.tz);
        let occurrences: Vec<i64> = if repeat == "none" {
            vec![start]
        } else {
            dates_in(from, to, lead, u.tz)
                .into_iter()
                .filter(|d| repeat_hits(&repeat, origin.date_naive(), *d))
                .filter_map(|d| tz::at_time(d, origin.time(), u.tz))
                .collect()
        };
        for occ in occurrences.into_iter().filter(|o| inwin(o - lead)) {
            let mut body = format!("{} 开始", fmt(occ, u.tz));
            if let Some(n) = note.as_deref().filter(|n| !n.is_empty()) {
                body = format!("{body}\n{n}");
            }
            due.push(Due {
                key: format!("event:{id}:{occ}"),
                user_id,
                kind: "event",
                title: format!("📅 {title}"),
                body,
                channels: parse_channels(channels.clone()),
            });
        }
    }

    // 课程（按各自的开学日期；默认法定假日不提醒）
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, room, teacher, weekday, start_time, end_time, weeks, remind_before, user_id, channels
         FROM courses WHERE remind_before IS NOT NULL",
    )?;
    for r in stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, u32>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, String>(6)?,
            r.get::<_, String>(7)?,
            r.get::<_, i64>(8)?,
            r.get::<_, i64>(9)?,
            r.get::<_, Option<String>>(10)?,
        ))
    })? {
        let (id, name, room, teacher, weekday, st, et, weeks, before, user_id, channels) = r?;
        let Some(u) = users.get(&user_id) else { continue };
        let Some(term_start) = u.get("term_start") else { continue };
        let skip_holiday = u.get("course_skip_holiday") != Some("0");
        for d in dates_in(from, to, before * 60, u.tz) {
            let off = holidays.get(&d.to_string()).is_some_and(|h| h.1);
            let ok = d.weekday().number_from_monday() == weekday
                && term_week(term_start, d).is_some_and(|w| in_weeks(&weeks, w))
                && !(skip_holiday && off);
            let Some(occ) = at(d, &st, u.tz).filter(|o| ok && inwin(o - before * 60)) else { continue };
            let place = [room.as_deref(), teacher.as_deref()]
                .into_iter()
                .flatten()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            due.push(Due {
                key: format!("course:{id}:{occ}"),
                user_id,
                kind: "course",
                title: format!("📚 {name}"),
                body: format!("{st}-{et}  {place}"),
                channels: parse_channels(channels.clone()),
            });
        }
    }

    // 习惯：计划日到点且当天未完成（计数型需达到目标次数）
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, remind_time, user_id, days, goal, unit, channels FROM habits
         WHERE archived = 0 AND remind_time IS NOT NULL AND remind_time != ''",
    )?;
    let habits = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (id, name, time, user_id, days, goal, unit, channels) in habits {
        let Some(u) = users.get(&user_id) else { continue };
        for d in dates_in(from, to, 0, u.tz) {
            if !habit_day(days.as_deref(), d) {
                continue;
            }
            let Some(fire) = at(d, &time, u.tz).filter(|t| inwin(*t)) else { continue };
            let count: i64 = conn
                .prepare_cached("SELECT count FROM habit_logs WHERE habit_id = ? AND day = ?")?
                .query_row((id, d.to_string()), |r| r.get(0))
                .optional()?
                .unwrap_or(0);
            if count >= goal {
                continue;
            }
            let body = if goal > 1 {
                format!("「{name}」今天完成了 {count}/{goal}{}，继续加油！", unit.as_deref().unwrap_or(" 次"))
            } else {
                format!("今天还没有完成「{name}」，加油！")
            };
            due.push(Due {
                key: format!("habit:{id}:{fire}"),
                user_id,
                kind: "habit",
                title: "✅ 习惯打卡".into(),
                body,
                channels: parse_channels(channels.clone()),
            });
        }
    }

    // 每日概览 / 每周回顾
    for (&user_id, u) in &users {
        if let Some(time) = u.get("digest_time") {
            for d in dates_in(from, to, 0, u.tz) {
                if let Some(fire) = at(d, time, u.tz).filter(|t| inwin(*t)) {
                    due.push(Due {
                        key: format!("digest:{user_id}:{fire}"),
                        user_id,
                        kind: "digest",
                        title: "☀️ 今日概览".into(),
                        body: digest(conn, user_id, u, d, &holidays)?,
                        channels: None,
                    });
                }
            }
        }
        if let (Some(day), Some(time)) = (u.get("weekly_day"), u.get("weekly_time")) {
            for d in dates_in(from, to, 0, u.tz) {
                if day.parse() != Ok(d.weekday().number_from_monday()) {
                    continue;
                }
                if let Some(fire) = at(d, time, u.tz).filter(|t| inwin(*t)) {
                    due.push(Due {
                        key: format!("weekly:{user_id}:{fire}"),
                        user_id,
                        kind: "weekly",
                        title: "📊 本周回顾".into(),
                        body: weekly(conn, user_id, u, d)?,
                        channels: None,
                    });
                }
            }
        }
    }

    // 去重：sent_log 插入成功的才继续
    let mut out = Vec::with_capacity(due.len());
    for d in due {
        if conn.execute("INSERT OR IGNORE INTO sent_log (key, at) VALUES (?, ?)", (&d.key, to))? == 1 {
            out.push(d);
        }
    }
    conn.execute("DELETE FROM sent_log WHERE at < ?", [to - 90 * 86400])?;
    Ok(out)
}

// ---------- 2. 入队 ----------

/// 免打扰：落在 [start, end) 内的消息推迟到 end（支持跨午夜，如 23:00-07:00）
pub fn quiet_until(ts: i64, start: &str, end: &str, tz: Tz) -> i64 {
    let (Some(s), Some(e)) = (tz::parse_hm(start), tz::parse_hm(end)) else { return ts };
    if s == e {
        return ts;
    }
    let t = local(ts, tz);
    let now = t.time().with_second(0).unwrap();
    let inside = if s < e { now >= s && now < e } else { now >= s || now < e };
    if !inside {
        return ts;
    }
    // 跨午夜且还在「当天晚上」那一段时，结束时刻在明天
    let day = if s > e && now >= s { t.date_naive().succ_opt().unwrap() } else { t.date_naive() };
    tz::at_time(day, e, tz).unwrap_or(ts)
}

/// 条目指定了渠道就只发这些；否则发给接收该类型（kinds 为空表示全部）的启用渠道
fn route(conn: &Connection, user_id: i64, kind: &str, only: &Option<Vec<i64>>) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare_cached("SELECT id, kinds FROM channels WHERE user_id = ? AND enabled = 1")?;
    let rows = stmt.query_map([user_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?)))?;
    Ok(rows
        .flatten()
        .filter(|(id, kinds)| match only {
            Some(ids) => ids.contains(id),
            None => kinds
                .as_deref()
                .and_then(|k| serde_json::from_str::<Vec<String>>(k).ok())
                .is_none_or(|ks| ks.iter().any(|k| k == kind)),
        })
        .map(|(id, _)| id)
        .collect())
}

fn enqueue(conn: &Connection, due: Vec<Due>, now: i64) -> rusqlite::Result<()> {
    if due.is_empty() {
        return Ok(());
    }
    let users = load_users(conn)?;
    for d in due {
        let Some(u) = users.get(&d.user_id) else { continue };
        let next_at = match (u.get("quiet_start"), u.get("quiet_end")) {
            (Some(s), Some(e)) => quiet_until(now, s, e, u.tz),
            _ => now,
        };
        for channel_id in route(conn, d.user_id, d.kind, &d.channels)? {
            conn.execute(
                "INSERT INTO outbox (user_id, channel_id, kind, title, body, next_at) VALUES (?, ?, ?, ?, ?, ?)",
                (d.user_id, channel_id, d.kind, &d.title, &d.body, next_at),
            )?;
        }
    }
    Ok(())
}

// ---------- 3. 发送 ----------

const BACKOFF: [i64; 3] = [60, 300, 1800];

pub async fn dispatch(state: &Shared, now: i64) {
    type Job = (i64, i64, String, String, String, i64, Option<(String, String, bool)>);
    let jobs: Vec<Job> = {
        let conn = state.db.lock().unwrap();
        let Ok(mut stmt) = conn.prepare_cached(
            "SELECT o.id, o.channel_id, o.kind, o.title, o.body, o.attempts, c.kind, c.config, c.enabled
             FROM outbox o LEFT JOIN channels c ON c.id = o.channel_id
             WHERE o.status = 'pending' AND o.next_at <= ? ORDER BY o.next_at LIMIT 100",
        ) else {
            return;
        };
        stmt.query_map([now], |r| {
            let channel = match r.get::<_, Option<String>>(6)? {
                Some(kind) => Some((kind, r.get(7)?, r.get(8)?)),
                None => None,
            };
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, channel))
        })
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    };

    for (id, channel_id, kind, title, body, attempts, channel) in jobs {
        let result = match channel {
            None => Err(notify::SendError::Other("渠道已删除".into())),
            Some((_, _, false)) => Err(notify::SendError::Other("渠道已停用".into())),
            Some((ckind, config, true)) => {
                let cfg: Value = serde_json::from_str(&config).unwrap_or_default();
                let msg = notify::Msg { title, body, kind };
                notify::send(&state.http, &state.vapid, &ckind, &cfg, &msg).await
            }
        };
        let conn = state.db.lock().unwrap();
        let _ = match result {
            Ok(()) => conn.execute(
                "UPDATE outbox SET status = 'sent', attempts = attempts + 1, sent_at = ?, error = NULL WHERE id = ?",
                (now, id),
            ),
            Err(e) => {
                let attempts = attempts + 1;
                // 订阅失效 / 渠道不存在时不再重试
                let gone = matches!(e, notify::SendError::Gone(_));
                let final_fail = gone || attempts as usize >= BACKOFF.len() || e.message().starts_with("渠道已");
                if gone {
                    let _ = conn.execute("UPDATE channels SET enabled = 0 WHERE id = ?", [channel_id]);
                }
                eprintln!("[notify] outbox {id} channel {channel_id} attempt {attempts}: {}", e.message());
                conn.execute(
                    "UPDATE outbox SET status = ?, attempts = ?, next_at = ?, error = ? WHERE id = ?",
                    (
                        if final_fail { "failed" } else { "pending" },
                        attempts,
                        now + BACKOFF[(attempts as usize - 1).min(BACKOFF.len() - 1)],
                        e.message(),
                        id,
                    ),
                )
            }
        };
    }

    let conn = state.db.lock().unwrap();
    let _ = conn.execute("DELETE FROM outbox WHERE created_at < ?", [now - 30 * 86400]);
}

// ---------- 概览内容 ----------

fn digest(
    conn: &Connection,
    user_id: i64,
    u: &Ctx,
    d: NaiveDate,
    holidays: &HashMap<String, (String, bool)>,
) -> rusqlite::Result<String> {
    let start = day_start(d, u.tz);
    let end = day_start(d.succ_opt().unwrap(), u.tz);
    let mut lines = Vec::new();

    if let Some((name, off)) = holidays.get(&d.to_string()) {
        lines.push(if *off { format!("🎉 今天是{name}假期") } else { format!("💼 今天{name}调休上班") });
    }

    if let Some(w) = u.get("term_start").and_then(|ts| term_week(ts, d)) {
        let mut stmt = conn.prepare_cached(
            "SELECT name, start_time, weeks FROM courses WHERE user_id = ? AND weekday = ? ORDER BY start_time",
        )?;
        let courses: Vec<String> = stmt
            .query_map((user_id, d.weekday().number_from_monday()), |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })?
            .flatten()
            .filter(|(_, _, weeks)| in_weeks(weeks, w))
            .map(|(n, t, _)| format!("{t} {n}"))
            .collect();
        if !courses.is_empty() {
            lines.push(format!("📚 第 {w} 周 · {} 节课：{}", courses.len(), courses.join("、")));
        }
    }

    let todo: i64 = conn.query_row(
        "SELECT count(*) FROM todos WHERE user_id = ? AND done = 0 AND due_at IS NOT NULL AND due_at < ?",
        [user_id, end],
        |r| r.get(0),
    )?;
    if todo > 0 {
        lines.push(format!("📝 今天到期 / 已逾期待办 {todo} 项"));
    }

    let events: i64 = conn.query_row(
        "SELECT count(*) FROM events WHERE user_id = ? AND repeat = 'none' AND start_at >= ? AND start_at < ?",
        [user_id, start, end],
        |r| r.get(0),
    )?;
    if events > 0 {
        lines.push(format!("📅 今日日程 {events} 个"));
    }

    if let Ok((title, target)) = conn.query_row(
        "SELECT title, target_at FROM countdowns WHERE user_id = ? AND repeat = 'none' AND target_at > ?
         ORDER BY target_at LIMIT 1",
        [user_id, start],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
    ) {
        let days = (local(target, u.tz).date_naive() - d).num_days();
        lines.push(format!("⏳ 距离「{title}」还有 {days} 天"));
    }

    if lines.is_empty() {
        lines.push("今天没有安排，享受自由的一天 ☕".into());
    }
    Ok(lines.join("\n"))
}

/// 每周回顾：过去 7 天（含今天）完成情况 + 未来 7 天安排
fn weekly(conn: &Connection, user_id: i64, u: &Ctx, d: NaiveDate) -> rusqlite::Result<String> {
    let from_day = d - chrono::Duration::days(6);
    let from = day_start(from_day, u.tz);
    let to = day_start(d.succ_opt().unwrap(), u.tz);
    let next = day_start(d + chrono::Duration::days(8), u.tz);
    let mut lines = Vec::new();

    let done: i64 = conn.query_row(
        "SELECT count(*) FROM todos WHERE user_id = ? AND done_at >= ? AND done_at < ?",
        [user_id, from, to],
        |r| r.get(0),
    )?;
    let open: i64 =
        conn.query_row("SELECT count(*) FROM todos WHERE user_id = ? AND done = 0", [user_id], |r| r.get(0))?;
    lines.push(format!("📝 完成待办 {done} 项，还剩 {open} 项"));

    let focus: i64 = conn.query_row(
        "SELECT coalesce(sum(minutes), 0) FROM focus_sessions WHERE user_id = ? AND started_at >= ? AND started_at < ?",
        [user_id, from, to],
        |r| r.get(0),
    )?;
    if focus > 0 {
        lines.push(format!("🍅 专注 {} 小时 {} 分钟", focus / 60, focus % 60));
    }

    let mut stmt =
        conn.prepare_cached("SELECT id, name, days, goal FROM habits WHERE user_id = ? AND archived = 0 ORDER BY id")?;
    let habits: Vec<(i64, String, Option<String>, i64)> = stmt
        .query_map([user_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .flatten()
        .collect();
    let mut parts = Vec::new();
    for (id, name, days, goal) in habits {
        let planned = (0..7).filter(|i| habit_day(days.as_deref(), from_day + chrono::Duration::days(*i))).count();
        let hit: i64 = conn.query_row(
            "SELECT count(*) FROM habit_logs WHERE habit_id = ? AND day >= ? AND day <= ? AND count >= ?",
            (id, from_day.to_string(), d.to_string(), goal),
            |r| r.get(0),
        )?;
        if planned > 0 {
            parts.push(format!("{name} {hit}/{planned}"));
        }
    }
    if !parts.is_empty() {
        lines.push(format!("✅ 习惯：{}", parts.join("、")));
    }

    let events: i64 = conn.query_row(
        "SELECT count(*) FROM events WHERE user_id = ? AND repeat = 'none' AND start_at >= ? AND start_at < ?",
        [user_id, to, next],
        |r| r.get(0),
    )?;
    if events > 0 {
        lines.push(format!("📅 下周有 {events} 个日程"));
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weeks_parse() {
        assert!(in_weeks("1-16", 1));
        assert!(in_weeks("1-8,10,12-16", 10));
        assert!(!in_weeks("1-8,10,12-16", 9));
        assert!(!in_weeks("", 1));
    }

    #[test]
    fn week_number() {
        let d = |s| NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap();
        // 2026-09-07 是周一
        assert_eq!(term_week("2026-09-07", d("2026-09-07")), Some(1));
        assert_eq!(term_week("2026-09-07", d("2026-09-13")), Some(1));
        assert_eq!(term_week("2026-09-09", d("2026-09-14")), Some(2));
    }

    #[test]
    fn quiet_hours() {
        let tz = chrono_tz::Asia::Shanghai;
        let d = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let t = |date: NaiveDate, hm| at(date, hm, tz).unwrap();
        let next = d.succ_opt().unwrap();
        // 跨午夜 23:00-07:00
        assert_eq!(quiet_until(t(d, "23:30"), "23:00", "07:00", tz), t(next, "07:00"));
        assert_eq!(quiet_until(t(d, "03:00"), "23:00", "07:00", tz), t(d, "07:00"));
        assert_eq!(quiet_until(t(d, "12:00"), "23:00", "07:00", tz), t(d, "12:00"));
        // 当天内 12:00-14:00
        assert_eq!(quiet_until(t(d, "13:00"), "12:00", "14:00", tz), t(d, "14:00"));
        assert_eq!(quiet_until(t(d, "14:00"), "12:00", "14:00", tz), t(d, "14:00"));
    }

    #[test]
    fn habit_days() {
        let mon = NaiveDate::from_ymd_opt(2026, 10, 19).unwrap();
        assert!(habit_day(None, mon));
        assert!(habit_day(Some("1,3,5"), mon));
        assert!(!habit_day(Some("2,4"), mon));
    }
}
