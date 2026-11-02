//! 版本树扫描 / 登记 / 删除（设计说明 §4.3、§5.5）。
//!
//! 一个版本 = 一份完整安装树（`DSH NEXT.exe` + `resources\app.asar`）。
//! 登记默认**不复制**（300MB 起），而是在 `versions\<tag>` 建一个目录 junction 指过去；
//! 这样扫描规则、路径形态、删除规则都不变，磁盘占用为零。

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::asar;
use crate::errors::AppError;
use crate::model::VersionInfo;
use crate::util::now_iso;

pub const EXE_NAME: &str = "DSH NEXT.exe";

#[derive(Debug, Clone)]
pub struct VersionScanner {
    pub root: PathBuf,
}

impl VersionScanner {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// 遍历 `versions\*`，给出每一条的可用性。按 tag 倒序（新的在前）。
    pub fn scan(&self) -> Vec<VersionInfo> {
        let mut out = Vec::new();
        let Ok(entries) = fs::read_dir(&self.root) else {
            return out;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(tag) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
                continue;
            };
            if tag.starts_with('.') {
                continue;
            }
            out.push(inspect_dir(&path, &tag));
        }
        out.sort_by(|a, b| b.tag.cmp(&a.tag));
        out
    }

    pub fn valid_tags(&self) -> Vec<String> {
        self.scan()
            .into_iter()
            .filter(|v| v.valid)
            .map(|v| v.tag)
            .collect()
    }

    pub fn find(&self, tag: &str) -> Option<VersionInfo> {
        self.scan().into_iter().find(|v| v.tag == tag)
    }

    /// 登记一个已解包目录：校验 → 建 junction → 返回登记后的条目。
    pub fn register(&self, src: &Path) -> Result<VersionInfo, AppError> {
        let src = normalize_abs(src)?;
        let exe = src.join(EXE_NAME);
        let asar = src.join("resources").join("app.asar");
        if !exe.is_file() || !asar.is_file() {
            return Err(AppError::new(
                "E1004",
                format!("{} 缺少 DSH NEXT.exe 或 resources\\app.asar", src.display()),
            ));
        }

        let tag = asar::read_package(&asar)
            .map(|p| p.shell_version)
            .unwrap_or_else(|_| "?".to_string());
        let tag = if tag == "?" || !is_safe_tag(&tag) {
            src.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .filter(|n| is_safe_tag(n))
                .ok_or_else(|| {
                    AppError::new(
                        "E1004",
                        "无法从 package.json 或目录名确定版本号".to_string(),
                    )
                })?
        } else {
            tag
        };

        fs::create_dir_all(&self.root)
            .map_err(|e| AppError::new("E1004", format!("创建 versions 目录失败：{e}")))?;
        let link = self.root.join(&tag);

        if link.exists() || is_link(&link) {
            // 已登记：不覆盖，直接回报现状（幂等）
            return Ok(inspect_dir(&link, &tag));
        }

        // 源目录就在 versions 下：无需建链接
        if crate::util::path_eq(src.parent().unwrap_or(&src), &self.root) {
            return Ok(inspect_dir(&src, &tag));
        }

        create_junction(&link, &src)?;

        let info = inspect_dir(&link, &tag);
        if !info.valid {
            let _ = crate::versions::remove_link_only(&link);
            return Err(AppError::new(
                "E1004",
                format!(
                    "登记后校验失败：{}",
                    info.invalid_reason.unwrap_or_default()
                ),
            ));
        }
        Ok(info)
    }

    /// 删除版本登记：有实例引用则拒绝（设计 §5.5）。
    pub fn remove(&self, tag: &str, instances: &[crate::model::Instance]) -> Result<(), AppError> {
        let users: Vec<String> = instances
            .iter()
            .filter(|i| i.version == tag)
            .map(|i| format!("{}（{}）", i.name, i.id))
            .collect();
        if !users.is_empty() {
            return Err(AppError::new(
                "E1010",
                format!("{} 仍被 {} 引用", tag, users.join("、")),
            ));
        }
        let path = self.root.join(tag);
        if !path.exists() && !is_link(&path) {
            return Ok(());
        }
        if is_link(&path) {
            return remove_link_only(&path);
        }
        // 真目录：只有空目录才允许删，避免误删整棵安装树
        let empty = fs::read_dir(&path)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if !empty {
            return Err(AppError::new(
                "E1007",
                format!(
                    "{} 是真实目录且非空，启动器只删自己建的链接",
                    path.display()
                ),
            ));
        }
        fs::remove_dir(&path).map_err(|e| AppError::new("E1000", format!("删除版本目录失败：{e}")))
    }

    /// 首次运行时，若 versions 为空，就从本机找到的安装树登记一个。
    pub fn ensure_default_version(&self) -> Option<String> {
        if let Some(tag) = self.valid_tags().first().cloned() {
            return Some(tag);
        }
        for dir in install_candidates() {
            if !dir.join(EXE_NAME).is_file() || !dir.join("resources").join("app.asar").is_file() {
                continue;
            }
            if let Ok(info) = self.register(&dir) {
                if info.valid {
                    return Some(info.tag);
                }
            }
        }
        None
    }
}

