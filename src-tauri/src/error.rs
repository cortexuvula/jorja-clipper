use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("FFmpeg error: {0}")]
    Ffmpeg(String),

    #[error("Clip error: {0}")]
    Clip(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("No video loaded")]
    NoVideoLoaded,

    #[error("Clip already in progress")]
    ClipInProgress,
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Storage(err.to_string())
    }
}

impl From<AppError> for tauri::Error {
    fn from(err: AppError) -> Self {
        tauri::Error::Anyhow(err.into())
    }
}

pub type AppResult<T> = Result<T, AppError>;
