//! 进程扫描（设计说明 §5.3）。
//!
//! 规则（按顺序尝试，只认实际命中，不缓存）：
//! 1. 进程名 `DSH NEXT.exe`；
//! 2. 命令行含 `--user-data-dir=<home>\electron-user-data` → 归入该 home；
//! 3. 命令行含 `--expose-internals` 且含 `<home>\profiles\` → 该进程是 host。
//!
//! 主进程命令行里**没有** `--user-data-dir`（阶段 0 实测），所以主进程 pid 靠
//! “从带标记的子进程往上走，找到第一个不带 `--type=` / `--expose-internals` 的 DSH 进程”。

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use crate::model::RunningInfo;

pub const TARGET_EXE: &str = "DSH NEXT.exe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent: Option<u32>,
    pub name: String,
    pub cmd: String,
    pub start_time: u64,
}

impl ProcInfo {
    pub fn is_dsh(&self) -> bool {
        self.name.eq_ignore_ascii_case(TARGET_EXE)
    }

    /// 真正拥有窗口的主进程：DSH 进程、没有 `--type=`、不是 host、也不是 runner。
    pub fn is_shell(&self) -> bool {
        self.is_dsh()
            && !self.cmd.contains("--type=")
            && !self.cmd.contains("--expose-internals")
            && !self.cmd.contains("runner.js")
    }

    pub fn is_host(&self) -> bool {
        self.is_dsh() && self.cmd.contains("--expose-internals")
    }
}

/// 枚举进程（每次界面刷新重扫）。
///
/// 注意：必须用 `ProcessRefreshKind::everything()`。sysinfo 0.33 在 Windows 上默认的刷新
/// 级别**不读命令行**（`cmd()` 恒为空），只看默认值会把所有 home 判成“没在跑”。
pub fn scan_processes() -> Vec<ProcInfo> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );
    sys.processes()
        .iter()
        .map(|(pid, proc_)| {
            let cmd = proc_
                .cmd()
                .iter()
                .map(|c| c.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(" ");
            ProcInfo {
                pid: pid.as_u32(),
                parent: proc_.parent().map(|p| p.as_u32()),
                name: proc_.name().to_string_lossy().to_string(),
                cmd: if cmd.is_empty() {
                    // 有些进程拿不到命令行；用 exe 路径兜底，至少能判名字
                    proc_
                        .exe()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default()
                } else {
                    cmd
                },
                start_time: proc_.start_time(),
            }
        })
        .collect()
}

/// 扫描当前运行中的实例；同一 home 只报一条。
pub fn scan_running() -> Vec<RunningInfo> {
    scan_running_from(&scan_processes())
}

pub fn scan_running_from(procs: &[ProcInfo]) -> Vec<RunningInfo> {
    let mut homes: Vec<(String, u32, Option<u32>, bool)> = Vec::new(); // home, 首个标记 pid, host pid, 安全模式

    for p in procs.iter().filter(|p| p.is_dsh()) {
        let home = parse_user_data_home(&p.cmd).or_else(|| {
            if p.is_host() {
                parse_host_home(&p.cmd)
            } else {
                None
            }
        });
        let Some(home) = home else { continue };
        let host_pid = if p.is_host() { Some(p.pid) } else { None };
        // 安全模式判据（取自 DSH-NEXT 自身）：命令行带 `--next-safe-mode`，
        // 或 userData 落在 `electron-user-data\safe-mode`（安全模式会换这个目录）。
        let safe = p.cmd.contains("--next-safe-mode")
            || p.cmd.contains("electron-user-data\\safe-mode")
            || p.cmd.contains("electron-user-data/safe-mode");
        match homes.iter_mut().find(|(h, _, _, _)| {
            crate::util::path_eq(std::path::Path::new(h), std::path::Path::new(&home))
        }) {
            Some(slot) => {
                if slot.2.is_none() && host_pid.is_some() {
                    slot.2 = host_pid;
                }
                slot.3 = slot.3 || safe;
            }
            None => homes.push((home, p.pid, host_pid, safe)),
        }
    }

    let mut out = Vec::new();
    for (home, marker_pid, host_pid, safe_mode) in homes {
        let shell_pid = find_shell_pid(procs, marker_pid)
            .or_else(|| find_shell_pid(procs, host_pid.unwrap_or(marker_pid)))
            .unwrap_or(marker_pid);
        let started_at = procs
            .iter()
            .find(|p| p.pid == shell_pid)
            .map(|p| p.start_time)
            .unwrap_or(0);
        out.push(RunningInfo {
            home,
            shell_pid,
            host_pid,
            started_at,
            safe_mode,
        });
    }
    out.sort_by_key(|a| a.home.to_lowercase());
    out
}

