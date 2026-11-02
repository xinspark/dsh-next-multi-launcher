//! 全局指针文件（设计说明 §2.1、§5.2）。
//!
//! 唯一入口：`<启动器> 写指针 → spawn 实例`。其它地方只读。
//! 写之前必须先拿到旧值（文件不存在记为 absent），失败立刻用备份还原。

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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointerBackup {
    /// 写之前的原始字节（`None` = 文件本来不存在）。
    pub previous: Option<String>,
    /// 备份落盘位置（`None` = 本来不存在，回滚就是删文件）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
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

    /// 写指针：先备份旧值，再原子写。返回的回滚句柄必须被调用方持有到启动判定结束。
    pub fn write(&self, home: &str) -> Result<PointerBackup, AppError> {
        let previous = self.read_raw();
        let backup_path = match &previous {
            Some(raw) => {
                let bak = self.pick_backup_path();
                fs::write(&bak, raw.as_bytes())
                    .map_err(|e| AppError::new("E1001", format!("备份指针失败：{e}")))?;
                Some(bak.to_string_lossy().to_string())
            }
            None => None,
        };

        let body = PointerFile { home: home.to_string() };
        let mut text = serde_json::to_string_pretty(&body)?;
        text.push('\n');
        atomic_write(&self.path, text.as_bytes())
            .map_err(|e| AppError::new("E1001", format!("{}：{e}", self.path.display())))?;

        Ok(PointerBackup {
            previous,
            backup_path,
            target_home: home.to_string(),
        })
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
        let backup = self.write(default_home)?;
        // 这里不需要回滚句柄；备份文件保留，便于事后诊断。
        let _ = backup;
        Ok(())
    }

    fn pick_backup_path(&self) -> PathBuf {
        let base = format!("{}.launcher-bak", self.path.display());
        let p = PathBuf::from(&base);
        if !p.exists() {
            return p;
        }
        for i in 2..10 {
            let p = PathBuf::from(format!("{base}{i}"));
            if !p.exists() {
                return p;
            }
        }
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        PathBuf::from(format!("{base}-{stamp}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_pointer(tag: &str) -> (Pointer, PathBuf) {
        let dir = crate::testutil::root(&format!("pointer-{tag}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("home")).unwrap();
        (Pointer::at(dir.join("home").join("desktop-next-location.json")), dir)
    }

    #[test]
    fn write_then_restore_roundtrip() {
        let (p, dir) = temp_pointer("round");
        let backup = p.write("C:\\Users\\FXS\\.dsh-lab").unwrap();
        assert_eq!(p.read_home().unwrap(), "C:\\Users\\FXS\\.dsh-lab");
        // 备份文件确实存在
        assert!(backup.backup_path.is_none(), "首次写入本无旧值");

        let backup2 = p.write("C:\\Users\\FXS\\.dsh-other").unwrap();
        assert!(backup2.backup_path.is_some());
        p.restore(&backup2).unwrap();
        assert_eq!(p.read_home().unwrap(), "C:\\Users\\FXS\\.dsh-lab");
        p.restore(&backup).unwrap();
        assert!(!p.exists(), "回滚到 absent 应删除文件");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn writes_two_space_indented_json() {
        let (p, dir) = temp_pointer("format");
        p.write("C:\\Users\\FXS\\.dsh-b").unwrap();
        let raw = p.read_raw().unwrap();
        assert!(raw.starts_with("{\n  \"home\": \"C:\\\\Users\\\\FXS\\\\.dsh-b\"\n}\n"), "实际：{raw:?}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn snapshot_reports_missing_and_writable() {
        let (p, dir) = temp_pointer("snap");
        let snap = p.snapshot();
        assert!(!snap.exists);
        assert!(snap.writable);
        assert!(snap.home.is_none());
        p.write("C:\\Users\\FXS\\.dsh").unwrap();
        let snap = p.snapshot();
        assert!(snap.exists);
        assert_eq!(snap.home.unwrap(), "C:\\Users\\FXS\\.dsh");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn restore_default_writes_default() {
        let (p, dir) = temp_pointer("default");
        p.write("C:\\Users\\FXS\\.dsh-lab").unwrap();
        p.restore_default("C:\\Users\\FXS\\.dsh").unwrap();
        assert_eq!(p.read_home().unwrap(), "C:\\Users\\FXS\\.dsh");
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
}
