# Rust/Tauri Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port Jorja Clipper from Python/PySide6 to Rust/Tauri to resolve mpv embedding issues on Linux Wayland.

**Architecture:** Three-layer architecture with Rust backend (business logic, FFmpeg, mpv), Tauri IPC bridge, and Svelte frontend. mpv runs as a child process with `--wid` embedding managed by Tauri's windowing layer.

**Tech Stack:** Rust, Tauri 2.0, Svelte 5, TypeScript, mpv (via IPC), FFmpeg (subprocess), SQLite (rusqlite)

---

## Phase 1: Project Scaffolding

### Task 1: Initialize Tauri + Svelte Project

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/build.rs`
- Create: `package.json`
- Create: `svelte.config.js`
- Create: `vite.config.ts`
- Create: `tsconfig.json`

- [ ] **Step 1: Create Tauri project structure**

```bash
mkdir -p src-tauri/src
mkdir -p src/lib/components
mkdir -p src/lib/stores
mkdir -p src/routes
```

- [ ] **Step 2: Create `src-tauri/Cargo.toml`**

```toml
[package]
name = "jorja-clipper"
version = "0.1.0"
description = "Cross-platform desktop app for instant sports highlight extraction"
authors = ["you"]
edition = "2021"

[build-dependencies]
tauri-build = { version = "2.0", features = [] }

[dependencies]
tauri = { version = "2.0", features = [] }
tauri-plugin-shell = "2.0"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
rusqlite = { version = "0.31", features = ["bundled"] }
thiserror = "1"
dirs = "5"
chrono = { version = "0.4", features = ["serde"] }

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

- [ ] **Step 3: Create `src-tauri/tauri.conf.json`**

```json
{
  "productName": "Jorja Clipper",
  "version": "0.1.0",
  "identifier": "com.jorja.clipper",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:1420",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "windows": [
      {
        "title": "Jorja Clipper",
        "width": 1200,
        "height": 700,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

- [ ] **Step 4: Create `src-tauri/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 5: Create `src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: Create `package.json`**

```json
{
  "name": "jorja-clipper",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite dev",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-check --tsconfig ./tsconfig.json",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "@tauri-apps/plugin-shell": "^2.0.0"
  },
  "devDependencies": {
    "@sveltejs/adapter-static": "^3.0.0",
    "@sveltejs/kit": "^2.0.0",
    "@tauri-apps/cli": "^2.0.0",
    "@types/node": "^20.0.0",
    "svelte": "^5.0.0",
    "svelte-check": "^4.0.0",
    "typescript": "^5.0.0",
    "vite": "^5.0.0"
  }
}
```

- [ ] **Step 7: Create `svelte.config.js`**

```javascript
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      pages: 'dist',
      assets: 'dist',
      fallback: 'index.html',
      precompress: false,
      strict: true
    })
  }
};

export default config;
```

- [ ] **Step 8: Create `vite.config.ts`**

```typescript
import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';

export default defineConfig({
  plugins: [sveltekit()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
});
```

- [ ] **Step 9: Create `tsconfig.json`**

```json
{
  "extends": "./.svelte-kit/tsconfig.json",
  "compilerOptions": {
    "allowJs": true,
    "checkJs": true,
    "esModuleInterop": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "skipLibCheck": true,
    "sourceMap": true,
    "strict": true,
    "moduleResolution": "bundler"
  }
}
```

- [ ] **Step 10: Install dependencies and verify build**

```bash
npm install
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: Both commands succeed without errors.

- [ ] **Step 11: Commit**

```bash
git add .
git commit -m "chore: initialize Tauri + Svelte project structure"
```

### Task 2: Create Basic Svelte Layout

**Files:**
- Create: `src/routes/+layout.svelte`
- Create: `src/routes/+page.svelte`
- Create: `src/app.html`
- Create: `static/.gitkeep`

- [ ] **Step 1: Create `src/app.html`**

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    %sveltekit.head%
  </head>
  <body data-sveltekit-preload-data="hover">
    <div style="display: contents">%sveltekit.body%</div>
  </body>
</html>
```

- [ ] **Step 2: Create `src/routes/+layout.svelte`**

```svelte
<script lang="ts">
  import '../app.css';
</script>

<div class="app-container">
  <slot />
</div>

<style>
  .app-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background-color: #1a1a2e;
    color: #e0e0e0;
  }
</style>
```

- [ ] **Step 3: Create `src/app.css`**

```css
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
  overflow: hidden;
}
```

- [ ] **Step 4: Create `src/routes/+page.svelte`**

```svelte
<script lang="ts">
</script>

<div class="main-content">
  <h1>Jorja Clipper</h1>
  <p>Port in progress...</p>
</div>

<style>
  .main-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 1rem;
  }
  
  h1 {
    font-size: 2rem;
    color: #e94560;
  }
  
  p {
    color: #888;
  }
