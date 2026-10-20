//! 提醒调度：每分钟醒来一次，计算 (last, now] 窗口内应触发的提醒并推送。
//! 每条提醒有唯一 key 写入 sent_log，重启或时间抖动都不会重复推送。

use std::time::Duration;

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveTime, TimeZone};
use rusqlite::Connection;

use crate::{Shared, db::get_setting, notify};

pub fn spawn(state: Shared) {
    tokio::spawn(async move {
        let mut last = Local::now().timestamp() - 60;
        loop {
            let now = Local::now().timestamp();
            tokio::time::sleep(Duration::from_secs((60 - now % 60 + 1) as u64)).await;
            let now = Local::now().timestamp();
            let msgs = {
                let conn = state.db.lock().unwrap();
                collect(&conn, last, now).unwrap_or_else(|e| {
                    eprintln!("[scheduler] {e}");
                    Vec::new()
                })
            };
            last = now;
            for m in &msgs {
                notify::broadcast(&state, m).await;
            }
        }
    });
}

// ---------- 时间工具 ----------

fn local(ts: i64) -> DateTime<Local> {
    Local.timestamp_opt(ts, 0).single().unwrap_or_else(Local::now)
}

fn at(date: NaiveDate, hm: &str) -> Option<i64> {
    let t = NaiveTime::parse_from_str(hm, "%H:%M").ok()?;
    date.and_time(t).and_local_timezone(Local).earliest().map(|d| d.timestamp())
}

/// 窗口 (from, to] 平移 lead 秒后覆盖到的本地日期（通常 1 个，跨午夜时 2 个）
fn dates_in(from: i64, to: i64, lead: i64) -> Vec<NaiveDate> {
    let a = local(from + 1 + lead).date_naive();
    let b = local(to + lead).date_naive();
    if a == b { vec![a] } else { vec![a, b] }
}

fn hm(ts: i64) -> String {
    local(ts).format("%m-%d %H:%M").to_string()
}

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

fn repeat_hits(rule: &str, origin: NaiveDate, d: NaiveDate) -> bool {
    d >= origin
        && match rule {
            "daily" => true,
            "weekly" => d.weekday() == origin.weekday(),
            "monthly" => d.day() == origin.day(),
            "yearly" => d.month() == origin.month() && d.day() == origin.day(),
            _ => false,
        }
}

// ---------- 收集到期提醒 ----------

struct Due {
    key: String,
    msg: notify::Msg,
}

