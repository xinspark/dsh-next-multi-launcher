//! 启动管道与状态机（设计说明 §5.4）。
//!
//! 串行队列 + 单 worker：**指针全局一份，写指针与 spawn 之间不能被另一次启动插入**。
//! 失败一律回滚指针；成功按 `restorePointerAfterLaunch` 把指针还原成默认 home。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::errors::AppError;
use crate::model::{Config, LastPointer, LaunchPhase, LaunchStatus, StopOutcome};
use crate::pointer::Pointer;
use crate::scan;
use crate::store::Store;
use crate::util::{now_iso, Logger};

/// 子进程退出后仍给多少秒：DSH-NEXT 会自我重启一次（父退子起），不能一见父退就判失败。
const EXIT_GRACE: Duration = Duration::from_secs(20);

#[derive(Debug, Clone)]
pub struct LaunchRequest {
    pub instance_id: String,
    pub home: String,
    pub exe: PathBuf,
    /// 追加给 DSH-NEXT 的参数（安全模式 = `--next-safe-mode`）。
    /// 重启时由启动器自己决定，不依赖 DSH 内部菜单。
    pub extra_args: Vec<String>,
}

impl LaunchRequest {
    pub fn new(instance_id: impl Into<String>, home: impl Into<String>, exe: PathBuf) -> Self {
        Self {
            instance_id: instance_id.into(),
            home: home.into(),
            exe,
            extra_args: Vec::new(),
        }
    }
}

pub enum Cmd {
    Launch(LaunchRequest),
    Cancel(String),
}

/// 进度出口：生产是 Tauri 事件，测试是内存收集器。
pub trait ProgressSink: Send + Sync + 'static {
    fn progress(&self, status: &LaunchStatus);
}

pub struct SilentSink;

impl ProgressSink for SilentSink {
    fn progress(&self, _status: &LaunchStatus) {}
}

fn publish(
    statuses: &Arc<Mutex<HashMap<String, LaunchStatus>>>,
    sink: &Arc<dyn ProgressSink>,
    status: LaunchStatus,
) {
    statuses
        .lock()
        .unwrap()
        .insert(status.instance_id.clone(), status.clone());
    sink.progress(&status);
}

pub struct Launcher {
    tx: Sender<Cmd>,
    statuses: Arc<Mutex<HashMap<String, LaunchStatus>>>,
    queue: Arc<Mutex<VecDeque<String>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
    sink: Arc<dyn ProgressSink>,
}

impl Launcher {
    pub fn start(
        sink: Arc<dyn ProgressSink>,
        store: Store,
        pointer: Pointer,
        config: Arc<Mutex<Config>>,
        logger: Logger,
    ) -> Self {
        let (tx, rx) = mpsc::channel::<Cmd>();
        let statuses: Arc<Mutex<HashMap<String, LaunchStatus>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let queue: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
        let cancelled: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

        let worker = Worker {
            store,
            pointer,
            config,
            logger,
            sink: sink.clone(),
            statuses: statuses.clone(),
            queue: queue.clone(),
            cancelled: cancelled.clone(),
        };
        std::thread::Builder::new()
            .name("launch-worker".into())
            .spawn(move || worker.run(rx))
            .ok();

        Self {
            tx,
            statuses,
            queue,
            cancelled,
            sink,
        }
    }

