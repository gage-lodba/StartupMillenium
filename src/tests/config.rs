use crate::config::Settings;
use serial_test::serial;
use std::env;

#[tokio::test]
#[serial]
async fn test_write_config() {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let original_dir = env::current_dir().expect("Failed to get current dir");
    env::set_current_dir(&dir).expect("Failed to set current dir");

    let settings = Settings::default();
    settings
        .write_config()
        .await
        .expect("Failed to write config.");

    let read_settings = Settings::read_config()
        .await
        .expect("Failed to read config.");

    let _ = env::set_current_dir(&original_dir);
    assert_eq!(settings, read_settings);
}

#[tokio::test]
#[serial]
async fn test_read_config() {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let original_dir = env::current_dir().expect("Failed to get current dir");
    env::set_current_dir(&dir).expect("Failed to set current dir");

    let expected = Settings::default();
    expected
        .write_config()
        .await
        .expect("Failed to write config.");

    let settings = Settings::read_config()
        .await
        .expect("Failed to read config.");

    let _ = env::set_current_dir(&original_dir);
    assert_eq!(settings, expected);
}
