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

#[test]
fn test_process_names_accepts_string_or_list() {
    // A bare string is accepted and becomes a one-element list.
    let single: Settings =
        serde_json::from_str(r#"{ "steam_app_id": 4000, "process_names": "gmod.exe" }"#)
            .expect("single string should parse");
    assert_eq!(single.process_names, vec!["gmod.exe".to_string()]);

    // A list is accepted as-is (covering multiple branches).
    let many: Settings =
        serde_json::from_str(r#"{ "steam_app_id": 4000, "process_names": ["hl2_linux", "gmod"] }"#)
            .expect("list should parse");
    assert_eq!(
        many.process_names,
        vec!["hl2_linux".to_string(), "gmod".to_string()]
    );
}

#[test]
fn test_idle_threshold_defaults_when_missing() {
    // A config from before idle_read_threshold_mb_s existed should still load,
    // falling back to the default rather than failing to parse.
    let s: Settings =
        serde_json::from_str(r#"{ "steam_app_id": 4000, "process_names": "gmod.exe" }"#)
            .expect("config without idle_read_threshold_mb_s should still parse");
    assert_eq!(s.idle_read_threshold_mb_s, 10.0);
}

#[tokio::test]
async fn test_read_config_self_heals_invalid() {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let dir_path = dir.path().to_path_buf();

    // A config left over from an older version, whose shape no longer matches.
    let stale = r#"{ "game_path": "C:/old/gmod.exe", "steam_app_id": null }"#;
    tokio::fs::write(dir_path.join("config.json"), stale)
        .await
        .expect("Failed to write stale config.");

    // Reading it should recover to defaults rather than erroring out...
    let settings = Settings::read_config(&dir_path)
        .await
        .expect("read_config should self-heal, not error");
    assert_eq!(settings, Settings::default());

    // ...and the invalid file should have been rewritten as valid config.
    let reread = Settings::read_config(&dir_path)
        .await
        .expect("Failed to read healed config.");
    assert_eq!(reread, Settings::default());

    // ...with the original (invalid) contents preserved as a backup.
    assert!(
        dir_path.join("config.json.bak").exists(),
        "invalid config should be backed up before resetting"
    );
}

#[tokio::test]
async fn test_nonpositive_threshold_resets_to_default() {
    let dir = tempfile::tempdir().expect("Failed to create temp dir");
    let dir_path = dir.path().to_path_buf();

    // A parseable but nonsensical threshold should degrade to the default.
    tokio::fs::write(
        dir_path.join("config.json"),
        r#"{ "steam_app_id": 4000, "process_names": "gmod.exe", "idle_read_threshold_mb_s": 0 }"#,
    )
    .await
    .expect("Failed to write config.");

    let settings = Settings::read_config(&dir_path)
        .await
        .expect("Failed to read config.");
    assert_eq!(settings.idle_read_threshold_mb_s, 10.0);
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
