//! 无界面 CLI：自检与端到端验收（给自动化与排障用，不影响正常 GUI 启动）。
//!
//! `DSH-NEXT-Multi.exe --selftest`
//! `DSH-NEXT-Multi.exe --e2e-launch <instanceId>`
//! `DSH-NEXT-Multi.exe --e2e-stop <instanceId> [--force]`
//! `DSH-NEXT-Multi.exe --e2e-status`

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use crate::launcher::{Launcher, LaunchRequest, ProgressSink};
use crate::model::{Config, LaunchPhase, LaunchStatus};
use crate::pointer::Pointer;
use crate::store::Store;
use crate::util::{atomic_write, now_iso, Logger};
use crate::versions::VersionScanner;

/// 命令行模式的输出：既打 stdout（有控制台时），也写一份到文件。
///
/// release 构建是 Windows GUI 子系统（没有控制台），stdout 拿不到，
/// 所以报告必须落盘：默认 `<data_dir>\cache\last-cli.json`，可用 `DSH_MULTI_CLI_OUT` 覆盖。
pub fn emit_text(text: &str) {
    println!("{text}");
    let path = std::env::var_os("DSH_MULTI_CLI_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::util::data_dir().join("cache").join("last-cli.json"));
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = atomic_write(&path, text.as_bytes());
}

macro_rules! emit {
    ($($arg:tt)*) => {{
        let __text = format!($($arg)*);
        $crate::cli::emit_text(&__text);
    }};
}

pub struct StdoutSink;

impl ProgressSink for StdoutSink {
    fn progress(&self, status: &LaunchStatus) {
        eprintln!(
            "[{}] {} phase={} pids={:?}/{:?} err={}",
            now_iso(),
            status.instance_id,
            status.phase.as_str(),
            status.shell_pid,
            status.host_pid,
            status
                .error
                .as_ref()
                .map(|e| format!("{} {}", e.code, e.detail.clone().unwrap_or_default()))
                .unwrap_or_default()
        );
    }
}

pub fn run_if_requested() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first()?.as_str();
    match first {
        "--selftest" => Some(selftest()),
        "--e2e-launch" => Some(e2e_launch(args.get(1).map(String::as_str)?)),
        "--e2e-stop" => Some(e2e_stop(
            args.get(1).map(String::as_str)?,
            args.iter().any(|a| a == "--force"),
        )),
        "--e2e-status" => Some(e2e_status()),
        "--scan-debug" => Some(scan_debug()),
        _ => None,
    }
}

/// 排障用：把扫到的 DSH 进程逐条打出来（名字 / pid / 父 pid / 命令行）。
fn scan_debug() -> i32 {
    let procs = crate::scan::scan_processes();
    let dsh: Vec<_> = procs
        .iter()
        .filter(|p| p.is_dsh() || p.cmd.to_lowercase().contains("dsh next.exe"))
        .map(|p| {
            json!({
                "pid": p.pid,
                "parent": p.parent,
                "name": p.name,
                "isShell": p.is_shell(),
                "isHost": p.is_host(),
                "userDataHome": crate::scan::parse_user_data_home(&p.cmd),
                "hostHome": crate::scan::parse_host_home(&p.cmd),
                "cmd": p.cmd,
            })
        })
        .collect();
    emit!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "totalProcesses": procs.len(),
            "dsh": dsh,
            "running": crate::scan::scan_running(),
        }))
        .unwrap()
    );
    0
}