fn collect(conn: &Connection, from: i64, to: i64) -> rusqlite::Result<Vec<notify::Msg>> {
    let mut due: Vec<Due> = Vec::new();
    let inwin = |t: i64| t > from && t <= to;

    // 待办
    let mut stmt = conn.prepare_cached(
        "SELECT id, title, due_at, remind_at FROM todos
         WHERE done = 0 AND remind_at > ?1 AND remind_at <= ?2",
    )?;
    for r in stmt.query_map((from, to), |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<i64>>(2)?, r.get::<_, i64>(3)?))
    })? {
        let (id, title, due_at, remind_at) = r?;
        let body = match due_at {
            Some(d) => format!("{title}\n截止：{}", hm(d)),
            None => title,
        };
        due.push(Due {
            key: format!("todo:{id}:{remind_at}"),
            msg: notify::Msg { title: "📝 待办提醒".into(), body, kind: "todo" },
        });
    }

    // 倒计时
    let mut stmt = conn.prepare_cached(
        "SELECT id, title, target_at, remind_before FROM countdowns
         WHERE remind_before IS NOT NULL AND target_at - remind_before * 60 > ?1 AND target_at - remind_before * 60 <= ?2",
    )?;
    for r in stmt.query_map((from, to), |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
    })? {
        let (id, title, target, before) = r?;
        due.push(Due {
            key: format!("countdown:{id}:{target}:{before}"),
            msg: notify::Msg {
                title: format!("⏳ {title}"),
                body: format!("还有 {}（{}）", humanize(before), hm(target)),
                kind: "countdown",
            },
        });
    }

    // 日程（含重复）
    let mut stmt = conn.prepare_cached(
        "SELECT id, title, start_at, repeat, remind_before, note FROM events WHERE remind_before IS NOT NULL",
    )?;
    for r in stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    })? {
        let (id, title, start, repeat, before, note) = r?;
        let lead = before * 60;
        let origin = local(start);
        let occurrences: Vec<i64> = if repeat == "none" {
            vec![start]
        } else {
            dates_in(from, to, lead)
                .into_iter()
                .filter(|d| repeat_hits(&repeat, origin.date_naive(), *d))
                .filter_map(|d| d.and_time(origin.time()).and_local_timezone(Local).earliest())
                .map(|d| d.timestamp())
                .collect()
        };
        for occ in occurrences.into_iter().filter(|o| inwin(o - lead)) {
            let mut body = format!("{} 开始", hm(occ));
            if let Some(n) = note.as_deref().filter(|n| !n.is_empty()) {
                body = format!("{body}\n{n}");
            }
            due.push(Due {
                key: format!("event:{id}:{occ}"),
                msg: notify::Msg { title: format!("📅 {title}"), body, kind: "event" },
            });
        }
    }

    // 课程
    if let Some(term_start) = get_setting(conn, "term_start") {
        let mut stmt = conn.prepare_cached(
            "SELECT id, name, room, teacher, weekday, start_time, end_time, weeks, remind_before
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
            ))
        })? {
            let (id, name, room, teacher, weekday, st, et, weeks, before) = r?;
            for d in dates_in(from, to, before * 60) {
                let ok = d.weekday().number_from_monday() == weekday
                    && term_week(&term_start, d).is_some_and(|w| in_weeks(&weeks, w));
                let Some(occ) = at(d, &st).filter(|o| ok && inwin(o - before * 60)) else { continue };
                let place = [room.as_deref(), teacher.as_deref()]
                    .into_iter()
                    .flatten()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                due.push(Due {
                    key: format!("course:{id}:{occ}"),
                    msg: notify::Msg {
                        title: format!("📚 {name}"),
                        body: format!("{st}-{et}  {place}"),
                        kind: "course",
                    },
                });
            }
        }
    }

    // 习惯：到点且当天未打卡
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, remind_time FROM habits WHERE archived = 0 AND remind_time IS NOT NULL AND remind_time != ''",
    )?;
    let habits = stmt
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (id, name, time) in habits {
        for d in dates_in(from, to, 0) {
            let Some(fire) = at(d, &time).filter(|t| inwin(*t)) else { continue };
            let day = d.format("%Y-%m-%d").to_string();
            let done: bool = conn
                .prepare_cached("SELECT EXISTS(SELECT 1 FROM habit_logs WHERE habit_id = ? AND day = ?)")?
                .query_row((id, &day), |r| r.get(0))?;
            if !done {
                due.push(Due {
                    key: format!("habit:{id}:{fire}"),
                    msg: notify::Msg {
                        title: "✅ 习惯打卡".into(),
                        body: format!("今天还没有完成「{name}」，加油！"),
                        kind: "habit",
                    },
                });
            }
        }
    }

    // 每日概览
    if let Some(time) = get_setting(conn, "digest_time").filter(|t| !t.is_empty()) {
        for d in dates_in(from, to, 0) {
            if let Some(fire) = at(d, &time).filter(|t| inwin(*t)) {
                due.push(Due {
                    key: format!("digest:{fire}"),
                    msg: notify::Msg { title: "☀️ 今日概览".into(), body: digest(conn, d)?, kind: "digest" },
                });
            }
        }
    }

    // 去重：sent_log 插入成功的才真正发送
    let mut out = Vec::with_capacity(due.len());
    for d in due {
        if conn.execute("INSERT OR IGNORE INTO sent_log (key, at) VALUES (?, ?)", (&d.key, to))? == 1 {
            out.push(d.msg);
        }
    }
    conn.execute("DELETE FROM sent_log WHERE at < ?", [to - 90 * 86400])?;
    Ok(out)
}

fn digest(conn: &Connection, d: NaiveDate) -> rusqlite::Result<String> {
    let start = at(d, "00:00").unwrap_or_default();
    let end = start + 86400;
    let mut lines = Vec::new();

    if let Some(ts) = get_setting(conn, "term_start") {
        if let Some(w) = term_week(&ts, d) {
            let mut stmt = conn.prepare_cached(
                "SELECT name, start_time, weeks FROM courses WHERE weekday = ? ORDER BY start_time",
            )?;
            let courses: Vec<String> = stmt
                .query_map([d.weekday().number_from_monday()], |r| {
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
    }

    let todo: i64 = conn.query_row(
        "SELECT count(*) FROM todos WHERE done = 0 AND due_at IS NOT NULL AND due_at < ?",
        [end],
        |r| r.get(0),
    )?;
    if todo > 0 {
        lines.push(format!("📝 今天到期 / 已逾期待办 {todo} 项"));
    }

    let events: i64 = conn.query_row(
        "SELECT count(*) FROM events WHERE repeat = 'none' AND start_at >= ? AND start_at < ?",
        [start, end],
        |r| r.get(0),
    )?;
    if events > 0 {
        lines.push(format!("📅 今日日程 {events} 个"));
    }

    if let Ok((title, target)) = conn.query_row(
        "SELECT title, target_at FROM countdowns WHERE target_at > ? ORDER BY target_at LIMIT 1",
        [start],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
    ) {
        let days = (local(target).date_naive() - d).num_days();
        lines.push(format!("⏳ 距离「{title}」还有 {days} 天"));
    }

    if lines.is_empty() {
        lines.push("今天没有安排，享受自由的一天 ☕".into());
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
}
