use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub buffer_before: f64,
    pub buffer_after: f64,
    pub clip_key: String,
    pub output_dir: Option<PathBuf>,
    pub theme: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            buffer_before: 5.0,
            buffer_after: 5.0,
            clip_key: "c".to_string(),
            output_dir: None,
            theme: "dark".to_string(),
        }
    }
}

impl Settings {
    pub fn load() -> AppResult<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            let settings = Self::default();
            settings.save()?;
            return Ok(settings);
        }

        let content = std::fs::read_to_string(&config_path)?;
        let settings: Settings =
            serde_json::from_str(&content).map_err(|e| AppError::Storage(e.to_string()))?;

        Ok(settings)
    }

    pub fn save(&self) -> AppResult<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content =
            serde_json::to_string_pretty(self).map_err(|e| AppError::Storage(e.to_string()))?;

        std::fs::write(&config_path, content)?;

        Ok(())
    }

    fn config_path() -> AppResult<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| AppError::Storage("Could not determine config directory".to_string()))?;

        Ok(config_dir.join("jorja-clipper").join("config.json"))
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
        assert_eq!(settings.theme, "dark");
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
            theme: "light".to_string(),
        };

        let content = serde_json::to_string_pretty(&settings).unwrap();
        fs::write(&config_path, content).unwrap();

        let loaded: Settings =
            serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(loaded.buffer_before, 10.0);
        assert_eq!(loaded.clip_key, "x");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
