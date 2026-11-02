//! Tauri IPC 命令层：只做“读状态 / 改状态 / 调管道”，业务规则都在各自模块里。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::errors::AppError;
use crate::guard::{assert_safe_delete_target, SafetyRoots};
use crate::launcher::{stop_instance, LaunchRequest};
use crate::model::{
    Config, Instance, InstanceView, LaunchPhase, Snapshot, StopOutcome, VersionInfo,
};
use crate::scan;
use crate::store::{self, normalize_home, same_home, slugify};
use crate::util::{now_iso, path_eq};
use crate::versions::inspect_dir;
use crate::AppState;

// ---------------------------------------------------------------- 快照

#[tauri::command]
pub fn permission_report() -> crate::perm::PermissionReport {
    crate::perm::report()
}

/// 以管理员身份重启自己。返回 true 表示 UAC 已接受（调用方应提示用户关闭本窗口）。
#[tauri::command]
pub fn relaunch_elevated() -> Result<bool, AppError> {
    Ok(crate::perm::relaunch_elevated())
}

#[tauri::command]
pub fn snapshot(state: State<'_, AppState>) -> Result<Snapshot, AppError> {
    let cfg = state.config.lock().unwrap().clone();
    let pointer = state.pointer.snapshot();
    let running = scan::scan_running();
    let versions = state.versions_now();
    let instances = state.store.list_instances();
    let statuses = state.launcher.lock().unwrap().statuses();
    let queue = state.launcher.lock().unwrap().queue();

    let views: Vec<InstanceView> = instances
        .iter()
        .map(|inst| {
            let run = running
                .iter()
                .find(|r| path_eq(Path::new(&r.home), Path::new(&inst.home)))
                .cloned();
            let launch = statuses
                .get(&inst.id)
                .cloned()
                .unwrap_or_else(|| crate::model::LaunchStatus::idle(&inst.id));
            let version = versions
                .iter()
                .find(|v| v.tag == inst.version)
                .cloned()
                .or_else(|| {
                    // 版本目录被手工删掉时也给出“无效”条目，界面能说明白
                    Some(inspect_dir(
                        &state.versions_dir.join(&inst.version),
                        &inst.version,
                    ))
                });
            InstanceView {
                instance: inst.clone(),
                running: run,
                launch,
                version,
                is_default: !cfg.default_home.is_empty()
                    && same_home(&cfg.default_home, &inst.home),
                home_exists: Path::new(&inst.home).is_dir(),
            }
        })
        .collect();

    Ok(Snapshot {
        config: cfg,
        pointer,
        versions,
        instances: views,
        running,
        queue,
        data_dir: state.data_dir().to_string_lossy().to_string(),
        versions_dir: state.versions_dir.to_string_lossy().to_string(),
        log_dir: state.store.logs_dir().to_string_lossy().to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        user_profile: crate::util::user_home().to_string_lossy().to_string(),
    })
}

// ---------------------------------------------------------------- 实例

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstancePayload {
    pub id: String,
    pub name: String,
    pub version: String,
    pub home: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub is_new: bool,
    #[serde(default)]
    pub archived: bool,
}

#[tauri::command]
pub fn instance_save(
    state: State<'_, AppState>,
    payload: InstancePayload,
) -> Result<Instance, AppError> {
    let all = state.store.list_instances();
    let id = if payload.is_new {
        slugify(&payload.id)
    } else {
        payload.id.clone()
    };

    // home 留空 = 用数据目录树根推导的默认值（`<homeRoot>\.dsh-<id>`）；
    // 用户填了就用填的，不做“必须在根下”的校验。
    let home = if payload.home.trim().is_empty() {
        let cfg = state.config.lock().unwrap().clone();
        store::default_home_for(&cfg, &id)
    } else {
        normalize_home(&payload.home)
    };

    let mut inst = if payload.is_new {
        Instance::new(&id, payload.name.trim(), payload.version.trim(), &home)
    } else {
        let existing = state
            .store
            .read_instance(&payload.id)
            .map_err(|_| AppError::new("E1009", payload.id.clone()))?;
        Instance {
            name: payload.name.trim().to_string(),
            version: payload.version.trim().to_string(),
            home: home.clone(),
            notes: payload.notes.clone(),
            archived: payload.archived,
            ..existing
        }
    };
    if !inst.notes.is_empty() || payload.is_new {
        inst.notes = payload.notes.clone();
    }

    state.store.validate_instance(&inst, &all, payload.is_new)?;
    state.store.write_instance(&inst)?;
    state.logger().info(format!(
        "{} 实例 {}（{}）",
        if payload.is_new { "新建" } else { "保存" },
        inst.id,
        inst.home
    ));
    Ok(inst)
}

