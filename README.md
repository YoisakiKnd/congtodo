# Congtodo

倒计时 · 日历 · 课表 · 待办 · 习惯养成，支持 ntfy / Bark / Webhook 推送提醒的自托管效率应用。

> **Alpha 阶段**：功能可用、数据库会通过迁移平滑升级，但接口和界面仍可能调整。升级前请备份数据。

- **单二进制**：Rust 后端 + 内嵌前端，一个文件即可运行；Docker 镜像支持 amd64 / arm64
- **低占用**：单线程 tokio + SQLite，常驻内存约 10MB
- **多用户**：用户名密码登录（argon2id），每个账号的数据、设置、推送渠道相互隔离
- **前端**：Svelte 5 + [shadcn-svelte](https://shadcn-svelte.com)（Tailwind v4）+ [Schedule-X](https://schedule-x.dev)（shadcn 主题）日历 / 课表；浅色 / 深色主题；桌面侧栏 / 移动端底栏
- **离线也能提醒**：调度器在服务端运行，浏览器关了照样推送

## 部署

### Docker Compose（推荐）

```bash
curl -O https://raw.githubusercontent.com/YoisakiKnd/congtodo/main/docker-compose.yml
docker compose up -d
```

打开 `http://服务器:8080`，**第一个注册的账号成为管理员**。默认策略下之后就关闭注册，
需要给家人朋友开账号时把 `CONGTODO_SIGNUP` 临时改成 `open`。

### 单文件

从 [Releases](https://github.com/YoisakiKnd/congtodo/releases) 下载对应架构的压缩包（Linux amd64 / arm64）：

```bash
tar xzf congtodo-*-linux-amd64.tar.gz
TZ=Asia/Shanghai CONGTODO_DB=/var/lib/congtodo/congtodo.db ./congtodo
```

### 反向代理（HTTPS）

公网访问务必放在 HTTPS 后面。代理需要传 `X-Forwarded-Proto`，服务端据此给会话 Cookie 加 `Secure`：

```caddy
todo.example.com {
    reverse_proxy 127.0.0.1:8080   # Caddy 默认会传 X-Forwarded-Proto
}
```

```nginx
location / {
    proxy_pass http://127.0.0.1:8080;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

### 配置

| 环境变量 | 默认 | 说明 |
| --- | --- | --- |
| `CONGTODO_ADDR` | `0.0.0.0:8080` | 监听地址 |
| `CONGTODO_DB` | `congtodo.db`（镜像内为 `/data/congtodo.db`） | SQLite 文件路径 |
| `CONGTODO_SIGNUP` | `first` | `first` 只允许创建第一个账号；`open` 开放注册；`closed` 禁止注册 |
| `CONGTODO_SECURE_COOKIE` | 空 | 设为 `1` 时强制 Cookie 加 `Secure`（代理不传 `X-Forwarded-Proto` 时使用） |
| `TZ` | 系统时区（镜像内 `Asia/Shanghai`） | 课表、习惯提醒等"几点几分"按此时区解释 |

健康检查：`GET /api/health` 返回 `ok <版本号>`。

### 备份与升级

数据都在一个 SQLite 文件里（WAL 模式，旁边会有 `-wal` / `-shm` 文件）。

- **备份**：停止服务后复制整个数据目录；或在线执行 `sqlite3 congtodo.db ".backup backup.db"`。只复制 `.db` 文件而漏掉 `-wal` 会丢失最近的写入
- **升级**：拉取新镜像 / 替换二进制后重启，数据库迁移会自动执行
- **从无账号版本升级**：旧数据会自动归入第一个注册的账号

### 安全提示

- Webhook 渠道会让服务器向用户填写的地址发请求。开放注册（`open`）时，任何注册用户都能借此访问服务器所在内网，请只对信任的人开放
- 登录接口暂未做频率限制，Alpha 阶段建议配合反向代理的限流使用

## 开发

```bash
cd server && cargo run                 # API: 127.0.0.1:8080
cd web && npm i && npm run dev         # 前端热更新: 127.0.0.1:5173（/api 自动代理到 8080）

# 构建单文件
cd web && npm run build
cd ../server && cargo build --release  # release 会把 web/dist 打进二进制

# 或从源码构建镜像
docker build -t congtodo .
```

发布：推送 `v*` 标签后，GitHub Actions 会在 x86 / ARM 机器上原生编译，发布 GitHub Release 和多架构镜像 `ghcr.io/yoisakiknd/congtodo`。

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
  main.rs       启动、路由组装、优雅退出
  auth.rs       注册 / 登录 / 会话 / 改密码，require_user 中间件
  db.rs         表结构与迁移（MIGRATIONS 只追加）
  api.rs        RESOURCES 表驱动的通用 CRUD
  scheduler.rs  提醒调度（todo/倒计时/日程含重复/课程/习惯/每日概览）
  notify.rs     推送渠道实现
  web.rs        前端静态资源
web/src/
  App.svelte            外壳与路由表（侧栏 / 底栏、ModeWatcher、Toaster）
  app.css               shadcn 主题令牌 + Schedule-X 配色映射
  lib/store.svelte.ts   全局状态：clock、route、Collection、settings、habitLogs
  lib/time.ts           日期工具；occursOn/inWeeks 与后端规则一致
  lib/schedule.ts       日程 / 课程 / 待办 → Schedule-X 事件（按可见范围展开重复规则）
  lib/components/ui/    shadcn-svelte 组件（由 CLI 生成，可直接改）
  components/           ScheduleX（日历封装）Sheet（表单对话框）Field Choice ColorPick Page
  views/                各功能页面
```

添加更多 shadcn 组件：`cd web && npx shadcn-svelte@latest add <组件名>`。

日历使用 Schedule-X 的框架无关 API（`createCalendar().render(el)`）而非 `@schedule-x/svelte` 适配器——后者尚未支持 v4。v4 的拖拽插件转为付费版，所以日程通过点击 / 双击打开对话框编辑。

### API

除 `auth/config`、`auth/register`、`auth/login`、`auth/logout`、`health` 外都需要登录（会话 Cookie `congtodo_session`），且只能访问自己的数据。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/api/auth/config` | `{signup, first}`：是否可注册、是否还没有用户 |
| POST | `/api/auth/register` · `/api/auth/login` | `{username, password}`，成功后设置会话 Cookie |
| POST | `/api/auth/logout` | 注销当前会话 |
| GET | `/api/auth/me` | 当前用户 |
| POST | `/api/auth/password` | `{old, new}`，其他设备的会话会失效 |
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
| 每日概览 | 设置里的 `digest_time` | `digest:user:fire` |

提醒只推送到该条目所属用户启用的渠道。已发送的 key 记录在 `sent_log` 表中（保留 90 天），重启不会重复推送。服务停机期间错过的提醒不会补发。

### Webhook 模板

Body 模板中的 `{{title}}` `{{body}}` `{{kind}}` 会以 JSON 字符串转义后替换。设置页内置企业微信、钉钉、飞书、Discord、Telegram 预设。

## 扩展

- **加一个数据模块**：`db.rs` 追加一条迁移 → `api.rs` 的 `RESOURCES` 加一行 → `types.ts` 加接口 → `store.svelte.ts` 的 `db` 加一个 `Collection` → 写 `views/Xxx.svelte` 并在 `App.svelte` 的路由表登记
- **加一种推送渠道**：在 `notify.rs` 的 `send` 中加分支，同时修改 `channels.kind` 的 CHECK 约束（需新迁移），并在 `Settings.svelte` 的 `KINDS` 里登记表单字段
- **路线图建议**：管理员用户管理界面、登录限流、按条目选择推送渠道、导入教务系统课表 / ICS、Web Push（VAPID）、PWA 离线缓存、数据导出
