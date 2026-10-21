//! 内存滑动窗口限流，用于登录 / 注册 / 改密码防暴力破解。
//! 单实例部署足够；重启后计数清零。

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Mutex,
};

use axum::http::HeaderMap;

#[derive(Clone, Copy)]
pub struct Rule {
    pub max: usize,
    pub window: i64, // 秒
}

/// 同一 IP 的登录失败：15 分钟 20 次
pub const LOGIN_IP: Rule = Rule { max: 20, window: 15 * 60 };
/// 同一用户名的登录失败：15 分钟 10 次（防止分布式猜一个账号的密码）
pub const LOGIN_USER: Rule = Rule { max: 10, window: 15 * 60 };
/// 同一 IP 的注册尝试：1 小时 5 次
pub const REGISTER_IP: Rule = Rule { max: 5, window: 60 * 60 };
/// 修改密码时原密码错误：15 分钟 5 次
pub const PASSWORD_USER: Rule = Rule { max: 5, window: 15 * 60 };

const MAX_WINDOW: i64 = 60 * 60;

#[derive(Default)]
pub struct Limiter {
    hits: Mutex<HashMap<String, Vec<i64>>>,
}

impl Limiter {
    /// 被限制时返回还需等待的秒数
    pub fn check(&self, key: &str, rule: Rule, now: i64) -> Option<i64> {
        let mut map = self.hits.lock().unwrap();
        let v = map.get_mut(key)?;
        v.retain(|t| *t > now - rule.window);
        (v.len() >= rule.max).then(|| (v[0] + rule.window - now).max(1))
    }

    pub fn hit(&self, key: &str, now: i64) {
        let mut map = self.hits.lock().unwrap();
        map.entry(key.to_string()).or_default().push(now);
        // 防止大量不同 key 撑爆内存：定期清掉过期记录
        if map.len() > 4096 {
            map.retain(|_, v| {
                v.retain(|t| *t > now - MAX_WINDOW);
                !v.is_empty()
            });
        }
    }

    pub fn clear(&self, key: &str) {
        self.hits.lock().unwrap().remove(key);
    }
}

fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || (v6.segments()[0] & 0xfe00) == 0xfc00 || v6.to_ipv4_mapped().is_some_and(|v4| is_local(v4.into()))
        }
    }
}

/// 客户端 IP。对端是本机 / 内网地址（即反向代理）或设置了 CONGTODO_TRUST_PROXY=1 时，
/// 取 X-Forwarded-For 最右侧的地址（由最近一层代理追加，无法被客户端伪造）。
pub fn client_ip(headers: &HeaderMap, peer: SocketAddr) -> IpAddr {
    let trust = is_local(peer.ip()) || std::env::var("CONGTODO_TRUST_PROXY").is_ok_and(|v| v == "1" || v == "true");
    if trust {
        let forwarded = headers
            .get_all("x-forwarded-for")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|v| v.split(','))
            .filter_map(|s| s.trim().parse::<IpAddr>().ok())
            .last();
        if let Some(ip) = forwarded {
            return ip;
        }
    }
    peer.ip()
}

pub fn fmt_wait(secs: i64) -> String {
    if secs < 60 { format!("{secs} 秒") } else { format!("{} 分钟", (secs + 59) / 60) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rule = Rule { max: 3, window: 100 };

    #[test]
    fn sliding_window() {
        let l = Limiter::default();
        for t in 0..3 {
            assert_eq!(l.check("k", R, t), None);
            l.hit("k", t);
        }
        // 第 4 次被限制，需要等到最早一次（t=0）出窗口
        assert_eq!(l.check("k", R, 10), Some(90));
        assert_eq!(l.check("other", R, 10), None);
        // 窗口滑过最早的一次后放行
        assert_eq!(l.check("k", R, 101), None);
        l.clear("k");
        assert_eq!(l.check("k", R, 101), None);
    }

    #[test]
    fn forwarded_ip_only_from_proxy() {
        let mut h = HeaderMap::new();
        h.insert("x-forwarded-for", "1.1.1.1, 2.2.2.2".parse().unwrap());
        let proxy: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let direct: SocketAddr = "8.8.8.8:5000".parse().unwrap();
        assert_eq!(client_ip(&h, proxy), "2.2.2.2".parse::<IpAddr>().unwrap());
        // 公网直连时忽略可伪造的头
        assert_eq!(client_ip(&h, direct), "8.8.8.8".parse::<IpAddr>().unwrap());
        assert_eq!(client_ip(&HeaderMap::new(), proxy), "127.0.0.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn wait_format() {
        assert_eq!(fmt_wait(30), "30 秒");
        assert_eq!(fmt_wait(61), "2 分钟");
    }
}