#[tauri::command]
pub fn instance_archive(
    state: State<'_, AppState>,
    id: String,
    archived: bool,
) -> Result<Instance, AppError> {
    let mut inst = state.store.read_instance(&id)?;
    let all = state.store.list_instances();
    // 归档前校验 home 唯一（未归档实例之间），恢复时同样要过这一关
    let mut candidate = inst.clone();
    candidate.archived = archived;
    state.store.validate_instance(&candidate, &all, false)?;
    inst.archived = archived;
    state.store.write_instance(&inst)?;
    Ok(inst)
}

#[tauri::command]
pub fn instance_delete(
    state: State<'_, AppState>,
    id: String,
    delete_data: bool,
) -> Result<(), AppError> {
    let inst = state.store.read_instance(&id)?;

    if scan::is_home_running(&inst.home).is_some() {
        return Err(AppError::new(
            "E1011",
            format!("{} 仍在运行，先停止实例", inst.home),
        ));
    }

    if delete_data {
        let cfg = state.config.lock().unwrap().clone();
        let roots = SafetyRoots::current(Path::new(&cfg.default_home));
        let target = Path::new(&inst.home);
        assert_safe_delete_target(target, &roots)?;
        if target.is_dir() {
            std::fs::remove_dir_all(target)
                .map_err(|e| AppError::new("E1000", format!("删除数据目录失败：{e}")))?;
            state.logger().warn(format!("已删除数据目录 {}", inst.home));
        }
    }

    state.store.delete_instance_file(&id)?;

    // 清理全局状态里指向它的残留（设计 §4.4）
    {
        let mut cfg = state.config.lock().unwrap();
        if let Some(lp) = &cfg.last_pointer {
            if same_home(&lp.home, &inst.home) {
                cfg.last_pointer = None;
            }
        }
        let snapshot = cfg.clone();
        drop(cfg);
        let _ = state.store.save_config(&snapshot);
    }
    if let Some(current) = state.pointer.read_home() {
        if same_home(&current, &inst.home) {
            let cfg = state.config.lock().unwrap().clone();
            let _ = state.pointer.restore_default(&cfg.default_home);
        }
    }
    state
        .logger()
        .info(format!("删除实例 {id}（删数据={delete_data}）"));
    Ok(())
}

// ---------------------------------------------------------------- 启动 / 停止

#[tauri::command]
pub fn instance_launch(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let inst = state.store.read_instance(&id)?;
    if inst.archived {
        return Err(AppError::new("E1012", "已归档的实例不能启动".to_string()));
    }
    let version = state
        .versions_now()
        .into_iter()
        .find(|v| v.tag == inst.version)
        .ok_or_else(|| AppError::new("E1004", format!("版本 {} 未登记", inst.version)))?;
    let exe = version.exe.clone().map(PathBuf::from).ok_or_else(|| {
        AppError::new("E1004", format!("版本 {} 缺少 DSH NEXT.exe", inst.version))
    })?;

    state
        .launcher
        .lock()
        .unwrap()
        .launch(LaunchRequest::new(inst.id, inst.home, exe))
}

#[tauri::command]
pub fn instance_cancel(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    state.launcher.lock().unwrap().cancel(&id);
    Ok(())
}

