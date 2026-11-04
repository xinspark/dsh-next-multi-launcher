//! 全局指针文件（设计说明 §2.1、§5.2）。
//!
//! 唯一入口：`<启动器> 写指针 → spawn 实例`。其它地方只读。
//! 写之前必须先拿到旧值（文件不存在记为 absent），失败立刻用备份还原。
//!
//! 2.0.22 起 DSH-NEXT 还会写一份**上次启动目录**（`desktop-next-last-home.json`，同目录）：
//! 本次解析出的 home 与它不同就弹「数据目录与上次不同」模态，默认按钮是「改用上次的目录」。
//! 多开时每次切实例都会撞上它，所以指针与这份记录必须同步写（见 [`LastHome`]）。

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::model::PointerSnapshot;
use crate::util::atomic_write;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PointerFile {
    home: String,
}

/// 写指针之前的现场：用于回滚。
///
/// 备份只在内存里（`previous`）：同一目录下再落一份 `.launcher-bak` 文件既没用又会
/// 越攒越多（每次启动一份，实测攒到 39 份），所以不再写盘。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointerBackup {
    /// 写之前的原始字节（`None` = 文件本来不存在）。
    pub previous: Option<String>,
    pub target_home: String,
}

#[derive(Debug, Clone)]
pub struct Pointer {
    path: PathBuf,
}

impl Pointer {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_path() -> Self {
        Self::at(crate::util::pointer_path())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 读原始文本（文件不存在 → None；读失败 → None，并把原因交给调用方记日志）。
    pub fn read_raw(&self) -> Option<String> {
        fs::read_to_string(&self.path).ok()
    }

    /// 读 home。解析失败返回 None（调用方记日志，不猜）。
    pub fn read_home(&self) -> Option<String> {
        let raw = self.read_raw()?;
        let parsed: PointerFile = serde_json::from_str(&raw).ok()?;
        let home = parsed.home.trim().to_string();
        if home.is_empty() {
            None
        } else {
            Some(home)
        }
    }

    pub fn exists(&self) -> bool {
        self.path.is_file()
    }

    pub fn writable(&self) -> bool {
        let dir = match self.path.parent() {
            Some(d) => d,
            None => return false,
        };
        if !dir.is_dir() {
            return false;
        }
        let probe = dir.join(format!(".launcher-write-probe-{}", std::process::id()));
        match fs::write(&probe, b"probe") {
            Ok(()) => {
                let _ = fs::remove_file(&probe);
                true
            }
            Err(_) => false,
        }
    }

    pub fn snapshot(&self) -> PointerSnapshot {
        PointerSnapshot {
            path: self.path.to_string_lossy().to_string(),
            home: self.read_home(),
            raw: self.read_raw().unwrap_or_default(),
            exists: self.exists(),
            writable: self.writable(),
        }
    }

    /// 写指针：先记下旧值，再原子写。返回的回滚句柄必须被调用方持有到启动判定结束。
    pub fn write(&self, home: &str) -> Result<PointerBackup, AppError> {
        let previous = self.read_raw();
        self.write_body(home)?;

        Ok(PointerBackup {
            previous,
            target_home: home.to_string(),
        })
    }

    /// 只写内容、不留备份：`restore_default` 用（它写完就结束，回滚句柄没人持有）。
    fn write_body(&self, home: &str) -> Result<(), AppError> {
        let body = PointerFile {
            home: home.to_string(),
        };
        let mut text = serde_json::to_string_pretty(&body)?;
        text.push('\n');
        atomic_write(&self.path, text.as_bytes())
            .map_err(|e| AppError::new("E1001", format!("{}：{e}", self.path.display())))
    }

    /// 回滚到写之前的样子。回滚失败 → E1002（严重）。
    pub fn restore(&self, backup: &PointerBackup) -> Result<(), AppError> {
        match &backup.previous {
            Some(raw) => atomic_write(&self.path, raw.as_bytes())
                .map_err(|e| AppError::new("E1002", format!("{}：{e}", self.path.display()))),
            None => match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(AppError::new("E1002", format!("删除指针失败：{err}"))),
            },
        }
    }

    /// 把指针还原成默认 home（启动成功后按 `restorePointerAfterLaunch` 调用）。
    pub fn restore_default(&self, default_home: &str) -> Result<(), AppError> {
        self.write_body(default_home)
    }
}

// ---------------------------------------------------------------- 上次启动目录

/// DSH-NEXT 的「上次启动目录」记录，与指针同目录、同格式。
///
/// 启动器切实例时：本文件若还是上一个实例的 home，DSH-NEXT 会弹
/// 「数据目录与上次不同」模态（默认按钮会把这次启动改回上一个实例）。
/// 所以写指针的同时把它改成同一个 home，启动失败再回滚。
#[derive(Debug, Clone)]
pub struct LastHome {
    path: PathBuf,
}

impl LastHome {
    pub fn at(path: PathBuf) -> Self {
        Self { path }
    }

    /// 与指针同目录的 `desktop-next-last-home.json`。
    pub fn beside(pointer: &Pointer) -> Self {
        let dir = pointer
            .path()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        Self::at(dir.join("desktop-next-last-home.json"))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read_raw(&self) -> Option<String> {
        fs::read_to_string(&self.path).ok()
    }

    pub fn read_home(&self) -> Option<String> {
        let raw = self.read_raw()?;
        let parsed: PointerFile = serde_json::from_str(&raw).ok()?;
        let home = parsed.home.trim().to_string();
        if home.is_empty() {
            None
        } else {
            Some(home)
        }
    }

    /// 写成 `home`，返回写之前的原始字节（用于回滚）。
    pub fn write(&self, home: &str) -> Result<Option<String>, AppError> {
        let previous = self.read_raw();
        self.write_body(home)?;
        Ok(previous)
    }

    /// 回滚到写之前的样子。
    pub fn restore(&self, previous: &Option<String>) -> Result<(), AppError> {
        match previous {
            Some(raw) => atomic_write(&self.path, raw.as_bytes())
                .map_err(|e| AppError::new("E1002", format!("{}：{e}", self.path.display()))),
            None => match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(AppError::new("E1002", format!("删除记录失败：{err}"))),
            },
        }
    }