</style>
```

- [ ] **Step 5: Create `static/.gitkeep`**

```bash
touch static/.gitkeep
```

- [ ] **Step 6: Verify dev server works**

```bash
npm run dev
```

Expected: Server starts on http://localhost:1420, page shows "Jorja Clipper" heading.

- [ ] **Step 7: Commit**

```bash
git add .
git commit -m "feat: add basic Svelte layout and homepage"
```

## Phase 2: Rust Backend - Error Handling & Storage

### Task 3: Define Error Types

**Files:**
- Create: `src-tauri/src/error.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Create `src-tauri/src/error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("FFmpeg error: {0}")]
    Ffmpeg(String),
    
    #[error("mpv IPC error: {0}")]
    MpvIpc(String),
    
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
```

- [ ] **Step 2: Update `src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;

use error::AppError;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Verify compilation**

```bash
cd src-tauri && cargo build
```

Expected: Builds successfully with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/
git commit -m "feat: define unified error types for backend"
```

### Task 4: Implement Settings Module

**Files:**
- Create: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Create `src-tauri/src/settings.rs`**

```rust
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
        let settings: Settings = serde_json::from_str(&content)
            .map_err(|e| AppError::Storage(e.to_string()))?;
        
        Ok(settings)
    }
    
    pub fn save(&self) -> AppResult<()> {
        let config_path = Self::config_path()?;
        
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| AppError::Storage(e.to_string()))?;
        
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
        
        let loaded: Settings = serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(loaded.buffer_before, 10.0);
        assert_eq!(loaded.clip_key, "x");
        
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
```

- [ ] **Step 2: Update `src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod settings;

use error::AppError;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test settings::
```

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/
git commit -m "feat: implement settings module with JSON persistence"
```

### Task 5: Implement Clip Storage

**Files:**
- Create: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Create `src-tauri/src/storage.rs`**

```rust
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub id: i64,
    pub video_path: String,
    pub clip_path: String,
    pub start_time: f64,
    pub end_time: f64,
    pub created_at: DateTime<Utc>,
}

pub struct ClipStore {
    conn: Connection,
}

impl ClipStore {
    pub fn new() -> AppResult<Self> {
        let db_path = Self::db_path()?;
        
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let conn = Connection::open(&db_path)?;
        let store = Self { conn };
        store.init_schema()?;
        
        Ok(store)
    }
    
    fn init_schema(&self) -> AppResult<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS clips (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                video_path TEXT NOT NULL,
                clip_path TEXT NOT NULL,
                start_time REAL NOT NULL,
                end_time REAL NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        
        Ok(())
    }
    
    pub fn add_clip(
        &self,
        video_path: &str,
        clip_path: &str,
        start_time: f64,
        end_time: f64,
    ) -> AppResult<Clip> {
        let created_at = Utc::now();
        
        self.conn.execute(
            "INSERT INTO clips (video_path, clip_path, start_time, end_time, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![video_path, clip_path, start_time, end_time, created_at.to_rfc3339()],
        )?;
        
        let id = self.conn.last_insert_rowid();
        
        Ok(Clip {
            id,
            video_path: video_path.to_string(),
            clip_path: clip_path.to_string(),
            start_time,
            end_time,
            created_at,
        })
    }
    
