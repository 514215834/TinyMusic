use serde::Serialize;

/// 统一错误（技术设计文档 §5）：命令层通过 [`AppErrorDto`] 序列化到前端
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("元数据解析失败: {0}")]
    Meta(#[from] lofty::error::FileParseError),
    #[error("标签写入失败（文件可能只读或正被占用）: {0}")]
    Encoding(#[from] lofty::error::FileEncodingError),
    #[error("网络错误: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Tauri 错误: {0}")]
    Tauri(#[from] tauri::Error),
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
}

pub type AppResult<T> = Result<T, AppError>;

/// 命令层错误 DTO（specta 类型生成要求 Type）
#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct AppErrorDto {
    pub message: String,
}

impl From<AppError> for AppErrorDto {
    fn from(e: AppError) -> Self {
        Self { message: e.to_string() }
    }
}
