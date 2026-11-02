//! 删除安全网（设计说明 §5.6）。
//!
//! 只回答一个问题：这个目录能不能被“连数据一起删”。
//! 拒绝条件：非绝对路径、盘根、含 `..`、长度 < 8、等于默认 home、
//! 是 `%APPDATA%` / `%USERPROFILE%` / `%SystemRoot%` / 启动器数据目录或其上级。

use std::path::{Component, Path, PathBuf};

use crate::errors::AppError;
use crate::util::{norm_path, path_eq, path_starts_with};

#[derive(Debug, Clone)]
pub struct SafetyRoots {
    pub appdata: Option<PathBuf>,
    pub localappdata: Option<PathBuf>,
    pub user_profile: Option<PathBuf>,
    pub system_root: Option<PathBuf>,
    pub data_dir: PathBuf,
    pub default_home: PathBuf,
}

impl SafetyRoots {
    pub fn current(default_home: &Path) -> Self {
        Self {
            appdata: crate::util::env_path("APPDATA"),
            localappdata: crate::util::env_path("LOCALAPPDATA"),
            user_profile: crate::util::env_path("USERPROFILE"),
            system_root: crate::util::env_path("SystemRoot"),
            data_dir: crate::util::data_dir(),
            default_home: default_home.to_path_buf(),
        }
    }

    /// 受保护路径清单（用 `;` 拼起来便于写进错误详情）。
    fn protected(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        out.push(self.default_home.clone());
        out.push(self.data_dir.clone());
        for p in [
            &self.appdata,
            &self.localappdata,
            &self.user_profile,
            &self.system_root,
        ]
        .into_iter()
        .flatten()
        {
            out.push(p.clone());
        }
        out
    }
}

pub fn assert_safe_delete_target(target: &Path, roots: &SafetyRoots) -> Result<(), AppError> {
    let reject = |reason: &str| Err(AppError::new("E1007", reason.to_string()));

    if !target.is_absolute() {
        return reject(&format!("不是绝对路径：{}", target.display()));
    }
    let text = target.to_string_lossy();
    if text.trim().len() < 8 {
        return reject(&format!("路径太短（<8）：{text}"));
    }
    if target
        .components()
        .any(|c| matches!(c, Component::ParentDir))
    {
        return reject(&format!("路径含 ..：{text}"));
    }
    if target.parent().is_none() || norm_path(target).len() <= 2 {
        return reject(&format!("盘根不允许删除：{text}"));
    }

    for protected in roots.protected() {
        if path_eq(target, &protected) {
            return reject(&format!(
                "受保护路径（等于 {}）：{text}",
                protected.display()
            ));
        }
        // 目标若是受保护路径的上级，删除会连坐，拒绝
        if path_starts_with(&protected, target) {
            return reject(&format!(
                "受保护路径的上级：{text}（会波及 {}）",
                protected.display()
            ));
        }
    }

    // 目标位于系统目录内部也没意义，但那些目录不归启动器管；只额外拦一层 Windows 根
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots() -> SafetyRoots {
        SafetyRoots {
            appdata: Some(PathBuf::from(r"C:\Users\FXS\AppData\Roaming")),
            localappdata: Some(PathBuf::from(r"C:\Users\FXS\AppData\Local")),
            user_profile: Some(PathBuf::from(r"C:\Users\FXS")),
            system_root: Some(PathBuf::from(r"C:\Windows")),
            data_dir: PathBuf::from(r"C:\Users\FXS\AppData\Local\DSH-NEXT-Multi"),
            default_home: PathBuf::from(r"C:\Users\FXS\.dsh"),
        }
    }

    #[test]
    fn allows_normal_instance_home() {
        assert!(assert_safe_delete_target(Path::new(r"C:\Users\FXS\.dsh-lab"), &roots()).is_ok());
    }

    #[test]
    fn rejects_drive_root() {
        assert_eq!(
            assert_safe_delete_target(Path::new(r"C:\"), &roots())
                .unwrap_err()
                .code,
            "E1007"
        );
    }

    #[test]
    fn rejects_default_home() {
        let err = assert_safe_delete_target(Path::new(r"C:\Users\FXS\.dsh"), &roots()).unwrap_err();
        assert_eq!(err.code, "E1007");
    }

    #[test]
    fn rejects_user_profile_and_ancestors() {
        assert!(assert_safe_delete_target(Path::new(r"C:\Users\FXS"), &roots()).is_err());
        assert!(assert_safe_delete_target(Path::new(r"C:\Users"), &roots()).is_err());
        assert!(assert_safe_delete_target(Path::new(r"C:\"), &roots()).is_err());
    }

    #[test]
    fn rejects_dotdot_and_short_paths() {
        assert!(
            assert_safe_delete_target(Path::new(r"C:\Users\FXS\..\Windows"), &roots()).is_err()
        );
        assert!(assert_safe_delete_target(Path::new(r"C:\a"), &roots()).is_err());
    }

    #[test]
    fn rejects_launcher_data_dir() {
        assert!(assert_safe_delete_target(
            Path::new(r"C:\Users\FXS\AppData\Local\DSH-NEXT-Multi"),
            &roots()
        )
        .is_err());
    }

    #[test]
    fn case_insensitive() {
        assert!(assert_safe_delete_target(Path::new(r"c:\users\fxs\.DSH"), &roots()).is_err());
    }
}
