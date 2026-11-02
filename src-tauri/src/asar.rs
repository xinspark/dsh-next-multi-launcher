//! app.asar 头解析（设计说明 §4.3、§5.5）。
//!
//! 格式：`[u32 4][u32 pickleLen][u32 jsonLen][u32 jsonLen][json][data...]`
//! 数据区起点 = 8 + pickleLen；索引里的每个文件带 `offset`（相对数据区）与 `size`。

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde_json::Value;

/// 单文件上限，防止损坏的头把内存吃穿。
const MAX_HEADER: u32 = 64 * 1024 * 1024;

/// asar 索引里的数字字段可能是 JSON number 或数字字符串。
fn json_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsarPackage {
    /// `package.json` 的 `version`
    pub shell_version: String,
    /// `dependencies["@deepseek-ai/dsh"]`
    pub kernel_version: String,
}

/// 读 `app.asar!/package.json` 里的壳版本与内核版本。
pub fn read_package(asar: &Path) -> Result<AsarPackage, String> {
    let mut file = File::open(asar).map_err(|e| format!("打开失败：{e}"))?;
    let mut head = [0u8; 16];
    file.read_exact(&mut head).map_err(|e| format!("头部不足 16 字节：{e}"))?;

    let pickle_len = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
    let json_a = u32::from_le_bytes([head[8], head[9], head[10], head[11]]);
    let json_b = u32::from_le_bytes([head[12], head[13], head[14], head[15]]);
    let json_len = if json_b > 0 { json_b } else { json_a };
    if json_len == 0 || json_len > MAX_HEADER {
        return Err(format!("头长度不合理：{json_len}"));
    }
    let data_start = 8u64 + pickle_len as u64;

    let mut raw = vec![0u8; json_len as usize];
    file.read_exact(&mut raw).map_err(|e| format!("索引不足 {json_len} 字节：{e}"))?;
    let index: Value = serde_json::from_slice(&raw).map_err(|e| format!("索引不是 JSON：{e}"))?;

    let entry = index
        .get("files")
        .and_then(|f| f.get("package.json"))
        .ok_or_else(|| "索引里没有 package.json".to_string())?;
    if entry.get("unpacked").and_then(Value::as_bool).unwrap_or(false) {
        return Err("package.json 被标记为 unpacked".into());
    }
    // asar 索引里的 offset/size 既可能是数字，也可能是字符串（Electron 产物用字符串）
    let offset = entry.get("offset").and_then(json_u64).ok_or("package.json 没有 offset")?;
    let size = entry.get("size").and_then(json_u64).ok_or("package.json 没有 size")?;
    if size == 0 || size > 4 * 1024 * 1024 {
        return Err(format!("package.json 大小不合理：{size}"));
    }

    file.seek(SeekFrom::Start(data_start + offset)).map_err(|e| format!("定位失败：{e}"))?;
    let mut body = vec![0u8; size as usize];
    file.read_exact(&mut body).map_err(|e| format!("读取 package.json 失败：{e}"))?;
    let pkg: Value = serde_json::from_slice(&body).map_err(|e| format!("package.json 不是 JSON：{e}"))?;

    let shell_version = pkg
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();
    let kernel_version = pkg
        .get("dependencies")
        .and_then(|d| d.get("@deepseek-ai/dsh"))
        .and_then(Value::as_str)
        .unwrap_or("?")
        .to_string();

    Ok(AsarPackage { shell_version, kernel_version })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// 造一个最小 asar：只有一个 package.json。
    fn write_fake_asar(path: &Path, pkg_json: &str) -> Vec<u8> {
        let body = pkg_json.as_bytes();
        let index = format!(
            "{{\"files\":{{\"package.json\":{{\"size\":{},\"offset\":\"0\"}}}}}}",
            body.len()
        );
        let index_bytes = index.as_bytes();
        let pickle_len = 8 + index_bytes.len();
        let mut out = Vec::new();
        out.extend_from_slice(&4u32.to_le_bytes());
        out.extend_from_slice(&(pickle_len as u32).to_le_bytes());
        out.extend_from_slice(&(index_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&(index_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(index_bytes);
        out.extend_from_slice(body);
        let mut f = File::create(path).unwrap();
        f.write_all(&out).unwrap();
        out
    }

    #[test]
    fn reads_versions_from_index() {
        let dir = crate::testutil::root("dsh-multi-asar");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("app.asar");
        write_fake_asar(
            &path,
            r#"{"version":"2.0.21-next","dependencies":{"@deepseek-ai/dsh":"0.2.1-rc.4"}}"#,
        );
        let pkg = read_package(&path).unwrap();
        assert_eq!(pkg.shell_version, "2.0.21-next");
        assert_eq!(pkg.kernel_version, "0.2.1-rc.4");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_kernel_degrades_to_question_mark() {
        let dir = crate::testutil::root("dsh-multi-asar2");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("app.asar");
        write_fake_asar(&path, r#"{"version":"3.0.0"}"#);
        let pkg = read_package(&path).unwrap();
        assert_eq!(pkg.shell_version, "3.0.0");
        assert_eq!(pkg.kernel_version, "?");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn garbage_header_is_an_error_not_a_panic() {
        let dir = crate::testutil::root("dsh-multi-asar3");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("app.asar");
        std::fs::write(&path, [4u8, 0, 0, 0, 16, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0]).unwrap();
        assert!(read_package(&path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