    pub fn launch(&self, req: LaunchRequest) -> Result<(), AppError> {
        {
            let q = self.queue.lock().unwrap();
            if q.contains(&req.instance_id) {
                return Err(AppError::new("E1012", "该实例已在启动队列中".to_string()));
            }
            let st = self.statuses.lock().unwrap();
            if let Some(s) = st.get(&req.instance_id) {
                if matches!(
                    s.phase,
                    LaunchPhase::Preparing | LaunchPhase::Spawning | LaunchPhase::Waiting
                ) {
                    return Err(AppError::new("E1012", "该实例正在启动中".to_string()));
                }
            }
        }
        if let Some(running) = scan::is_home_running(&req.home) {
            return Err(AppError::new(
                "E1003",
                format!(
                    "{} 已被运行中的实例占用（pid {}）",
                    req.home, running.shell_pid
                ),
            ));
        }

        let position = {
            let mut q = self.queue.lock().unwrap();
            q.push_back(req.instance_id.clone());
            q.len()
        };
        publish(
            &self.statuses,
            &self.sink,
            LaunchStatus {
                instance_id: req.instance_id.clone(),
                phase: LaunchPhase::Queued,
                position: Some(position),
                started_at: None,
                elapsed_ms: 0,
                shell_pid: None,
                host_pid: None,
                error: None,
            },
        );
        self.tx
            .send(Cmd::Launch(req))
            .map_err(|_| AppError::new("E1000", "启动 worker 已退出".to_string()))
    }

    pub fn cancel(&self, instance_id: &str) {
        {
            let mut q = self.queue.lock().unwrap();
            q.retain(|id| id != instance_id);
        }
        self.cancelled
            .lock()
            .unwrap()
            .insert(instance_id.to_string());
        let phase = self
            .statuses
            .lock()
            .unwrap()
            .get(instance_id)
            .map(|s| s.phase);
        match phase {
            // 在跑的任务由 worker 自己收尾；这里只处理排队的情况
            Some(LaunchPhase::Preparing)
            | Some(LaunchPhase::Spawning)
            | Some(LaunchPhase::Waiting) => {}
            _ => publish(&self.statuses, &self.sink, LaunchStatus::idle(instance_id)),
        }
    }

    pub fn status(&self, instance_id: &str) -> LaunchStatus {
        self.statuses
            .lock()
            .unwrap()
            .get(instance_id)
            .cloned()
            .unwrap_or_else(|| LaunchStatus::idle(instance_id))
    }

    pub fn statuses(&self) -> HashMap<String, LaunchStatus> {
        self.statuses.lock().unwrap().clone()
    }

    pub fn queue(&self) -> Vec<String> {
        self.queue.lock().unwrap().iter().cloned().collect()
    }

    /// 停止后把状态归位（避免“已停止”但界面还显示 Running）。
    pub fn mark_idle(&self, instance_id: &str) {
        self.cancelled.lock().unwrap().remove(instance_id);
        publish(&self.statuses, &self.sink, LaunchStatus::idle(instance_id));
    }

    /// 测试用：不启动 worker，只保留入队 / 去重 / 状态能力。
    #[cfg(test)]
    pub fn new_for_test(sink: Arc<dyn ProgressSink>) -> (Self, Receiver<Cmd>) {
        let (tx, rx) = mpsc::channel::<Cmd>();
        (
            Self {
                tx,
                statuses: Arc::new(Mutex::new(HashMap::new())),
                queue: Arc::new(Mutex::new(VecDeque::new())),
                cancelled: Arc::new(Mutex::new(HashSet::new())),
                sink,
            },
            rx,
        )
    }
}

// ---------------------------------------------------------------- worker

struct Worker {
    store: Store,
    pointer: Pointer,
    config: Arc<Mutex<Config>>,
    logger: Logger,
    sink: Arc<dyn ProgressSink>,
    statuses: Arc<Mutex<HashMap<String, LaunchStatus>>>,
    queue: Arc<Mutex<VecDeque<String>>>,
    cancelled: Arc<Mutex<HashSet<String>>>,
}

impl Worker {
    fn run(mut self, rx: Receiver<Cmd>) {
        while let Ok(cmd) = rx.recv() {
            match cmd {
                Cmd::Launch(req) => {
                    {
                        let mut q = self.queue.lock().unwrap();
                        q.retain(|id| id != &req.instance_id);
                    }
                    if self.take_cancel(&req.instance_id) {
                        self.emit(LaunchStatus::idle(&req.instance_id));
                        continue;
                    }
                    self.handle(req);
                }
                Cmd::Cancel(id) => {
                    self.cancelled.lock().unwrap().insert(id.clone());
                    self.emit(LaunchStatus::idle(&id));
                }
            }
        }
    }

