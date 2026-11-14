//! DSH-NEXT 的替身进程，只做启动管道验收需要的三件事：
//! 1. 读指针文件拿到目标 home（真实 DSH 也只在启动时读一次）；
//! 2. 拉起一个 `--type=gpu-process --user-data-dir=<home>\electron-user-data` 子进程
//!    和一个 `--expose-internals ... <home>\profiles\desktop` 的 host 子进程；
//! 3. 活一段时间（父进程像真身一样可以先退出：`STUB_PARENT_EXIT_MS`）。
//!
//! 环境变量（都由启动器继承下来）：
//!   DSH_MULTI_POINTER     指针文件路径（测试夹具显式覆盖；优先于下面两项）
//!   APPDATA               `%APPDATA%`；没给 DSH_MULTI_POINTER 时从
//!                         `<APPDATA>\DSH NEXT\home\desktop-next-location.json` 读（与真身一致）
//!   DSH_DESKTOP_NEXT_HOME 上游的兜底变量：没有定位文件时用它（隔离模式就是走这条）
//!   STUB_START_DELAY_MS   主进程延迟多久才拉起子进程（默认 0，用于测 E1005 超时）
//!   STUB_PARENT_EXIT_MS   主进程存活多久后退出（默认 120000）
//!   STUB_EXIT_CODE        主进程直接退出码（设置后立即退出，用于测 E1006）
//!   STUB_CHILD_LIFE_MS    子进程存活时长（默认 120000）
//!
//! 解析顺序按真身 `lib/main.js` 的 `readDataDirectory`：
//! 定位文件（应用内选择）> `DSH_DESKTOP_NEXT_HOME` > 默认 `~/.dsh`。

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

fn env_num(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

/// 解析 home：定位文件 > `DSH_DESKTOP_NEXT_HOME` > 默认。
///
/// 定位文件的位置与真身一致：打包后的根是 `<appData>\DSH NEXT\home`
/// （真身 `dataDirectoryLocationRoot`），而 `appData` 就是 `%APPDATA%`。
/// `DSH_MULTI_POINTER` 仍然是显式夹具覆盖（e2e 把指针放在 workspace 里）。
fn read_home() -> PathBuf {
    let pointer = resolve_pointer_path();
    if let Some(home) = read_pointer(&pointer) {
        return home;
    }
    if let Ok(home) = std::env::var("DSH_DESKTOP_NEXT_HOME") {
        let home = home.trim();
        if !home.is_empty() {
            return PathBuf::from(home);
        }
    }
    let profile = std::env::var("USERPROFILE").unwrap_or_else(|_| r"C:\Users\Public".to_string());
    PathBuf::from(profile).join(".dsh")
}

fn resolve_pointer_path() -> String {
    if let Ok(explicit) = std::env::var("DSH_MULTI_POINTER") {
        if !explicit.trim().is_empty() {
            return explicit;
        }
    }
    match std::env::var("APPDATA") {
        Ok(appdata) if !appdata.trim().is_empty() => PathBuf::from(appdata)
            .join("DSH NEXT")
            .join("home")
            .join("desktop-next-location.json")
            .to_string_lossy()
            .to_string(),
        _ => String::new(),
    }
}

fn read_pointer(pointer: &str) -> Option<PathBuf> {
    if pointer.is_empty() {
        return None;
    }
    let text = std::fs::read_to_string(pointer).ok()?;
    // 极简 JSON 取值：定位 "home" → 冒号 → 第一个字符串字面量（把 \\ 还原成 \）
    let idx = text.find("\"home\"")?;
    let rest = &text[idx + 6..];
    let colon = rest.find(':')?;
    let after = &rest[colon + 1..];
    let q1 = after.find('"')?;
    let body = &after[q1 + 1..];
    let q2 = body.find('"')?;
    let raw = &body[..q2];
    let unescaped = raw.replace("\\\\", "\\");
    if unescaped.trim().is_empty() {
        None
    } else {
        Some(PathBuf::from(unescaped.trim()))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_child = args
        .iter()
        .any(|a| a.starts_with("--type=") || a == "--expose-internals");

    if is_child {
        std::thread::sleep(Duration::from_millis(env_num("STUB_CHILD_LIFE_MS", 120_000)));
        return;
    }

    if let Ok(code) = std::env::var("STUB_EXIT_CODE") {
        if let Ok(code) = code.trim().parse::<i32>() {
            std::process::exit(code);
        }
    }

    let delay = env_num("STUB_START_DELAY_MS", 0);
    if delay > 0 {
        std::thread::sleep(Duration::from_millis(delay));
    }

    let home = read_home();
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("e2e-stub.exe"));
    let user_data = format!("{}\\electron-user-data", home.display());
    let profiles = format!("{}\\profiles\\desktop", home.display());
    let asar = format!("{}", exe.with_file_name("app.asar").display());

    let _ = Command::new(&exe)
        .arg("--type=gpu-process")
        .arg(format!("--user-data-dir=\"{}\"", user_data))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    let _ = Command::new(&exe)
        .arg("--expose-internals")
        .arg(&asar)
        .arg(&asar)
        .arg(&profiles)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    std::thread::sleep(Duration::from_millis(env_num("STUB_PARENT_EXIT_MS", 120_000)));
}
