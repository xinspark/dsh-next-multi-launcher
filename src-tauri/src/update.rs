//! 启动器自身的更新检查（**只提示，不下载、不替换文件**）。
//!
//! 数据源：本仓库的 GitHub Releases（`/releases/latest` 天然排除预发布与草稿）。
//! 失败一律静默降级为 `error`，由前端决定是否提示——断网不该在界面上报错。

use serde::Serialize;
use std::time::Duration;

/// 本仓库（更新检查用）。
pub const REPO: &str = "xinspark/dsh-next-multi-launcher";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    pub has_update: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// 检查失败的原因（已脱敏，只给前端做"检查失败"文案用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 比较两个版本号（忽略 `v` 前缀与预发布后缀），返回 `a < b`。
///
/// 按数字段比较，避免字符串比较把 `1.0.10` 判成小于 `1.0.9`。
pub fn is_newer(current: &str, latest: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        let core = s.trim().trim_start_matches(['v', 'V']);
        // 去掉 `-beta.1` / `+build` 之类后缀
        let core = core.split(['-', '+']).next().unwrap_or(core);
        core.split('.')
            .map(|p| p.trim().parse::<u64>().unwrap_or(0))
            .collect()
    };
    let a = parse(current);
    let b = parse(latest);
    let n = a.len().max(b.len());
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return y > x;
        }
    }
    false
}

/// 组装更新检查的 API 地址；`mirror` 非空时走加速前缀（与版本目录共用同一个设置）。
fn api_url(mirror: &str) -> String {
    let base = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let prefix = mirror.trim().trim_end_matches('/');
    if prefix.is_empty() {
        base
    } else {
        format!("{prefix}/{base}")
    }
}

/// 查询最新发布；网络失败时返回带 `error` 的结果而不是 `Err`。
///
/// `mirror` 是设置里的「下载加速前缀」：GitHub API 在国内常不可达，
/// 配了前缀就一起走它（实测 `gh-proxy.com` 可代理 `api.github.com`）。
pub fn check(current: &str, mirror: &str) -> UpdateInfo {
    let url = api_url(mirror);
    let client = match reqwest::blocking::Client::builder()
        .user_agent(concat!(
            "dsh-next-multi-launcher/",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(e) => return failed(current, format!("初始化网络客户端失败：{e}")),
    };

    let resp = match client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
    {
        Ok(r) => r,
        Err(e) => return failed(current, format!("请求失败：{e}")),
    };

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let reset = resp
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<i64>().ok());
        return failed(current, crate::catalog::github_error(status, reset));
    }

    let body: serde_json::Value = match resp.json() {
        Ok(v) => v,
        Err(e) => return failed(current, format!("解析响应失败：{e}")),
    };

    let tag = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let html_url = body
        .get("html_url")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let published_at = body
        .get("published_at")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    if tag.is_empty() {
        return failed(current, "响应里没有 tag_name".to_string());
    }

    UpdateInfo {
        current: current.to_string(),
        has_update: is_newer(current, &tag),
        latest: Some(tag),
        url: html_url,
        published_at,
        error: None,
    }
}

pub fn failed(current: &str, reason: String) -> UpdateInfo {
    UpdateInfo {
        current: current.to_string(),
        latest: None,
        has_update: false,
        url: None,
        published_at: None,
        error: Some(reason),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_prefix_is_applied_to_api_url() {
        let direct = api_url("");
        assert_eq!(
            direct,
            format!("https://api.github.com/repos/{REPO}/releases/latest")
        );
        // 空白 = 未配置
        assert_eq!(api_url("   "), direct);
        // 结尾有没有斜杠都拼得对
        assert_eq!(
            api_url("https://gh-proxy.com/"),
            format!("https://gh-proxy.com/{direct}")
        );
        assert_eq!(
            api_url("https://gh-proxy.com"),
            format!("https://gh-proxy.com/{direct}")
        );
    }

    #[test]
    fn numeric_segments_not_string_order() {
        assert!(is_newer("1.0.9", "1.0.10"));
        assert!(!is_newer("1.0.10", "1.0.9"));
        assert!(is_newer("1.9.0", "2.0.0"));
        assert!(!is_newer("2.0.0", "2.0.0"));
    }

    #[test]
    fn tolerates_v_prefix_and_prerelease() {
        assert!(!is_newer("v1.0.2", "1.0.2"));
        assert!(is_newer("1.0.2", "v1.1.0"));
        assert!(is_newer("1.0.2", "1.0.3-beta.1"));
    }

    #[test]
    fn shorter_version_pads_with_zero() {
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(is_newer("1.0", "1.0.1"));
    }
}
