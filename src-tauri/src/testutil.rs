//! 仅测试使用：本机沙箱只允许在工程目录内写文件（`%TEMP%` / `%LOCALAPPDATA%` 一律 ACCESS DENIED，
//! 见 docs/env-check.md 的“写盘边界”一节），所以测试的临时根目录固定放在 `target\test-tmp` 下。

use std::path::PathBuf;

pub fn root(tag: &str) -> PathBuf {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target").join("test-tmp");
    let dir = base.join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("创建测试目录失败");
    dir
}