/// 目录 → 版本条目（纯读取，不落盘）。
pub fn inspect_dir(dir: &Path, tag: &str) -> VersionInfo {
    let exe = dir.join(EXE_NAME);
    let asar = dir.join("resources").join("app.asar");
    let exe_ok = exe.is_file();
    let asar_ok = asar.is_file();
    let (shell, kernel, read_err) = if asar_ok {
        match asar::read_package(&asar) {
            Ok(pkg) => (pkg.shell_version, pkg.kernel_version, None),
            Err(err) => ("?".to_string(), "?".to_string(), Some(err)),
        }
    } else {
        (
            "?".to_string(),
            "?".to_string(),
            Some("缺少 resources\\app.asar".to_string()),
        )
    };

    let invalid_reason = if !exe_ok {
        Some(format!("缺少 {EXE_NAME}"))
    } else if !asar_ok {
        Some("缺少 resources\\app.asar".to_string())
    } else {
        None
    };

    let is_link_dir = is_link(dir);
    let link_target = if is_link_dir {
        fs::read_link(dir)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    } else {
        None
    };

    VersionInfo {
        tag: tag.to_string(),
        dir: dir.to_string_lossy().to_string(),
        exe: exe_ok.then(|| exe.to_string_lossy().to_string()),
        asar: asar_ok.then(|| asar.to_string_lossy().to_string()),
        shell_version: shell,
        kernel_version: kernel,
        valid: invalid_reason.is_none(),
        invalid_reason: invalid_reason.or(read_err),
        is_link: is_link_dir,
        link_target,
    }
}

/// 本机可能的安装树位置（先看正在运行的进程，再看常见路径）。
pub fn install_candidates() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(dir) = crate::util::env_path("DSH_INSTALL_DIR") {
        out.push(dir);
    }
    if let Some(dir) = install_dir_from_running() {
        out.push(dir);
    }
    if let Some(local) = crate::util::env_path("LOCALAPPDATA") {
        out.push(local.join("Programs").join("DSH NEXT"));
        out.push(local.join("Programs").join("DSH Desktop"));
    }
    if let Some(pf) = crate::util::env_path("ProgramFiles") {
        out.push(pf.join("DSH NEXT"));
        out.push(pf.join("DSH Desktop"));
    }
    out
}

fn install_dir_from_running() -> Option<PathBuf> {
    let procs = crate::scan::scan_processes();
    for p in procs.iter().filter(|p| p.is_dsh()) {
        let cmd = p.cmd.trim();
        let exe = if let Some(rest) = cmd.strip_prefix('"') {
            rest.split('"').next().unwrap_or("")
        } else {
            cmd.split(char::is_whitespace).next().unwrap_or("")
        };
        if exe.to_lowercase().ends_with(&EXE_NAME.to_lowercase()) {
            let parent = Path::new(exe).parent()?;
            if parent.join("resources").join("app.asar").is_file() {
                return Some(parent.to_path_buf());
            }
        }
    }
    None
}

