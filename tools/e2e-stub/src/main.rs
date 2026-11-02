//! DSH-NEXT 的替身进程，只做启动管道验收需要的三件事：
//! 1. 读指针文件拿到目标 home（真实 DSH 也只在启动时读一次）；
//! 2. 拉起一个 `--type=gpu-process --user-data-dir=<home>\electron-user-data` 子进程
//!    和一个 `--expose-internals ... <home>\profiles\desktop` 的 host 子进程；
//! 3. 活一段时间（父进程像真身一样可以先退出：`STUB_PARENT_EXIT_MS`）。
//!
//! 环境变量（都由启动器继承下来）：
//!   DSH_MULTI_POINTER     指针文件路径（读 home）
//!   STUB_START_DELAY_MS   主进程延迟多久才拉起子进程（默认 0，用于测 E1005 超时）
//!   STUB_PARENT_EXIT_MS   主进程存活多久后退出（默认 120000）
//!   STUB_EXIT_CODE        主进程直接退出码（设置后立即退出，用于测 E1006）
//!   STUB_CHILD_LIFE_MS    子进程存活时长（默认 120000）

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

fn env_num(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn read_home() -> PathBuf {
    let pointer = std::env::var("DSH_MULTI_POINTER").unwrap_or_default();
    if pointer.is_empty() {
        return PathBuf::from(r"C:\Users\Public\.dsh-stub-default");
    }
    let text = std::fs::read_to_string(&pointer).unwrap_or_default();
    // 极简 JSON 取值：定位 "home" → 冒号 → 第一个字符串字面量（把 \\ 还原成 \）
    if let Some(idx) = text.find("\"home\"") {
        let rest = &text[idx + 6..];
        if let Some(colon) = rest.find(':') {
            let after = &rest[colon + 1..];
            if let Some(q1) = after.find('"') {
                let body = &after[q1 + 1..];
                if let Some(q2) = body.find('"') {
                    let raw = &body[..q2];
                    let unescaped = raw.replace("\\\\", "\\");
                    if !unescaped.trim().is_empty() {
                        return PathBuf::from(unescaped.trim());
                    }
                }
            }
        }
    }
    PathBuf::from(r"C:\Users\Public\.dsh-stub-default")
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
