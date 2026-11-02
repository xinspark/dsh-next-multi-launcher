//! 权限相关：当前进程是否管理员、关键目录是否可写、以管理员身份重启自己。
//!
//! 背景：DSH-NEXT 的全局指针目录（`%APPDATA%\DSH NEXT\home`）与 WebView2 数据目录
//! 如果曾被**管理员身份**的进程创建过，ACL 可能只允许管理员写入；此后普通权限的启动器
//! 就会写不了指针（启动实例失败 E1001）或建不了 WebView2 目录。
//! 因此启动器需要在权限不足时能明确告知，并支持“以管理员身份重启”。

/// 当前进程是否以管理员（高完整性）身份运行。
pub fn is_admin() -> bool {
    #[cfg(windows)]
    unsafe {
        IsUserAnAdmin() != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(windows)]
unsafe extern "system" {
    fn IsUserAnAdmin() -> i32;
    fn ShellExecuteW(
        hwnd: *mut core::ffi::c_void,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> isize;
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 以管理员身份重启自己。返回 true 表示 UAC 已接受、新进程已启动（本进程应退出）。
pub fn relaunch_elevated() -> bool {
    #[cfg(windows)]
    {
        let exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return false,
        };
        // 原样带上命令行参数，并追加 `--elevated` 标记防循环
        let mut args: Vec<String> = std::env::args()
            .skip(1)
            .filter(|a| a != "--elevated")
            .collect();
        args.push("--elevated".to_string());
        let params = args.join(" ");
        let op = wide("runas");
        let file = wide(&exe.to_string_lossy());
        let params_w = wide(&params);
        // SW_SHOWNORMAL = 1；返回值 > 32 表示成功
        let ret = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                file.as_ptr(),
                params_w.as_ptr(),
                std::ptr::null(),
                1,
            )
        };
        ret > 32
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// 需要提权的判据：指针目录写不了，或数据目录已退到“便携/临时”位置。
/// 是否需要提权：**只看指针目录能不能写**。
///
/// 早先这里还用「数据目录是否等于 `%LOCALAPPDATA%\DSH-NEXT-Multi`」来判断，
/// 结果把「便携模式 / 自定义数据目录」误判成需要提权，横幅还会写成「指针目录不可写」——
/// 两种情况的原因完全不同。数据目录能跑起来就说明它可写，不需要再推断。
pub fn needs_elevation(pointer_writable: bool) -> bool {
    !pointer_writable
}

/// 一次性权限体检，供界面展示与排障。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionReport {
    pub is_admin: bool,
    pub elevated_by_env: bool,
    pub data_dir: String,
    pub data_writable: bool,
    pub pointer_path: String,
    pub pointer_writable: bool,
    pub needs_elevation: bool,
}

pub fn report() -> PermissionReport {
    let pointer = crate::pointer::Pointer::default_path();
    let pointer_file = pointer.path().to_path_buf();
    let pointer_writable = pointer_file
        .parent()
        .map(crate::util::is_writable_dir)
        .unwrap_or(false);
    let data_dir = crate::util::data_dir();
    let data_writable = crate::util::is_writable_dir(&data_dir);
    PermissionReport {
        is_admin: is_admin(),
        elevated_by_env: std::env::var_os("DSH_MULTI_ELEVATED").is_some(),
        data_dir: data_dir.to_string_lossy().to_string(),
        data_writable,
        pointer_path: pointer_file.to_string_lossy().to_string(),
        pointer_writable,
        needs_elevation: needs_elevation(pointer_writable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 只有指针不可写才需要提权：便携模式/自定义数据目录不再误报。
    #[test]
    fn elevation_only_depends_on_pointer_writability() {
        assert!(needs_elevation(false));
        assert!(!needs_elevation(true));
    }
}