#[tauri::command]
pub fn instance_stop(
    state: State<'_, AppState>,
    id: String,
    force: bool,
) -> Result<StopOutcome, AppError> {
    let inst = state.store.read_instance(&id)?;
    let outcome = stop_instance(&inst.home, force, &state.logger())?;
    if outcome.stopped {
        if let Ok(mut inst) = state.store.read_instance(&id) {
            inst.last_exit_clean = true;
            let _ = state.store.write_instance(&inst);
        }
        state.launcher.lock().unwrap().mark_idle(&id);

        state.logger().info(format!("实例 {id} 已停止"));
    }
    Ok(outcome)
}

/// 重启实例：停止 → 等进程消失 → 按模式重新启动。
///
/// mode: `normal`（普通重启）| `safe`（带 `--next-safe-mode`）| `exitSafe`（不带，即退出安全模式）。
/// 参数名取自 DSH-NEXT 自身（`lib/main.js` 的 `SAFE_ARGUMENT`），不是猜的。
#[tauri::command]
pub fn instance_restart(
    state: State<'_, AppState>,
    id: String,
    mode: Option<String>,
) -> Result<(), AppError> {
    let inst = state.store.read_instance(&id)?;
    let mode = mode.unwrap_or_else(|| "normal".to_string());

    // 1) 先停（强制，避免优雅关闭的 15 秒等待把重启拖长）
    if let Err(err) = stop_instance(&inst.home, true, &state.logger()) {
        state
            .logger()
            .warn(format!("[{id}] 重启前停止失败（继续尝试）：{err}"));
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        if scan::is_home_running(&inst.home).is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    // 2) 版本树里的 exe
    let exe = state.versions_dir.join(&inst.version).join("DSH NEXT.exe");
    if !exe.is_file() {
        return Err(AppError::new(
            "E1004",
            format!("版本 {} 缺少 DSH NEXT.exe", inst.version),
        ));
    }

    // 3) 按模式拼参数（与 DSH-NEXT 自身一致：安全模式 = --next-safe-mode）
    let mut req = LaunchRequest::new(inst.id.clone(), inst.home.clone(), exe);
    if mode == "safe" {
        req.extra_args.push("--next-safe-mode".to_string());
    }

    state.launcher.lock().unwrap().launch(req)?;
    state.logger().info(format!(
        "[{id}] 已请求重启（{}）",
        match mode.as_str() {
            "safe" => "进入安全模式",
            "exitSafe" => "退出安全模式",
            _ => "普通",
        }
    ));
    Ok(())
}

// ---------------------------------------------------------------- 指针 / 配置

#[tauri::command]
pub fn pointer_restore_default(
    state: State<'_, AppState>,
) -> Result<crate::model::PointerSnapshot, AppError> {
    let cfg = state.config.lock().unwrap().clone();
    state.pointer.restore_default(&cfg.default_home)?;
    state
        .logger()
        .info(format!("指针已还原为默认 home {}", cfg.default_home));
    Ok(state.pointer.snapshot())
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPatch {
    pub default_home: Option<String>,
    /// 数据目录树根：只影响“新建实例的默认 home”，不限制用户手改。
    pub home_root: Option<String>,
    pub launch_timeout_sec: Option<u64>,
    pub poll_interval_ms: Option<u64>,
    pub restore_pointer_after_launch: Option<bool>,
}

#[tauri::command]
pub fn config_set(state: State<'_, AppState>, patch: ConfigPatch) -> Result<Config, AppError> {
    let mut cfg = state.config.lock().unwrap().clone();
    if let Some(root) = patch.home_root {
        store::validate_home_root(&root)?;
        cfg = store::apply_home_root(&cfg, &root);
    }
    if let Some(home) = patch.default_home {
        let home = normalize_home(&home);
        if home.is_empty() || !Path::new(&home).is_absolute() {
            return Err(AppError::new(
                "E1012",
                format!("默认数据目录必须是绝对路径：{home}"),
            ));
        }
        cfg.default_home = home;
    }
    if let Some(v) = patch.launch_timeout_sec {
        cfg.launch_timeout_sec = v;
    }
    if let Some(v) = patch.poll_interval_ms {
        cfg.poll_interval_ms = v;
    }
    if let Some(v) = patch.restore_pointer_after_launch {
        cfg.restore_pointer_after_launch = v;
    }
    cfg = cfg.sanitized();
    state.store.save_config(&cfg)?;
    *state.config.lock().unwrap() = cfg.clone();
    Ok(cfg)
}

// ---------------------------------------------------------------- 版本

#[tauri::command]
pub fn version_scan(state: State<'_, AppState>) -> Result<Vec<VersionInfo>, AppError> {
    state.invalidate_versions();
    Ok(state.versions_now())
}

#[tauri::command]
pub fn version_import(state: State<'_, AppState>, dir: String) -> Result<VersionInfo, AppError> {
    let info = state.versions.register(Path::new(&dir))?;
    state.invalidate_versions();
    state
        .logger()
        .info(format!("登记版本 {} → {}", info.tag, info.dir));
    Ok(info)
}

/// 自动发现 DSH-NEXT 安装目录（快捷方式 / 注册表 / 常见路径 / 运行中进程）。
///
/// 返回候选列表，前端展示后由用户勾选，再逐个调用 `version_import`。
/// 只读操作，不改版本树。
#[tauri::command]
pub fn version_discover(
    state: State<'_, AppState>,
) -> Result<Vec<crate::discover::DiscoverCandidate>, AppError> {
    let registered: Vec<PathBuf> = state
        .versions_now()
        .into_iter()
        .map(|v| PathBuf::from(v.dir))
        .collect();
    let found = crate::discover::scan(&registered);
    state.logger().info(format!(
        "自动发现：{} 个候选（有效 {}）",
        found.len(),
        found.iter().filter(|c| c.valid).count()
    ));
    Ok(found)
}

#[tauri::command]
pub fn version_remove(state: State<'_, AppState>, tag: String) -> Result<(), AppError> {
    let all = state.store.list_instances();
    state.versions.remove(&tag, &all)?;
    state.invalidate_versions();
    state.logger().info(format!("移除版本文档 {tag}"));
    Ok(())
}

/// 选目录（导入版本用），返回绝对路径。
#[tauri::command]
pub async fn pick_directory(app: tauri::AppHandle) -> Result<Option<String>, AppError> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app.dialog().file().blocking_pick_folder();
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().to_string()))
}

