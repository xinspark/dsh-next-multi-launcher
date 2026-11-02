//! DSH-NEXT 应用多开启动器 —— 核心库。

pub mod asar;
pub mod cli;
pub mod commands;
pub mod discover;
pub mod errors;
pub mod guard;
pub mod launcher;
pub mod model;
pub mod perm;
pub mod pointer;
pub mod scan;
pub mod store;
pub mod util;
pub mod versions;

#[cfg(test)]
mod testutil;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{Emitter, Manager};

use crate::launcher::{Launcher, ProgressSink};
use crate::model::{Config, LaunchStatus, VersionInfo};
use crate::pointer::Pointer;
use crate::store::Store;
use crate::util::Logger;
use crate::versions::VersionScanner;

/// 版本扫描缓存：默认 5 秒内复用，避免每次快照都去读 5MB 的 asar 索引。
struct VersionCache {
    at: Option<Instant>,
    list: Vec<VersionInfo>,
}

pub struct AppState {
    pub store: Store,
    pub versions: VersionScanner,
    pub versions_dir: PathBuf,
    pub pointer: Pointer,
    pub config: Arc<Mutex<Config>>,
    pub launcher: Mutex<Launcher>,
    logger: Logger,
    cache: Mutex<VersionCache>,
}

impl AppState {
    pub fn data_dir(&self) -> &Path {
        &self.store.data_dir
    }

    pub fn logger(&self) -> Logger {
        self.logger.clone()
    }

    /// 当前版本列表（带缓存）。
    pub fn versions_now(&self) -> Vec<VersionInfo> {
        let mut cache = self.cache.lock().unwrap();
        let fresh = cache
            .at
            .map(|at| at.elapsed() < Duration::from_secs(5))
            .unwrap_or(false);
        if fresh {
            return cache.list.clone();
        }
        let list = self.versions.scan();
        cache.at = Some(Instant::now());
        cache.list = list.clone();
        list
    }

    pub fn invalidate_versions(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.at = None;
        cache.list.clear();
    }
}

/// 把启动进度推给前端。
struct EventSink {
    app: tauri::AppHandle,
}

impl ProgressSink for EventSink {
    fn progress(&self, status: &LaunchStatus) {
        let _ = self.app.emit("launch:progress", status);
    }
}

fn build_state(app: tauri::AppHandle, logger: Logger) -> AppState {
    let store = Store::from_env();
    let _ = store.ensure_dirs();
    let cfg = store.load_config().unwrap_or_else(|err| {
        logger.error(format!("读取 app.json 失败，改用默认值：{err}"));
        Config::with_default_home(store::default_home_string())
    });
    logger.info(format!(
        "启动器 v{} 启动：data={} versions={}",
        env!("CARGO_PKG_VERSION"),
        store.data_dir.display(),
        store.versions_dir.display()
    ));

    let versions_dir = store.versions_dir.clone();
    let versions = VersionScanner::new(versions_dir.clone());
    match versions.ensure_default_version() {
        Some(tag) => logger.info(format!("可用版本：{tag}")),
        None => logger.warn("没有可用版本，先在界面里导入一份解包目录"),
    }
    let first_version = versions.valid_tags().first().cloned().unwrap_or_default();
    if let Some(inst) = store.ensure_default_instance(&cfg, &first_version) {
        logger.info(format!("首次运行：已为默认数据目录建实例 {}", inst.id));
    }

    let config = Arc::new(Mutex::new(cfg));
    let pointer = Pointer::default_path();
    let launcher = Launcher::start(
        Arc::new(EventSink { app }),
        store.clone(),
        pointer.clone(),
        config.clone(),
        logger.clone(),
    );

    AppState {
        store,
        versions,
        versions_dir,
        pointer,
        config,
        launcher: Mutex::new(launcher),
        logger,
        cache: Mutex::new(VersionCache {
            at: None,
            list: Vec::new(),
        }),
    }
}