fn selftest() -> i32 {
    let mut checks = Vec::new();
    let mut ok = true;

    let data_dir = crate::util::data_dir();
    let versions_dir = crate::util::versions_dir();
    let pointer_path = crate::util::pointer_path();
    let app_root = crate::util::app_root();
    checks.push(json!({
        "name": "paths",
        "ok": true,
        "detail": {
            "exe": std::env::current_exe().map(|p| p.to_string_lossy().to_string()).unwrap_or_default(),
            "appRoot": app_root.to_string_lossy().to_string(),
            "dataDir": data_dir.to_string_lossy().to_string(),
            "versionsDir": versions_dir.to_string_lossy().to_string(),
            "pointer": pointer_path.to_string_lossy().to_string(),
        }
    }));

    let store = Store::from_env();
    let cfg: Result<Config, _> = store.load_config();
    match &cfg {
        Ok(c) => checks.push(json!({
            "name": "config",
            "ok": true,
            "detail": {"defaultHome": c.default_home, "timeoutSec": c.launch_timeout_sec,
                       "pollMs": c.poll_interval_ms, "restorePointer": c.restore_pointer_after_launch}
        })),
        Err(e) => {
            ok = false;
            checks.push(json!({"name": "config", "ok": false, "detail": e.to_string()}));
        }
    }

    let scanner = VersionScanner::new(versions_dir.clone());
    let versions = scanner.scan();
    checks.push(json!({
        "name": "versions",
        "ok": !versions.is_empty(),
        "detail": versions.iter().map(|v| json!({
            "tag": v.tag, "valid": v.valid, "shell": v.shell_version,
            "kernel": v.kernel_version, "reason": v.invalid_reason
        })).collect::<Vec<_>>()
    }));
    if versions.is_empty() {
        ok = false;
    }

    let running = crate::scan::scan_running();
    checks.push(json!({
        "name": "processes",
        "ok": true,
        "detail": {
            "processCount": crate::scan::scan_processes().len(),
            "running": running.iter().map(|r| json!({
                "home": r.home, "shellPid": r.shell_pid, "hostPid": r.host_pid, "startedAt": r.started_at
            })).collect::<Vec<_>>()
        }
    }));

    let instances = store.list_instances();
    let mut invalid = Vec::new();
    for inst in &instances {
        if let Err(err) = store.validate_instance(inst, &instances, false) {
            invalid.push(json!({"id": inst.id, "error": err.to_string()}));
        }
    }
    checks.push(json!({
        "name": "instances",
        "ok": invalid.is_empty(),
        "detail": {"count": instances.len(), "invalid": invalid}
    }));
    if !invalid.is_empty() {
        ok = false;
    }

    // 原子写在临时目录里自检一次（不碰真实配置）
    let probe_dir = std::env::var("DSH_MULTI_SELFTEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| crate::util::data_dir().join("cache"))
        .join("atomic-probe");
    let probe_file = probe_dir.join("probe.json");
    let write_ok = std::fs::create_dir_all(&probe_dir).is_ok()
        && atomic_write(&probe_file, b"{\"ok\":true}").is_ok()
        && std::fs::read_to_string(&probe_file).map(|s| s == "{\"ok\":true}").unwrap_or(false);
    let _ = std::fs::remove_dir_all(&probe_dir);
    checks.push(json!({"name": "atomicWrite", "ok": write_ok, "detail": {"dir": probe_dir.to_string_lossy()}}));
    if !write_ok {
        ok = false;
    }

    let pointer = Pointer::default_path();
    let snap = pointer.snapshot();
    checks.push(json!({
        "name": "pointer",
        "ok": snap.writable,
        "detail": {"path": snap.path, "home": snap.home, "exists": snap.exists, "writable": snap.writable}
    }));
    if !snap.writable {
        ok = false;
    }

    emit!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "ok": ok,
            "time": now_iso(),
            "checks": checks
        }))
        .unwrap_or_else(|_| "{\"ok\":false}".into())
    );
    if ok {
        0
    } else {
        1
    }
}

fn load_shared() -> (Store, Config, Pointer) {
    let store = Store::from_env();
    let _ = store.ensure_dirs();
    let cfg = store
        .load_config()
        .unwrap_or_else(|_| Config::with_default_home(crate::store::default_home_string()));
    let pointer = Pointer::default_path();
    (store, cfg, pointer)
}

