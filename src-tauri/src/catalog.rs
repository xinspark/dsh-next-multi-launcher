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
    /// 资产形态：`installer`（安装包）/ `portable`（便携包）。
    /// 同一 release 同时挂多个资产时按下面的 `pick_asset` 优先级挑选。
    pub kind: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    pub prerelease: bool,
    /// 该版本对应的内核（`@deepseek-ai/dsh`）版本，从 release 正文的
    /// `上游版本：DeepSeek Harness vX` 实时解析；解析不到就是 `None`，不猜。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_version: Option<String>,
    /// 仅「latest」条目有：它此刻指向的真实应用版本（如 `v2.0.21-next`）。
    /// `latest` 本身不是固定地址，是每次加载**实时**取最新 release 得到的。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
}

/// 资产的形态标签，前端据此显示「安装包 / 便携版」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    Installer,
    Portable,
}

impl AssetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AssetKind::Installer => "installer",
            AssetKind::Portable => "portable",
        }
    }
}

/// 在一条 release 的资产名里挑「用户该下载的那个」。
///
/// 优先级（先命中先返回）：
/// 1. 非便携的 `.exe` 且名字含 setup / installer / 安装 → 安装包
/// 2. 任意非便携 `.exe`（electron-builder 默认产物名不带 setup）→ 安装包
/// 3. `.zip` → 便携包（历史资产形态）
/// 4. 便携 `.exe`（`*Portable.exe`）→ 便携包
///
/// `.blockmap` / `latest.yml` / `.dmg` 之类不会被选中。
pub fn pick_asset<'a>(names: impl IntoIterator<Item = &'a str>) -> Option<(&'a str, AssetKind)> {
    let names: Vec<&str> = names.into_iter().collect();
    let is_exe = |n: &str| n.to_ascii_lowercase().ends_with(".exe");
    let is_portable = |n: &str| n.to_ascii_lowercase().contains("portable");
    let is_setup = |n: &str| {
        let lower = n.to_ascii_lowercase();
        lower.contains("setup") || lower.contains("installer") || n.contains("安装")
    };

    // 1) 明确的安装包
    if let Some(name) = names
        .iter()
        .copied()
        .find(|n| is_exe(n) && !is_portable(n) && is_setup(n))
    {
        return Some((name, AssetKind::Installer));
    }
    // 2) 其它非便携 exe
    if let Some(name) = names.iter().copied().find(|n| is_exe(n) && !is_portable(n)) {
        return Some((name, AssetKind::Installer));
    }
    // 3) zip（历史上只有便携包）
    if let Some(name) = names
        .iter()
        .copied()
        .find(|n| n.to_ascii_lowercase().ends_with(".zip"))
    {
        return Some((name, AssetKind::Portable));
    }
    // 4) 兜底：便携 exe
    names
        .iter()
        .copied()
        .find(|n| is_exe(n))
        .map(|name| (name, AssetKind::Portable))
}

/// 查某个渠道的可用版本。
///
/// `mirror` 是用户在设置里填的加速前缀（空串 = 直连）。API 请求与安装包地址共用它：
/// 实测 `gh-proxy.com` 能代理 `api.github.com`，因此「版本查询」也能走同一条路。
pub fn list(channel: &str, mirror: &str) -> Result<Vec<CatalogEntry>, String> {
    match channel {
        "native" => Ok(native_entry()),
        "next" => github_entries(false, mirror),
        "next-preview" => github_entries(true, mirror),
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
        kind: AssetKind::Installer.as_str().to_string(),
        url: NATIVE_WIN_URL.into(),
        digest: None,
        prerelease: false,
        kernel_version: None,
        app_version: None,
    }]
}

/// 从 release 正文里解析内核版本：`上游版本：DeepSeek Harness v0.2.1-rc.4`。
///
/// 这是**实时**取的信息（正文随 release 变），不写死；解析不到返回 `None`。
fn kernel_from_body(body: &str) -> Option<String> {
    const MARKER: &str = "DeepSeek Harness v";
    let idx = body.find(MARKER)?;
    let rest = &body[idx + MARKER.len()..];
    let ver: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
        .collect();
    let ver = ver.trim_end_matches(['.', '-', '+']).to_string();
    if ver.is_empty() || !ver.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        None
    } else {
        Some(ver)
    }
}

/// 把 GitHub 的状态码翻成人话。
///
/// 403/429 几乎总是**未登录限流**（60 次/小时/IP）——用户看到的应该是
/// “几点恢复 / 换个办法”，而不是一个裸数字。
pub(crate) fn github_error(code: u16, reset_at: Option<i64>) -> String {
    match code {
        403 | 429 => {
            let when = reset_at
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                .map(|d| d.with_timezone(&chrono::Local).format("%H:%M").to_string());
            match when {
                Some(t) => format!(
                    "GitHub 限流（{code}）：未登录每小时 60 次，{t} 后恢复；也可点「打开 Releases」在浏览器下载"
                ),
                None => format!(
                    "GitHub 限流（{code}）：未登录每小时 60 次，请稍后重试，或点「打开 Releases」在浏览器下载"
                ),
            }
        }
        404 => "上游仓库或发布不存在（404）".to_string(),
        other => format!("GitHub 返回 {other}"),
    }
}

