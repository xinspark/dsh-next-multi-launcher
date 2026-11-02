//! 数据模型（设计说明 §4）。
//!
//! 落盘字段名固定为 camelCase，和 `app.json` / `instances\<id>.json` 的示例逐字对应；
//! 读取时对缺省字段给默认值，保证老文件升级不会崩。

use serde::{Deserialize, Serialize};

use crate::errors::AppError;

pub const SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------- Config

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastPointer {
    pub home: String,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub schema_version: u32,
    /// 普通双击桌面图标应落入的环境（黄条判据之一）。
    pub default_home: String,
    /// 数据目录树根：新建实例时 home 的**默认值**前缀，形如 `<homeRoot>\.dsh-<id>`。
    /// 只影响默认值，不限制用户手改 home。空串 = 用 `%USERPROFILE%`。
    #[serde(default)]
    pub home_root: String,
    pub launch_timeout_sec: u64,
    pub poll_interval_ms: u64,
    /// 启动成功后是否把指针还原为 `default_home`（默认 true）。
    pub restore_pointer_after_launch: bool,
    /// 下载加速前缀（可选）：GitHub 安装包地址前面拼上它再交给浏览器。
    /// 空串 = 直连 GitHub。第三方代理随时会失效，所以做成用户可改的配置，
    /// 不在代码里写死任何一家。
    #[serde(default)]
    pub download_mirror: String,
    /// 主题：`system`（跟随系统）/ `light` / `dark`；空串等同 `system`。
    #[serde(default)]
    pub theme: String,
    /// 首次使用引导（老配置缺省 = 未完成）
    #[serde(default)]
    pub onboarding: OnboardingState,
    #[serde(default)]
    pub last_pointer: Option<LastPointer>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            default_home: String::new(), // 由 `Config::with_default_home` 填
            home_root: String::new(),    // 空 = 用 %USERPROFILE%
            launch_timeout_sec: 90,
            poll_interval_ms: 500,
            restore_pointer_after_launch: true,
            download_mirror: String::new(),
            theme: "system".to_string(),
            onboarding: OnboardingState::default(),
            last_pointer: None,
        }
    }
}

impl Config {
    pub fn with_default_home(home: impl Into<String>) -> Self {
        Self {
            default_home: home.into(),
            ..Self::default()
        }
    }

    /// 数据目录树根的实际值：空串回落到 `%USERPROFILE%`，去掉结尾分隔符。
    pub fn effective_home_root(&self) -> String {
        let root = self.home_root.trim();
        if root.is_empty() {
            crate::util::user_home().to_string_lossy().to_string()
        } else {
            root.trim_end_matches(['\\', '/']).to_string()
        }
    }

    /// 把明显越界的值夹回合法区间，避免手改配置把启动管道拖死。
    pub fn sanitized(mut self) -> Self {
        self.launch_timeout_sec = self.launch_timeout_sec.clamp(10, 600);
        self.poll_interval_ms = self.poll_interval_ms.clamp(100, 5_000);
        self.onboarding = self.onboarding.sanitized();
        self.schema_version = SCHEMA_VERSION;
        self
    }
}

// ---------------------------------------------------------------- 初始化向导

/// 首次使用引导的状态（设计见 `docs/design-onboarding.md` §4）。
///
/// 旧配置里没有这个字段 → `Default` 全空 → 视为「从未完成」，下次启动仍会展开。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingState {
    /// 完成时间；有值后不再自动展开
    #[serde(default)]
    pub completed_at: Option<String>,
    /// 跳过时间；两个时间都为空时下次仍会展开
    #[serde(default)]
    pub skipped_at: Option<String>,
    /// 中断后回到哪一步（1–4）；0 = 还没开始
    #[serde(default)]
    pub last_step: u8,
}

impl OnboardingState {
    /// 是否应该自动展开向导：既没完成过、也没跳过过。
    pub fn auto_open(&self) -> bool {
        self.completed_at.is_none() && self.skipped_at.is_none()
    }

    /// 步骤夹取到 0–4，避免手改配置写出越界值。
    pub fn sanitized(mut self) -> Self {
        self.last_step = self.last_step.min(4);
        self
    }
}

// ---------------------------------------------------------------- Instance

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    pub schema_version: u32,
    /// `[a-z0-9-]{1,32}`，创建后不可改；等于文件名。
    pub id: String,
    pub name: String,
    /// 必须等于 versions\ 下的某个目录名。
    pub version: String,
    /// 绝对路径；未归档实例之间不允许重复。
    pub home: String,
    pub archived: bool,
    pub created_at: String,
    #[serde(default)]
    pub last_launch_at: Option<String>,
    #[serde(default)]
    pub last_exit_clean: bool,
    #[serde(default)]
    pub notes: String,
    /// 上次启动是否带安全模式参数（`--next-safe-mode`）；决定下拉显示“进入/退出安全模式”。
    #[serde(default)]
    pub safe_mode: bool,
}

