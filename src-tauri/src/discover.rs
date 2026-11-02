//! 自动发现 DSH-NEXT 安装目录。
//!
//! 四条渠道，按可靠性排序：
//! 1. 快捷方式（开始菜单 / 桌面 `.lnk` 的目标）—— 覆盖正常安装
//! 2. 注册表卸载项（`InstallLocation` / `DisplayIcon`）—— 覆盖走安装器的
//! 3. 常见路径（`%LOCALAPPDATA%\Programs`、`Program Files` 下同名目录）
//! 4. 运行中的进程（`DSH NEXT.exe` 的真实路径）
//!
//! 快捷方式与注册表依赖 Windows COM，这里用一次 PowerShell 子进程批量拿，
//! 避免为低频操作引入 `windows` crate；路径校验与版本读取仍在 Rust 侧。

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::model::VersionInfo;
use crate::versions;

/// 一个扫描到的候选安装目录。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverCandidate {
    pub path: String,
    /// 来源：`shortcut` / `registry` / `common` / `process`
    pub source: String,
    /// 目录里同时有 `DSH NEXT.exe` 与 `resources\app.asar`
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel_version: Option<String>,
    /// 是否已经在版本树里登记过
    pub registered: bool,
}

/// 扫描候选安装目录。
///
/// `registered` 是已登记的版本目录（用来标记「已登记」而不是过滤掉，
/// 让前端可以显示但默认不勾选）。
pub fn scan(registered: &[PathBuf]) -> Vec<DiscoverCandidate> {
    let mut found: BTreeMap<PathBuf, String> = BTreeMap::new();

    for (path, source) in from_powershell() {
        insert_unique(&mut found, &path, &source);
    }
    for (path, source) in from_common_paths() {
        insert_unique(&mut found, &path, &source);
    }
    for (path, source) in from_processes() {
        insert_unique(&mut found, &path, &source);
    }

    let mut out: Vec<DiscoverCandidate> = found
        .into_iter()
        .map(|(dir, source)| {
            let info: VersionInfo = versions::inspect_dir(&dir, "probe");
            let valid = info.exe.is_some() && info.asar.is_some();
            let registered = registered.iter().any(|r| same_path(r, &dir));
            DiscoverCandidate {
                path: dir.to_string_lossy().to_string(),
                source,
                valid,
                shell_version: if valid {
                    Some(info.shell_version)
                } else {
                    None
                },
                kernel_version: if valid {
                    Some(info.kernel_version)
                } else {
                    None
                },
                registered,
            }
        })
        .collect();

    // 有效的在前，其次路径稳定排序
    out.sort_by(|a, b| {
        b.valid
            .cmp(&a.valid)
            .then(a.registered.cmp(&b.registered))
            .then(a.path.cmp(&b.path))
    });
    out
}

fn insert_unique(map: &mut BTreeMap<PathBuf, String>, path: &str, source: &str) {
    let p = PathBuf::from(path.trim());
    if p.as_os_str().is_empty() {
        return;
    }
    // 解析 junction/符号链接，避免把版本树链接与真实目录算成两个
    let real = std::fs::canonicalize(&p).unwrap_or(p);
    let real = strip_verbatim(&real);
    map.entry(real).or_insert_with(|| source.to_string());
}

/// 去掉 `\\?\` 前缀（canonicalize 的扩展长度路径），让展示与后续导入都用普通路径。
/// 网络路径的 `\\?\UNC\` 需要还原成 `\\`，否则 UNC 会失效。
fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(rest.to_string());
    }
    p.to_path_buf()
}

fn same_path(a: &Path, b: &Path) -> bool {
    let ca = std::fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let cb = std::fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    ca.to_string_lossy()
        .eq_ignore_ascii_case(&cb.to_string_lossy())
}

