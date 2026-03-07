use crate::config::Settings;

#[tokio::test]
async fn test_write_config() {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let dir_path = dir.path().to_path_buf();

    let settings = Settings::default();
    settings
        .write_config(&dir_path)
        .await
        .expect("Failed to write config.");

    let read_settings = Settings::read_config(&dir_path)
        .await
        .expect("Failed to read config.");

    assert_eq!(settings, read_settings);
}

#[tokio::test]
async fn test_read_config() {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let dir_path = dir.path().to_path_buf();

    let expected = Settings::default();
    expected
        .write_config(&dir_path)
        .await
        .expect("Failed to write config.");

    let settings = Settings::read_config(&dir_path)
        .await
        .expect("Failed to read config.");

    assert_eq!(settings, expected);
}