fn e2e_launch(instance_id: &str) -> i32 {
    let (store, cfg, pointer) = load_shared();
    let logger = Logger::new(store.logs_dir()).with_echo(false);
    let original_pointer = pointer.read_raw();

    let inst = match store.read_instance(instance_id) {
        Ok(i) => i,
        Err(err) => {
            emit!("{}", json!({"ok": false, "error": err.to_string()}));
            return 1;
        }
    };
    let scanner = VersionScanner::new(store.versions_dir.clone());
    let version = match scanner.find(&inst.version) {
        Some(v) => v,
        None => {
            emit!("{}", json!({"ok": false, "error": format!("版本 {} 未登记", inst.version)}));
            return 1;
        }
    };
    let Some(exe) = version.exe.clone().map(PathBuf::from) else {
        let err = crate::errors::AppError::new(
            "E1004",
            format!("版本 {} 缺少 DSH NEXT.exe", inst.version),
        );
        emit!("{}", json!({"ok": false, "code": err.code, "error": err.to_string()}));
        return 1;
    };

    let cfg_arc = Arc::new(Mutex::new(cfg.clone()));
    let launcher = Launcher::start(
        Arc::new(StdoutSink),
        store.clone(),
        pointer.clone(),
        cfg_arc.clone(),
        logger.clone(),
    );
    let started = Instant::now();
    if let Err(err) = launcher.launch(LaunchRequest::new(inst.id.clone(), inst.home.clone(), exe)) {
        emit!("{}", json!({"ok": false, "error": err.to_string()}));
        return 1;
    }

    let deadline = started + Duration::from_secs(cfg.launch_timeout_sec + 45);
    #[allow(unused_assignments)]
    let mut last_phase = LaunchPhase::Queued;
    loop {
        std::thread::sleep(Duration::from_millis(200));
        let status = launcher.status(&inst.id);
        last_phase = status.phase;
        if matches!(status.phase, LaunchPhase::Running | LaunchPhase::Failed | LaunchPhase::Idle) {
            break;
        }
        if Instant::now() >= deadline {
            break;
        }
    }

    // 双保险：把指针恢复成调用前的原文（worker 成功/失败都会回滚，这里再兜一层）
    let restored = match &original_pointer {
        Some(raw) => {
            if pointer.read_raw().as_deref() != Some(raw.as_str()) {
                let _ = atomic_write(pointer.path(), raw.as_bytes());
                true
            } else {
                true
            }
        }
        None => {
            let _ = std::fs::remove_file(pointer.path());
            true
        }
    };

    let running = crate::scan::scan_running();
    let status = launcher.status(&inst.id);
    emit!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "ok": last_phase == LaunchPhase::Running,
            "instanceId": inst.id,
            "home": inst.home,
            "phase": last_phase.as_str(),
            "elapsedMs": started.elapsed().as_millis(),
            "shellPid": status.shell_pid,
            "hostPid": status.host_pid,
            "error": status.error,
            "pointerRestored": restored,
            "pointerNow": pointer.read_home(),
            "running": running.iter().map(|r| json!({"home": r.home, "shell": r.shell_pid, "host": r.host_pid})).collect::<Vec<_>>()
        }))
        .unwrap()
    );

    if last_phase == LaunchPhase::Running {
        0
    } else {
        1
    }
}

fn e2e_stop(instance_id: &str, force: bool) -> i32 {
    let (store, _cfg, _pointer) = load_shared();
    let logger = Logger::new(store.logs_dir()).with_echo(false);
    let inst = match store.read_instance(instance_id) {
        Ok(i) => i,
        Err(err) => {
            emit!("{}", json!({"ok": false, "error": err.to_string()}));
            return 1;
        }
    };
    match crate::launcher::stop_instance(&inst.home, force, &logger) {
        Ok(outcome) => {
            emit!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": outcome.stopped,
                    "stopped": outcome.stopped,
                    "needsForce": outcome.needs_force,
                    "pid": outcome.pid,
                    "running": crate::scan::scan_running().iter().map(|r| r.home.clone()).collect::<Vec<_>>()
                }))
                .unwrap()
            );
            if outcome.stopped { 0 } else { 1 }
        }
        Err(err) => {
            emit!("{}", json!({"ok": false, "error": err.to_string()}));
            1
        }
    }
}

fn e2e_status() -> i32 {
    let (store, cfg, pointer) = load_shared();
    let running = crate::scan::scan_running();
    let scanner = VersionScanner::new(store.versions_dir.clone());
    let versions = scanner.scan();
    let instances = store.list_instances();
    emit!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "ok": true,
            "time": now_iso(),
            "dataDir": store.data_dir.to_string_lossy(),
            "versionsDir": store.versions_dir.to_string_lossy(),
            "appRoot": crate::util::app_root().to_string_lossy(),
            "defaultHome": cfg.default_home,
            "pointer": {"path": pointer.path().to_string_lossy(), "home": pointer.read_home()},
            "versions": versions.iter().map(|v| json!({"tag": v.tag, "valid": v.valid, "shell": v.shell_version, "kernel": v.kernel_version, "isLink": v.is_link, "linkTarget": v.link_target})).collect::<Vec<_>>(),
            "instances": instances.iter().map(|i| json!({"id": i.id, "name": i.name, "home": i.home, "version": i.version, "archived": i.archived})).collect::<Vec<_>>(),
            "running": running.iter().map(|r| json!({"home": r.home, "shell": r.shell_pid, "host": r.host_pid, "startedAt": r.started_at})).collect::<Vec<_>>()
        }))
        .unwrap()
    );
    0
}

#[allow(dead_code)]
fn path_display(p: &Path) -> String {
    p.to_string_lossy().to_string()
}
