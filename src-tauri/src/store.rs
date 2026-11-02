//! 实例与全局配置的读写（设计说明 §4、§5.1）。

use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::AppError;
use crate::model::{Config, Instance, SCHEMA_VERSION};
use crate::util::{atomic_write, norm_path, now_iso, path_eq, user_home};

#[derive(Debug, Clone)]
pub struct Store {
    pub data_dir: PathBuf,
    pub versions_dir: PathBuf,
}

impl Store {
    pub fn new(data_dir: PathBuf, versions_dir: PathBuf) -> Self {
        Self {
            data_dir,
            versions_dir,
        }
    }

    /// 生产环境默认路径（`%LOCALAPPDATA%\DSH-NEXT-Multi` + `<应用根>\versions`）。
    pub fn from_env() -> Self {
        Self::new(crate::util::data_dir(), crate::util::versions_dir())
    }

    pub fn config_path(&self) -> PathBuf {
        self.data_dir.join("app.json")
    }

    pub fn instances_dir(&self) -> PathBuf {
        self.data_dir.join("instances")
    }

    pub fn instance_path(&self, id: &str) -> PathBuf {
        self.instances_dir().join(format!("{id}.json"))
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join("cache")
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.instances_dir())?;
        fs::create_dir_all(self.logs_dir())?;
        fs::create_dir_all(self.cache_dir())?;
        fs::create_dir_all(&self.versions_dir)?;
        Ok(())
    }

    // ------------------------------------------------------------ 配置

    /// 读配置；文件缺失就用默认值落盘（设计 §5.1：缺失时返回默认值并落盘）。
    pub fn load_config(&self) -> Result<Config, AppError> {
        let path = self.config_path();
        match fs::read_to_string(&path) {
            Ok(text) => {
                let cfg: Config = serde_json::from_str(&text)
                    .map_err(|e| AppError::new("E1000", format!("app.json 解析失败：{e}")))?;
                let mut cfg = cfg.sanitized();
                if cfg.default_home.trim().is_empty() {
                    cfg.default_home = default_home_string();
                }
                if cfg.home_root.trim().is_empty() {
                    cfg.home_root = user_home().to_string_lossy().to_string();
                }
                Ok(cfg)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let mut cfg = Config::with_default_home(default_home_string());
                cfg.home_root = user_home().to_string_lossy().to_string();
                self.save_config(&cfg)?;
                Ok(cfg)
            }
            Err(err) => Err(AppError::new("E1000", format!("读取 app.json 失败：{err}"))),
        }
    }

    pub fn save_config(&self, cfg: &Config) -> Result<(), AppError> {
        let cfg = cfg.clone().sanitized();
        let bytes = serde_json::to_vec_pretty(&cfg)?;
        atomic_write(&self.config_path(), &bytes)
            .map_err(|e| AppError::new("E1000", format!("写入 app.json 失败：{e}")))?;
        Ok(())
    }

    // ------------------------------------------------------------ 实例

    /// 列实例：坏文件跳过并记录，不让一个坏文件拖垮整个列表。
    pub fn list_instances(&self) -> Vec<Instance> {
        let mut out = Vec::new();
        let dir = self.instances_dir();
        let Ok(entries) = fs::read_dir(&dir) else {
            return out;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Some(mut inst) = fs::read_to_string(&path)
                .ok()
                .and_then(|t| serde_json::from_str::<Instance>(&t).ok())
            {
                if inst.schema_version == 0 {
                    inst.schema_version = SCHEMA_VERSION;
                }
                out.push(inst);
            }
        }
        out.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        out
    }

    pub fn read_instance(&self, id: &str) -> Result<Instance, AppError> {
        let path = self.instance_path(id);
        let text = fs::read_to_string(&path)
            .map_err(|_| AppError::new("E1009", format!("instances\\{id}.json")))?;
        serde_json::from_str(&text)
            .map_err(|e| AppError::new("E1000", format!("实例文件损坏：{e}")))
    }

    pub fn write_instance(&self, inst: &Instance) -> Result<(), AppError> {
        let bytes = serde_json::to_vec_pretty(inst)?;
        atomic_write(&self.instance_path(&inst.id), &bytes)
            .map_err(|e| AppError::new("E1000", format!("写入实例文件失败：{e}")))
    }

    pub fn delete_instance_file(&self, id: &str) -> Result<(), AppError> {
        let path = self.instance_path(id);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(AppError::new("E1000", format!("删除实例文件失败：{err}"))),
        }
    }

    /// 校验（设计 §4.1）。`is_new` 决定 id 是否允许已存在、创建时间是否必须为空。
    pub fn validate_instance(
        &self,
        inst: &Instance,
        all: &[Instance],
        is_new: bool,
    ) -> Result<(), AppError> {
        if !is_valid_id(&inst.id) {
            return Err(AppError::new(
                "E1012",
                format!("ID 只能是 1–32 位小写字母 / 数字 / 连字符：{}", inst.id),
            ));
        }
        let name = inst.name.trim();
        if name.is_empty() {
            return Err(AppError::new("E1012", "名称不能为空"));
        }
        if name.chars().count() > 40 {
            return Err(AppError::new("E1012", "名称最多 40 个字符"));
        }
        if inst.version.trim().is_empty() {
            return Err(AppError::new("E1012", "必须选择一个版本"));
        }
        self.assert_version_exists(&inst.version)?;
        validate_home_path(&inst.home)?;

        for other in all.iter().filter(|o| o.id != inst.id && !o.archived) {
            if !inst.archived && path_eq(Path::new(&other.home), Path::new(&inst.home)) {
                return Err(AppError::new(
                    "E1003",
                    format!("{} 已被实例「{}」使用", inst.home, other.name),
                ));
            }
        }
        if is_new && all.iter().any(|o| o.id == inst.id) {
            return Err(AppError::new("E1008", inst.id.clone()));
        }
        Ok(())
    }

    fn assert_version_exists(&self, tag: &str) -> Result<(), AppError> {
        let dir = self.versions_dir.join(tag);
        let exe = dir.join("DSH NEXT.exe");
        let asar = dir.join("resources").join("app.asar");
        if exe.is_file() && asar.is_file() {
            Ok(())
        } else {
            Err(AppError::new(
                "E1004",
                format!("{} 缺少 DSH NEXT.exe 或 resources\\app.asar", dir.display()),
            ))
        }
    }

    /// 首次运行：没有实例时，按 defaultHome 建一个，避免空白界面。
    pub fn ensure_default_instance(&self, cfg: &Config, version: &str) -> Option<Instance> {
        if !self.list_instances().is_empty() {
            return None;
        }
        let home = cfg.default_home.trim();
        if home.is_empty() || !Path::new(home).is_dir() {
            return None;
        }
        if version.is_empty() {
            return None;
        }
        let inst = Instance::new("default", "默认", version, &normalize_home(home));
        self.write_instance(&inst).ok()?;
        Some(inst)
    }
}

