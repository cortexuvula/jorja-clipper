use thiserror::Error;

/// Generate a helpful "FFmpeg not found" error message
pub fn ffmpeg_not_found_error(operation: &str) -> AppError {
    AppError::Ffmpeg(format!(
        "FFmpeg not found while {}. Please install FFmpeg:\n\
        • macOS: brew install ffmpeg\n\
        • Windows: Download from https://ffmpeg.org/download.html\n\
        • Linux: sudo apt install ffmpeg",
        operation
    ))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffmpeg_not_found_error_message() {
        let err = ffmpeg_not_found_error("converting video");
        match err {
            AppError::Ffmpeg(msg) => {
                assert!(msg.contains("FFmpeg not found"));
                assert!(msg.contains("converting video"));
                assert!(msg.contains("brew install ffmpeg"));
            }
            _ => panic!("Expected Ffmpeg error variant"),
        }
    }

    #[test]
    fn test_error_display_messages() {
        let ffmpeg_err = AppError::Ffmpeg("test error".to_string());
        assert_eq!(ffmpeg_err.to_string(), "FFmpeg error: test error");

        let clip_err = AppError::Clip("clip failed".to_string());
        assert_eq!(clip_err.to_string(), "Clip error: clip failed");

        let storage_err = AppError::Storage("db error".to_string());
        assert_eq!(storage_err.to_string(), "Storage error: db error");

        let no_video_err = AppError::NoVideoLoaded;
        assert_eq!(no_video_err.to_string(), "No video loaded");

        let clip_in_progress = AppError::ClipInProgress;
        assert_eq!(clip_in_progress.to_string(), "Clip already in progress");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let app_err: AppError = io_err.into();
        match app_err {
            AppError::Io(_) => {} // Expected
            _ => panic!("Expected Io error variant"),
        }
        assert!(app_err.to_string().contains("file not found"));
    }

    #[test]
    fn test_rusqlite_error_conversion() {
        let sqlite_err = rusqlite::Error::InvalidParameterName("test".to_string());
        let app_err: AppError = sqlite_err.into();
        match app_err {
            AppError::Storage(msg) => {
                assert!(msg.contains("test"));
            }
            _ => panic!("Expected Storage error variant"),
        }
    }

    #[test]
    fn test_error_debug_trait() {
        let err = AppError::Ffmpeg("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Ffmpeg"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_error_variants() {
        let errors = vec![
            AppError::Ffmpeg("ffmpeg error".to_string()),
            AppError::Clip("clip error".to_string()),
            AppError::Storage("storage error".to_string()),
            AppError::NoVideoLoaded,
            AppError::ClipInProgress,
        ];

        for err in errors {
            let display = err.to_string();
            assert!(!display.is_empty());
        }
    }

    #[test]
    fn test_io_error_different_kinds() {
        let kinds = vec![
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::AlreadyExists,
        ];

        for kind in kinds {
            let io_err = std::io::Error::new(kind, "test error");
            let app_err: AppError = io_err.into();
            match app_err {
                AppError::Io(_) => {}
                _ => panic!("Expected Io error variant"),
            }
        }
    }

    #[test]
    fn test_app_error_to_tauri_error_conversion() {
        let app_err = AppError::Ffmpeg("test ffmpeg error".to_string());
        let tauri_err: tauri::Error = app_err.into();

        // Verify the conversion worked
        match tauri_err {
            tauri::Error::Anyhow(_) => {} // Expected
            _ => panic!("Expected Anyhow variant"),
        }
    }

    #[test]
    fn test_all_error_variants_to_tauri_error() {
        let errors = vec![
            AppError::Ffmpeg("ffmpeg".to_string()),
            AppError::Clip("clip".to_string()),
            AppError::Storage("storage".to_string()),
            AppError::NoVideoLoaded,
            AppError::ClipInProgress,
        ];

        for err in errors {
            let _: tauri::Error = err.into();
        }
    }

    #[test]
    fn test_ffmpeg_not_found_error_different_operations() {
        let operations = vec!["converting video", "extracting clip", "processing file"];

        for op in operations {
            let err = ffmpeg_not_found_error(op);
            match err {
                AppError::Ffmpeg(msg) => {
                    assert!(msg.contains(op));
                    assert!(msg.contains("FFmpeg not found"));
                }
                _ => panic!("Expected Ffmpeg error variant"),
            }
        }
    }

    #[test]
    fn test_ffmpeg_not_found_error_full_message() {
        let err = ffmpeg_not_found_error("testing operation");

        if let AppError::Ffmpeg(msg) = err {
            // Verify the complete message structure
            assert!(msg.starts_with("FFmpeg not found while testing operation"));
            assert!(msg.contains("Please install FFmpeg"));
            assert!(msg.contains("• macOS: brew install ffmpeg"));
            assert!(msg.contains("• Windows: Download from https://ffmpeg.org/download.html"));
            assert!(msg.contains("• Linux: sudo apt install ffmpeg"));
        } else {
            panic!("Expected Ffmpeg error variant");
        }
    }

    #[test]
    fn test_app_result_type() {
        // Test that AppResult works as expected
        fn returns_ok() -> AppResult<i32> {
            Ok(42)
        }

        fn returns_err() -> AppResult<i32> {
            Err(AppError::Clip("test error".to_string()))
        }

        assert_eq!(returns_ok().unwrap(), 42);
        assert!(returns_err().is_err());
    }

    #[test]
    fn test_from_app_error_for_tauri_error_all_variants() {
        let test_cases = vec![
            AppError::Ffmpeg("test".to_string()),
            AppError::Clip("test".to_string()),
            AppError::Storage("test".to_string()),
            AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, "test")),
            AppError::NoVideoLoaded,
            AppError::ClipInProgress,
        ];

        for app_err in test_cases {
            let tauri_err: tauri::Error = app_err.into();
            // Verify we can convert to string without panicking
            let _ = format!("{:?}", tauri_err);
        }
    }

    #[test]
    fn test_ffmpeg_not_found_error_message_content() {
        // Test with different operations to ensure the format string is evaluated
        let operations = [
            "loading video",
            "converting format",
            "saving clip",
            "extracting audio",
        ];

        for op in operations.iter() {
            let err = ffmpeg_not_found_error(op);
            if let AppError::Ffmpeg(msg) = err {
                // Verify the operation name is in the message
                assert!(msg.contains(op), "Message should contain operation: {}", op);

                // Verify all platform instructions are present
                assert!(msg.contains("macOS"));
                assert!(msg.contains("brew install"));
                assert!(msg.contains("Windows"));
                assert!(msg.contains("ffmpeg.org"));
                assert!(msg.contains("Linux"));
                assert!(msg.contains("apt install"));

                // Verify the message is properly formatted
                let expected_prefix = format!("FFmpeg not found while {}", op);
                assert!(msg.starts_with(&expected_prefix));
            } else {
                panic!("Expected Ffmpeg variant");
            }
        }
    }

    #[test]
    fn test_app_error_source() {
        use std::error::Error;

        // Test that AppError implements std::error::Error properly
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let app_err: AppError = io_err.into();

        // Should be able to get source
        assert!(app_err.source().is_some());

        // Test other variants don't have source
        let clip_err = AppError::Clip("test".to_string());
        assert!(clip_err.source().is_none());

        let storage_err = AppError::Storage("test".to_string());
        assert!(storage_err.source().is_none());
    }

    #[test]
    fn test_app_error_display_all_variants() {
        let errors = vec![
            (AppError::Ffmpeg("test".to_string()), "FFmpeg error: test"),
            (AppError::Clip("test".to_string()), "Clip error: test"),
            (AppError::Storage("test".to_string()), "Storage error: test"),
            (AppError::NoVideoLoaded, "No video loaded"),
            (AppError::ClipInProgress, "Clip already in progress"),
        ];

        for (err, expected) in errors {
            assert_eq!(err.to_string(), expected);
        }
    }
}
