//! 通知推送。新增一种渠道：在 `send` 里加一个分支，并在 db.rs 的 CHECK 约束和前端表单里登记。
//! 实际发送由 scheduler 的推送队列（outbox）驱动，这里只负责「把一条消息发到一个渠道」。

use reqwest::{Client, Method, RequestBuilder, StatusCode};
use serde_json::{Value, json};

use crate::webpush::{self, Vapid};

pub struct Msg {
    pub title: String,
    pub body: String,
    /// 来源模块：todo / event / course / habit / countdown / digest / weekly / test
    pub kind: String,
}

/// 发送失败的原因；Gone 表示浏览器订阅已失效，应停用该渠道
pub enum SendError {
    Gone(String),
    Other(String),
}

impl SendError {
    pub fn message(&self) -> &str {
        match self {
            SendError::Gone(m) | SendError::Other(m) => m,
        }
    }
}

impl From<String> for SendError {
    fn from(e: String) -> Self {
        SendError::Other(e)
    }
}

fn s<'a>(cfg: &'a Value, key: &str) -> Option<&'a str> {
    cfg.get(key).and_then(Value::as_str).filter(|v| !v.is_empty())
}

pub async fn send(http: &Client, vapid: &Vapid, kind: &str, cfg: &Value, msg: &Msg) -> Result<(), SendError> {
    let req = match kind {
        "ntfy" => ntfy(http, cfg, msg)?,
        "bark" => bark(http, cfg, msg)?,
        "webhook" => webhook(http, cfg, msg)?,
        "webpush" => webpush::request(
            http,
            vapid,
            cfg,
            &json!({ "title": msg.title, "body": msg.body, "kind": msg.kind, "url": "/" }),
        )?,
        other => return Err(format!("unknown channel kind: {other}").into()),
    };
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }
    let text = resp.text().await.unwrap_or_default();
    let err = format!("{status}: {}", text.chars().take(200).collect::<String>());
    // 推送服务返回 404 / 410 说明浏览器订阅已过期或被用户取消
    if kind == "webpush" && matches!(status, StatusCode::NOT_FOUND | StatusCode::GONE) {
        return Err(SendError::Gone(err));
    }
    Err(err.into())
}

/// ntfy: { server?, topic, token?, priority? }  — https://docs.ntfy.sh/publish/#publish-as-json
fn ntfy(http: &Client, cfg: &Value, msg: &Msg) -> Result<RequestBuilder, String> {
    let server = s(cfg, "server").unwrap_or("https://ntfy.sh").trim_end_matches('/');
    let topic = s(cfg, "topic").ok_or("ntfy: topic 未配置")?;
    let mut req = http.post(server).json(&json!({
        "topic": topic,
        "title": msg.title,
        "message": msg.body,
        "tags": [msg.kind],
        "priority": cfg.get("priority").and_then(Value::as_i64).unwrap_or(3),
    }));
    if let Some(token) = s(cfg, "token") {
        req = req.bearer_auth(token);
    }
    Ok(req)
}

/// Bark: { server?, key, group?, sound?, level? }  — POST /push (v2 JSON API)
fn bark(http: &Client, cfg: &Value, msg: &Msg) -> Result<RequestBuilder, String> {
    let server = s(cfg, "server").unwrap_or("https://api.day.app").trim_end_matches('/');
    let key = s(cfg, "key").ok_or("bark: key 未配置")?;
    Ok(http.post(format!("{server}/push")).json(&json!({
        "device_key": key,
        "title": msg.title,
        "body": msg.body,
        "group": s(cfg, "group").unwrap_or("Congtodo"),
        "sound": s(cfg, "sound"),
        "level": s(cfg, "level"),
    })))
}

/// 通用 Webhook: { url, method?, headers?, template? }
/// template 为空时发送 {"title","body","kind","time"}；
/// 否则把 {{title}} {{body}} {{kind}} 替换后作为 body 发送，可适配企业微信/飞书/钉钉/Discord/Telegram 等。
fn webhook(http: &Client, cfg: &Value, msg: &Msg) -> Result<RequestBuilder, String> {
    let url = s(cfg, "url").ok_or("webhook: url 未配置")?;
    let method = Method::from_bytes(s(cfg, "method").unwrap_or("POST").as_bytes()).map_err(|e| e.to_string())?;
    let mut req = http.request(method, url).header("content-type", "application/json");
    if let Some(headers) = cfg.get("headers").and_then(Value::as_object) {
        for (k, v) in headers {
            if let Some(v) = v.as_str() {
                req = req.header(k, v);
            }
        }
    }
    let body = match s(cfg, "template") {
        // 模板一般是 JSON，替换值需要按 JSON 字符串转义（去掉外层引号）
        Some(tpl) => {
            let esc = |v: &str| {
                let quoted = Value::from(v).to_string();
                quoted[1..quoted.len() - 1].to_string()
            };
            tpl.replace("{{title}}", &esc(&msg.title))
                .replace("{{body}}", &esc(&msg.body))
                .replace("{{kind}}", &msg.kind)
        }
        None => json!({
            "title": msg.title,
            "body": msg.body,
            "kind": msg.kind,
            "time": chrono::Local::now().to_rfc3339(),
        })
        .to_string(),
    };
    Ok(req.body(body))
}
