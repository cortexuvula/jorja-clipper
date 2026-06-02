use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Dark,
    Light,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::Dark
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub buffer_before: f64,
    pub buffer_after: f64,
    pub clip_key: String,
    pub output_dir: Option<PathBuf>,
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            output_dir: None,
            theme: Theme::Dark,
        }
    }
}

impl Settings {
    pub fn load() -> AppResult<Self> {
        let config_path = Self::config_path();
        Self::load_with_path(&config_path)
    }

    /// Load settings from a specific path (useful for testing)
    pub fn load_with_path(config_path: &std::path::Path) -> AppResult<Self> {
        if !config_path.exists() {
            let settings = Self::default();
            settings.save_to_path(config_path)?;
            return Ok(settings);
        }

        let content = std::fs::read_to_string(config_path)?;
        let settings: Settings =
            serde_json::from_str(&content).map_err(|e| AppError::Storage(e.to_string()))?;

        Ok(settings)
    }

    pub fn save(&self) -> AppResult<()> {
        let config_path = Self::config_path();
        self.save_to_path(&config_path)
    }

    /// Save settings to a specific path (useful for testing)
    pub fn save_to_path(&self, config_path: &std::path::Path) -> AppResult<()> {
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content =
            serde_json::to_string_pretty(self).map_err(|e| AppError::Storage(e.to_string()))?;

        std::fs::write(config_path, content)?;

        Ok(())
    }

    fn config_path() -> PathBuf {
        crate::util::app_config_dir().join("config.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.buffer_before, 5.0);
        assert_eq!(settings.buffer_after, 5.0);
        assert_eq!(settings.clip_key, "c");
        assert_eq!(settings.theme, Theme::Dark);
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let config_path = temp_dir.join("config.json");

        let settings = Settings {
            buffer_before: 10.0,
            buffer_after: 3.0,
            clip_key: "x".to_string(),
            output_dir: None,
            theme: Theme::Light,
        };

        let content = serde_json::to_string_pretty(&settings).unwrap();
        fs::write(&config_path, content).unwrap();

        let loaded: Settings =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(loaded.buffer_before, 10.0);
        assert_eq!(loaded.clip_key, "x");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_theme_serialization() {
        let dark = Theme::Dark;
        let light = Theme::Light;

        let dark_json = serde_json::to_string(&dark).unwrap();
        let light_json = serde_json::to_string(&light).unwrap();

        assert_eq!(dark_json, "\"dark\"");
        assert_eq!(light_json, "\"light\"");

        // Deserialization
        let dark_parsed: Theme = serde_json::from_str("\"dark\"").unwrap();
        let light_parsed: Theme = serde_json::from_str("\"light\"").unwrap();

        assert_eq!(dark_parsed, Theme::Dark);
        assert_eq!(light_parsed, Theme::Light);
    }

    #[test]
    fn test_settings_with_output_dir() {
        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            output_dir: Some(PathBuf::from("/custom/output")),
            theme: Theme::Dark,
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("/custom/output"));

        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.output_dir, Some(PathBuf::from("/custom/output")));
    }