/// 通过一次 PowerShell 子进程拿快捷方式与注册表候选。
fn from_powershell() -> Vec<(String, String)> {
    // 注意：这里只读，不写；输出一行一个 `source<TAB>path`，避免 JSON 编码问题。
    let script = r#"
$ErrorActionPreference = 'SilentlyContinue'
$out = New-Object System.Collections.ArrayList

$shell = New-Object -ComObject WScript.Shell
foreach ($d in @("$env:APPDATA\Microsoft\Windows\Start Menu\Programs", "$env:USERPROFILE\Desktop", "$env:PUBLIC\Desktop")) {
  if (Test-Path $d) {
    Get-ChildItem $d -Recurse -Filter *.lnk -ErrorAction SilentlyContinue | ForEach-Object {
      $sc = $shell.CreateShortcut($_.FullName)
      if ($sc.TargetPath -match 'DSH|Harness|DeepSeek') {
        $null = $out.Add("shortcut`t" + (Split-Path $sc.TargetPath -Parent))
      }
    }
  }
}

foreach ($k in @(
  'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
  'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
  'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*'
)) {
  Get-ItemProperty $k -ErrorAction SilentlyContinue | Where-Object { $_.DisplayName -match 'DSH|Harness|DeepSeek' } | ForEach-Object {
    if ($_.InstallLocation) { $null = $out.Add("registry`t" + $_.InstallLocation) }
    if ($_.DisplayIcon) {
      $icon = ($_.DisplayIcon -replace ',\d+$', '' -replace '"', '').Trim()
      if (Test-Path $icon) { $null = $out.Add("registry`t" + (Split-Path $icon -Parent)) }
    }
  }
}

$out | ForEach-Object { $_ }
"#;

    let exe = if which("pwsh").is_some() {
        "pwsh"
    } else {
        "powershell"
    };
    let output = std::process::Command::new(exe)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output();

    let Ok(out) = output else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .filter_map(|line| {
            let (source, path) = line.split_once('\t')?;
            let source = source.trim();
            let path = path.trim();
            if path.is_empty() {
                return None;
            }
            Some((path.to_string(), source.to_string()))
        })
        .collect()
}

fn which(name: &str) -> Option<PathBuf> {
    let exe = format!("{name}.exe");
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let p = dir.join(&exe);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// 常见安装路径（便宜，先扫一遍）。
fn from_common_paths() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(local).join("Programs"));
    }
    if let Some(pf) = std::env::var_os("ProgramFiles") {
        roots.push(PathBuf::from(pf));
    }
    if let Some(pf86) = std::env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(pf86));
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        roots.push(
            PathBuf::from(profile)
                .join("AppData")
                .join("Local")
                .join("Programs"),
        );
    }

    for root in &roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !(name.contains("dsh") || name.contains("harness") || name.contains("deepseek")) {
                continue;
            }
            if entry.path().is_dir() {
                out.push((entry.path().to_string_lossy().to_string(), "common".into()));
            }
        }
        // 直接命中固定名称
        for fixed in ["DSH NEXT", "DSH Desktop", "DSH-NEXT"] {
            let p = root.join(fixed);
            if p.is_dir() {
                out.push((p.to_string_lossy().to_string(), "common".into()));
            }
        }
    }
    out
}

/// 运行中的 DSH 进程反推安装目录。
fn from_processes() -> Vec<(String, String)> {
    let procs = crate::scan::scan_processes();
    let mut out = Vec::new();
    for p in procs.iter().filter(|p| p.is_dsh()) {
        let cmd = p.cmd.trim();
        let exe = if let Some(rest) = cmd.strip_prefix('"') {
            rest.split('"').next().unwrap_or("")
        } else {
            cmd.split(char::is_whitespace).next().unwrap_or("")
        };
        if exe
            .to_lowercase()
            .ends_with(versions::EXE_NAME.to_lowercase().as_str())
        {
            if let Some(parent) = Path::new(exe).parent() {
                if parent.join("resources").join("app.asar").is_file() {
                    out.push((parent.to_string_lossy().to_string(), "process".into()));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_verbatim_removes_win32_prefix() {
        let got = strip_verbatim(Path::new(r"\\?\E:\FXS\APP\DSH NEXT"));
        assert_eq!(got, PathBuf::from(r"E:\FXS\APP\DSH NEXT"));
    }

    #[test]
    fn strip_verbatim_keeps_plain_path() {
        let p = Path::new(r"E:\FXS\APP\DSH NEXT");
        assert_eq!(strip_verbatim(p), p.to_path_buf());
    }

    #[test]
    fn strip_verbatim_restores_unc() {
        // 网络路径必须还原成 \\server\share，否则导入会失效
        let got = strip_verbatim(Path::new(r"\\?\UNC\server\share\DSH"));
        assert_eq!(got, PathBuf::from(r"\\server\share\DSH"));
    }

    #[test]
    fn same_path_compares_case_insensitively() {
        let a = PathBuf::from(r"E:\FXS\APP\DSH NEXT");
        assert!(same_path(&a, Path::new(r"E:\FXS\APP\DSH NEXT")));
        assert!(!same_path(&a, Path::new(r"E:\FXS\APP\DSH")));
    }

    #[test]
    fn insert_unique_keeps_first_source() {
        let mut map = BTreeMap::new();
        insert_unique(&mut map, r"E:\FXS\APP\DSH NEXT", "shortcut");
        insert_unique(&mut map, r"E:\FXS\APP\DSH NEXT", "process");
        assert_eq!(map.len(), 1);
        assert_eq!(map.values().next().unwrap(), "shortcut");
    }
}
