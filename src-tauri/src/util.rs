//! 路径 / 时间 / 原子写 / 日志 —— 基础设施，不含业务判断。

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Local;

/// 本地时间 ISO-8601（秒精度，带时区偏移）。
pub fn now_iso() -> String {
    Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

pub fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).map(PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

/// `%LOCALAPPDATA%\DSH-NEXT-Multi`（可用 `DSH_MULTI_DATA_DIR` 覆盖，测试用）。
/// 数据目录：按「可写」逐级兜底，保证应用在任何环境下都能起来。
///
/// 1. `DSH_MULTI_DATA_DIR`（显式指定，测试 / 便携部署用）
/// 2. `%LOCALAPPDATA%\DSH-NEXT-Multi`（标准安装）
/// 3. `<应用根>\DSH-NEXT-Multi-data`（便携模式：系统目录不可写时，数据跟着 exe 走）
/// 4. `%TEMP%\DSH-NEXT-Multi`（最后兜底：能跑，但数据不持久）
///
/// 第 3 档是**必需**的：在受限进程里（例如继承了沙箱的运行环境），
/// `%LOCALAPPDATA%` 与 `%TEMP%` 下新建目录会被拒绝，而 exe 所在目录往往仍可写。
pub fn data_dir() -> PathBuf {
    if let Some(p) = env_path("DSH_MULTI_DATA_DIR") {
        return p;
    }
    static CACHE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            let standard = env_path("LOCALAPPDATA")
                .or_else(|| env_path("APPDATA"))
                .unwrap_or_else(|| std::env::temp_dir())
                .join("DSH-NEXT-Multi");
            if is_writable_dir(&standard) {
                return standard;
            }
            let portable = app_root().join("DSH-NEXT-Multi-data");
            if is_writable_dir(&portable) {
                return portable;
            }
            let temp = std::env::temp_dir().join("DSH-NEXT-Multi");
            if is_writable_dir(&temp) {
                return temp;
            }
            standard
        })
        .clone()
}

/// 目录可写 = 能建目录 + 能写探针文件（两者都过才算）。
pub fn is_writable_dir(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(format!(".write-probe-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"1").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// 版本树根目录：默认 `<应用根>\versions`，可用 `DSH_MULTI_VERSIONS` 覆盖。
pub fn versions_dir() -> PathBuf {
    if let Some(p) = env_path("DSH_MULTI_VERSIONS") {
        return p;
    }
    app_root().join("versions")
}

/// 应用根目录。开发态 exe 落在 `src-tauri\target\{debug,release}\`，要回退三级，
/// 这样 `versions\` 始终落在工程根目录，而不是 target 里。
pub fn app_root() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let dir = exe.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let is_target = dir
        .file_name()
        .map(|n| n.eq_ignore_ascii_case("debug") || n.eq_ignore_ascii_case("release"))
        .unwrap_or(false);
    if is_target {
        if let Some(t) = dir.parent() {
            if t.file_name().map(|n| n.eq_ignore_ascii_case("target")).unwrap_or(false) {
                if let Some(root) = t.parent().and_then(Path::parent) {
                    return root.to_path_buf();
                }
            }
        }
    }
    dir
}

/// DSH-NEXT 的全局指针文件（可用 `DSH_MULTI_POINTER` 覆盖，测试用）。
pub fn pointer_path() -> PathBuf {
    if let Some(p) = env_path("DSH_MULTI_POINTER") {
        return p;
    }
    let base = env_path("APPDATA").unwrap_or_else(|| data_dir());
    base.join("DSH NEXT").join("home").join("desktop-next-location.json")
}

/// 用户 home（`%USERPROFILE%`）。
pub fn user_home() -> PathBuf {
    env_path("USERPROFILE").unwrap_or_else(|| data_dir())
}

/// Windows 路径比较：大小写不敏感，去掉结尾分隔符。
pub fn path_eq(a: &Path, b: &Path) -> bool {
    norm_path(a) == norm_path(b)
}

/// `b` 是否在 `a` 之下（不含相等）。
pub fn path_starts_with(a: &Path, b: &Path) -> bool {
    let na = norm_path(a);
    let nb = norm_path(b);
    na != nb && na.starts_with(&format!("{nb}\\"))
}

pub fn norm_path(p: &Path) -> String {
    let mut s = p.to_string_lossy().replace('/', "\\");
    while s.len() > 3 && s.ends_with('\\') {
        s.pop();
    }
    s.to_lowercase()
}

/// 原子写：同目录 tmp → flush → sync → rename（遇到占用重试几次）。
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = tmp_sibling(path);
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.flush()?;
        f.sync_all()?;
    }
    let mut last_err: Option<std::io::Error> = None;
    for attempt in 0..5 {
        match fs::rename(&tmp, path) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                std::thread::sleep(std::time::Duration::from_millis(40 * (attempt + 1) as u64));
            }
        }
    }
    let _ = fs::remove_file(&tmp);
    Err(last_err.unwrap_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "rename failed")))
}

fn tmp_sibling(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
    let tmp = format!(".{name}.{}.tmp", std::process::id());
    match path.parent() {
        Some(p) => p.join(tmp),
        None => PathBuf::from(tmp),
    }
}

// ---------------------------------------------------------------- 日志

/// 极简日志：`logs\launcher-YYYYMMDD.log`，一行一条，失败不 panic。
#[derive(Debug, Clone)]
pub struct Logger {
    dir: PathBuf,
    echo: bool,
}

impl Logger {
    pub fn new(dir: PathBuf) -> Self {
        let _ = fs::create_dir_all(&dir);
        Self { dir, echo: cfg!(debug_assertions) }
    }

    pub fn with_echo(mut self, echo: bool) -> Self {
        self.echo = echo;
        self
    }

    pub fn path_today(&self) -> PathBuf {
        let day = Local::now().format("%Y%m%d");
        self.dir.join(format!("launcher-{day}.log"))
    }

    pub fn info(&self, message: impl AsRef<str>) {
        self.write("INFO", message.as_ref());
    }

    pub fn warn(&self, message: impl AsRef<str>) {
        self.write("WARN", message.as_ref());
    }

    pub fn error(&self, message: impl AsRef<str>) {
        self.write("ERROR", message.as_ref());
    }

    pub fn write(&self, level: &str, message: &str) {
        let line = format!("{} [{}] {}\n", Local::now().format("%Y-%m-%d %H:%M:%S"), level, message);
        if self.echo {
            print!("{line}");
        }
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(self.path_today()) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_path_is_case_insensitive_and_trimmed() {
        let a = Path::new("C:\\Users\\FXS\\.dsh\\");
        let b = Path::new("c:/users/fxs/.dsh");
        assert_eq!(norm_path(a), norm_path(b));
        assert!(path_eq(a, b));
    }

    #[test]
    fn starts_with_handles_boundary() {
        let root = Path::new("C:\\Users\\FXS\\.dsh");
        assert!(path_starts_with(Path::new("C:\\Users\\FXS\\.dsh-a"), root) == false);
        assert!(path_starts_with(Path::new("C:\\Users\\FXS\\.dsh-a\\x"), root) == false);
        assert!(path_starts_with(Path::new("C:\\Users\\FXS\\.dsh\\profiles"), root));
    }

    #[test]
    fn atomic_write_roundtrip() {
        let dir = crate::testutil::root("dsh-multi-test");
        let _ = fs::create_dir_all(&dir);
        let file = dir.join("a.json");
        atomic_write(&file, b"{\"a\":1}").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "{\"a\":1}");
        atomic_write(&file, b"{\"a\":2}").unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), "{\"a\":2}");
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