/// 从任意一个带标记的 DSH 子进程往上走，找到主进程。
pub fn find_shell_pid(procs: &[ProcInfo], start_pid: u32) -> Option<u32> {
    let mut cur = start_pid;
    for _ in 0..16 {
        let p = procs.iter().find(|p| p.pid == cur)?;
        if p.is_shell() {
            return Some(p.pid);
        }
        {
            let parent = p.parent?;
            let parent_is_dsh = procs.iter().any(|q| q.pid == parent && q.is_dsh());
            if !parent_is_dsh {
                // 起点自己也可能是主进程（拿不到子进程标记时）
                return if p.is_dsh() { Some(p.pid) } else { None };
            }
            cur = parent;
        }
    }
    None
}

/// 该 home 归属的全部 DSH 进程（主进程 + 子进程），用于强制结束整组。
pub fn pids_for_home(home: &str) -> Vec<u32> {
    let procs = scan_processes();
    let mut out = Vec::new();
    for p in procs.iter().filter(|p| p.is_dsh()) {
        let h = parse_user_data_home(&p.cmd).or_else(|| {
            if p.is_host() {
                parse_host_home(&p.cmd)
            } else {
                None
            }
        });
        if let Some(h) = h {
            if crate::util::path_eq(std::path::Path::new(&h), std::path::Path::new(home)) {
                out.push(p.pid);
            }
        }
    }
    // 主进程命令行里没有 home，靠“子进程的父进程”补进来
    if let Some(shell) = find_shell_pid(&procs, out.first().copied().unwrap_or(0)) {
        if !out.contains(&shell) {
            out.push(shell);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

pub fn is_home_running(home: &str) -> Option<RunningInfo> {
    scan_running()
        .into_iter()
        .find(|r| crate::util::path_eq(std::path::Path::new(&r.home), std::path::Path::new(home)))
}

/// 把 Windows 命令行里的转义引号 `\"` 归一成 `"`。
fn normalize_cmdline(cmd: &str) -> String {
    if cmd.contains("\\\"") {
        cmd.replace("\\\"", "\"")
    } else {
        cmd.to_string()
    }
}

/// `--user-data-dir="<home>\electron-user-data"` → `<home>`
///
/// 注意 Windows 命令行里内嵌引号会被转义成 `\"`（Electron 自己就是这么传的），
/// 所以先把 `\"` 归一成 `"`，再按普通引号规则取值。
pub fn parse_user_data_home(cmd: &str) -> Option<String> {
    let cmd = normalize_cmdline(cmd);
    let lower = cmd.to_lowercase();
    let marker = "--user-data-dir=";
    let idx = lower.find(marker)?;
    let rest = &cmd[idx + marker.len()..];
    let value = if let Some(stripped) = rest.strip_prefix('"') {
        let end = stripped.find('"')?;
        &stripped[..end]
    } else {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        &rest[..end]
    };
    let trimmed = value.trim_end_matches(['\\', '/']);
    let lower_value = trimmed.to_lowercase();
    // 安全模式会把 userData 换成 <home>\electron-user-data\safe-mode，两种都要认
    const SUFFIX_SAFE: &str = "\\electron-user-data\\safe-mode";
    const SUFFIX: &str = "\\electron-user-data";
    let suffix = if lower_value.ends_with(SUFFIX_SAFE) {
        SUFFIX_SAFE
    } else if lower_value.ends_with(SUFFIX) {
        SUFFIX
    } else {
        return None;
    };
    let home = &trimmed[..trimmed.len() - suffix.len()];
    if home.is_empty() {
        None
    } else {
        Some(home.to_string())
    }
}

/// host 进程命令行里 `<home>\profiles\<profile>` → `<home>`
///
/// home 路径里可能有空格（本机就叫 `DSH NEXT Multi-Launcher`），所以不能“按最后一个空格切”；
/// 从 marker 往前找**以盘符开头的 token**，它前面必须是空格 / 引号 / 行首。
pub fn parse_host_home(cmd: &str) -> Option<String> {
    let cmd = normalize_cmdline(cmd);
    let lower = cmd.to_lowercase();
    let marker = "\\profiles\\";
    let idx = lower.find(marker)?;
    let head = &cmd[..idx];
    let bytes = head.as_bytes();

    let mut start = None;
    for i in (1..bytes.len()).rev() {
        if bytes[i] == b':' && bytes[i - 1].is_ascii_alphabetic() {
            if i + 1 >= bytes.len() || !(bytes[i + 1] == b'\\' || bytes[i + 1] == b'/') {
                continue;
            }
            let boundary = if i <= 1 {
                true
            } else {
                matches!(bytes[i - 2], b' ' | b'"' | b'\'' | b'\t')
            };
            if boundary {
                start = Some(i - 1);
                break;
            }
        }
    }
    // UNC（\\server\share\...）也算绝对路径
    if start.is_none() {
        if let Some(pos) = head.find("\\\\") {
            let boundary = pos == 0 || matches!(bytes[pos - 1], b' ' | b'"' | b'\'' | b'\t');
            if boundary {
                start = Some(pos);
            }
        }
    }

    let start = start?;
    let home = head[start..].trim().trim_start_matches('"');
    if home.is_empty() {
        return None;
    }
    Some(home.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: u32, parent: Option<u32>, cmd: &str) -> ProcInfo {
        ProcInfo {
            pid,
            parent,
            name: TARGET_EXE.to_string(),
            cmd: cmd.to_string(),
            start_time: 1_700_000_000 + pid as u64,
        }
    }

    // 取自阶段 0 的实测命令行
    const GPU_CMD: &str = r#""E:\FXS\APP\DSH NEXT\DSH NEXT.exe" --type=gpu-process --user-data-dir="C:\Users\FXS\.dsh\electron-user-data" --gpu-preferences=WAAAAAAAAADgAAAEAAAAAAAAAAAAAGAAQAAAAAAAAAADAAAAAAAAADg"#;
    const CRASHPAD_CMD: &str = r#""E:\FXS\APP\DSH NEXT\DSH NEXT.exe" --type=crashpad-handler --user-data-dir=C:\Users\FXS\.dsh\electron-user-data /prefetch:4 --no-rate-limit"#;
    const HOST_CMD: &str = r#""E:\FXS\APP\DSH NEXT\DSH NEXT.exe" --expose-internals "E:\FXS\APP\DSH NEXT\resources\app.asar\lib\host.js" "E:\FXS\APP\DSH NEXT\resources\app.asar" C:\Users\FXS\.dsh\profiles\desktop "E:\FXS\APP\DSH NEXT\resources\runtime\x64\primary-runtime""#;
    const MAIN_CMD: &str = r#""E:\FXS\APP\DSH NEXT\DSH NEXT.exe""#;

    // 未加引号、且 home 路径本身带空格的形态（替身进程 / 某些 shell 传参就是这样）
    const HOST_CMD_UNQUOTED: &str = r"E:\FXS\Data\DSH000\DSH-NEXT Multi-Launcher\src-tauri\target\e2e\versions\2.0.99-next\DSH NEXT.exe --expose-internals E:\FXS\Data\DSH000\DSH-NEXT Multi-Launcher\src-tauri\target\e2e\versions\2.0.99-next\app.asar E:\FXS\Data\DSH000\DSH-NEXT Multi-Launcher\src-tauri\target\e2e\homes\lab1\profiles\desktop";

    #[test]
    fn detects_safe_mode_from_cmdline() {
        // 安全模式：命令行带 --next-safe-mode，且 userData 落在 ...\electron-user-data\safe-mode
        const SAFE_MAIN: &str = r#""E:\FXS\APP\DSH NEXT\DSH NEXT.exe" --next-safe-mode"#;
        const SAFE_GPU: &str = r#""E:\FXS\APP\DSH NEXT\DSH NEXT.exe" --type=gpu-process --user-data-dir="C:\Users\FXS\.dsh\electron-user-data\safe-mode""#;

        let normal = vec![proc(10, None, MAIN_CMD), proc(11, Some(10), GPU_CMD)];
        assert!(
            scan_running_from(&normal).iter().all(|r| !r.safe_mode),
            "普通启动不应被判成安全模式"
        );

        let safe = vec![proc(20, None, SAFE_MAIN), proc(21, Some(20), SAFE_GPU)];
        let running = scan_running_from(&safe);
        assert_eq!(running.len(), 1);
        assert!(running[0].safe_mode, "带 --next-safe-mode 应判为安全模式");
    }

    #[test]
    fn parses_host_home_with_spaces_in_path() {
        assert_eq!(
            parse_host_home(HOST_CMD_UNQUOTED).unwrap(),
            r"E:\FXS\Data\DSH000\DSH-NEXT Multi-Launcher\src-tauri\target\e2e\homes\lab1"
        );
    }

    #[test]
    fn parses_quoted_user_data_dir() {
        assert_eq!(parse_user_data_home(GPU_CMD).unwrap(), r"C:\Users\FXS\.dsh");
    }

    #[test]
    fn parses_unquoted_user_data_dir() {
        assert_eq!(
            parse_user_data_home(CRASHPAD_CMD).unwrap(),
            r"C:\Users\FXS\.dsh"
        );
    }

    #[test]
    fn rejects_non_electron_user_data() {
        assert!(parse_user_data_home(MAIN_CMD).is_none());
        assert!(parse_user_data_home("--user-data-dir=C:\\Temp\\foo").is_none());
    }

    #[test]
    fn parses_host_home() {
        assert_eq!(parse_host_home(HOST_CMD).unwrap(), r"C:\Users\FXS\.dsh");
    }

    #[test]
    fn classifies_roles() {
        let main = proc(100, Some(1), MAIN_CMD);
        let host = proc(200, Some(100), HOST_CMD);
        let gpu = proc(300, Some(100), GPU_CMD);
        assert!(main.is_shell() && !main.is_host());
        assert!(host.is_host() && !host.is_shell());
        assert!(!gpu.is_shell() && !gpu.is_host());
    }

    #[test]
    fn parses_escaped_quote_form() {
        // Windows 命令行里 Electron 传的是 --user-data-dir=\"C:\...\electron-user-data\"
        let cmd = r#""E:\x\DSH NEXT.exe" --type=gpu-process "--user-data-dir=\"C:\Users\FXS\.dsh-lab\electron-user-data\"""#;
        assert_eq!(parse_user_data_home(cmd).unwrap(), r"C:\Users\FXS\.dsh-lab");
    }

    #[test]
    fn groups_by_home_and_finds_shell() {
        let procs = vec![
            proc(100, Some(1), MAIN_CMD),
            proc(200, Some(100), HOST_CMD),
            proc(300, Some(100), GPU_CMD),
            proc(400, Some(100), CRASHPAD_CMD),
        ];
        let running = scan_running_from(&procs);
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].home, r"C:\Users\FXS\.dsh");
        assert_eq!(running[0].shell_pid, 100);
        assert_eq!(running[0].host_pid, Some(200));
        assert_eq!(running[0].started_at, 1_700_000_100);
    }

    #[test]
    fn two_homes_two_instances() {
        let gpu_b = GPU_CMD.replace(".dsh", ".dsh-b");
        let host_b = HOST_CMD.replace(".dsh", ".dsh-b");
        let procs = vec![
            proc(100, Some(1), MAIN_CMD),
            proc(110, Some(1), MAIN_CMD),
            proc(200, Some(100), HOST_CMD),
            proc(210, Some(110), &host_b),
            proc(300, Some(100), GPU_CMD),
            proc(310, Some(110), &gpu_b),
        ];
        let running = scan_running_from(&procs);
        assert_eq!(running.len(), 2);
        let homes: Vec<&str> = running.iter().map(|r| r.home.as_str()).collect();
        assert!(homes.contains(&r"C:\Users\FXS\.dsh"));
        assert!(homes.contains(&r"C:\Users\FXS\.dsh-b"));
        let b = running.iter().find(|r| r.home.ends_with(".dsh-b")).unwrap();
        assert_eq!(b.shell_pid, 110);
    }

    #[test]
    fn ignores_unrelated_processes() {
        let procs = vec![ProcInfo {
            pid: 900,
            parent: None,
            name: "explorer.exe".into(),
            cmd: "C:\\Windows\\explorer.exe".into(),
            start_time: 0,
        }];
        assert!(scan_running_from(&procs).is_empty());
    }
}