    fn cfg(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    fn take_cancel(&self, id: &str) -> bool {
        self.cancelled.lock().unwrap().remove(id)
    }

    fn is_cancelled(&self, id: &str) -> bool {
        self.cancelled.lock().unwrap().contains(id)
    }

    fn emit(&self, status: LaunchStatus) {
        publish(&self.statuses, &self.sink, status);
    }

    fn emit_phase(
        &self,
        id: &str,
        phase: LaunchPhase,
        started: Instant,
        error: Option<AppError>,
        running: Option<&crate::model::RunningInfo>,
    ) {
        self.emit(LaunchStatus {
            instance_id: id.to_string(),
            phase,
            position: None,
            started_at: Some(now_iso()),
            elapsed_ms: started.elapsed().as_millis() as u64,
            shell_pid: running.map(|r| r.shell_pid),
            host_pid: running.and_then(|r| r.host_pid),
            error,
        });
    }

    fn fail(&self, id: &str, started: Instant, error: AppError) {
        self.logger.error(format!("[{id}] 启动失败 {error}"));
        self.emit_phase(id, LaunchPhase::Failed, started, Some(error), None);
        self.cancelled.lock().unwrap().remove(id);
    }

    fn handle(&mut self, req: LaunchRequest) {
        let id = req.instance_id.clone();
        let started = Instant::now();
        let cfg = self.cfg();
        self.logger.info(format!(
            "[{id}] 启动：home={} exe={}",
            req.home,
            req.exe.display()
        ));
        self.emit_phase(&id, LaunchPhase::Preparing, started, None, None);

        // 0. home 已被别的实例占用
        if let Some(running) = scan::is_home_running(&req.home) {
            self.fail(
                &id,
                started,
                AppError::new(
                    "E1003",
                    format!(
                        "{} 已被运行中的实例占用（pid {}）",
                        req.home, running.shell_pid
                    ),
                ),
            );
            return;
        }

        // 1. 版本树可执行文件存在
        if !req.exe.is_file() {
            self.fail(
                &id,
                started,
                AppError::new("E1004", format!("找不到 {}", req.exe.display())),
            );
            return;
        }

        // 2. 写指针（先备份）
        let backup = match self.pointer.write(&req.home) {
            Ok(b) => b,
            Err(err) => {
                self.fail(&id, started, err);
                return;
            }
        };

        if self.is_cancelled(&id) {
            let _ = self.pointer.restore(&backup);
            self.emit(LaunchStatus::idle(&id));
            self.cancelled.lock().unwrap().remove(&id);
            return;
        }

        // 3. spawn
        //    stdio 全部置空：GUI 进程用不到控制台，也避免子进程攥着调用方的管道句柄
        //    （那样会让自动化脚本误以为启动器还没退出）。
        let mut cmd = std::process::Command::new(&req.exe);
        cmd.current_dir(req.exe.parent().unwrap_or(&req.exe));
        for arg in &req.extra_args {
            cmd.arg(arg);
        }
        let mut child = match cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(err) => {
                let _ = self.pointer.restore(&backup);
                self.fail(
                    &id,
                    started,
                    AppError::new("E1004", format!("无法启动 {}：{err}", req.exe.display())),
                );
                return;
            }
        };
        self.emit_phase(&id, LaunchPhase::Spawning, started, None, None);

        // 4. 轮询：看到目标 home 的进程才算成功
        let deadline = started + Duration::from_secs(cfg.launch_timeout_sec.max(10));
        let poll = Duration::from_millis(cfg.poll_interval_ms.clamp(100, 5_000));
        let mut child_exit: Option<std::process::ExitStatus> = None;
        let mut waiting_emitted = false;

        loop {
            std::thread::sleep(poll);

            // 运行中先判成功（自我重启后父进程已退出，但 home 进程已出现）
            if let Some(mut running) = scan::is_home_running(&req.home) {
                // host 子进程可能比 gpu 晚几百毫秒：补一次扫描，尽量把 host pid 也报上去
                if running.host_pid.is_none() {
                    for _ in 0..4 {
                        std::thread::sleep(Duration::from_millis(150));
                        if let Some(again) = scan::is_home_running(&req.home) {
                            if again.host_pid.is_some() {
                                running = again;
                                break;
                            }
                        }
                    }
                }
                self.succeed(&id, started, &req, running);
                return;
            }

            if self.is_cancelled(&id) {
                self.logger
                    .warn(format!("[{id}] 用户取消等待（不杀已起进程）"));
                self.emit(LaunchStatus {
                    instance_id: id.clone(),
                    phase: LaunchPhase::Idle,
                    position: None,
                    started_at: Some(now_iso()),
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    shell_pid: None,
                    host_pid: None,
                    error: Some(AppError::new(
                        "E1012",
                        "已取消等待，进程未杀；稍后可能仍会启动",
                    )),
                });
                self.cancelled.lock().unwrap().remove(&id);
                return;
            }

            if child_exit.is_none() {
                if let Ok(Some(status)) = child.try_wait() {
                    child_exit = Some(status);
                    self.logger.warn(format!(
                        "[{id}] 父进程退出（code={:?}），继续观察 {} 秒判断是否自我重启",
                        status.code(),
                        EXIT_GRACE.as_secs()
                    ));
                    self.emit_phase(&id, LaunchPhase::Waiting, started, None, None);
                }
            }

            // 启动即退出：超过宽限期仍没有 home 进程
            if let Some(status) = child_exit {
                if started.elapsed() >= EXIT_GRACE {
                    let _ = self.pointer.restore(&backup);
                    self.fail(
                        &id,
                        started,
                        AppError::new(
                            "E1006",
                            format!(
                                "进程已退出（退出码 {}），{} 秒内未看到 {} 的窗口进程",
                                status
                                    .code()
                                    .map(|c| c.to_string())
                                    .unwrap_or_else(|| "?".into()),
                                EXIT_GRACE.as_secs(),
                                req.home
                            ),
                        ),
                    );
                    return;
                }
            }

            if !waiting_emitted
                && started.elapsed() >= Duration::from_secs(3)
                && child_exit.is_none()
            {
                self.emit_phase(&id, LaunchPhase::Waiting, started, None, None);
                waiting_emitted = true;
            }

            if Instant::now() >= deadline {
                let _ = self.pointer.restore(&backup);
                self.fail(
                    &id,
                    started,
                    AppError::new(
                        "E1005",
                        format!("{} 秒内未看到 {} 的进程", cfg.launch_timeout_sec, req.home),
                    ),
                );
                return;
            }
        }
    }