    #[test]
    fn test_settings_none_output_dir() {
        let settings = Settings {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            output_dir: None,
            theme: Theme::Dark,
        };

        let json = serde_json::to_string(&settings).unwrap();
        let loaded: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.output_dir, None);
    }

    #[test]
    fn test_invalid_json_handling() {
        let invalid_json = "{ invalid json }";
        let result: Result<Settings, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_theme_default() {
        let theme = Theme::default();
        assert_eq!(theme, Theme::Dark);
    }

    #[test]
    fn test_settings_save_and_load_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.json");

        // Create custom settings
        let settings = Settings {
            buffer_before: 7.5,
            buffer_after: 2.5,
            clip_key: "z".to_string(),
            output_dir: Some(PathBuf::from("/custom/path")),
            theme: Theme::Light,
        };

        // Serialize and write
        let content = serde_json::to_string_pretty(&settings).unwrap();
        std::fs::write(&config_path, content).unwrap();

        // Read and deserialize
        let loaded_content = std::fs::read_to_string(&config_path).unwrap();
        let loaded: Settings = serde_json::from_str(&loaded_content).unwrap();

        assert_eq!(loaded.buffer_before, 7.5);
        assert_eq!(loaded.buffer_after, 2.5);
        assert_eq!(loaded.clip_key, "z");
        assert_eq!(loaded.output_dir, Some(PathBuf::from("/custom/path")));
        assert_eq!(loaded.theme, Theme::Light);
    }

    #[test]
    fn test_settings_serialization_all_fields() {
        let settings = Settings {
            buffer_before: 0.0,
            buffer_after: 60.0,
            clip_key: " ".to_string(),
            output_dir: None,
            theme: Theme::Dark,
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"buffer_before\":0.0"));
        assert!(json.contains("\"buffer_after\":60.0"));
        assert!(json.contains("\"clip_key\":\" \""));
        assert!(json.contains("\"output_dir\":null"));
        assert!(json.contains("\"theme\":\"dark\""));
    }

    #[test]
    fn test_settings_default() {
        let settings = Settings::default();
        assert_eq!(settings.buffer_before, 5.0);
        assert_eq!(settings.buffer_after, 5.0);
        assert_eq!(settings.clip_key, "c");
        assert!(settings.output_dir.is_none());
        assert_eq!(settings.theme, Theme::Dark);
    }

    #[test]
    fn test_settings_load_and_save_roundtrip() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.json");

        // Create settings and save them
        let original_settings = Settings {
            buffer_before: 15.0,
            buffer_after: 20.0,
            clip_key: "x".to_string(),
            output_dir: Some(std::path::PathBuf::from("/custom/output")),
            theme: Theme::Light,
        };

        let content = serde_json::to_string(&original_settings).unwrap();
        fs::write(&config_path, content).unwrap();

        // Load settings from the file
        let loaded_content = fs::read_to_string(&config_path).unwrap();
        let loaded_settings: Settings = serde_json::from_str(&loaded_content).unwrap();

        // Verify they match
        assert_eq!(loaded_settings.buffer_before, 15.0);
        assert_eq!(loaded_settings.buffer_after, 20.0);
        assert_eq!(loaded_settings.clip_key, "x");
        assert_eq!(
            loaded_settings.output_dir,
            Some(std::path::PathBuf::from("/custom/output"))
        );
        assert_eq!(loaded_settings.theme, Theme::Light);
    }

    #[test]
    fn test_settings_default_implementation() {
        let settings = Settings::default();

        assert_eq!(settings.buffer_before, 5.0);
        assert_eq!(settings.buffer_after, 5.0);
        assert_eq!(settings.clip_key, "c");
        assert_eq!(settings.output_dir, None);
        assert_eq!(settings.theme, Theme::Dark);
    }

    #[test]
    fn test_settings_load_nonexistent_file() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent_config.json");

        // Config file doesn't exist, should return default settings
        let settings = Settings::load_with_path(&config_path).unwrap();

        assert_eq!(settings.buffer_before, 5.0);
        assert_eq!(settings.buffer_after, 5.0);
        assert_eq!(settings.clip_key, "c");
        assert_eq!(settings.output_dir, None);
        assert_eq!(settings.theme, Theme::Dark);
    }

    #[test]
    fn test_settings_load_invalid_json() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid_config.json");

        // Write invalid JSON to the config file
        fs::write(&config_path, "{ invalid json }").unwrap();

        // Should return an error
        let result = Settings::load_with_path(&config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_settings_save_to_path_creates_directories() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir
            .path()
            .join("level1")
            .join("level2")
            .join("config.json");

        let settings = Settings::default();

        // Save to nested path - should create directories automatically
        let result = settings.save_to_path(&nested_path);
        assert!(result.is_ok());
        assert!(nested_path.exists());
    }

    #[test]
    fn test_settings_load_with_path_roundtrip() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.json");

        // Create custom settings
        let original = Settings {
            buffer_before: 12.5,
            buffer_after: 8.3,
            clip_key: "z".to_string(),
            output_dir: None,
            theme: Theme::Light,
        };

        // Save to specific path
        let save_result = original.save_to_path(&config_path);
        assert!(save_result.is_ok());

        // Load from same path
        let loaded_result = Settings::load_with_path(&config_path);
        assert!(loaded_result.is_ok());

        let loaded = loaded_result.unwrap();
        assert_eq!(loaded.buffer_before, 12.5);
        assert_eq!(loaded.buffer_after, 8.3);
        assert_eq!(loaded.clip_key, "z");
        assert_eq!(loaded.theme, Theme::Light);
    }

    #[test]
    fn test_settings_save_to_path_with_nested_directories() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir
            .path()
            .join("a")
            .join("b")
            .join("c")
            .join("config.json");

        let settings = Settings::default();

        // Save to deeply nested path - should create all parent directories
        let result = settings.save_to_path(&nested_path);
        assert!(result.is_ok());
        assert!(nested_path.exists());

        // Verify we can load it back
        let loaded = Settings::load_with_path(&nested_path).unwrap();
        assert_eq!(loaded.buffer_before, settings.buffer_before);
    }

    #[test]
    fn test_settings_load_with_path_nonexistent_creates_default() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("nonexistent.json");

        // File doesn't exist yet
        assert!(!config_path.exists());

        // Load should create the file with default settings
        let result = Settings::load_with_path(&config_path);
        assert!(result.is_ok());

        // File should now exist
        assert!(config_path.exists());

        // Should have default values
        let settings = result.unwrap();
        assert_eq!(settings.buffer_before, 5.0);
        assert_eq!(settings.buffer_after, 5.0);
        assert_eq!(settings.clip_key, "c");
    }
}
