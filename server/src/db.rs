use rusqlite::{Connection, types::ValueRef};
use serde_json::{Map, Value};

/// 按顺序追加迁移；已执行到哪一步记录在 PRAGMA user_version。
/// 时间戳统一用 unix 秒；"HH:MM" / "YYYY-MM-DD" 按服务器本地时区（TZ 环境变量）解释。
const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE countdowns (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    target_at INTEGER NOT NULL,
    color TEXT,
    pinned INTEGER NOT NULL DEFAULT 0,
    remind_before INTEGER,            -- 分钟，NULL = 不提醒
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE events (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    start_at INTEGER NOT NULL,
    end_at INTEGER,
    all_day INTEGER NOT NULL DEFAULT 0,
    repeat TEXT NOT NULL DEFAULT 'none', -- none | daily | weekly | monthly | yearly
    color TEXT,
    note TEXT,
    remind_before INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX events_start ON events(start_at);
CREATE TABLE courses (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    teacher TEXT,
    room TEXT,
    weekday INTEGER NOT NULL,         -- 1=周一 … 7=周日
    start_time TEXT NOT NULL,         -- "08:00"
    end_time TEXT NOT NULL,
    weeks TEXT NOT NULL DEFAULT '1-20', -- "1-16" / "1-8,10,12-16"
    color TEXT,
    remind_before INTEGER
);
CREATE TABLE todos (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    note TEXT,
    due_at INTEGER,
    remind_at INTEGER,
    priority INTEGER NOT NULL DEFAULT 0, -- 0 普通 1 重要 2 紧急
    done INTEGER NOT NULL DEFAULT 0,
    done_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX todos_remind ON todos(remind_at) WHERE done = 0;
CREATE TABLE habits (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    icon TEXT,
    color TEXT,
    target INTEGER NOT NULL DEFAULT 7, -- 每周目标次数
    remind_time TEXT,                  -- "21:00"
    archived INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE habit_logs (
    habit_id INTEGER NOT NULL REFERENCES habits(id) ON DELETE CASCADE,
    day TEXT NOT NULL,                 -- "2026-10-20"
    PRIMARY KEY (habit_id, day)
) WITHOUT ROWID;
CREATE TABLE channels (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('webhook', 'ntfy', 'bark')),
    config TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE sent_log (key TEXT PRIMARY KEY, at INTEGER NOT NULL) WITHOUT ROWID;
"#,
// v2：多用户。user_id = 0 表示启用账号系统之前的数据，由第一个注册的用户认领
r#"
CREATE TABLE users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL UNIQUE COLLATE NOCASE,
    password_hash TEXT NOT NULL,
    is_admin INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,       -- sha256(token)，数据库泄露也拿不到可用的会话
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
) WITHOUT ROWID;
CREATE INDEX sessions_user ON sessions(user_id);

ALTER TABLE countdowns ADD COLUMN user_id INTEGER NOT NULL DEFAULT 0;
ALTER TABLE events ADD COLUMN user_id INTEGER NOT NULL DEFAULT 0;
ALTER TABLE courses ADD COLUMN user_id INTEGER NOT NULL DEFAULT 0;
ALTER TABLE todos ADD COLUMN user_id INTEGER NOT NULL DEFAULT 0;
ALTER TABLE habits ADD COLUMN user_id INTEGER NOT NULL DEFAULT 0;
ALTER TABLE channels ADD COLUMN user_id INTEGER NOT NULL DEFAULT 0;
CREATE INDEX countdowns_user ON countdowns(user_id);
CREATE INDEX events_user ON events(user_id);
CREATE INDEX courses_user ON courses(user_id);
CREATE INDEX todos_user ON todos(user_id);
CREATE INDEX habits_user ON habits(user_id);
CREATE INDEX channels_user ON channels(user_id);

CREATE TABLE user_settings (
    user_id INTEGER NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (user_id, key)
) WITHOUT ROWID;
INSERT INTO user_settings (user_id, key, value) SELECT 0, key, value FROM settings;
DROP TABLE settings;
"#];

/// 带 user_id 的业务表，第一个用户注册时认领其中 user_id = 0 的旧数据
pub const USER_TABLES: &[&str] = &["countdowns", "events", "courses", "todos", "habits", "channels", "user_settings"];

pub fn open(path: &str) -> rusqlite::Result<Connection> {
    let mut conn = Connection::open(path)?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA cache_size = -2000;",
    )?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as i64 + 1)?;
        tx.commit()?;
    }
    Ok(conn)
}

/// 把一行转成 JSON 对象；`json_cols` 里的列按 JSON 文本解析
pub fn row_to_json(row: &rusqlite::Row, json_cols: &[&str]) -> rusqlite::Result<Value> {
    let stmt = row.as_ref();
    let mut obj = Map::with_capacity(stmt.column_count());
    for i in 0..stmt.column_count() {
        let name = stmt.column_name(i)?;
        let v = match row.get_ref(i)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => n.into(),
            ValueRef::Real(f) => f.into(),
            ValueRef::Text(t) => {
                let s = String::from_utf8_lossy(t);
                if json_cols.contains(&name) {
                    serde_json::from_str(&s).unwrap_or(Value::Null)
                } else {
                    Value::String(s.into_owned())
                }
            }
            ValueRef::Blob(_) => Value::Null,
        };
        obj.insert(name.to_string(), v);
    }
    Ok(Value::Object(obj))
}

pub fn get_setting(conn: &Connection, user_id: i64, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM user_settings WHERE user_id = ? AND key = ?",
        (user_id, key),
        |r| r.get(0),
    )
    .ok()
}
