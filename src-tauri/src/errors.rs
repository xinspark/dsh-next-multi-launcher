//! 错误码与中文文案（设计说明 §8）。
//!
//! 前端只认 `code`，文案在这里集中给一次，避免两处各写一套。
//! 截断 45 字符 + 悬停全文的规则由前端执行（这里保留完整 detail）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    /// 稳定错误码，例如 `E1003`。前端据此分支。
    pub code: String,
    /// 中文人话 + 英文（设计 §8 文案规则）。
    pub message: String,
    /// 具体现场（路径、pid、退出码……）。悬停显示全文。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl AppError {
    pub fn new(code: &str, detail: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message_for(code).to_string(),
            detail: Some(detail.into()),
        }
    }

    pub fn bare(code: &str) -> Self {
        Self {
            code: code.to_string(),
            message: message_for(code).to_string(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.detail {
            Some(d) => write!(f, "{} {} — {}", self.code, self.message, d),
            None => write!(f, "{} {}", self.code, self.message),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::new("E1000", err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::new("E1000", err.to_string())
    }
}

/// 错误码 → 中文文案。英文短句跟在斜杠后，方便贴 issue。
pub fn message_for(code: &str) -> &'static str {
    match code {
        "E1001" => "无法写入数据目录指针（权限或占用） / cannot write pointer file",
        "E1002" => "指针回滚失败，请手工检查指针文件 / pointer rollback failed",
        "E1003" => "数据目录已被占用 / home already in use",
        "E1004" => "版本目录无效：缺少 DSH NEXT.exe 或 app.asar / invalid version directory",
        "E1005" => "启动超时（未看到该目录的进程）/ launch timed out",
        "E1006" => "启动后进程立即退出 / process exited immediately",
        "E1007" => "该路径不允许删除（盘根/系统目录/默认环境）/ unsafe delete target",
        "E1008" => "实例 ID 已存在 / instance id already exists",
        "E1009" => "实例不存在 / instance not found",
        "E1010" => "该版本正被实例引用，不能删除 / version in use",
        "E1011" => "数据目录仍有进程占用，先停止实例 / home still in use",
        "E1012" => "参数无效 / invalid argument",
        "E1000" => "内部错误 / internal error",
        _ => "未知错误 / unknown error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_have_chinese_messages() {
        for code in ["E1001", "E1002", "E1003", "E1004", "E1005", "E1006", "E1007", "E1008", "E1009", "E1010", "E1011", "E1012"] {
            let msg = message_for(code);
            assert!(msg.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)), "{code} 缺中文文案");
            assert!(msg.len() > 4);
        }
    }

    #[test]
    fn serializes_camel_case() {
        let err = AppError::new("E1003", "C:\\x");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"detail\""));
        assert!(json.contains("E1003"));
    }
}