// ---------------------------------------------------------------- Windows 目录链接

/// Windows junction（`mklink /J`）：不需要管理员权限，删除时不影响目标。
#[cfg(windows)]
fn create_junction(link: &Path, target: &Path) -> Result<(), AppError> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(target)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| AppError::new("E1004", format!("调用 mklink 失败：{e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(AppError::new(
            "E1004",
            format!(
                "创建目录链接失败（{} → {}）：{stderr}",
                link.display(),
                target.display()
            ),
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn create_junction(link: &Path, target: &Path) -> Result<(), AppError> {
    std::os::unix::fs::symlink(target, link)
        .map_err(|e| AppError::new("E1004", format!("创建符号链接失败：{e}")))
}

pub fn is_link(path: &Path) -> bool {
    if let Ok(md) = fs::symlink_metadata(path) {
        if md.file_type().is_symlink() {
            return true;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
            if md.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return true;
            }
        }
    }
    fs::read_link(path).is_ok()
}

/// 只删链接本身（`remove_dir` 对 junction 是安全的：不会进目标目录删内容）。
pub fn remove_link_only(path: &Path) -> Result<(), AppError> {
    fs::remove_dir(path).map_err(|e| AppError::new("E1000", format!("删除目录链接失败：{e}")))
}

fn is_safe_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 60
        && !tag.starts_with('.')
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
}

fn normalize_abs(p: &Path) -> Result<PathBuf, AppError> {
    if !p.is_absolute() {
        return Err(AppError::new(
            "E1012",
            format!("必须是绝对路径：{}", p.display()),
        ));
    }
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            Component::ParentDir => {
                return Err(AppError::new(
                    "E1012",
                    format!("路径不允许包含 ..：{}", p.display()),
                ))
            }
            other => out.push(other.as_os_str()),
        }
    }
    Ok(out)
}