/// 启动早期就要有日志：窗口创建失败时也留痕（否则 GUI 子系统下就是“静默退出”）。
fn startup_logger() -> Logger {
    let candidates = [
        Store::from_env().logs_dir(),
        std::env::temp_dir().join("DSH-NEXT-Multi").join("logs"),
    ];
    for dir in candidates {
        if std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let probe = dir.join(".write-probe");
        if std::fs::write(&probe, b"1").is_ok() {
            let _ = std::fs::remove_file(&probe);
            return Logger::new(dir);
        }
    }
    Logger::new(std::env::temp_dir())
}

#[cfg(windows)]
unsafe extern "system" {
    fn MessageBoxW(
        hwnd: *mut core::ffi::c_void,
        text: *const u16,
        caption: *const u16,
        flags: u32,
    ) -> i32;
}

fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 致命启动错误：记日志 + 写兜底文件 + 弹框，然后退出。
/// 不弹框的场合（自动化）设 `DSH_MULTI_NO_DIALOG=1`。
fn fatal(logger: &Logger, summary: &str, detail: &str) -> ! {
    logger.error(format!("{summary}：{detail}"));
    let text = format!(
        "{summary}\n\n{detail}\n\n日志：{}\n\n建议（任一即可）：\n1) 把 DSH-NEXT-Multi.exe 放到可写目录（桌面 / D 盘）再运行；\n2) 设置环境变量 DSH_MULTI_WEBVIEW_DATA 指向一个可写目录；\n3) 若系统缺少 WebView2 运行时，先安装 Microsoft Edge WebView2 Runtime。",
        logger.path_today().display()
    );
    if std::fs::write(
        std::env::temp_dir().join("DSH-NEXT-Multi-startup-error.txt"),
        &text,
    )
    .is_err()
    {
        eprintln!("failed to write startup error report");
    }
    if std::env::var_os("DSH_MULTI_NO_DIALOG").is_none() {
        #[cfg(windows)]
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                to_wide(&text).as_ptr(),
                to_wide("DSH-NEXT 应用多开启动器 — 启动失败").as_ptr(),
                0x10,
            );
        }
    }
    std::process::exit(1)
}

/// 显式指定 WebView2 数据目录建窗口：不依赖系统默认的 identifier 目录，
/// 那个目录不可写时 WebView2 会直接失败（而且什么都不说）。
fn build_main_window(
    app: &tauri::App,
    width: f64,
    height: f64,
    data_dir: std::path::PathBuf,
) -> Result<(), tauri::Error> {
    tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
        .title("DSH-NEXT 应用多开启动器")
        .inner_size(width, height)
        .min_inner_size(900.0, 600.0)
        .center()
        .resizable(true)
        .data_directory(data_dir)
        .build()
        .map(|_| ())
}