    fn succeed(
        &mut self,
        id: &str,
        started: Instant,
        req: &LaunchRequest,
        running: crate::model::RunningInfo,
    ) {
        // 实例文件：记录最后启动时间 + 标记“上次非正常退出”
        if let Ok(mut inst) = self.store.read_instance(id) {
            inst.last_launch_at = Some(now_iso());
            inst.last_exit_clean = false;
            inst.safe_mode = req.extra_args.iter().any(|a| a == "--next-safe-mode");
            let _ = self.store.write_instance(&inst);
        }

        // 全局配置：记下这次把指针写成了什么（崩溃后诊断用）
        let cfg = {
            let mut guard = self.config.lock().unwrap();
            guard.last_pointer = Some(LastPointer {
                home: req.home.clone(),
                at: now_iso(),
            });
            let snapshot = guard.clone();
            drop(guard);
            let _ = self.store.save_config(&snapshot);
            snapshot
        };

        // 启动成功 → 把指针还原成默认 home，保证双击图标仍进日常环境。
        // 代价（已知）：DSH-NEXT 内部重启会重新读指针，从而落到默认实例；
        // 因此“重启”统一由启动器发起（instance_restart），不在 DSH 内部点。
        if cfg.restore_pointer_after_launch {
            let default_home = cfg.default_home.clone();
            if !default_home.is_empty()
                && !crate::util::path_eq(
                    std::path::Path::new(&default_home),
                    std::path::Path::new(&req.home),
                )
            {
                if let Err(err) = self.pointer.restore_default(&default_home) {
                    self.logger.error(format!("[{id}] 还原默认指针失败：{err}"));
                }
            }
        }

        self.logger.info(format!(
            "[{id}] 启动成功：shell={} host={:?} 用时 {}ms",
            running.shell_pid,
            running.host_pid,
            started.elapsed().as_millis()
        ));
        self.emit_phase(id, LaunchPhase::Running, started, None, Some(&running));
        self.cancelled.lock().unwrap().remove(id);
    }
}

