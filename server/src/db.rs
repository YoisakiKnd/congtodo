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
"#,
// v3：停用账号 + 全站设置（如注册策略，管理员可在界面上修改）
r#"
ALTER TABLE users ADD COLUMN disabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN last_login_at INTEGER;
CREATE TABLE server_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
"#,
// v4：待办清单/子任务/重复、共享清单、习惯计数、倒数日重复与图片、节次、
//     专注记录、推送队列、按条目选择渠道、ICS 订阅、节假日
r#"
CREATE TABLE todo_lists (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,          -- 创建者
    name TEXT NOT NULL,
    color TEXT,
    sort INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX todo_lists_user ON todo_lists(user_id);
CREATE TABLE list_members (            -- 共享清单的成员（不含创建者）
    list_id INTEGER NOT NULL REFERENCES todo_lists(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (list_id, user_id)
) WITHOUT ROWID;
CREATE INDEX list_members_user ON list_members(user_id);

ALTER TABLE todos ADD COLUMN list_id INTEGER REFERENCES todo_lists(id) ON DELETE SET NULL;
ALTER TABLE todos ADD COLUMN parent_id INTEGER REFERENCES todos(id) ON DELETE CASCADE;
ALTER TABLE todos ADD COLUMN repeat TEXT NOT NULL DEFAULT 'none'; -- none|daily|weekdays|weekly|monthly|yearly
CREATE INDEX todos_list ON todos(list_id);
CREATE INDEX todos_parent ON todos(parent_id);

-- 按条目选择推送渠道：JSON 数组（渠道 id），NULL = 按渠道的类型设置
ALTER TABLE todos ADD COLUMN channels TEXT;
ALTER TABLE events ADD COLUMN channels TEXT;
ALTER TABLE courses ADD COLUMN channels TEXT;
ALTER TABLE habits ADD COLUMN channels TEXT;
ALTER TABLE countdowns ADD COLUMN channels TEXT;

-- 渠道表重建：kind 增加 webpush，新增 kinds（接收哪些类型的提醒，NULL = 全部）
CREATE TABLE channels_v4 (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('webhook', 'ntfy', 'bark', 'webpush')),
    config TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 1,
    user_id INTEGER NOT NULL DEFAULT 0,
    kinds TEXT
);
INSERT INTO channels_v4 (id, name, kind, config, enabled, user_id)
    SELECT id, name, kind, config, enabled, user_id FROM channels;
DROP TABLE channels;
ALTER TABLE channels_v4 RENAME TO channels;
CREATE INDEX channels_user ON channels(user_id);

ALTER TABLE habits ADD COLUMN days TEXT;              -- "1,2,3,4,5"，NULL = 每天
ALTER TABLE habits ADD COLUMN goal INTEGER NOT NULL DEFAULT 1; -- 每天需完成的次数
ALTER TABLE habits ADD COLUMN unit TEXT;              -- 计数单位，如「杯」
ALTER TABLE habit_logs ADD COLUMN count INTEGER NOT NULL DEFAULT 1;
ALTER TABLE habit_logs ADD COLUMN note TEXT;

ALTER TABLE countdowns ADD COLUMN repeat TEXT NOT NULL DEFAULT 'none'; -- none|yearly
ALTER TABLE countdowns ADD COLUMN image_id INTEGER;

ALTER TABLE courses ADD COLUMN period_from INTEGER;   -- 按节次录入时的起止节
ALTER TABLE courses ADD COLUMN period_to INTEGER;

CREATE TABLE files (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    mime TEXT NOT NULL,
    data BLOB NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX files_user ON files(user_id);

CREATE TABLE focus_sessions (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    todo_id INTEGER REFERENCES todos(id) ON DELETE SET NULL,
    started_at INTEGER NOT NULL,
    minutes INTEGER NOT NULL,
    note TEXT
);
CREATE INDEX focus_user ON focus_sessions(user_id, started_at);

-- 推送队列：每条消息每个渠道一行；免打扰延后、失败重试、推送记录都基于它
CREATE TABLE outbox (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    channel_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending', -- pending | sent | failed
    attempts INTEGER NOT NULL DEFAULT 0,
    next_at INTEGER NOT NULL,
    error TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    sent_at INTEGER
);
CREATE INDEX outbox_pending ON outbox(next_at) WHERE status = 'pending';
CREATE INDEX outbox_user ON outbox(user_id, created_at);

-- 订阅的外部日历（ICS）
CREATE TABLE feeds (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    color TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    last_sync INTEGER,
    last_error TEXT
);
CREATE INDEX feeds_user ON feeds(user_id);
CREATE TABLE feed_events (
    feed_id INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
    uid TEXT NOT NULL,
    title TEXT NOT NULL,
    start_at INTEGER NOT NULL,
    end_at INTEGER,
    all_day INTEGER NOT NULL DEFAULT 0,
    repeat TEXT NOT NULL DEFAULT 'none',
    location TEXT,
    PRIMARY KEY (feed_id, uid, start_at)
);

-- 法定节假日 / 调休缓存（来源：NateScarlet/holiday-cn）
CREATE TABLE holidays (
    date TEXT PRIMARY KEY,              -- "2026-10-01"
    name TEXT NOT NULL,
    off INTEGER NOT NULL                -- 1 放假，0 调休上班
) WITHOUT ROWID;
"#];

/// 带 user_id 的业务表：第一个用户注册时认领其中 user_id = 0 的旧数据；删除用户时一并清理
pub const USER_TABLES: &[&str] = &[
    "countdowns",
    "events",
    "courses",
    "todos",
    "todo_lists",
    "habits",
    "channels",
    "user_settings",
    "files",
    "focus_sessions",
    "outbox",
    "feeds",
];

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

pub fn get_server_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM server_settings WHERE key = ?", [key], |r| r.get(0))
        .ok()
}

pub fn get_setting(conn: &Connection, user_id: i64, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM user_settings WHERE user_id = ? AND key = ?",
        (user_id, key),
        |r| r.get(0),
    )
    .ok()
}