    pub fn get_clips_for_video(&self, video_path: &str) -> AppResult<Vec<Clip>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, video_path, clip_path, start_time, end_time, created_at
             FROM clips
             WHERE video_path = ?1
             ORDER BY created_at DESC"
        )?;
        
        let clips = stmt.query_map(params![video_path], |row| {
            Ok(Clip {
                id: row.get(0)?,
                video_path: row.get(1)?,
                clip_path: row.get(2)?,
                start_time: row.get(3)?,
                end_time: row.get(4)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })?;
        
        let mut result = Vec::new();
        for clip in clips {
            result.push(clip?);
        }
        
        Ok(result)
    }
    
    pub fn delete_clip(&self, id: i64) -> AppResult<()> {
        self.conn.execute("DELETE FROM clips WHERE id = ?1", params![id])?;
        Ok(())
    }
    
    fn db_path() -> AppResult<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| AppError::Storage("Could not determine config directory".to_string()))?;
        
        Ok(config_dir.join("jorja-clipper").join("clips.db"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_add_and_retrieve_clip() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let db_path = temp_dir.join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        
        let store = ClipStore { conn };
        store.init_schema().unwrap();
        
        let clip = store.add_clip(
            "/path/to/video.mp4",
            "/path/to/clip.mp4",
            10.0,
            20.0,
        ).unwrap();
        
        assert_eq!(clip.start_time, 10.0);
        assert_eq!(clip.end_time, 20.0);
        
        let clips = store.get_clips_for_video("/path/to/video.mp4").unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].clip_path, "/path/to/clip.mp4");
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
    
    #[test]
    fn test_delete_clip() {
        let temp_dir = std::env::temp_dir().join("jorja-clipper-test-db2");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let db_path = temp_dir.join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        
        let store = ClipStore { conn };
        store.init_schema().unwrap();
        
        let clip = store.add_clip(
            "/path/to/video.mp4",
            "/path/to/clip.mp4",
            10.0,
            20.0,
        ).unwrap();
        
        store.delete_clip(clip.id).unwrap();
        
        let clips = store.get_clips_for_video("/path/to/video.mp4").unwrap();
        assert_eq!(clips.len(), 0);
        
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
```

- [ ] **Step 2: Update `src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod settings;
mod storage;

use error::AppError;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test storage::
```

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/
git commit -m "feat: implement clip storage with SQLite"
```

## Phase 3: Rust Backend - FFmpeg Integration

### Task 6: Implement Clipper Module

**Files:**
- Create: `src-tauri/src/clipper.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Create `src-tauri/src/clipper.rs`**

```rust
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipResult {
    pub success: bool,
    pub path: String,
    pub start_time: f64,
    pub end_time: f64,
    pub error: Option<String>,
}

pub struct Clipper {
    pub buffer_before: f64,
    pub buffer_after: f64,
}

impl Clipper {
    pub fn new(buffer_before: f64, buffer_after: f64) -> Self {
        Self {
            buffer_before,
            buffer_after,
        }
    }
    
    pub fn calculate_times(
        &self,
        current_pos: f64,
        duration: f64,
    ) -> (f64, f64) {
        let start = (current_pos - self.buffer_before).max(0.0);
        let end = (current_pos + self.buffer_after).min(duration);
        (start, end)
    }
    
    pub fn output_path(&self, video_path: &Path, clip_number: i32) -> AppResult<PathBuf> {
        let video_dir = video_path.parent()
            .ok_or_else(|| AppError::Ffmpeg("Video has no parent directory".to_string()))?;
        
        let clips_dir = video_dir.join("clips");
        std::fs::create_dir_all(&clips_dir)?;
        
        let video_stem = video_path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| AppError::Ffmpeg("Video has no filename".to_string()))?;
        
        let clip_filename = format!("{}_clip_{:04}.mp4", video_stem, clip_number);
        
        Ok(clips_dir.join(clip_filename))
    }
    
    pub async fn save_clip(
        &self,
        video_path: &Path,
        start_time: f64,
        end_time: f64,
        output_path: &Path,
    ) -> AppResult<ClipResult> {
        // Verify FFmpeg is available
        let ffmpeg_check = Command::new("ffmpeg")
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
        
        if ffmpeg_check.is_err() {
            return Err(AppError::Ffmpeg("FFmpeg not found. Please install FFmpeg.".to_string()));
        }
        
        // Run FFmpeg with stream copy (lossless)
        let output = Command::new("ffmpeg")
            .args(&[
                "-y",                          // Overwrite output
                "-ss", &format!("{:.3}", start_time),
                "-to", &format!("{:.3}", end_time),
                "-i", video_path.to_str().unwrap(),
                "-c", "copy",                  // Stream copy (no re-encoding)
                "-avoid_negative_ts", "1",
                output_path.to_str().unwrap(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            
            // Clean up partial output
            if output_path.exists() {
                let _ = std::fs::remove_file(output_path);
            }
            
            return Err(AppError::Ffmpeg(format!("FFmpeg failed: {}", stderr)));
        }
        
        Ok(ClipResult {
            success: true,
            path: output_path.to_string_lossy().to_string(),
            start_time,
            end_time,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_calculate_times() {
        let clipper = Clipper::new(5.0, 5.0);
        
        let (start, end) = clipper.calculate_times(30.0, 120.0);
        assert_eq!(start, 25.0);
        assert_eq!(end, 35.0);
        
        // Test clamping at start
        let (start, end) = clipper.calculate_times(2.0, 120.0);
        assert_eq!(start, 0.0);
        assert_eq!(end, 7.0);
        
        // Test clamping at end
        let (start, end) = clipper.calculate_times(118.0, 120.0);
        assert_eq!(start, 113.0);
        assert_eq!(end, 120.0);
    }
    
    #[test]
    fn test_output_path() {
        let clipper = Clipper::new(5.0, 5.0);
        let video_path = Path::new("/videos/game.mp4");
        
        let output = clipper.output_path(video_path, 1).unwrap();
        
        assert!(output.to_str().unwrap().contains("/videos/clips/"));
        assert!(output.to_str().unwrap().contains("game_clip_0001.mp4"));
    }
    
    #[tokio::test]
    async fn test_save_clip_with_invalid_video() {
        let clipper = Clipper::new(5.0, 5.0);
        let video_path = Path::new("/nonexistent/video.mp4");
        let output_path = Path::new("/tmp/test_clip.mp4");
        
        let result = clipper.save_clip(video_path, 0.0, 10.0, output_path).await;
        
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Update `src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod settings;
mod storage;
mod clipper;

use error::AppError;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri && cargo test clipper::
```

Expected: All tests pass (note: `test_save_clip_with_invalid_video` requires FFmpeg installed).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/
git commit -m "feat: implement FFmpeg clipper with stream-copy"
```

## Phase 4: Rust Backend - mpv Integration

### Task 7: Implement mpv Player Module

**Files:**
- Create: `src-tauri/src/player.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Create `src-tauri/src/player.rs`**

```rust
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use crate::error::{AppError, AppResult};

const IPC_SOCKET: &str = "/tmp/jorja-mpv-socket";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MpvRequest {
    command: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MpvResponse {
    error: String,
    data: Option<serde_json::Value>,
}

pub struct Player {
    process: Option<Child>,
    socket: Mutex<Option<UnixStream>>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            process: None,
            socket: Mutex::new(None),
        }
    }
    
    pub async fn spawn(&mut self, wid: Option<u64>) -> AppResult<()> {
        // Clean up old socket
        let _ = std::fs::remove_file(IPC_SOCKET);
        
        let mut cmd = Command::new("mpv");
        cmd.args(&[
            "--idle",
            "--force-window",
            "--input-ipc-server=/tmp/jorja-mpv-socket",
        ]);
        
        if let Some(wid) = wid {
            cmd.arg(format!("--wid={}", wid));
        }
        
        // Prevent mpv from detecting Wayland
        let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
        if wid.is_some() {
            cmd.env_remove("WAYLAND_DISPLAY");
        }
        
        let child = cmd
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        
        self.process = Some(child);
        
        // Restore WAYLAND_DISPLAY
        if let Some(display) = wayland_display {
            std::env::set_var("WAYLAND_DISPLAY", display);
        }
        
        // Wait for socket to be ready
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Connect to IPC socket
        self.connect_ipc().await?;
        
        Ok(())
    }
    
    async fn connect_ipc(&self) -> AppResult<()> {
        let mut attempts = 0;
        let max_attempts = 10;
        
        loop {
            match UnixStream::connect(IPC_SOCKET).await {
                Ok(stream) => {
                    *self.socket.lock().await = Some(stream);
                    return Ok(());
                }
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(AppError::MpvIpc(format!(
                            "Failed to connect to mpv IPC socket after {} attempts: {}",
                            max_attempts, e
                        )));
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
    }
    
    async fn send_command(&self, command: Vec<serde_json::Value>) -> AppResult<Option<serde_json::Value>> {
        let mut socket_guard = self.socket.lock().await;
        let socket = socket_guard.as_mut()
            .ok_or_else(|| AppError::MpvIpc("Not connected to mpv".to_string()))?;
        
        let request = MpvRequest { command };
        let request_json = serde_json::to_string(&request)
            .map_err(|e| AppError::MpvIpc(e.to_string()))?;
        
        socket.write_all(request_json.as_bytes()).await?;
        socket.write_all(b"\n").await?;
        socket.flush().await?;
        
        let mut reader = BufReader::new(socket);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;
        
        let response: MpvResponse = serde_json::from_str(&response_line)
            .map_err(|e| AppError::MpvIpc(e.to_string()))?;
        
        if response.error != "success" {
            return Err(AppError::MpvIpc(response.error));
        }
        
        Ok(response.data)
    }
    
    pub async fn load(&mut self, path: &Path) -> AppResult<f64> {
        let path_str = path.to_str()
            .ok_or_else(|| AppError::MpvIpc("Invalid path".to_string()))?;
        
        self.send_command(vec![
            "loadfile".into(),
            path_str.into(),
            "replace".into(),
        ]).await?;
        
        // Wait for file to load
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        
        // Get duration
        let duration = self.get_duration().await?;
        
        Ok(duration)
    }
    
    pub async fn toggle_pause(&self) -> AppResult<()> {
        self.send_command(vec![
            "cycle".into(),
            "pause".into(),
        ]).await?;
        
        Ok(())
    }
    
    pub async fn get_position(&self) -> AppResult<f64> {
        let data = self.send_command(vec![
            "get_property".into(),
            "time-pos".into(),
        ]).await?;
        
        data.and_then(|v| v.as_f64())
            .ok_or_else(|| AppError::MpvIpc("Invalid position response".to_string()))
    }
    
    pub async fn get_duration(&self) -> AppResult<f64> {
        let data = self.send_command(vec![
            "get_property".into(),
            "duration".into(),
        ]).await?;
        
        data.and_then(|v| v.as_f64())
            .ok_or_else(|| AppError::MpvIpc("Invalid duration response".to_string()))
    }
    
    pub async fn seek(&self, seconds: f64, relative: bool) -> AppResult<()> {
        let mode = if relative { "relative" } else { "absolute" };
        
        self.send_command(vec![
            "seek".into(),
            seconds.into(),
            mode.into(),
        ]).await?;
        
        Ok(())
    }
    
    pub async fn shutdown(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.kill().await;
        }
        
        let _ = std::fs::remove_file(IPC_SOCKET);
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        if let Some(mut child) = self.process.take() {
            let _ = child.start_kill();
        }
        let _ = std::fs::remove_file(IPC_SOCKET);
    }
}
```

- [ ] **Step 2: Update `src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod settings;
mod storage;
mod clipper;
mod player;

use error::AppError;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Verify compilation**

```bash
cd src-tauri && cargo build
```

Expected: Builds successfully (mpv must be installed for runtime testing).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/
git commit -m "feat: implement mpv player with IPC control"
```

### Task 8: Implement Controller Module

**Files:**
- Create: `src-tauri/src/controller.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Create `src-tauri/src/controller.rs`**

```rust
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::clipper::{Clipper, ClipResult};
use crate::error::{AppError, AppResult};
use crate::player::Player;
use crate::settings::Settings;
use crate::storage::{Clip, ClipStore};

pub struct Controller {
    pub player: Player,
    pub clipper: Clipper,
    pub settings: Settings,
    pub store: ClipStore,
    pub current_video: Option<PathBuf>,
    pub clip_count: i32,
    pub is_clipping: bool,
}

impl Controller {
    pub async fn new() -> AppResult<Self> {
        let settings = Settings::load()?;
        let store = ClipStore::new()?;
        let clipper = Clipper::new(settings.buffer_before, settings.buffer_after);
        let player = Player::new();
        
        Ok(Self {
            player,
            clipper,
            settings,
            store,
            current_video: None,
            clip_count: 0,
            is_clipping: false,
        })
    }
    
    pub async fn open_video(&mut self, path: PathBuf, wid: Option<u64>) -> AppResult<f64> {
        // Spawn mpv if not already running
        if self.player.process.is_none() {
            self.player.spawn(wid).await?;
        }
        
        let duration = self.player.load(&path).await?;
        self.current_video = Some(path.clone());
        
        // Load clips for this video
        let clips = self.store.get_clips_for_video(path.to_str().unwrap())?;
        self.clip_count = clips.len() as i32;
        
        Ok(duration)
    }
    
    pub async fn toggle_pause(&self) -> AppResult<()> {
        self.player.toggle_pause().await
    }
    
    pub async fn seek(&self, seconds: f64) -> AppResult<()> {
        self.player.seek(seconds, true).await
    }
    
    pub async fn get_position(&self) -> AppResult<f64> {
        self.player.get_position().await
    }
    
    pub async fn save_clip(&mut self) -> AppResult<ClipResult> {
        if self.is_clipping {
            return Err(AppError::ClipInProgress);
        }
        
        let video_path = self.current_video.as_ref()
            .ok_or(AppError::NoVideoLoaded)?
            .clone();
        
        self.is_clipping = true;
        
        let result = async {
            let current_pos = self.player.get_position().await?;
            let duration = self.player.get_duration().await?;
            
            let (start_time, end_time) = self.clipper.calculate_times(current_pos, duration);
            let clip_number = self.clip_count + 1;
            let output_path = self.clipper.output_path(&video_path, clip_number)?;
            
            let result = self.clipper.save_clip(&video_path, start_time, end_time, &output_path).await?;
            
            if result.success {
                let clip = self.store.add_clip(
                    video_path.to_str().unwrap(),
                    &result.path,
                    start_time,
                    end_time,
                )?;
                self.clip_count += 1;
            }
            
            Ok(result)
        }.await;
        
        self.is_clipping = false;
        
        result
    }
    
    pub fn get_clips(&self) -> AppResult<Vec<Clip>> {
        if let Some(video_path) = &self.current_video {
            self.store.get_clips_for_video(video_path.to_str().unwrap())
        } else {
            Ok(Vec::new())
        }
    }
    
    pub async fn shutdown(&mut self) {
        self.player.shutdown().await;
    }
}
```

- [ ] **Step 2: Update `src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod settings;
mod storage;
mod clipper;
mod player;
mod controller;

use error::AppError;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Verify compilation**

```bash
cd src-tauri && cargo build
```

Expected: Builds successfully.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/
git commit -m "feat: implement controller orchestration layer"
```

## Phase 5: Tauri IPC Commands

### Task 9: Implement Tauri Commands

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Create `src-tauri/src/commands.rs`**

```rust
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use crate::clipper::ClipResult;
use crate::controller::Controller;
use crate::error::AppError;
use crate::storage::Clip;

#[tauri::command]
pub async fn open_video(
    state: State<'_, Arc<Mutex<Controller>>>,
    path: String,
    wid: Option<u64>,
) -> Result<f64, String> {
    let mut ctrl = state.lock().await;
    ctrl.open_video(PathBuf::from(path), wid)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_pause(
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), String> {
    let ctrl = state.lock().await;
    ctrl.toggle_pause()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn seek(
    state: State<'_, Arc<Mutex<Controller>>>,
    seconds: f64,
) -> Result<(), String> {
    let ctrl = state.lock().await;
    ctrl.seek(seconds)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_position(
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<f64, String> {
    let ctrl = state.lock().await;
    ctrl.get_position()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_clip(
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<ClipResult, String> {
    let mut ctrl = state.lock().await;
    ctrl.save_clip()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_clips(
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<Vec<Clip>, String> {
    let ctrl = state.lock().await();
    ctrl.get_clips()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn shutdown(
    state: State<'_, Arc<Mutex<Controller>>>,
) -> Result<(), String> {
    let mut ctrl = state.lock().await;
    ctrl.shutdown().await;
    Ok(())
}
```

- [ ] **Step 2: Update `src-tauri/src/main.rs`**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod error;
mod settings;
mod storage;
mod clipper;
mod player;
mod controller;
mod commands;

use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let controller = Arc::new(Mutex::new(
        controller::Controller::new()
            .await
            .expect("Failed to initialize controller")
    ));
    
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(controller)
        .invoke_handler(tauri::generate_handler![
            commands::open_video,
            commands::toggle_pause,
            commands::seek,
            commands::get_position,
            commands::save_clip,
            commands::get_clips,
            commands::shutdown,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 3: Verify compilation**

```bash
cd src-tauri && cargo build
```

Expected: Builds successfully.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/
git commit -m "feat: implement Tauri IPC commands"
```

## Phase 6: Svelte Frontend - Core Components

### Task 10: Create API Layer and Types

**Files:**
- Create: `src/lib/api.ts`
- Create: `src/lib/types.ts`

- [ ] **Step 1: Create `src/lib/types.ts`**

```typescript
export interface ClipResult {
  success: boolean;
  path: string;
  start_time: number;
  end_time: number;
  error?: string;
}

export interface Clip {
  id: number;
  video_path: string;
  clip_path: string;
  start_time: number;
  end_time: number;
  created_at: string;
}

export interface Settings {
  buffer_before: number;
  buffer_after: number;
  clip_key: string;
  output_dir?: string;
  theme: string;
}
```

- [ ] **Step 2: Create `src/lib/api.ts`**

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { Clip, ClipResult } from './types';

export const api = {
  openVideo: (path: string, wid?: number) => 
    invoke<number>('open_video', { path, wid }),
  
  togglePause: () => 
    invoke<void>('toggle_pause'),
  
  seek: (seconds: number) => 
    invoke<void>('seek', { seconds }),
  
  getPosition: () => 
    invoke<number>('get_position'),
  
  saveClip: () => 
    invoke<ClipResult>('save_clip'),
  
  getClips: () => 
    invoke<Clip[]>('get_clips'),
  
  shutdown: () => 
    invoke<void>('shutdown'),
};
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/
git commit -m "feat: add TypeScript types and API layer"
```

### Task 11: Create VideoPlayer Component

**Files:**
- Create: `src/lib/components/VideoPlayer.svelte`

- [ ] **Step 1: Create `src/lib/components/VideoPlayer.svelte`**

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';

  export let videoLoaded = false;

  let container: HTMLDivElement;
  let rect: DOMRect | null = null;

  async function updateMpvWindow() {
    if (!rect || !videoLoaded) return;
    
    const appWindow = getCurrentWindow();
    const scaleFactor = await appWindow.scaleFactor();
    
    // Convert from logical to physical pixels
    const physicalRect = {
      x: Math.round(rect.x * scaleFactor),
      y: Math.round(rect.y * scaleFactor),
      width: Math.round(rect.width * scaleFactor),
      height: Math.round(rect.height * scaleFactor),
    };
    
    // Position mpv window (placeholder - actual implementation depends on mpv window management)
    await invoke('position_mpv_window', physicalRect);
  }

  onMount(() => {
    const observer = new ResizeObserver(() => {
      rect = container.getBoundingClientRect();
      updateMpvWindow();
    });
    
    observer.observe(container);
    
    return () => observer.disconnect();
  });
</script>

<div bind:this={container} class="video-container">
  {#if !videoLoaded}
    <div class="placeholder">
      <p>No video loaded</p>
      <p class="hint">Press O to open a video file</p>
    </div>
  {/if}
</div>

<style>
  .video-container {
    width: 100%;
    aspect-ratio: 16/9;
    background: #000;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  
  .placeholder {
    text-align: center;
    color: #888;
  }
  
  .hint {
    font-size: 0.9rem;
    margin-top: 0.5rem;
    opacity: 0.7;
  }
</style>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/
git commit -m "feat: add VideoPlayer component with placeholder pattern"
```

### Task 12: Create Main Page Layout

**Files:**
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Update `src/routes/+page.svelte`**

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { open } from '@tauri-apps/plugin-dialog';
  import VideoPlayer from '$lib/components/VideoPlayer.svelte';
  
  let videoLoaded = false;
  let videoPath = '';
  let duration = 0;
  let position = 0;
  let paused = true;
  
  async function openVideo() {
    const selected = await open({
      multiple: false,
      filters: [{
        name: 'Video',
        extensions: ['mp4', 'mkv', 'avi', 'mov', 'webm', 'ts']
      }]
    });
    
    if (selected) {
      videoPath = selected;
      duration = await api.openVideo(selected);
      videoLoaded = true;
      paused = true;
    }
  }
  
  async function togglePause() {
    await api.togglePause();
    paused = !paused;
  }
  
  async function seek(seconds: number) {
    await api.seek(seconds);
    position = await api.getPosition();
  }
  
  async function saveClip() {
    const result = await api.saveClip();
    if (result.success) {
      console.log('Clip saved:', result.path);
    } else {
      console.error('Clip failed:', result.error);
    }
  }
  
  onMount(() => {
    // Register global shortcuts
    const handleKeydown = async (e: KeyboardEvent) => {
      if (e.key === 'o' || e.key === 'O') {
        await openVideo();
      } else if (e.key === ' ') {
        e.preventDefault();
        await togglePause();
      } else if (e.key === 'c' || e.key === 'C') {
        await saveClip();
      } else if (e.key === 'ArrowLeft') {
        await seek(e.shiftKey ? -1 : -5);
      } else if (e.key === 'ArrowRight') {
        await seek(e.shiftKey ? 1 : 5);
      }
    };
    
    window.addEventListener('keydown', handleKeydown);
    
    return () => {
      window.removeEventListener('keydown', handleKeydown);
      api.shutdown();
    };
  });
</script>

<div class="main-layout">
  <div class="video-section">
    <VideoPlayer {videoLoaded} />
    
    <div class="controls">
      <button on:click={openVideo}>Open (O)</button>
      <button on:click={togglePause} disabled={!videoLoaded}>
        {paused ? 'Play' : 'Pause'} (Space)
      </button>
      <button on:click={saveClip} disabled={!videoLoaded}>
        Clip (C)
      </button>
    </div>
    
    {#if videoLoaded}
      <div class="status">
        Position: {position.toFixed(1)}s / {duration.toFixed(1)}s
      </div>
    {/if}
  </div>
  
  <div class="clips-section">
    <h2>Saved Clips</h2>
    <p class="placeholder">No clips yet</p>
  </div>
</div>

<style>
  .main-layout {
    display: grid;
    grid-template-columns: 2fr 1fr;
    height: 100%;
    gap: 1rem;
    padding: 1rem;
  }
  
  .video-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  
  .controls {
    display: flex;
    gap: 0.5rem;
  }
  
  button {
    padding: 0.5rem 1rem;
    background: #16213e;
    color: #e0e0e0;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 1rem;
  }
  
  button:hover:not(:disabled) {
    background: #0f3460;
  }
  
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  
  .status {
    color: #888;
    font-size: 0.9rem;
  }
  
  .clips-section {
    background: #16213e;
    padding: 1rem;
    border-radius: 4px;
    overflow-y: auto;
  }
  
  h2 {
    margin-bottom: 1rem;
    color: #e94560;
  }
  
  .placeholder {
    color: #888;
    font-style: italic;
  }
</style>
```

- [ ] **Step 2: Test the app**

```bash
npm run tauri dev
```

Expected: App launches, shows video placeholder, "Open" button works, file dialog opens.

- [ ] **Step 3: Commit**

```bash
git add src/
git commit -m "feat: implement main page with video controls"
```

## Phase 7: Integration & Testing

### Task 13: End-to-End Testing

**Files:**
- None (manual testing)

- [ ] **Step 1: Test video opening**

```bash
npm run tauri dev
```

Actions:
- Click "Open" button
- Select a video file
- Verify video loads (placeholder disappears, mpv window should appear)

Expected: Video opens successfully, duration shown in status.

- [ ] **Step 2: Test playback controls**

Actions:
- Click "Play" button or press Space
- Verify video plays
- Click "Pause" or press Space again
- Verify video pauses
- Press ArrowLeft/Right to seek
- Verify position updates

Expected: All playback controls work.

- [ ] **Step 3: Test clipping**

Actions:
- Seek to a position in the video
- Click "Clip" button or press C
- Verify clip is created in `clips/` folder next to video
- Check console for success message

Expected: Clip saves successfully, appears in clips list.

- [ ] **Step 4: Test on Wayland**

```bash
# On a Wayland session
npm run tauri dev
```

Actions:
- Open a video
- Verify mpv window embeds correctly (no separate window)
- Test playback and clipping

Expected: Video embeds in main window, no Wayland-specific issues.

- [ ] **Step 5: Commit any fixes**

```bash
git add .
git commit -m "fix: resolve integration issues found during testing"
```

## Phase 8: Polish & Documentation

### Task 14: Add Settings Dialog

**Files:**
- Create: `src/lib/components/SettingsDialog.svelte`
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Create `src/lib/components/SettingsDialog.svelte`**

```svelte
<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { Settings } from '$lib/types';
  
  export let open = false;
  export let settings: Settings;
  
  const dispatch = createEventDispatcher();
  
  function save() {
    dispatch('save', settings);
    open = false;
  }
  
  function cancel() {
    dispatch('cancel');
    open = false;
  }
</script>

{#if open}
  <div class="modal-backdrop" on:click={cancel}>
    <div class="modal" on:click|stopPropagation>
      <h2>Settings</h2>
      
      <div class="field">
        <label for="buffer-before">Buffer Before (seconds)</label>
        <input
          id="buffer-before"
          type="number"
          step="0.5"
          bind:value={settings.buffer_before}
        />
      </div>
      
      <div class="field">
        <label for="buffer-after">Buffer After (seconds)</label>
        <input
          id="buffer-after"
          type="number"
          step="0.5"
          bind:value={settings.buffer_after}
        />
      </div>
      
      <div class="field">
        <label for="clip-key">Clip Hotkey</label>
        <input
          id="clip-key"
          type="text"
          bind:value={settings.clip_key}
        />
      </div>
      
      <div class="actions">
        <button on:click={cancel}>Cancel</button>
        <button on:click={save}>Save</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  
  .modal {
    background: #1a1a2e;
    padding: 2rem;
    border-radius: 8px;
    min-width: 400px;
  }
  
  h2 {
    margin-bottom: 1.5rem;
    color: #e94560;
  }
  
  .field {
    margin-bottom: 1rem;
  }
  
  label {
    display: block;
    margin-bottom: 0.5rem;
    color: #e0e0e0;
  }
  
  input {
    width: 100%;
    padding: 0.5rem;
    background: #16213e;
    color: #e0e0e0;
    border: 1px solid #0f3460;
    border-radius: 4px;
    font-size: 1rem;
  }
  
  .actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
    margin-top: 2rem;
  }
  
  button {
    padding: 0.5rem 1.5rem;
    background: #16213e;
    color: #e0e0e0;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }
  
  button:hover {
    background: #0f3460;
  }
</style>
```

- [ ] **Step 2: Update `src/routes/+page.svelte` to include settings**

Add to script section:
```typescript
import SettingsDialog from '$lib/components/SettingsDialog.svelte';
import type { Settings } from '$lib/types';

let settingsOpen = false;
let settings: Settings = {
  buffer_before: 5.0,
  buffer_after: 5.0,
  clip_key: 'c',
  theme: 'dark'
};

function openSettings() {
  settingsOpen = true;
}

function saveSettings(e: CustomEvent<Settings>) {
  settings = e.detail;
  // TODO: Save to backend
}
```

Add to template:
```svelte
<button on:click={openSettings}>Settings</button>

<SettingsDialog
  bind:open={settingsOpen}
  {settings}
  on:save={saveSettings}
  on:cancel={() => settingsOpen = false}
/>
```

- [ ] **Step 3: Test settings dialog**

```bash
npm run tauri dev
```

Actions:
- Click "Settings" button
- Verify dialog opens
- Change buffer times
- Click Save
- Verify dialog closes

Expected: Settings dialog works, values persist (visual only for now).

- [ ] **Step 4: Commit**

```bash
git add src/
git commit -m "feat: add settings dialog"
```

### Task 15: Final Integration Test & Documentation

**Files:**
- Create: `README.md` (update)

- [ ] **Step 1: Update README.md**

```markdown
# Jorja Clipper

Cross-platform desktop app for instant sports highlight extraction.

## Features

- **Instant clipping**: Press a hotkey during video playback to save a lossless clip
- **Configurable buffers**: Set pre/post buffers for perfect highlight timing
- **Stream copy**: Uses FFmpeg `-c copy` for instant, lossless clipping
- **Cross-platform**: Works on Linux (Wayland/X11), Windows, and macOS

## Tech Stack

- **Backend**: Rust (Tauri 2.0)
- **Frontend**: Svelte 5 + TypeScript
- **Video**: mpv (via IPC)
- **Clipping**: FFmpeg (subprocess)
- **Storage**: SQLite

## Development

### Prerequisites

- Rust (1.70+)
- Node.js (18+)
- mpv
- FFmpeg

### Setup

```bash
npm install
```

### Run Development Server

```bash
npm run tauri dev
```

### Build Release

```bash
npm run tauri build
```

## Usage

1. Click "Open" or press `O` to load a video
2. Press `Space` to play/pause
3. Press `C` to save a clip at current position
4. Use arrow keys to seek (±5s, or ±1s with Shift)

## Architecture

The app follows a three-layer architecture:

1. **Rust Backend**: Business logic, FFmpeg integration, mpv process management
2. **Tauri IPC**: Type-safe command interface
3. **Svelte Frontend**: UI rendering, user input

mpv runs as a child process with `--wid` embedding managed by Tauri's windowing layer, providing reliable video embedding on all platforms including Linux Wayland.

## License

MIT
```

- [ ] **Step 2: Run full test suite**

```bash
cd src-tauri && cargo test
npm run check
```

Expected: All tests pass, no TypeScript errors.

- [ ] **Step 3: Test complete workflow**

```bash
npm run tauri dev
```

Actions:
- Open video
- Play/pause
- Seek
- Save clip
- Verify clip in `clips/` folder
- Open settings
- Change buffer times
- Save another clip with new buffers

Expected: All features work end-to-end.

- [ ] **Step 4: Commit final changes**

```bash
git add .
git commit -m "docs: update README and complete integration"
```

- [ ] **Step 5: Tag release**

```bash
git tag v0.1.0
git push origin main --tags
```

Expected: Release tagged and pushed.

---

## Success Criteria Checklist

- [ ] Video opens and plays on Linux Wayland (no embedding issues)
- [ ] Clipping works (FFmpeg stream-copy, <1s for 10s clip)
- [ ] All hotkeys work (O, Space, C, Arrow keys)
- [ ] Settings dialog works
- [ ] Clips saved to SQLite and visible in list
- [ ] Binary size <50MB (excluding FFmpeg)
- [ ] Startup time <2s
- [ ] All Rust tests pass
- [ ] No TypeScript errors
- [ ] Works on Linux (tested)
- [ ] Works on Windows (build test)
- [ ] Works on macOS (build test)

## Next Steps After This Plan

1. Add clip list UI with undo functionality
2. Implement settings persistence
3. Add theming support
4. Add plugin system (WebAssembly-based)
5. Optimize mpv window positioning (smoother resizing)
6. Add batch clipping feature
7. Create CI/CD pipeline for automated builds
