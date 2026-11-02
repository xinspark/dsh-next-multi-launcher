//! 版本目录：三条渠道的"可下载版本"清单。
//!
//! | 渠道 | 来源 | 形态 |
//! |---|---|---|
//! | `next` | GitHub `anywhere-labs/dsh-desktop`，`prerelease=false` | 便携 zip |
//! | `next-preview` | 同仓库，`prerelease=true` | 便携 zip（前缀是 `DSH-Desktop-Beta-`） |
//! | `native` | 官网固定直链 | 安装器 exe，**只有 latest** |
//!
//! 只做清单与链接，真正的下载由前端触发（浏览器或下载器）。

use serde::Serialize;
use std::time::Duration;

/// DSH-NEXT 仓库。
pub const NEXT_REPO: &str = "anywhere-labs/dsh-desktop";
/// 原生 DSH 桌面端固定直链（官网只提供 latest，无历史版本、无公开哈希）。
pub const NATIVE_WIN_URL: &str = "https://download.deepseek.com/desktop/dsh-latest-windows-x64.exe";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    /// `next` / `next-preview` / `native`
    pub channel: String,
    pub tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// 文件大小（字节）；拿不到就是 0
    pub size: u64,
    pub asset_name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    pub prerelease: bool,
}

/// 查某个渠道的可用版本。
pub fn list(channel: &str) -> Result<Vec<CatalogEntry>, String> {
    match channel {
        "native" => Ok(native_entry()),
        "next" => github_entries(false),
        "next-preview" => github_entries(true),
        other => Err(format!("未知渠道：{other}")),
    }
}

/// 原生 DSH：只有一条 latest，且无法校验（官网不提供哈希）。
fn native_entry() -> Vec<CatalogEntry> {
    vec![CatalogEntry {
        channel: "native".into(),
        tag: "latest".into(),
        published_at: None,
        size: 0,
        asset_name: "dsh-latest-windows-x64.exe".into(),
        url: NATIVE_WIN_URL.into(),
        digest: None,
        prerelease: false,
    }]
}

fn github_entries(want_prerelease: bool) -> Result<Vec<CatalogEntry>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!(
            "dsh-next-multi-launcher/",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("初始化网络客户端失败：{e}"))?;

    let url = format!("https://api.github.com/repos/{NEXT_REPO}/releases?per_page=30");
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("请求失败：{e}"))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub 返回 {}", resp.status().as_u16()));
    }
    let body: serde_json::Value = resp.json().map_err(|e| format!("解析响应失败：{e}"))?;
    let Some(list) = body.as_array() else {
        return Err("响应不是数组".to_string());
    };

    let mut out = Vec::new();
    for rel in list {
        let prerelease = rel
            .get("prerelease")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if prerelease != want_prerelease {
            continue;
        }
        // 正式渠道里混着旧的 `v2.0.x`（无 -next 后缀），NEXT 已取代稳定通道，只取 -next
        let tag = rel
            .get("tag_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if !want_prerelease && !tag.ends_with("-next") {
            continue;
        }

        let assets = rel.get("assets").and_then(|v| v.as_array());
        let Some(assets) = assets else { continue };
        // 预览版的前缀是 `DSH-Desktop-Beta-`，只能按后缀匹配
        let asset = assets.iter().find(|a| {
            a.get("name")
                .and_then(|v| v.as_str())
                .map(|n| n.ends_with("-x64-Portable.zip"))
                .unwrap_or(false)
        });
        let Some(asset) = asset else { continue };

        out.push(CatalogEntry {
            channel: if want_prerelease {
                "next-preview".into()
            } else {
                "next".into()
            },
            tag: tag.to_string(),
            published_at: rel
                .get("published_at")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            size: asset.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
            asset_name: asset
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            url: asset
                .get("browser_download_url")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            digest: asset
                .get("digest")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            prerelease,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_channel_has_single_latest() {
        let list = list("native").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].tag, "latest");
        assert!(list[0].url.starts_with("https://download.deepseek.com/"));
    }

    #[test]
    fn unknown_channel_is_an_error() {
        assert!(list("bogus").is_err());
    }
}
