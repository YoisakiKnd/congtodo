# Congtodo

以「待办 + 日历提醒」为核心的自托管效率应用：课表导入即成日历图层，倒数日、习惯、专注随手可用；支持浏览器推送 / ntfy / Bark / Webhook 提醒。

> **Alpha 阶段**：功能可用、数据库会通过迁移平滑升级，但接口和界面仍可能调整。升级前请备份数据。

- **单二进制**：Rust 后端 + 内嵌前端，一个文件即可运行；Docker 镜像支持 amd64 / arm64
- **低占用**：单线程 tokio + SQLite，常驻内存约 10MB
- **多用户**：用户名密码登录（argon2id），每个账号的数据、设置、推送渠道相互隔离
- **前端**：Svelte 5 + [shadcn-svelte](https://shadcn-svelte.com)（Tailwind v4）+ [Schedule-X](https://schedule-x.dev)（shadcn 主题）日历 / 课表；浅色 / 深色主题；桌面侧栏 / 移动端底栏
- **离线也能提醒**：调度器在服务端运行，浏览器关了照样推送；可安装为 PWA，支持浏览器原生推送

## 功能

核心是**待办 + 日历提醒**，界面只有四个入口：

| 入口 | 内容 |
| --- | --- |
| **今天** | 当天时间线（课程、日程、订阅日历按时间排列，标出「现在」）、今天到期和逾期的待办、倒数日、习惯打卡、专注、接下来 7 天 |
| **待办** | 收件箱、清单（可共享给其他用户一起编辑）、子任务、重复（每天 / 工作日 / 每周 / 每月 / 每年，完成后自动顺延）、优先级、提醒 |
| **日历** | 月 / 周 / 日 / 日程视图；日程、课程、待办、倒数日、订阅日历按图层叠加；农历、节气、节日、法定假日「休 / 班」 |
| **设置** | 时区与提醒、通知渠道、日历订阅、数据导出导入、账号；管理员的用户管理 |

- **课表即日历图层**：在「日历 → 课表」中导入 WakeUp 课程表 / CSV、设置开学日期和作息、添加课程；「查看本周课表」一键切到只看课程的周视图。课程按周次和单双周出现，上课前提醒，法定假日自动跳过
- **习惯、倒数日、专注**从「今天」打开面板管理：习惯支持指定打卡日、计数（如每天 8 杯水）、备注、热力图和近 12 周完成率；倒数日支持每年重复（生日 / 纪念日）和背景图；专注是番茄钟，可从任意待办开始，计时以悬浮小窗显示
- **提醒**：按用户时区计算；每个渠道可只收部分类型，单个条目可指定渠道；免打扰时段；失败自动重试；推送记录；每日概览与每周回顾
- **日历同步**：生成 ICS 订阅地址给手机 / 电脑日历；订阅外部 ICS（校历、工作日历），每 6 小时同步
- **数据**：每个账号可导出 / 导入 JSON；管理员可下载整库备份

## 部署

### Docker Compose（推荐）

```bash
curl -O https://raw.githubusercontent.com/YoisakiKnd/congtodo/main/docker-compose.yml
docker compose up -d
```

打开 `http://服务器:8080`，**第一个注册的账号成为管理员**。默认策略下之后就关闭注册，
管理员可以在「用户管理」页面直接创建账号，或临时开放注册。

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
| `CONGTODO_SIGNUP` | `first` | 注册策略默认值：`first` 只允许创建第一个账号；`open` 开放注册；`closed` 禁止注册。管理员在界面上修改后以界面为准 |
| `CONGTODO_TRUST_PROXY` | 空 | 设为 `1` 时始终信任 `X-Forwarded-For`。默认只在请求来自本机 / 内网（即反向代理）时信任，用于限流识别客户端 IP |
| `CONGTODO_SECURE_COOKIE` | 空 | 设为 `1` 时强制 Cookie 加 `Secure`（代理不传 `X-Forwarded-Proto` 时使用） |
| `TZ` | 系统时区（镜像内 `Asia/Shanghai`） | 用户还没设置时区时的默认值；每个用户的提醒按自己在设置里的时区计算（首次登录自动取浏览器时区） |
| `CONGTODO_VAPID_SUBJECT` | `mailto:congtodo@localhost` | 浏览器推送的联系方式（`mailto:` 或 `https://`），部分推送服务（如 Apple）要求有效值 |
| `CONGTODO_HOLIDAY_URL` | jsDelivr 上的 holiday-cn | 节假日数据地址，`{year}` 替换为年份；服务器访问不了 jsDelivr 时可换镜像 |

浏览器推送的 VAPID 密钥在首次启动时自动生成并存进数据库，备份数据库即可保留，迁移后已订阅的浏览器无需重新订阅。
iPhone / iPad 需要在 Safari 中「添加到主屏幕」后，从主屏幕打开才能开启浏览器通知（iOS 16.4+），且网站必须是 HTTPS。

节假日数据来自 [NateScarlet/holiday-cn](https://github.com/NateScarlet/holiday-cn)（根据国务院通知更新），服务器每天检查一次，缓存在数据库中。

健康检查：`GET /api/health` 返回 `ok <版本号>`。

### 备份与升级

数据都在一个 SQLite 文件里（WAL 模式，旁边会有 `-wal` / `-shm` 文件）。

- **备份**：管理员在「用户管理」点「备份数据库」即可下载一致性快照；也可以停止服务后复制整个数据目录，或在线执行 `sqlite3 congtodo.db ".backup backup.db"`。只复制 `.db` 文件而漏掉 `-wal` 会丢失最近的写入
- **个人数据**：每个用户可在「设置 → 数据」导出 / 导入 JSON（导入是追加，不会覆盖）
- **升级**：拉取新镜像 / 替换二进制后重启，数据库迁移会自动执行
- **从无账号版本升级**：旧数据会自动归入第一个注册的账号

### 用户管理

管理员在侧栏「用户」（移动端在「设置 → 用户管理」）中可以：创建账号、重置密码、设置 / 取消管理员、停用 / 启用、删除用户（连同其全部数据），以及修改注册策略。
停用或重置密码会让该用户所有设备立即下线，停用期间不再推送提醒。管理员不能降级、停用或删除自己，因此始终至少保留一个管理员。

### 安全

- 密码使用 argon2id 哈希；会话令牌只以 SHA-256 形式入库；Cookie 为 `HttpOnly` + `SameSite=Lax`
- 登录限流（内存计数，重启清零）：同一 IP 15 分钟内失败 20 次、同一用户名 15 分钟内失败 10 次后暂时拒绝；同一 IP 每小时最多注册 5 次；改密码时原密码错 5 次后暂停 15 分钟
- 按用户名限流意味着别人可以故意输错让某个账号暂时（15 分钟）无法登录，这是防暴力破解的常见取舍
- Webhook 渠道和 ICS 订阅会让服务器向用户填写的地址发请求。开放注册（`open`）时，任何注册用户都能借此访问服务器所在内网，请只对信任的人开放
- ICS 订阅地址带随机密钥，知道地址就能读取该用户的日程；泄露后可在设置里「重新生成」

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
│ scheduler.rs  收集到期提醒 → 路由入队 → 发送重试 │──► notify.rs ──► 浏览器 / ntfy / Bark / Webhook
│ db.rs         SQLite(WAL) + user_version 迁移  │
│ web.rs        rust-embed 静态资源 + SPA 回退     │
└───────────────────────────────────────────────┘
```

### 目录

```
server/src/
  main.rs       启动、路由组装、优雅退出
  auth.rs       注册 / 登录 / 会话 / 改密码，require_user 中间件
  admin.rs      管理员接口（用户管理、注册策略、数据库备份），require_admin 中间件
  ratelimit.rs  滑动窗口限流、客户端 IP 识别
  db.rs         表结构与迁移（MIGRATIONS 只追加）
  api.rs        RESOURCES 表驱动的通用 CRUD（含共享清单的可见范围、重复待办顺延）
  lists.rs      待办清单与共享成员
  data.rs       JSON 导出 / 导入、图片上传、推送记录
  ics.rs        ICS 订阅导出、外部 ICS 解析（含重复规则展开）与同步
  holidays.rs   法定节假日数据拉取与缓存
  tz.rs         用户时区、重复规则推算
  scheduler.rs  提醒调度：collect → enqueue（路由 / 免打扰）→ dispatch（重试）
  notify.rs     推送渠道实现
  webpush.rs    浏览器推送（RFC 8291 加密 + VAPID，纯 Rust）
  web.rs        前端静态资源
web/src/
  App.svelte            外壳与路由表（侧栏 / 底栏、ModeWatcher、Toaster）
  app.css               shadcn 主题令牌 + Schedule-X 配色映射
  lib/store.svelte.ts   全局状态：clock、route、Collection、settings、habitLogs
  lib/time.ts           日期工具；occursOn/inWeeks 与后端规则一致
  lib/schedule.ts       日程 / 课程 / 待办 / 订阅 → Schedule-X 事件（按可见范围展开重复规则）
  lib/calendar-cells.ts 日历格子自定义渲染（农历、节日、休 / 班）
  lib/lunar.ts          农历（浏览器 Intl 中国历法）、节气、节日
  lib/push.ts           浏览器推送订阅
  lib/timetable-import.ts  WakeUp 课程表 / CSV 解析
  lib/components/ui/    shadcn-svelte 组件（由 CLI 生成，可直接改）
  lib/ui.svelte.ts      全局编辑器 / 面板的打开状态（openEvent、openCourse、openPanel…）
  lib/focus.svelte.ts   专注计时（全局单例，悬浮小窗和面板共用）
  components/           ScheduleX（日历封装）Sheet（表单对话框）FocusWidget ChannelPick TrendBars Field Choice ColorPick Page
  components/editors/   全局编辑器：日程、课程、学期与作息、课表导入
  components/panels/    从「今天」打开的面板：习惯、倒数日、专注
  views/                四个页面（今天 / 待办 / 日历 / 设置）+ 登录、用户管理；settings/ 下是设置页的各个分区
public/sw.js            Service Worker：离线缓存 + 显示推送通知
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
| GET / POST | `/api/admin/users` | 管理员：用户列表 / 创建用户 `{username, password, is_admin}` |
| PATCH / DELETE | `/api/admin/users/{id}` | 管理员：`{is_admin?, disabled?, password?}` / 删除用户及其数据 |
| GET / PUT | `/api/admin/settings` | 管理员：`{signup}` 注册策略 |
| GET | `/api/admin/backup` | 管理员：下载数据库快照 |
| GET / POST | `/api/{res}` | 列表 / 新建，`res` ∈ countdowns events courses todos habits channels feeds focus_sessions |
| PATCH / DELETE | `/api/{res}/{id}` | 局部更新 / 删除；待办 `done: 1` 且有重复规则时返回顺延后的数据和 `_next: true` |
| GET / POST | `/api/lists` | 清单（含别人分享给我的）/ 新建 |
| PATCH / DELETE | `/api/lists/{id}` | 修改 / 删除清单（仅创建者） |
| POST · DELETE | `/api/lists/{id}/members` · `/{user_id}` | 按用户名分享 / 移除成员或自己退出 |
| GET / PUT | `/api/settings` | 键值设置：`timezone`、`term_start`、`periods`、`digest_time`、`weekly_day`、`weekly_time`、`quiet_start`、`quiet_end`、`course_skip_holiday` 等 |
| GET | `/api/habit_logs?from=YYYY-MM-DD` | 打卡记录（含次数和备注） |
| POST | `/api/habits/{id}/log` | `{day, count, note}` 设置某天的次数与备注 |
| POST | `/api/channels/{id}/test` | 发送测试通知 |
| GET | `/api/push/key` | 浏览器订阅推送用的 VAPID 公钥 |
| GET | `/api/outbox` | 最近 100 条推送记录 |
| GET / POST / DELETE | `/api/ics/token` | 查看 / 重新生成 / 关闭 ICS 订阅令牌 |
| GET | `/api/ics/{token}.ics` | ICS 订阅（无需登录，凭令牌） |
| GET · POST | `/api/feeds/events?from&to` · `/api/feeds/{id}/sync` | 订阅日历的事件 / 立即同步 |
| GET | `/api/holidays?year=YYYY` | 法定节假日与调休 |
| GET · POST | `/api/export` · `/api/import` | 导出 / 导入当前用户数据（JSON） |
| POST · GET · DELETE | `/api/files` · `/api/files/{id}` | 上传（原始二进制，JPEG/PNG/WebP ≤ 2MB）/ 读取 / 删除图片 |

时间戳一律 unix 秒；`HH:MM` / `YYYY-MM-DD` 按用户设置的时区。

### 提醒规则

| 来源 | 触发时间 | 去重 key |
| --- | --- | --- |
| 待办 | `remind_at`（未完成） | `todo:id:remind_at` |
| 倒数日 | 目标时间 `- remind_before`（每年重复的按当年日期） | `countdown:id:occurrence:before` |
| 日程 | 每次发生时间 `- remind_before`（支持每天/周/月/年重复） | `event:id:occurrence` |
| 课程 | 本学期周次命中当天的 `start_time - remind_before`；默认法定假日跳过 | `course:id:occurrence` |
| 习惯 | 计划日的 `remind_time`，且当天次数未达到目标 | `habit:id:fire` |
| 每日概览 | 设置里的 `digest_time` | `digest:user:fire` |
| 每周回顾 | 设置里的 `weekly_day` + `weekly_time` | `weekly:user:fire` |

到期的提醒按以下顺序处理：
1. **路由**：条目指定了渠道就只发这些；否则发给「接收该类型」的所有启用渠道（渠道的类型列表为空表示全部）
2. **免打扰**：落在用户免打扰时段内的消息推迟到结束时刻
3. **发送与重试**：写入推送队列 `outbox`，失败在 1 / 5 / 30 分钟后重试，共 3 次；浏览器订阅失效（404 / 410）时自动停用该渠道

已收集的 key 记录在 `sent_log` 表中（保留 90 天），重启不会重复推送。服务停机期间错过的提醒不会补发。

### 课表导入

- **WakeUp 课程表**：在 App 中导出为文件（`.wakeup_schedule`），会同时导入作息时间、开学日期和单双周。格式按公开的导出结构解析，如遇到无法识别的文件欢迎提 issue 附上样例
- **CSV**：导入对话框里可下载模板。列名支持「课程名、星期、开始时间、结束时间、节次、周次、教室、老师」，Excel 编辑后另存为 CSV 即可

### Webhook 模板

Body 模板中的 `{{title}}` `{{body}}` `{{kind}}` 会以 JSON 字符串转义后替换。设置页内置企业微信、钉钉、飞书、Discord、Telegram 预设。

## 扩展

- **加一个数据模块**：`db.rs` 追加一条迁移 → `api.rs` 的 `RESOURCES` 加一行 → `types.ts` 加接口 → `store.svelte.ts` 的 `db` 加一个 `Collection` → 优先做成「今天」里的一块或一个面板（`components/panels/`），确实需要独立入口时再在 `App.svelte` 的路由表登记
- **加一种推送渠道**：在 `notify.rs` 的 `send` 中加分支，同时修改 `channels.kind` 的 CHECK 约束（需新迁移）和 `api.rs` 的校验，并在 `views/settings/Channels.svelte` 的 `KINDS` 里登记表单字段
- **路线图**：见 issue 与发布说明