// ---------------------------------------------------------------- 校验工具

pub fn is_valid_id(id: &str) -> bool {
    let len = id.chars().count();
    if !(1..=32).contains(&len) {
        return false;
    }
    id.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// 把用户输入的 ID 转写成合法形式（新建对话框的“自动转写”）。
pub fn slugify(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    let mut s: String = trimmed.chars().take(32).collect();
    while s.ends_with('-') {
        s.pop();
    }
    if s.is_empty() {
        "instance".to_string()
    } else {
        s
    }
}

/// 去掉结尾分隔符、统一盘符大小写（保留用户原始大小写，只做规范化）。
pub fn normalize_home(home: &str) -> String {
    let mut s = home.trim().replace('/', "\\");
    while s.len() > 3 && s.ends_with('\\') {
        s.pop();
    }
    s
}

pub fn validate_home_path(home: &str) -> Result<(), AppError> {
    let h = normalize_home(home);
    if h.is_empty() {
        return Err(AppError::new("E1012", "数据目录不能为空"));
    }
    let p = Path::new(&h);
    if !p.is_absolute() {
        return Err(AppError::new(
            "E1012",
            format!("数据目录必须是绝对路径：{h}"),
        ));
    }
    if h.contains("..") {
        return Err(AppError::new(
            "E1012",
            format!("数据目录不允许包含 ..：{h}"),
        ));
    }
    // 盘根：`C:\` 的 parent 为 None
    if p.parent().is_none() {
        return Err(AppError::new("E1012", format!("数据目录不能是盘根：{h}")));
    }
    let trimmed = h.trim_end_matches(['\\', '/']);
    if trimmed.len() <= 2 {
        return Err(AppError::new("E1012", format!("数据目录不能是盘根：{h}")));
    }
    Ok(())
}

pub fn default_home_string() -> String {
    user_home().join(".dsh").to_string_lossy().to_string()
}

/// 新建实例的**默认** home：`<homeRoot>\.dsh-<id>`；`id == "default"` 用 `<homeRoot>\.dsh`。
///
/// 只提供默认值——实例的 `home` 字段仍然自由填写，不做“必须在根下”的校验。
pub fn default_home_for(cfg: &Config, id: &str) -> String {
    let root = cfg.effective_home_root();
    if id == "default" {
        if !cfg.default_home.trim().is_empty() {
            return cfg.default_home.clone();
        }
        return Path::new(&root).join(".dsh").to_string_lossy().to_string();
    }
    Path::new(&root)
        .join(format!(".dsh-{id}"))
        .to_string_lossy()
        .to_string()
}

/// 改数据目录树根：只有 `default_home` 仍是「旧根的 `.dsh`」时才跟随，
/// 避免覆盖用户自定义过的默认环境。
pub fn apply_home_root(cfg: &Config, new_root: &str) -> Config {
    let new_root = new_root.trim().trim_end_matches(['\\', '/']).to_string();
    let old_default = Path::new(&cfg.effective_home_root()).join(".dsh");
    let follows = path_eq(Path::new(&cfg.default_home), &old_default);
    let mut next = cfg.clone();
    next.home_root = new_root.clone();
    if follows {
        next.default_home = Path::new(&new_root)
            .join(".dsh")
            .to_string_lossy()
            .to_string();
    }
    next
}

/// 数据目录树根的校验：只挡明显危险的值（空、相对、盘根、含 `..`）。
pub fn validate_home_root(root: &str) -> Result<(), AppError> {
    let trimmed = root.trim();
    if trimmed.is_empty() {
        return Err(AppError::new("E1012", "数据目录树根不能为空"));
    }
    let path = Path::new(trimmed);
    if !path.is_absolute() {
        return Err(AppError::new(
            "E1012",
            format!("数据目录树根必须是绝对路径：{trimmed}"),
        ));
    }
    if trimmed.contains("..") {
        return Err(AppError::new(
            "E1012",
            format!("数据目录树根不能包含 ..：{trimmed}"),
        ));
    }
    if path.parent().is_none() || trimmed.len() <= 3 {
        return Err(AppError::new(
            "E1012",
            format!("数据目录树根不能是盘根：{trimmed}"),
        ));
    }
    Ok(())
}

/// 供 UI “按此目录新建”用的建议 ID。
pub fn suggest_id_from_home(home: &str) -> String {
    let last = Path::new(home)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let stripped = last.trim_start_matches('.');
    // `.dsh` → default；`.dsh-lab` → lab；`.dsh-wallpaper-engine` → wallpaper-engine
    let core = stripped
        .strip_prefix("dsh")
        .unwrap_or(stripped)
        .trim_start_matches(['-', '.', '_']);
    if core.is_empty() {
        "default".to_string()
    } else {
        slugify(core)
    }
}

/// 路径是否与给定 home 相同（大小写不敏感）。
pub fn same_home(a: &str, b: &str) -> bool {
    norm_path(Path::new(a)) == norm_path(Path::new(b))
}

pub fn created_stamp() -> String {
    now_iso()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(tag: &str) -> Store {
        let root = crate::testutil::root(&format!("store-{tag}"));
        let _ = fs::remove_dir_all(&root);
        let store = Store::new(root.join("data"), root.join("versions"));
        store.ensure_dirs().unwrap();
        // 造一个可用版本
        let vdir = store.versions_dir.join("2.0.21-next");
        fs::create_dir_all(vdir.join("resources")).unwrap();
        fs::write(vdir.join("DSH NEXT.exe"), b"stub").unwrap();
        fs::write(vdir.join("resources").join("app.asar"), b"stub").unwrap();
        store
    }

    #[test]
    fn config_defaults_and_roundtrip() {
        let store = temp_store("cfg");
        let cfg = store.load_config().unwrap();
        assert_eq!(cfg.launch_timeout_sec, 90);
        assert_eq!(cfg.poll_interval_ms, 500);
        assert!(cfg.restore_pointer_after_launch);
        assert!(store.config_path().is_file());

        let mut cfg2 = cfg.clone();
        cfg2.launch_timeout_sec = 3; // 越界 → 夹回 10
        store.save_config(&cfg2).unwrap();
        let reloaded = store.load_config().unwrap();
        assert_eq!(reloaded.launch_timeout_sec, 10);
        let _ = fs::remove_dir_all(store.data_dir.parent().unwrap());
    }

    #[test]
    fn home_root_drives_default_home_only() {
        // 根 = D:\dshdata：普通实例默认 .dsh-<id>，default 用 .dsh
        let mut cfg = Config::with_default_home("D:\\dshdata\\.dsh");
        cfg.home_root = "D:\\dshdata".into();
        assert_eq!(
            default_home_for(&cfg, "lab"),
            "D:\\dshdata\\.dsh-lab".to_string()
        );
        assert_eq!(
            default_home_for(&cfg, "default"),
            "D:\\dshdata\\.dsh".to_string()
        );

        // 结尾分隔符会被吃掉，不会产生 "D:\\dshdata\\.dsh-lab"
        cfg.home_root = "D:\\dshdata\\".into();
        assert_eq!(
            default_home_for(&cfg, "lab"),
            "D:\\dshdata\\.dsh-lab".to_string()
        );

        // 未自定义 default_home 时，改根跟随
        let mut cfg = Config::with_default_home("C:\\Users\\FXS\\.dsh");
        cfg.home_root = "C:\\Users\\FXS".into();
        let next = apply_home_root(&cfg, "E:\\data");
        assert_eq!(next.home_root, "E:\\data");
        assert_eq!(next.default_home, "E:\\data\\.dsh");

        // 用户自定义过 default_home → 改根不覆盖它
        let mut cfg = Config::with_default_home("C:\\custom\\.dsh-main");
        cfg.home_root = "C:\\Users\\FXS".into();
        let next = apply_home_root(&cfg, "E:\\data");
        assert_eq!(next.home_root, "E:\\data");
        assert_eq!(next.default_home, "C:\\custom\\.dsh-main");
    }

    #[test]
    fn home_root_validation_rejects_dangerous_values() {
        assert!(validate_home_root("C:\\Users\\FXS").is_ok());
        assert!(validate_home_root("E:\\data\\dsh").is_ok());

        assert!(validate_home_root("").is_err());
        assert!(validate_home_root("   ").is_err());
        assert!(validate_home_root("relative\\path").is_err());
        assert!(validate_home_root("C:\\").is_err());
        assert!(validate_home_root("C:\\Users\\..\\Windows").is_err());
    }

    #[test]
    fn instance_crud_and_validation() {
        let store = temp_store("inst");
        let inst = Instance::new("lab", "实验室", "2.0.21-next", "C:\\Users\\FXS\\.dsh-lab");
        store.validate_instance(&inst, &[], true).unwrap();
        store.write_instance(&inst).unwrap();

        let all = store.list_instances();
        assert_eq!(all.len(), 1);
        assert_eq!(store.read_instance("lab").unwrap().name, "实验室");

        // 同 home 的第二个实例被拒
        let mut dup = Instance::new("lab2", "第二", "2.0.21-next", "c:\\users\\fxs\\.dsh-lab");
        let err = store.validate_instance(&dup, &all, true).unwrap_err();
        assert_eq!(err.code, "E1003");

        // 版本不存在
        dup.home = "C:\\Users\\FXS\\.dsh-lab2".into();
        dup.version = "9.9.9".into();
        assert_eq!(
            store.validate_instance(&dup, &all, true).unwrap_err().code,
            "E1004"
        );

        // ID 非法
        dup.version = "2.0.21-next".into();
        dup.id = "Bad ID".into();
        assert_eq!(
            store.validate_instance(&dup, &all, true).unwrap_err().code,
            "E1012"
        );

        // 盘根
        dup.id = "lab2".into();
        dup.home = "C:\\".into();
        assert_eq!(
            store.validate_instance(&dup, &all, true).unwrap_err().code,
            "E1012"
        );

        // 重名 id
        let same_id = Instance::new("lab", "另一个", "2.0.21-next", "C:\\Users\\FXS\\.dsh-x");
        assert_eq!(
            store
                .validate_instance(&same_id, &all, true)
                .unwrap_err()
                .code,
            "E1008"
        );
        // 修改已有实例不算新增
        store.validate_instance(&same_id, &all, false).unwrap();

        store.delete_instance_file("lab").unwrap();
        assert!(store.list_instances().is_empty());
        let _ = fs::remove_dir_all(store.data_dir.parent().unwrap());
    }

    #[test]
    fn slugify_is_stable() {
        assert_eq!(slugify("灵枢 Lab!"), "lab");
        assert_eq!(slugify("  "), "instance");
        assert_eq!(slugify("a".repeat(40).as_str()).len(), 32);
        assert_eq!(suggest_id_from_home(r"C:\Users\FXS\.dsh-lab"), "lab");
        assert_eq!(suggest_id_from_home(r"C:\Users\FXS\.dsh"), "default");
    }

    #[test]
    fn default_instance_only_when_empty() {
        let store = temp_store("boot");
        let home = crate::testutil::root("dsh-multi-home");
        fs::create_dir_all(&home).unwrap();
        let cfg = Config::with_default_home(home.to_string_lossy().to_string());
        let inst = store.ensure_default_instance(&cfg, "2.0.21-next").unwrap();
        assert_eq!(inst.id, "default");
        assert!(store.ensure_default_instance(&cfg, "2.0.21-next").is_none());
        let _ = fs::remove_dir_all(store.data_dir.parent().unwrap());
        let _ = fs::remove_dir_all(home);
    }
}