// ---------------------------------------------------------------- 打开目录

#[tauri::command]
pub fn open_path(state: State<'_, AppState>, path: String, create: bool) -> Result<(), AppError> {
    let p = PathBuf::from(&path);
    if create {
        std::fs::create_dir_all(&p)
            .map_err(|e| AppError::new("E1000", format!("创建目录失败：{e}")))?;
    }
    if !p.exists() {
        return Err(AppError::new(
            "E1012",
            format!("路径不存在：{}", p.display()),
        ));
    }
    state.logger().info(format!("打开目录 {}", p.display()));
    open_in_explorer(&p).map_err(|e| AppError::new("E1000", format!("打开目录失败：{e}")))
}

fn open_in_explorer(path: &Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("explorer.exe")
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
}

/// 前端要的“最近一次启动/队列摘要”不单独开命令，直接读快照即可。

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub ok: bool,
    pub app_version: String,
    pub data_dir: String,
    pub versions_dir: String,
    pub pointer_path: String,
    pub pointer_writable: bool,
    pub running: usize,
    pub instances: usize,
    pub versions: usize,
    pub time: String,
}

#[tauri::command]
pub fn health(state: State<'_, AppState>) -> Result<Health, AppError> {
    let pointer = state.pointer.snapshot();
    Ok(Health {
        ok: true,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        data_dir: state.data_dir().to_string_lossy().to_string(),
        versions_dir: state.versions_dir.to_string_lossy().to_string(),
        pointer_path: pointer.path,
        pointer_writable: pointer.writable,
        running: scan::scan_running().len(),
        instances: state.store.list_instances().len(),
        versions: state.versions_now().len(),
        time: now_iso(),
    })
}

// 让 `LaunchPhase` 参与序列化（供前端类型生成时可见）
pub fn _phase_name(p: LaunchPhase) -> &'static str {
    p.as_str()
}
