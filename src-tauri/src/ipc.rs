//! 实例注册表（跨进程只读文件）—— 1.2「多实例工作协议」的最小地基。
//!
//! 分工：**启动器是唯一知道全部实例的进程**（id / home / 版本 / 运行状态），
//! 插件（`DSH-NUL`）是唯一知道"会话归属、工作区里谁在干活"的一方。
//! 两者之间不开端口、不开常驻服务，只走一个文件：
//!
//! ```text
//! <启动器数据目录>\ipc\instances.json
//! ```
//!
//! 启动器写、插件只读；写入走 `util::atomic_write`（同目录 tmp → rename），
//! 因此读到的永远是完整 JSON，不会撞上写了一半的内容。
//!
//! 契约稳定性：`schemaVersion` 变更才算破坏性变化。新增字段一律可选，
//! 插件侧必须容忍未知字段（`serde` 默认行为）。
//!
//! 这是**只读清单**，不是锁：这里没有任何"谁可以写某个文件"的判决，
//! 写锁要等心跳 + 超时自动失效那套机制先做扎实（见 docs-private/roadmap.md 1.2）。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::model::{Instance, RunningInfo};
use crate::util::{atomic_write, now_iso};

pub const SCHEMA_VERSION: u32 = 1;

/// 注册文件相对启动器数据目录的位置。
pub fn registry_path(data_dir: &Path) -> PathBuf {
    data_dir.join("ipc").join("instances.json")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryLauncher {
    pub pid: u32,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryInstance {
    pub id: String,
    pub name: String,
    /// 该实例的数据目录（绝对路径）。插件据此判断"是不是另一个 home"。
    pub home: String,
    /// 版本树里的 tag（`versions\<tag>`）。
    pub version: String,
    pub archived: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_pid: Option<u32>,
    /// 主进程启动时间（Unix 秒）；没在跑就是 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<u64>,
    pub safe_mode: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_launch_at: Option<String>,
}

impl RegistryInstance {
    pub fn from_parts(instance: &Instance, running: Option<&RunningInfo>) -> Self {
        Self {
            id: instance.id.clone(),
            name: instance.name.clone(),
            home: instance.home.clone(),
            version: instance.version.clone(),
            archived: instance.archived,
            shell_pid: running.map(|r| r.shell_pid),
            host_pid: running.and_then(|r| r.host_pid),
            started_at: running.map(|r| r.started_at),
            // 数据库里的 safe_mode 是"上次启动带没带安全模式参数"，运行时以进程为准；
            // 两边都取不到才算 false。
            safe_mode: running.map(|r| r.safe_mode).unwrap_or(instance.safe_mode),
            last_launch_at: instance.last_launch_at.clone(),
        }
    }

    pub fn running(&self) -> bool {
        self.shell_pid.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    pub schema_version: u32,
    /// 本文件生成时间（本地时间戳，用于判断插件读到的是不是陈旧数据）。
    pub generated_at: String,
    pub launcher: RegistryLauncher,
    /// 全部实例（含归档的），按 id 排序，保证同一次输入产出逐字节相同的内容。
    pub instances: Vec<RegistryInstance>,
}

/// 组装注册表。纯函数：不读盘、不扫进程，输入即输出，便于单测。
pub fn build(
    instances: &[Instance],
    running: &[RunningInfo],
    launcher_pid: u32,
    launcher_version: &str,
) -> Registry {
    let mut items: Vec<RegistryInstance> = instances
        .iter()
        .map(|inst| {
            let live = running
                .iter()
                .find(|r| crate::util::path_eq(Path::new(&r.home), Path::new(&inst.home)));
            RegistryInstance::from_parts(inst, live)
        })
        .collect();
    items.sort_by(|a, b| a.id.cmp(&b.id));

    Registry {
        schema_version: SCHEMA_VERSION,
        generated_at: now_iso(),
        launcher: RegistryLauncher {
            pid: launcher_pid,
            version: launcher_version.to_string(),
        },
        instances: items,
    }
}

/// 写注册表。目录不存在就建；原子写保证插件读不到半截 JSON。
pub fn write(path: &Path, registry: &Registry) -> Result<(), AppError> {
    let mut bytes = serde_json::to_vec_pretty(registry)?;
    bytes.push(b'\n');
    atomic_write(path, &bytes).map_err(|e| {
        AppError::new(
            "E1000",
            format!("写入实例注册表失败 {}：{e}", path.display()),
        )
    })
}

/// 从注册表里挑出"正在运行、且 home 与给定路径不同"的实例。
///
/// 这是 1.2 最小版的判据函数：插件拿它回答"这里还有谁在跑"。纯函数，方便单测。
pub fn others_running<'a>(registry: &'a Registry, home: &str) -> Vec<&'a RegistryInstance> {
    registry
        .instances
        .iter()
        .filter(|i| i.running())
        .filter(|i| !crate::util::path_eq(Path::new(&i.home), Path::new(home)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Instance;

    fn running(home: &str, shell: u32, host: Option<u32>, safe: bool) -> RunningInfo {
        RunningInfo {
            home: home.to_string(),
            shell_pid: shell,
            host_pid: host,
            started_at: 1_700_000_000,
            safe_mode: safe,
        }
    }

    #[test]
    fn maps_live_process_state_and_keeps_archived_entries() {
        let a = Instance::new("a", "自用", "2.0.23-next", r"C:\Users\demo\.dsh-a");
        let mut b = Instance::new("b", "归档", "2.0.23-next", r"C:\Users\demo\.dsh-b");
        b.archived = true;
        let live = running(r"C:\Users\demo\.dsh-a", 100, Some(200), true);

        let reg = build(&[a, b], &[live], 999, "1.1.3");

        assert_eq!(reg.schema_version, SCHEMA_VERSION);
        assert_eq!(reg.launcher.pid, 999);
        assert_eq!(
            reg.instances.len(),
            2,
            "归档实例也要列出来，插件才知道它存在"
        );

        let ia = reg.instances.iter().find(|i| i.id == "a").unwrap();
        assert_eq!(ia.shell_pid, Some(100));
        assert_eq!(ia.host_pid, Some(200));
        assert_eq!(ia.started_at, Some(1_700_000_000));
        assert!(ia.safe_mode, "运行时以进程判定为准");
        assert!(ia.running());

        let ib = reg.instances.iter().find(|i| i.id == "b").unwrap();
        assert_eq!(ib.shell_pid, None);
        assert!(!ib.running());
        assert!(ib.archived);
    }

    #[test]
    fn home_match_is_case_insensitive_and_trailing_slash_insensitive() {
        let inst = Instance::new("a", "x", "2.0.23-next", r"C:\Users\demo\.dsh-a");
        let live = running(r"c:/users/demo/.DSH-A/", 7, None, false);
        let reg = build(&[inst], &[live], 1, "1.1.3");
        assert_eq!(reg.instances[0].shell_pid, Some(7));
    }

    #[test]
    fn entries_are_sorted_by_id_for_byte_stable_output() {
        let a = Instance::new("z", "z", "t", r"C:\z");
        let b = Instance::new("a", "a", "t", r"C:\a");
        let reg = build(&[a, b], &[], 1, "1.1.3");
        assert_eq!(
            reg.instances
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "z"]
        );
    }

    #[test]
    fn others_running_excludes_own_home_and_stopped_instances() {
        let a = Instance::new("a", "a", "t", r"C:\Users\demo\.dsh-a");
        let b = Instance::new("b", "b", "t", r"C:\Users\demo\.dsh-b");
        let c = Instance::new("c", "c", "t", r"C:\Users\demo\.dsh-c");
        let live = vec![
            running(r"C:\Users\demo\.dsh-a", 1, None, false),
            running(r"C:\Users\demo\.dsh-b", 2, None, false),
        ];
        let reg = build(&[a, b, c], &live, 1, "1.1.3");

        let others = others_running(&reg, r"C:\Users\demo\.dsh-a");
        assert_eq!(others.len(), 1);
        assert_eq!(others[0].id, "b", "自己不算、没在跑的不算");
    }

    #[test]
    fn roundtrips_through_json_with_camel_case_keys() {
        let inst = Instance::new("a", "自用", "2.0.23-next", r"C:\Users\demo\.dsh-a");
        let reg = build(&[inst], &[], 1, "1.1.3");
        let text = serde_json::to_string_pretty(&reg).unwrap();
        assert!(
            text.contains("\"schemaVersion\""),
            "落盘一律 camelCase：{text}"
        );
        assert!(text.contains("\"generatedAt\""));
        let back: Registry = serde_json::from_str(&text).unwrap();
        assert_eq!(back.schema_version, SCHEMA_VERSION);
        assert!(!back.instances[0].running());
    }

    #[test]
    fn write_creates_dir_and_produces_readable_json() {
        let dir = crate::testutil::root("ipc-registry");
        let _ = std::fs::remove_dir_all(&dir);
        let path = registry_path(&dir);
        let inst = Instance::new("a", "自用", "2.0.23-next", r"C:\Users\demo\.dsh-a");
        let reg = build(&[inst], &[], 42, "1.1.3");

        write(&path, &reg).unwrap();

        assert!(path.is_file());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.ends_with('\n'));
        let back: Registry = serde_json::from_str(&text).unwrap();
        assert_eq!(back.launcher.pid, 42);
        // 目录里只应有注册表本身（原子写的临时文件已 rename 掉）
        let files: Vec<String> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(files, vec!["instances.json".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
