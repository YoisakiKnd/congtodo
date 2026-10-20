# Congtodo

倒计时 · 日历 · 课表 · 待办 · 习惯养成，支持 ntfy / Bark / Webhook 推送提醒的自托管个人效率应用。

- **单二进制**：Rust 后端 + 内嵌 Svelte 前端，约 7MB
- **低占用**：单线程 tokio，常驻内存约 7MB
- **轻前端**：零 UI 库、零图标库，JS+CSS gzip 后约 38KB；浅色 / 深色主题；桌面侧栏 / 移动端底栏
- **离线也能提醒**：调度器在服务端运行，浏览器关了照样推送

## 快速开始

```bash
# 开发
cd server && cargo run                 # API: 127.0.0.1:8080
cd web && npm i && npm run dev         # 前端热更新: 127.0.0.1:5173（/api 自动代理到 8080）

# 生产（单文件）
cd web && npm run build
cd ../server && cargo build --release  # release 会把 web/dist 打进二进制
TZ=Asia/Shanghai ./target/release/congtodo

# 或 Docker
docker compose up -d
```

| 环境变量 | 默认 | 说明 |
| --- | --- | --- |
| `CONGTODO_ADDR` | `0.0.0.0:8080` | 监听地址 |
| `CONGTODO_DB` | `congtodo.db` | SQLite 文件路径 |
| `CONGTODO_TOKEN` | 空 | 设置后 `/api` 需要 `Authorization: Bearer <token>`，公网部署务必设置 |
| `TZ` | 系统时区 | 课表、习惯提醒等"几点几分"按此时区解释 |

## 架构

```
┌──────────── 浏览器（Svelte 5 SPA）────────────┐
│ views/  Today Todos Calendar Timetable ...    │
│ lib/store.svelte.ts  Collection<T> 乐观更新    │
└──────────────────┬────────────────────────────┘
                   │ REST /api/*
┌──────────────────▼──── congtodo（Rust）───────┐
│ api.rs        通用 CRUD（表白名单）+ 打卡/设置   │
│ scheduler.rs  每分钟计算 (last, now] 内的提醒   │──► notify.rs ──► ntfy / Bark / Webhook
│ db.rs         SQLite(WAL) + user_version 迁移  │
│ web.rs        rust-embed 静态资源 + SPA 回退     │
└───────────────────────────────────────────────┘
```

### 目录

```
server/src/
  main.rs       启动、鉴权中间件
  db.rs         表结构与迁移（MIGRATIONS 只追加）
  api.rs        RESOURCES 表驱动的通用 CRUD
  scheduler.rs  提醒调度（todo/倒计时/日程含重复/课程/习惯/每日概览）
  notify.rs     推送渠道实现
  web.rs        前端静态资源
web/src/
  App.svelte            外壳与路由表
  app.css               设计令牌（颜色/圆角/阴影，暗色模式）
  lib/store.svelte.ts   全局状态：clock、route、Collection、settings、habitLogs
  lib/time.ts           日期工具；occursOn/inWeeks 与后端规则一致
  components/           Sheet(原生 dialog) ColorPick RemindSelect Icon
  views/                各功能页面
```

### API

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET / POST | `/api/{res}` | 列表 / 新建，`res` ∈ countdowns events courses todos habits channels |
| PATCH / DELETE | `/api/{res}/{id}` | 局部更新 / 删除 |
| GET / PUT | `/api/settings` | 键值设置：`term_start`、`digest_time` |
| GET | `/api/habit_logs?from=YYYY-MM-DD` | 打卡记录 |
| POST | `/api/habits/{id}/toggle` | `{day}` 切换某天打卡 |
| POST | `/api/channels/{id}/test` | 发送测试通知 |

时间戳一律 unix 秒；`HH:MM` / `YYYY-MM-DD` 按服务器时区。

### 提醒规则

| 来源 | 触发时间 | 去重 key |
| --- | --- | --- |
| 待办 | `remind_at`（未完成） | `todo:id:remind_at` |
| 倒计时 | `target_at - remind_before` | `countdown:id:target:before` |
| 日程 | 每次发生时间 `- remind_before`（支持每天/周/月/年重复） | `event:id:occurrence` |
| 课程 | 本学期周次命中当天的 `start_time - remind_before` | `course:id:occurrence` |
| 习惯 | `remind_time` 且当天未打卡 | `habit:id:fire` |
| 每日概览 | 设置里的 `digest_time` | `digest:fire` |

已发送的 key 记录在 `sent_log` 表中（保留 90 天），重启不会重复推送。服务停机期间错过的提醒不会补发。

### Webhook 模板

Body 模板中的 `{{title}}` `{{body}}` `{{kind}}` 会以 JSON 字符串转义后替换。设置页内置企业微信、钉钉、飞书、Discord、Telegram 预设。

## 扩展

- **加一个数据模块**：`db.rs` 追加一条迁移 → `api.rs` 的 `RESOURCES` 加一行 → `types.ts` 加接口 → `store.svelte.ts` 的 `db` 加一个 `Collection` → 写 `views/Xxx.svelte` 并在 `App.svelte` 的路由表登记
- **加一种推送渠道**：在 `notify.rs` 的 `send` 中加分支，同时修改 `channels.kind` 的 CHECK 约束（需新迁移），并在 `Settings.svelte` 的 `KINDS` 里登记表单字段
- **路线图建议**：按条目选择推送渠道、导入教务系统课表 / ICS、Web Push（VAPID）、PWA 离线缓存、数据导出与备份