fn github_entries(want_prerelease: bool, mirror: &str) -> Result<Vec<CatalogEntry>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!(
            "dsh-next-multi-launcher/",
            env!("CARGO_PKG_VERSION")
        ))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("初始化网络客户端失败：{e}"))?;

    let base = format!("https://api.github.com/repos/{NEXT_REPO}/releases?per_page=30");
    // 配了加速前缀就连 API 一起走它（实测 gh-proxy.com 能代理 api.github.com）
    let prefix = mirror.trim().trim_end_matches('/');
    let url = if prefix.is_empty() {
        base
    } else {
        format!("{prefix}/{base}")
    };
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .map_err(|e| format!("请求失败：{e}"))?;
    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        // `X-RateLimit-Reset`（Unix 秒）能直接告诉用户几点恢复，比干说“稍后重试”有用
        let reset = resp
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<i64>().ok());
        return Err(github_error(status, reset));
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
        // 一个 release 常同时挂安装包与便携包：挑“用户该下载的那个”（安装包优先）。
        let named: Vec<(&str, &serde_json::Value)> = assets
            .iter()
            .filter_map(|a| a.get("name").and_then(|v| v.as_str()).map(|name| (name, a)))
            .collect();
        let Some((chosen, kind)) = pick_asset(named.iter().map(|(name, _)| *name)) else {
            continue;
        };
        let Some(asset) = named
            .iter()
            .find(|(name, _)| *name == chosen)
            .map(|(_, a)| *a)
        else {
            continue;
        };

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
            asset_name: chosen.to_string(),
            kind: kind.as_str().to_string(),
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
            kernel_version: rel
                .get("body")
                .and_then(|v| v.as_str())
                .and_then(kernel_from_body),
            app_version: None,
        });
    }

    // 「latest」不是写死的地址：每次加载都用上面这次实时请求的结果现算——
    // 取当前最新 release 的版本号、安装包地址与内核版本，复制出一条 tag=latest 的条目。
    if let Some(newest) = out.first().cloned() {
        let mut latest = newest;
        latest.app_version = Some(latest.tag.clone());
        latest.tag = "latest".to_string();
        out.insert(0, latest);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_channel_has_single_latest() {
        let list = list("native", "").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].tag, "latest");
        assert!(list[0].url.starts_with("https://download.deepseek.com/"));
    }

    #[test]
    fn kernel_parsed_from_release_body() {
        // 取自真实 release 正文
        let stable = "**上游版本：** [DeepSeek Harness v0.2.1-rc.4](https://github.com/deepseek-ai/deepseek-harness/releases/tag/dsh-v0.2.1-rc.4)。";
        assert_eq!(kernel_from_body(stable).as_deref(), Some("0.2.1-rc.4"));

        let beta = "本次 Beta 更新将上游升级至预发布版 DeepSeek Harness v0.2.2-alpha.2，新增…";
        assert_eq!(kernel_from_body(beta).as_deref(), Some("0.2.2-alpha.2"));

        // 解析不到就返回 None，不猜
        assert_eq!(kernel_from_body("本次更新修复若干问题"), None);
        assert_eq!(kernel_from_body("DeepSeek Harness v"), None);
    }

    #[test]
    fn unknown_channel_is_an_error() {
        assert!(list("bogus", "").is_err());
    }

    #[test]
    fn native_is_an_installer() {
        let list = list("native", "").unwrap();
        assert_eq!(list[0].kind, "installer");
    }

    #[test]
    fn prefers_installer_over_portable_zip() {
        let names = [
            "DSH-Desktop-2.0.21-next-x64-Portable.zip",
            "DSH-Desktop-Setup-2.0.21-next-x64.exe",
        ];
        assert_eq!(
            pick_asset(names),
            Some((
                "DSH-Desktop-Setup-2.0.21-next-x64.exe",
                AssetKind::Installer
            ))
        );
    }

    #[test]
    fn bare_exe_counts_as_installer() {
        // electron-builder 的默认产物名不含 setup
        let names = [
            "latest.yml",
            "DSH-Desktop-2.0.21-next-x64.exe",
            "DSH-Desktop-2.0.21-next-x64.exe.blockmap",
        ];
        assert_eq!(
            pick_asset(names),
            Some(("DSH-Desktop-2.0.21-next-x64.exe", AssetKind::Installer))
        );
    }

    #[test]
    fn falls_back_to_portable_zip() {
        let names = ["DSH-Desktop-Beta-2.1.0-x64-Portable.zip", "latest.yml"];
        assert_eq!(
            pick_asset(names),
            Some((
                "DSH-Desktop-Beta-2.1.0-x64-Portable.zip",
                AssetKind::Portable
            ))
        );
    }

    #[test]
    fn portable_exe_is_last_resort() {
        let names = ["DSH-Desktop-2.0.21-next-x64-Portable.exe"];
        assert_eq!(
            pick_asset(names),
            Some((
                "DSH-Desktop-2.0.21-next-x64-Portable.exe",
                AssetKind::Portable
            ))
        );
    }

    #[test]
    fn skips_releases_without_usable_asset() {
        let names = [
            "latest.yml",
            "DSH-Desktop-2.0.21-next-x64.dmg",
            "checksums.txt",
        ];
        assert_eq!(pick_asset(names), None);
    }
}
