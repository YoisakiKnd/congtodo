//! 时区：每个用户在设置里保存自己的 IANA 时区（前端首次登录时自动上报浏览器时区）。
//! "HH:MM" / "YYYY-MM-DD" 这类本地时间都按用户时区解释。

use std::{collections::HashMap, sync::OnceLock};

use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Weekday};
use chrono_tz::Tz;
use rusqlite::Connection;

/// 服务器默认时区：TZ 环境变量 → 系统时区 → UTC
pub fn default_tz() -> Tz {
    static TZ: OnceLock<Tz> = OnceLock::new();
    *TZ.get_or_init(|| {
        std::env::var("TZ")
            .ok()
            .and_then(|s| s.trim_start_matches(':').parse().ok())
            .or_else(|| iana_time_zone::get_timezone().ok().and_then(|s| s.parse().ok()))
            .unwrap_or(Tz::UTC)
    })
}

pub fn parse(name: &str) -> Option<Tz> {
    name.parse().ok()
}

pub fn user_tz(conn: &Connection, user_id: i64) -> Tz {
    crate::db::get_setting(conn, user_id, "timezone").as_deref().and_then(parse).unwrap_or_else(default_tz)
}

/// 所有设置过时区的用户；没设置的用 default_tz()
pub fn all_user_tz(conn: &Connection) -> rusqlite::Result<HashMap<i64, Tz>> {
    let mut stmt = conn.prepare_cached("SELECT user_id, value FROM user_settings WHERE key = 'timezone'")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.flatten().filter_map(|(u, v)| Some((u, parse(&v)?))).collect())
}

pub fn local(ts: i64, tz: Tz) -> DateTime<Tz> {
    tz.timestamp_opt(ts, 0).single().unwrap_or_else(|| chrono::Utc::now().with_timezone(&tz))
}

pub fn parse_hm(hm: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(hm.trim(), "%H:%M").ok()
}

/// 某天某个本地时刻对应的时间戳（夏令时跳过的时刻返回 None）
pub fn at_time(date: NaiveDate, t: NaiveTime, tz: Tz) -> Option<i64> {
    tz.from_local_datetime(&date.and_time(t)).earliest().map(|d| d.timestamp())
}

pub fn at(date: NaiveDate, hm: &str, tz: Tz) -> Option<i64> {
    at_time(date, parse_hm(hm)?, tz)
}

/// 本地日期的 0 点
pub fn day_start(date: NaiveDate, tz: Tz) -> i64 {
    at_time(date, NaiveTime::MIN, tz).unwrap_or_else(|| tz.from_utc_datetime(&date.and_time(NaiveTime::MIN)).timestamp())
}

/// 窗口 (from, to] 平移 lead 秒后覆盖到的本地日期（通常 1 个，跨午夜时 2 个）
pub fn dates_in(from: i64, to: i64, lead: i64, tz: Tz) -> Vec<NaiveDate> {
    let a = local(from + 1 + lead, tz).date_naive();
    let b = local(to + lead, tz).date_naive();
    if a == b { vec![a] } else { vec![a, b] }
}

/// 月 / 年重复时，目标日不存在（如 31 号、2 月 29 日）就取当月最后一天
fn add_months_clamped(d: NaiveDate, months: u32, day: u32) -> NaiveDate {
    let first = d.with_day(1).unwrap() + Months::new(months);
    let last = (first + Months::new(1) - Duration::days(1)).day();
    first.with_day(day.min(last)).unwrap()
}

fn next_date(d: NaiveDate, rule: &str, anchor_day: u32) -> Option<NaiveDate> {
    Some(match rule {
        "daily" => d + Duration::days(1),
        "weekdays" => {
            let mut n = d + Duration::days(1);
            while matches!(n.weekday(), Weekday::Sat | Weekday::Sun) {
                n += Duration::days(1);
            }
            n
        }
        "weekly" => d + Duration::days(7),
        "monthly" => add_months_clamped(d, 1, anchor_day),
        "yearly" => add_months_clamped(d, 12, anchor_day),
        _ => return None,
    })
}

/// 重复待办完成后的下一次截止时间：按规则推进，直到晚于 now（至少推进一次），保持本地时刻不变。
/// 月 / 年重复以当前截止日为锚点：1/31 → 2/28 之后会按 28 号继续。
pub fn next_occurrence(due: i64, rule: &str, now: i64, tz: Tz) -> Option<i64> {
    let start = local(due, tz);
    let time = start.time();
    let anchor = start.day();
    let mut d = start.date_naive();
    for _ in 0..1000 {
        d = next_date(d, rule, anchor)?;
        let ts = tz.from_local_datetime(&NaiveDateTime::new(d, time)).earliest()?.timestamp();
        if ts > now {
            return Some(ts);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(tz: Tz, s: &str) -> i64 {
        tz.from_local_datetime(&NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M").unwrap()).unwrap().timestamp()
    }

    #[test]
    fn per_user_timezone_changes_wall_clock() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
        let sh = at(d, "08:00", chrono_tz::Asia::Shanghai).unwrap();
        let ny = at(d, "08:00", chrono_tz::America::New_York).unwrap();
        assert_eq!(ny - sh, 12 * 3600);
    }

    #[test]
    fn repeat_rules() {
        let tz = chrono_tz::Asia::Shanghai;
        let due = ts(tz, "2026-01-31 09:00");
        let n = |rule, now| local(next_occurrence(due, rule, now, tz).unwrap(), tz).format("%Y-%m-%d %H:%M").to_string();
        assert_eq!(n("daily", due), "2026-02-01 09:00");
        assert_eq!(n("monthly", due), "2026-02-28 09:00"); // 2 月没有 31 号
        assert_eq!(n("yearly", due), "2027-01-31 09:00");
        // 2026-01-30 是周五，下一个工作日是周一
        assert_eq!(local(next_occurrence(ts(tz, "2026-01-30 09:00"), "weekdays", 0, tz).unwrap(), tz).format("%a").to_string(), "Mon");
        // 逾期很久：推进到 now 之后
        assert_eq!(n("weekly", ts(tz, "2026-03-01 00:00")), "2026-03-07 09:00");
        assert!(next_occurrence(due, "none", due, tz).is_none());
    }
}
