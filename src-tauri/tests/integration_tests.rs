use jorja_clipper::{
    commands, controller::Controller, storage::ClipStore, video_server::VideoServer,
};
use std::sync::Arc;
use tauri::Manager;
use tempfile::TempDir;
use tokio::sync::Mutex;

// Helper to create a test controller without full Tauri app
async fn create_test_controller() -> (Arc<Mutex<Controller>>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let store = ClipStore::with_path(&db_path).unwrap();
    let clips_dir = temp_dir.path().join("clips");
    std::fs::create_dir_all(&clips_dir).unwrap();

    let controller = Controller {
        clipper: jorja_clipper::clipper::Clipper::new(5.0, 5.0),
        settings: jorja_clipper::settings::Settings::default(),
        store,
        current_video: None,
        clip_count: 0,
        is_clipping: false,
        clips_dir,
        last_play_path: None,
    };

    (Arc::new(Mutex::new(controller)), temp_dir)
}

#[tokio::test]
async fn test_get_settings_command() {
    let (controller_state, _temp_dir) = create_test_controller().await;

    // Directly test the logic without Tauri State wrapper
    let settings = {
        let ctrl = controller_state.lock().await;
        ctrl.settings.clone()
    };

    assert_eq!(settings.buffer_before, 5.0);
    assert_eq!(settings.buffer_after, 5.0);
    assert_eq!(settings.clip_key, "c");
}

#[tokio::test]
async fn test_save_settings_command() {
    let (controller_state, _temp_dir) = create_test_controller().await;

    let new_settings = jorja_clipper::settings::Settings {
        buffer_before: 10.0,
        buffer_after: 10.0,
        clip_key: "v".to_string(),
        output_dir: None,
        theme: jorja_clipper::settings::Theme::Dark,
    };

    // Test update_settings logic
    let result = {
        let mut ctrl = controller_state.lock().await;
        ctrl.update_settings(new_settings.clone())
    };

    assert!(result.is_ok());

    // Verify settings were updated
    let settings = {
        let ctrl = controller_state.lock().await;
        ctrl.settings.clone()
    };

    assert_eq!(settings.buffer_before, 10.0);
    assert_eq!(settings.buffer_after, 10.0);
    assert_eq!(settings.clip_key, "v");
}

#[tokio::test]
async fn test_get_clips_command() {
    let (controller_state, temp_dir) = create_test_controller().await;

    // Create a test video file
    let video_path = temp_dir.path().join("test_video.mp4");
    std::fs::write(&video_path, "fake video content").unwrap();

    // Create actual clip files
    let clip1_path = temp_dir.path().join("clip1.mp4");
    let clip2_path = temp_dir.path().join("clip2.mp4");
    std::fs::write(&clip1_path, "clip 1 content").unwrap();
    std::fs::write(&clip2_path, "clip 2 content").unwrap();

    // Set current video and add some clips
    {
        let mut ctrl = controller_state.lock().await;
        ctrl.current_video = Some(video_path.clone());
        ctrl.store
            .add_clip(
                video_path.to_str().unwrap(),
                clip1_path.to_str().unwrap(),
                10.0,
                20.0,
            )
            .unwrap();
        ctrl.store
            .add_clip(
                video_path.to_str().unwrap(),
                clip2_path.to_str().unwrap(),
                30.0,
                40.0,
            )
            .unwrap();
    }

    // Test get_clips logic
    let clips = {
        let mut ctrl = controller_state.lock().await;
        ctrl.get_clips().unwrap()
    };

    assert_eq!(clips.len(), 2);
}

#[tokio::test]
async fn test_delete_clip_command() {
    let (controller_state, temp_dir) = create_test_controller().await;

    // Create a test video file
    let video_path = temp_dir.path().join("test_video.mp4");
    std::fs::write(&video_path, "fake video content").unwrap();

    let clip_path = temp_dir.path().join("clip1.mp4");
    std::fs::write(&clip_path, "fake clip content").unwrap();

    // Add a clip
    let clip_id = {
        let mut ctrl = controller_state.lock().await;
        ctrl.current_video = Some(video_path.clone());
        let clip = ctrl
            .store
            .add_clip(
                video_path.to_str().unwrap(),
                clip_path.to_str().unwrap(),
                10.0,
                20.0,
            )
            .unwrap();
        clip.id
    };

    // Test delete_clip logic
    let result = {
        let ctrl = controller_state.lock().await;
        ctrl.delete_clip(clip_id, clip_path.to_str().unwrap())
    };

    assert!(result.is_ok());
    assert!(!clip_path.exists());
}

#[tokio::test]
async fn test_start_video_server_command() {
    let server = Arc::new(Mutex::new(VideoServer::new()));
    let temp_dir = TempDir::new().unwrap();
    let video_path = temp_dir.path().join("test_video.mp4");
    std::fs::write(&video_path, "fake video content").unwrap();

    // Test video server logic
    let port = {
        let mut srv = server.lock().await;
        srv.start(video_path).unwrap()
    };

    assert!(port > 0);
    assert!(port < 65535);
}

#[tokio::test]
async fn test_controller_initialization() {
    let (controller_state, _temp_dir) = create_test_controller().await;

    let ctrl = controller_state.lock().await;
    assert_eq!(ctrl.clip_count, 0);
    assert!(!ctrl.is_clipping);
    assert!(ctrl.current_video.is_none());
}

#[tokio::test]
async fn test_multiple_clips_workflow() {
    let (controller_state, temp_dir) = create_test_controller().await;

    let video_path = temp_dir.path().join("video.mp4");
    std::fs::write(&video_path, "video content").unwrap();

    // Add multiple clips
    {
        let mut ctrl = controller_state.lock().await;
        ctrl.current_video = Some(video_path.clone());

        for i in 0..5 {
            let clip_path = temp_dir.path().join(format!("clip_{}.mp4", i));
            std::fs::write(&clip_path, format!("clip {} content", i)).unwrap();

            ctrl.store
                .add_clip(
                    video_path.to_str().unwrap(),
                    clip_path.to_str().unwrap(),
                    i as f64 * 10.0,
                    i as f64 * 10.0 + 5.0,
                )
                .unwrap();
        }
    }

    // Get all clips
    let clips = {
        let mut ctrl = controller_state.lock().await;
        ctrl.get_clips().unwrap()
    };

    assert_eq!(clips.len(), 5);

    // Delete one clip
    let clip_to_delete = &clips[2];
    let result = {
        let ctrl = controller_state.lock().await;
        ctrl.delete_clip(clip_to_delete.id, &clip_to_delete.clip_path)
    };

    assert!(result.is_ok());

    // Verify deletion
    let remaining_clips = {
        let mut ctrl = controller_state.lock().await;
        ctrl.get_clips().unwrap()
    };

    assert_eq!(remaining_clips.len(), 4);
}
