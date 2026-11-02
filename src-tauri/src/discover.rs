//! 自动发现 DSH-NEXT 安装目录。
//!
//! 四条渠道，按可靠性排序：
//! 1. 快捷方式（开始菜单 / 桌面 `.lnk` 的目标）—— 覆盖正常安装
//! 2. 注册表卸载项（`InstallLocation` / `DisplayIcon`）—— 覆盖走安装器的
//! 3. 常见路径（`%LOCALAPPDATA%\Programs`、`Program Files` 下同名目录）
//! 4. 运行中的进程（`DSH NEXT.exe` 的真实路径）
//!
//! 全部走 Windows 原生 API（COM `IShellLink` + `winreg`），**不依赖 PowerShell 或任何外部命令**。
//!
//! 早期实现用 PowerShell 子进程取结果，中文路径（`D:\豆包\…`、`E:\DSH-工具`）经控制台
//! 代码页往返会变成 `D:\?????-` 之类的乱码，所有候选都被判为无效目录——见
//! `docs/experience/07-扫描乱码.md`。原生 API 直接读 UTF-16，没有编码环节。

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

    #[cfg(windows)]
    {
        for (path, source) in from_shortcuts() {
            insert_unique(&mut found, &path, &source);
        }
        for (path, source) in from_registry() {
            insert_unique(&mut found, &path, &source);
        }
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

    // 有效的在前，其次未登记、路径稳定排序
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

// ---------------- 1) 快捷方式 ----------------

#[cfg(windows)]
fn from_shortcuts() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(
            PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        dirs.push(PathBuf::from(profile).join("Desktop"));
    }
    if let Some(public) = std::env::var_os("PUBLIC") {
        dirs.push(PathBuf::from(public).join("Desktop"));
    }
    for dir in dirs {
        walk_shortcuts(&dir, &mut out, 0);
    }
    out
}

#[cfg(windows)]
fn walk_shortcuts(dir: &Path, out: &mut Vec<(String, String)>, depth: usize) {
    if depth > 5 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_shortcuts(&path, out, depth + 1);
            continue;
        }
        let is_lnk = path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("lnk"))
            .unwrap_or(false);
        if !is_lnk {
            continue;
        }
        let Some(target) = shortcut_target(&path) else {
            continue;
        };
        let lower = target.to_lowercase();
        if !(lower.contains("dsh") || lower.contains("harness") || lower.contains("deepseek")) {
            continue;
        }
        if let Some(parent) = Path::new(&target).parent() {
            out.push((parent.to_string_lossy().to_string(), "shortcut".into()));
        }
    }
}

/// 解析 `.lnk` 的目标路径。`SLGP_RAWPATH` 不做存在性解析，目标被移动后也能拿到原路径。
#[cfg(windows)]
fn shortcut_target(lnk: &Path) -> Option<String> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink, SLGP_RAWPATH};

    unsafe {
        // 同线程重复初始化返回 S_FALSE，套间模型不同返回 RPC_E_CHANGED_MODE——都可以继续用
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        let persist: IPersistFile = link.cast().ok()?;
        persist
            .Load(&HSTRING::from(lnk.as_os_str()), STGM_READ)
            .ok()?;

        let mut buf = [0u16; 1024];
        link.GetPath(
            &mut buf,
            std::ptr::null_mut::<windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW>(),
            SLGP_RAWPATH.0 as u32,
        )
        .ok()?;
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        if len == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}

// ---------------- 2) 注册表卸载项 ----------------