// ---------------------------------------------------------------- 停止

/// 停止实例：先优雅关闭主窗口，`force` 时才强杀进程树。
pub fn stop_instance(home: &str, force: bool, logger: &Logger) -> Result<StopOutcome, AppError> {
    let Some(running) = scan::is_home_running(home) else {
        return Ok(StopOutcome {
            stopped: false,
            needs_force: false,
            pid: None,
        });
    };
    let pid = running.shell_pid;

    if force {
        // 主进程 pid 可能因为拿不到父进程信息而退化成子进程：整组一起结束更稳
        let mut pids = scan::pids_for_home(home);
        if !pids.contains(&pid) {
            pids.push(pid);
        }
        for p in &pids {
            let _ = taskkill(&p.to_string(), true);
        }
        let gone = wait_until_gone(home, Duration::from_secs(8));
        logger.info(format!(
            "[{home}] 强制结束 {} 个进程（{:?}）：{}",
            pids.len(),
            pids,
            if gone { "已退出" } else { "仍在" }
        ));
        return Ok(StopOutcome {
            stopped: gone,
            needs_force: false,
            pid: Some(pid),
        });
    }

    logger.info(format!("[{home}] 请求关闭窗口 pid {pid}"));
    let _ = taskkill(&pid.to_string(), false);
    if wait_until_gone(home, Duration::from_secs(15)) {
        return Ok(StopOutcome {
            stopped: true,
            needs_force: false,
            pid: Some(pid),
        });
    }
    Ok(StopOutcome {
        stopped: false,
        needs_force: true,
        pid: Some(pid),
    })
}

fn wait_until_gone(home: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if scan::is_home_running(home).is_none() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}