    /// 与指针一起还原成默认 home（启动成功后调用）。
    pub fn restore_default(&self, default_home: &str) -> Result<(), AppError> {
        self.write_body(default_home)
    }

    fn write_body(&self, home: &str) -> Result<(), AppError> {
        let body = PointerFile {
            home: home.to_string(),
        };
        let mut text = serde_json::to_string_pretty(&body)?;
        text.push('\n');
        atomic_write(&self.path, text.as_bytes())
            .map_err(|e| AppError::new("E1001", format!("{}：{e}", self.path.display())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_pointer(tag: &str) -> (Pointer, PathBuf) {
        let dir = crate::testutil::root(&format!("pointer-{tag}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("home")).unwrap();
        (
            Pointer::at(dir.join("home").join("desktop-next-location.json")),
            dir,
        )
    }

    #[test]
    fn write_then_restore_roundtrip() {
        let (p, dir) = temp_pointer("round");
        let backup = p.write("C:\\Users\\demo\\.dsh-lab").unwrap();
        assert_eq!(p.read_home().unwrap(), "C:\\Users\\demo\\.dsh-lab");

        let backup2 = p.write("C:\\Users\\demo\\.dsh-other").unwrap();
        p.restore(&backup2).unwrap();
        assert_eq!(p.read_home().unwrap(), "C:\\Users\\demo\\.dsh-lab");
        p.restore(&backup).unwrap();
        assert!(!p.exists(), "回滚到 absent 应删除文件");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn write_leaves_no_backup_file_behind() {
        let (p, dir) = temp_pointer("nobak");
        p.write("C:\\Users\\demo\\.dsh-a").unwrap();
        p.write("C:\\Users\\demo\\.dsh-b").unwrap();
        p.restore_default("C:\\Users\\demo\\.dsh").unwrap();
        let files: Vec<String> = fs::read_dir(dir.join("home"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(
            files,
            vec!["desktop-next-location.json".to_string()],
            "备份只在内存里，目录里不该再出现 .launcher-bak"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn writes_two_space_indented_json() {
        let (p, dir) = temp_pointer("format");
        p.write("C:\\Users\\demo\\.dsh-b").unwrap();
        let raw = p.read_raw().unwrap();
        assert!(
            raw.starts_with("{\n  \"home\": \"C:\\\\Users\\\\demo\\\\.dsh-b\"\n}\n"),
            "实际：{raw:?}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn snapshot_reports_missing_and_writable() {
        let (p, dir) = temp_pointer("snap");
        let snap = p.snapshot();
        assert!(!snap.exists);
        assert!(snap.writable);
        assert!(snap.home.is_none());
        p.write("C:\\Users\\demo\\.dsh").unwrap();
        let snap = p.snapshot();
        assert!(snap.exists);
        assert_eq!(snap.home.unwrap(), "C:\\Users\\demo\\.dsh");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn restore_default_writes_default() {
        let (p, dir) = temp_pointer("default");
        p.write("C:\\Users\\demo\\.dsh-lab").unwrap();
        p.restore_default("C:\\Users\\demo\\.dsh").unwrap();
        assert_eq!(p.read_home().unwrap(), "C:\\Users\\demo\\.dsh");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn malformed_file_reads_as_none() {
        let (p, dir) = temp_pointer("bad");
        fs::write(p.path(), b"{not json").unwrap();
        assert!(p.read_home().is_none());
        assert!(p.snapshot().exists);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn last_home_lives_next_to_the_pointer() {
        let (p, dir) = temp_pointer("lhmate");
        let last = LastHome::beside(&p);
        assert_eq!(
            last.path(),
            dir.join("home").join("desktop-next-last-home.json")
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn last_home_write_and_restore_roundtrip() {
        let (p, dir) = temp_pointer("lhround");
        let last = LastHome::beside(&p);
        // 首次写入：本无旧值 → 回滚是删文件
        let prev = last.write("C:\\Users\\demo\\.dsh-lab").unwrap();
        assert!(prev.is_none());
        assert_eq!(last.read_home().unwrap(), "C:\\Users\\demo\\.dsh-lab");

        let prev2 = last.write("C:\\Users\\demo\\.dsh-other").unwrap();
        assert!(prev2.is_some());
        last.restore(&prev2).unwrap();
        assert_eq!(last.read_home().unwrap(), "C:\\Users\\demo\\.dsh-lab");

        last.restore(&prev).unwrap();
        assert!(last.read_home().is_none(), "回滚到 absent 应删除文件");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn last_home_restore_default_writes_plain_home() {
        let (p, dir) = temp_pointer("lhdefault");
        let last = LastHome::beside(&p);
        last.write("C:\\Users\\demo\\.dsh-inst").unwrap();
        last.restore_default("C:\\Users\\demo\\.dsh").unwrap();
        assert_eq!(last.read_home().unwrap(), "C:\\Users\\demo\\.dsh");
        let _ = fs::remove_dir_all(dir);
    }
}