#[cfg(windows)]
fn from_registry() -> Vec<(String, String)> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let mut out = Vec::new();
    let uninstall = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
    let uninstall_wow = r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
    let locations = [
        (HKEY_LOCAL_MACHINE, uninstall),
        (HKEY_LOCAL_MACHINE, uninstall_wow),
        (HKEY_CURRENT_USER, uninstall),
    ];

    for (hive, sub) in locations {
        let Ok(base) = RegKey::predef(hive).open_subkey_with_flags(sub, KEY_READ) else {
            continue;
        };
        for name in base.enum_keys().flatten() {
            let Ok(key) = base.open_subkey_with_flags(&name, KEY_READ) else {
                continue;
            };
            let display: String = key.get_value("DisplayName").unwrap_or_default();
            let lower = display.to_lowercase();
            if !(lower.contains("dsh") || lower.contains("harness") || lower.contains("deepseek")) {
                continue;
            }
            if let Ok(loc) = key.get_value::<String, _>("InstallLocation") {
                if !loc.trim().is_empty() {
                    out.push((loc, "registry".into()));
                }
            }
            if let Ok(icon) = key.get_value::<String, _>("DisplayIcon") {
                if let Some(parent) = display_icon_parent(&icon) {
                    out.push((parent, "registry".into()));
                }
            }
        }
    }
    out
}

/// 从 `DisplayIcon`（`"C:\…\DSH NEXT.exe",0` 或裸路径）取所在目录。
fn display_icon_parent(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let unquoted = trimmed.trim_matches('"');
    // 去掉尾部的 `,<index>`
    let no_index = match unquoted.rsplit_once(',') {
        Some((head, tail)) if tail.trim().parse::<i32>().is_ok() => head.trim(),
        _ => unquoted,
    };
    let cleaned = no_index.trim().trim_matches('"');
    if cleaned.is_empty() {
        return None;
    }
    let parent = Path::new(cleaned).parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    Some(parent.to_string_lossy().to_string())
}

// ---------------- 3) 常见路径 ----------------

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
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if !(name.contains("dsh") || name.contains("harness") || name.contains("deepseek"))
                {
                    continue;
                }
                if entry.path().is_dir() {
                    out.push((entry.path().to_string_lossy().to_string(), "common".into()));
                }
            }
        }
        // 固定名称（目录名不含关键词时也能命中）
        for fixed in ["DSH NEXT", "DSH Desktop", "DSH-NEXT", "DeepSeek Harness"] {
            let p = root.join(fixed);
            if p.is_dir() {
                out.push((p.to_string_lossy().to_string(), "common".into()));
            }
        }
    }
    out
}

// ---------------- 4) 运行中的进程 ----------------

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
        let got = strip_verbatim(Path::new(r"\\?\C:\Program Files\DSH NEXT"));
        assert_eq!(got, PathBuf::from(r"C:\Program Files\DSH NEXT"));
    }

    #[test]
    fn strip_verbatim_keeps_plain_path() {
        let p = Path::new(r"C:\Program Files\DSH NEXT");
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
        let a = PathBuf::from(r"C:\Program Files\DSH NEXT");
        assert!(same_path(&a, Path::new(r"C:\Program Files\DSH NEXT")));
        assert!(!same_path(&a, Path::new(r"C:\Program Files\DSH")));
    }

    #[test]
    fn insert_unique_keeps_first_source() {
        let mut map = BTreeMap::new();
        insert_unique(&mut map, r"C:\Program Files\DSH NEXT", "shortcut");
        insert_unique(&mut map, r"C:\Program Files\DSH NEXT", "process");
        assert_eq!(map.len(), 1);
        assert_eq!(map.values().next().unwrap(), "shortcut");
    }

    #[test]
    fn display_icon_parent_strips_quotes_and_index() {
        assert_eq!(
            display_icon_parent(r#""C:\Program Files\DSH NEXT\DSH NEXT.exe",0"#).as_deref(),
            Some(r"C:\Program Files\DSH NEXT")
        );
        assert_eq!(
            display_icon_parent(r"D:\Apps\DSH\DSH NEXT.exe").as_deref(),
            Some(r"D:\Apps\DSH")
        );
        assert_eq!(display_icon_parent(""), None);
        assert_eq!(display_icon_parent(",0"), None);
    }

    #[test]
    fn chinese_paths_are_preserved() {
        // 回归用例：中文路径必须原样保留（早先经 PowerShell 控制台会变成 D:\?????-）
        let mut map = BTreeMap::new();
        insert_unique(&mut map, r"D:\豆包\Dsh", "registry");
        let keys: Vec<String> = map
            .keys()
            .map(|k| k.to_string_lossy().to_string())
            .collect();
        assert!(keys.iter().any(|k| k.contains("豆包")), "got {keys:?}");
    }
}