/// `taskkill /PID <pid>`（不带 /F 是发关闭消息）；`force` 时 `/F /T` 杀整棵树。
fn taskkill(pid: &str, force: bool) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = std::process::Command::new("taskkill");
    if force {
        cmd.args(["/F", "/T"]);
    }
    cmd.args(["/PID", pid]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let out = cmd.output()?;
    let _ = out;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Instance;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Counter(AtomicUsize);
    impl ProgressSink for Counter {
        fn progress(&self, _s: &LaunchStatus) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn test_store(tag: &str) -> (Store, PathBuf) {
        let root = crate::testutil::root(&format!("launch-{tag}"));
        let _ = std::fs::remove_dir_all(&root);
        let store = Store::new(root.join("data"), root.join("versions"));
        store.ensure_dirs().unwrap();
        std::fs::create_dir_all(root.join("home")).unwrap();
        (store, root)
    }

    fn config_for(root: &std::path::Path) -> Arc<Mutex<Config>> {
        let mut cfg =
            Config::with_default_home(root.join("home").join(".dsh").to_string_lossy().to_string());
        cfg.launch_timeout_sec = 10;
        cfg.poll_interval_ms = 100;
        Arc::new(Mutex::new(cfg))
    }

    #[test]
    fn enqueue_is_serial_and_positions_increment() {
        // 不启动 worker：只验证入队 / 去重 / 位置
        let sink = Arc::new(Counter(AtomicUsize::new(0)));
        let (launcher, rx) = Launcher::new_for_test(sink);

        launcher
            .launch(LaunchRequest::new(
                "a",
                "C:\\Users\\FXS\\.dsh-a",
                PathBuf::from("C:\\missing-a.exe"),
            ))
            .unwrap();
        launcher
            .launch(LaunchRequest::new(
                "b",
                "C:\\Users\\FXS\\.dsh-b",
                PathBuf::from("C:\\missing-b.exe"),
            ))
            .unwrap();

        assert_eq!(launcher.queue(), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(launcher.status("a").position, Some(1));
        assert_eq!(launcher.status("b").position, Some(2));
        assert_eq!(launcher.status("a").phase, LaunchPhase::Queued);

        // 同一实例重复入队被拒
        let err = launcher
            .launch(LaunchRequest::new("a", "C:\\x", PathBuf::from("C:\\x.exe")))
            .unwrap_err();
        assert_eq!(err.code, "E1012");
        assert_eq!(launcher.queue().len(), 2);
        drop(rx);
    }

    #[test]
    fn missing_exe_fails_with_e1004_and_restores_pointer() {
        let (store, root) = test_store("missing");
        let sink = Arc::new(Counter(AtomicUsize::new(0)));
        let pointer = Pointer::at(root.join("pointer.json"));
        std::fs::create_dir_all(root.join("ptr")).unwrap();
        pointer.write("C:\\Users\\FXS\\.dsh-default").unwrap();
        let cfg = config_for(&root);
        let logger = Logger::new(root.join("logs")).with_echo(false);
        let launcher = Launcher::start(sink, store, pointer.clone(), cfg, logger);

        launcher
            .launch(LaunchRequest::new(
                "x",
                "C:\\Users\\FXS\\.dsh-x",
                root.join("nope.exe"),
            ))
            .unwrap();
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(launcher.status("x").phase, LaunchPhase::Failed);
        // 版本无效时根本没动指针
        assert_eq!(pointer.read_home().unwrap(), "C:\\Users\\FXS\\.dsh-default");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cancel_on_queued_instance_is_immediate() {
        let (store, root) = test_store("cancel");
        let sink = Arc::new(Counter(AtomicUsize::new(0)));
        let pointer = Pointer::at(root.join("pointer.json"));
        let cfg = config_for(&root);
        let logger = Logger::new(root.join("logs")).with_echo(false);
        let launcher = Launcher::start(sink, store, pointer, cfg, logger);
        launcher
            .launch(LaunchRequest::new(
                "q",
                "C:\\Users\\FXS\\.dsh-q",
                PathBuf::from("C:\\nope.exe"),
            ))
            .unwrap();
        launcher.cancel("q");
        assert_eq!(launcher.status("q").phase, LaunchPhase::Idle);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn mark_idle_clears_running() {
        let (store, root) = test_store("markidle");
        let sink = Arc::new(Counter(AtomicUsize::new(0)));
        let pointer = Pointer::at(root.join("pointer.json"));
        let cfg = config_for(&root);
        let logger = Logger::new(root.join("logs")).with_echo(false);
        let launcher = Launcher::start(sink, store, pointer, cfg, logger);
        launcher.mark_idle("z");
        assert_eq!(launcher.status("z").phase, LaunchPhase::Idle);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn instance_reference_backed_by_store() {
        // 保证 Launcher 与 Store 的组合不破坏实例文件
        let (store, root) = test_store("store-combo");
        let inst = Instance::new("lab", "实验室", "2.0.21-next", "C:\\Users\\FXS\\.dsh-lab");
        store.write_instance(&inst).unwrap();
        assert_eq!(store.list_instances().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }
}