impl Instance {
    pub fn new(id: &str, name: &str, version: &str, home: &str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            id: id.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            home: home.to_string(),
            archived: false,
            created_at: crate::util::now_iso(),
            last_launch_at: None,
            last_exit_clean: true,
            notes: String::new(),
            safe_mode: false,
        }
    }
}

// ---------------------------------------------------------------- 版本

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub tag: String,
    pub dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exe: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asar: Option<String>,
    /// app.asar!/package.json `.version`；读不到就是 "?"，不猜。
    pub shell_version: String,
    /// `dependencies["@deepseek-ai/dsh"]`
    pub kernel_version: String,
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid_reason: Option<String>,
    /// `versions\<tag>` 是否是指向安装树的目录链接（junction / symlink）。
    /// 启动器默认只建链接不复制，这个字段让界面能说清楚“它的实体在哪”。
    pub is_link: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_target: Option<String>,
}

// ---------------------------------------------------------------- 运行态

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunningInfo {
    pub home: String,
    pub shell_pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_pid: Option<u32>,
    /// 主进程启动时间（Unix 秒），前端自行算“已运行 N 分钟”。
    pub started_at: u64,
    /// 该实例是否以安全模式运行（命令行含 `--next-safe-mode`，或 userData 落在 safe-mode 子目录）。
    #[serde(default)]
    pub safe_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchPhase {
    Queued,
    Preparing,
    Spawning,
    Waiting,
    Running,
    Failed,
    Idle,
}

impl LaunchPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            LaunchPhase::Queued => "queued",
            LaunchPhase::Preparing => "preparing",
            LaunchPhase::Spawning => "spawning",
            LaunchPhase::Waiting => "waiting",
            LaunchPhase::Running => "running",
            LaunchPhase::Failed => "failed",
            LaunchPhase::Idle => "idle",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchStatus {
    pub instance_id: String,
    pub phase: LaunchPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    pub elapsed_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
}

impl LaunchStatus {
    pub fn idle(instance_id: &str) -> Self {
        Self {
            instance_id: instance_id.to_string(),
            phase: LaunchPhase::Idle,
            position: None,
            started_at: None,
            elapsed_ms: 0,
            shell_pid: None,
            host_pid: None,
            error: None,
        }
    }
}

// ---------------------------------------------------------------- 指针

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointerSnapshot {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home: Option<String>,
    /// 文件原始内容（用于诊断“上次把指针写成了什么”）。
    pub raw: String,
    pub exists: bool,
    pub writable: bool,
}

// ---------------------------------------------------------------- 视图

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceView {
    pub instance: Instance,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running: Option<RunningInfo>,
    pub launch: LaunchStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<VersionInfo>,
    /// `home == defaultHome` 实时计算，不落库。
    pub is_default: bool,
    pub home_exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub config: Config,
    pub pointer: PointerSnapshot,
    pub versions: Vec<VersionInfo>,
    pub instances: Vec<InstanceView>,
    pub running: Vec<RunningInfo>,
    /// 队列里的实例 id（按出队顺序）。
    pub queue: Vec<String>,
    pub data_dir: String,
    pub versions_dir: String,
    pub log_dir: String,
    pub app_version: String,
    /// `%USERPROFILE%`，供新建对话框推导默认 home。
    pub user_profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopOutcome {
    pub stopped: bool,
    /// true 表示 15 秒内没退出，需要用户确认后强杀。
    pub needs_force: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

#[cfg(test)]
mod onboarding_tests {
    use super::*;

    #[test]
    fn fresh_config_auto_opens() {
        let cfg = Config::default();
        assert!(cfg.onboarding.auto_open(), "全新配置应自动展开向导");
    }

    #[test]
    fn skip_or_complete_stops_auto_open() {
        let mut o = OnboardingState::default();
        assert!(o.auto_open());

        o.skipped_at = Some("2026-11-03T01:00:00+08:00".into());
        assert!(!o.auto_open(), "跳过后不该再自动展开");

        // 重开（清 skippedAt）后又会展开
        o.skipped_at = None;
        assert!(o.auto_open());

        o.completed_at = Some("2026-11-03T01:00:00+08:00".into());
        assert!(!o.auto_open(), "完成后不该再自动展开");
    }

    #[test]
    fn last_step_is_clamped_to_four() {
        let mut cfg = Config::default();
        cfg.onboarding.last_step = 99;
        assert_eq!(cfg.sanitized().onboarding.last_step, 4);
    }

    /// 老 `app.json` 没有 `onboarding` 字段：必须照常反序列化，并视为「未完成」。
    #[test]
    fn legacy_config_without_onboarding_still_loads() {
        let json = r#"{
            "schemaVersion": 1,
            "defaultHome": "C:\\Users\\demo\\.dsh",
            "homeRoot": "C:\\Users\\demo",
            "launchTimeoutSec": 90,
            "pollIntervalMs": 500,
            "restorePointerAfterLaunch": true
        }"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
        assert!(cfg.onboarding.auto_open());
        assert_eq!(cfg.onboarding.last_step, 0);
        // 回写时也会把 onboarding 一并落盘
        let text = serde_json::to_string(&cfg).unwrap();
        assert!(text.contains("\"onboarding\""));
    }
}