/// 版本条目的更新时间戳（诊断用）。
pub fn stamped() -> String {
    now_iso()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Instance;

    fn fake_install(dir: &Path, version: &str, kernel: &str) {
        fs::create_dir_all(dir.join("resources")).unwrap();
        fs::write(dir.join(EXE_NAME), b"stub").unwrap();
        // 复用 asar 测试里的最小文件构造
        let body = format!(
            r#"{{"version":"{version}","dependencies":{{"@deepseek-ai/dsh":"{kernel}"}}}}"#
        );
        let index = format!(
            r#"{{"files":{{"package.json":{{"size":{},"offset":"0"}}}}}}"#,
            body.len()
        );
        let mut out = Vec::new();
        out.extend_from_slice(&4u32.to_le_bytes());
        out.extend_from_slice(&((8 + index.len()) as u32).to_le_bytes());
        out.extend_from_slice(&(index.len() as u32).to_le_bytes());
        out.extend_from_slice(&(index.len() as u32).to_le_bytes());
        out.extend_from_slice(index.as_bytes());
        out.extend_from_slice(body.as_bytes());
        fs::write(dir.join("resources").join("app.asar"), out).unwrap();
    }

    fn temp_root(tag: &str) -> PathBuf {
        let dir = crate::testutil::root(&format!("ver-{tag}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn scans_and_marks_invalid() {
        let root = temp_root("scan");
        let versions = root.join("versions");
        fake_install(&versions.join("2.0.21-next"), "2.0.21-next", "0.2.1-rc.4");
        fake_install(&versions.join("2.0.20-next"), "2.0.20-next", "0.2.0-rc.9");
        fs::create_dir_all(versions.join("broken")).unwrap();

        let list = VersionScanner::new(versions.clone()).scan();
        assert_eq!(list.len(), 3);
        let good = list.iter().find(|v| v.tag == "2.0.21-next").unwrap();
        assert!(good.valid);
        assert_eq!(good.shell_version, "2.0.21-next");
        assert_eq!(good.kernel_version, "0.2.1-rc.4");
        let bad = list.iter().find(|v| v.tag == "broken").unwrap();
        assert!(!bad.valid);
        assert!(bad
            .invalid_reason
            .as_ref()
            .unwrap()
            .contains("DSH NEXT.exe"));
        assert_eq!(list[0].tag, "broken", "倒序：b > 2");
        let _ = fs::remove_dir_all(root);
    }

    /// 本机沙箱（DSH 会话内）禁止普通进程创建重解析点，此时链接相关断言要跳过；
    /// 真实用户态双击运行时 mklink /J 是通的。
    fn junction_supported(root: &Path) -> bool {
        let target = root.join("j-target");
        let link = root.join("j-link");
        if fs::create_dir_all(&target).is_err() {
            return false;
        }
        match create_junction(&link, &target) {
            Ok(()) => {
                let ok = is_link(&link) && link.exists();
                let _ = remove_link_only(&link);
                let _ = fs::remove_dir(&target);
                ok
            }
            Err(_) => false,
        }
    }

    #[test]
    fn register_validates_and_is_idempotent() {
        let root = temp_root("reg");
        let src = root.join("install");
        fake_install(&src, "2.0.21-next", "0.2.1-rc.4");
        let versions = root.join("versions");
        let scanner = VersionScanner::new(versions.clone());

        // 目录不存在 / 缺 exe → E1004
        assert_eq!(
            scanner.register(&root.join("nope")).unwrap_err().code,
            "E1004"
        );
        let bare = root.join("bare");
        fs::create_dir_all(&bare).unwrap();
        assert_eq!(scanner.register(&bare).unwrap_err().code, "E1004");

        if !junction_supported(&root) {
            // 沙箱内无法建链接：登记必须给出明确的 E1004，而不是静默成功
            let err = scanner.register(&src).unwrap_err();
            assert_eq!(err.code, "E1004");
            assert!(err.detail.unwrap().contains("创建目录链接失败"));
            let _ = fs::remove_dir_all(root);
            return;
        }

        let info = scanner.register(&src).unwrap();
        assert_eq!(info.tag, "2.0.21-next");
        assert!(info.valid);
        assert!(is_link(&versions.join("2.0.21-next")));

        // 幂等：再登记一次不报错、tag 不变
        let again = scanner.register(&src).unwrap();
        assert_eq!(again.tag, info.tag);

        // 扫描能看到这一条
        let list = scanner.scan();
        assert!(list.iter().any(|v| v.tag == "2.0.21-next" && v.valid));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn remove_refuses_when_referenced() {
        let root = temp_root("rm");
        let versions = root.join("versions");
        // 真实目录（不是链接）+ 空的第二份，覆盖两条删除分支
        fake_install(&versions.join("2.0.21-next"), "2.0.21-next", "0.2.1-rc.4");
        fs::create_dir_all(versions.join("empty-tag")).unwrap();
        let scanner = VersionScanner::new(versions.clone());

        let inst = Instance::new("lab", "实验室", "2.0.21-next", r"C:\Users\demo\.dsh-lab");
        let err = scanner.remove("2.0.21-next", &[inst]).unwrap_err();
        assert_eq!(err.code, "E1010");

        // 空目录可删
        scanner.remove("empty-tag", &[]).unwrap();
        assert!(!versions.join("empty-tag").exists());
        // 非空真实目录拒绝删（保护安装树）
        assert_eq!(
            scanner.remove("2.0.21-next", &[]).unwrap_err().code,
            "E1007"
        );
        assert!(versions.join("2.0.21-next").join(EXE_NAME).is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn never_deletes_real_non_empty_dir() {
        let root = temp_root("real");
        let versions = root.join("versions");
        let real = versions.join("2.0.21-next");
        fake_install(&real, "2.0.21-next", "0.2.1-rc.4");
        let scanner = VersionScanner::new(versions.clone());
        assert_eq!(
            scanner.remove("2.0.21-next", &[]).unwrap_err().code,
            "E1007"
        );
        let _ = fs::remove_dir_all(root);
    }
}