pub fn run() {
    if let Some(code) = cli::run_if_requested() {
        std::process::exit(code);
    }

    // 权限不足时自动以管理员身份重启：
    // DSH 的指针目录 / WebView2 数据目录若曾被管理员进程创建，ACL 可能只允许管理员写，
    // 普通权限会写不了指针（启动实例 E1001）。自动化用 DSH_MULTI_NO_ELEVATE=1 跳过。
    if std::env::var_os("DSH_MULTI_NO_ELEVATE").is_none()
        && !perm::is_admin()
        && !std::env::args().any(|a| a == "--elevated")
    {
        let report = perm::report();
        if report.needs_elevation && perm::relaunch_elevated() {
            std::process::exit(0);
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let logger = startup_logger();
            logger.info(format!(
                "启动器 v{} 启动（exe={}）",
                env!("CARGO_PKG_VERSION"),
                std::env::current_exe()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "?".into())
            ));

            // 尺寸默认 1120×760；`DSH_MULTI_WINDOW_SIZE=WxH` 可覆盖（多屏 / 小屏 / 自动化截图用）
            let (win_w, win_h) = std::env::var("DSH_MULTI_WINDOW_SIZE")
                .ok()
                .and_then(|v| {
                    let (w, h) = v.split_once('x')?;
                    Some((w.trim().parse::<f64>().ok()?, h.trim().parse::<f64>().ok()?))
                })
                .filter(|(w, h)| *w >= 640.0 && *h >= 420.0)
                .unwrap_or((1120.0, 760.0));

            // WebView2 数据目录按顺序尝试：显式覆盖 → 本应用数据目录 → exe 旁便携目录 → 临时目录
            let store = Store::from_env();
            let mut candidates: Vec<std::path::PathBuf> = Vec::new();
            if let Some(dir) = std::env::var_os("DSH_MULTI_WEBVIEW_DATA") {
                candidates.push(std::path::PathBuf::from(dir));
            }
            candidates.push(store.data_dir.join("webview2"));
            // data_dir 在受限环境里可能已退到 exe 旁；这里再显式兜一层，防止顺序被环境变量打乱
            candidates.push(
                crate::util::app_root()
                    .join("DSH-NEXT-Multi-data")
                    .join("webview2"),
            );
            candidates.push(std::env::temp_dir().join("DSH-NEXT-Multi").join("webview2"));

            let mut reasons: Vec<String> = Vec::new();
            let mut created = false;
            for dir in candidates {
                if std::fs::create_dir_all(&dir).is_err() {
                    let reason = format!("无法创建目录 {}", dir.display());
                    logger.warn(reason.clone());
                    reasons.push(reason);
                    continue;
                }
                match build_main_window(app, win_w, win_h, dir.clone()) {
                    Ok(()) => {
                        logger.info(format!("窗口已创建，WebView2 数据目录 {}", dir.display()));
                        created = true;
                        break;
                    }
                    Err(err) => {
                        logger.error(format!("窗口创建失败（{}）：{err}", dir.display()));
                        reasons.push(format!("{}：{err}", dir.display()));
                    }
                }
            }

            if !created {
                fatal(
                    &logger,
                    "无法创建窗口，启动器无法运行",
                    &format!(
                        "已尝试的 WebView2 数据目录：\n{}\n\n数据目录：{}\n版本目录：{}",
                        reasons
                            .iter()
                            .enumerate()
                            .map(|(i, r)| format!("{}. {}", i + 1, r))
                            .collect::<Vec<_>>()
                            .join("\n"),
                        store.data_dir.display(),
                        store.versions_dir.display()
                    ),
                );
            }

            let state = build_state(app.handle().clone(), logger.clone());
            // 把窗口尺寸写进日志：排障时能立刻分清“窗口太窄”还是“布局溢出”
            if let Some(win) = app.get_webview_window("main") {
                let scale = win.scale_factor().unwrap_or(1.0);
                match win.inner_size() {
                    Ok(size) => state.logger().info(format!(
                        "窗口内尺寸 {}x{} 物理像素，scale={scale} → CSS 视口约 {}x{}",
                        size.width,
                        size.height,
                        (size.width as f64 / scale).round() as u32,
                        (size.height as f64 / scale).round() as u32
                    )),
                    Err(err) => state.logger().warn(format!("读取窗口尺寸失败：{err}")),
                }
            }
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::snapshot,
            commands::instance_save,
            commands::instance_archive,
            commands::instance_delete,
            commands::instance_launch,
            commands::instance_cancel,
            commands::instance_stop,
            commands::instance_restart,
            commands::pointer_restore_default,
            commands::config_set,
            commands::version_scan,
            commands::version_import,
            commands::version_remove,
            commands::version_discover,
            commands::pick_directory,
            commands::open_path,
            commands::health,
            commands::permission_report,
            commands::relaunch_elevated,
        ])
        .run(tauri::generate_context!())
        .expect("启动 Tauri 应用失败");
}
