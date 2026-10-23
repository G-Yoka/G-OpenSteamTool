//! End-to-end check of the in-app channel generator against a real Steam build.
//!
//! Env-gated so CI without Steam still passes:
//!   OST_TEST_STEAMUI         path to a real steamui.dll
//!   OST_TEST_STEAMUI_GOLDEN  optional TOML produced by the python pipeline

use std::path::Path;

fn strip_comments(text: &str) -> String {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn generate_pattern_channel_for_real_steamui() {
    let Ok(dll) = std::env::var("OST_TEST_STEAMUI") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("steam.exe"), b"fake").unwrap();
    std::fs::copy(Path::new(&dll), dir.path().join("steamui.dll")).unwrap();

    let steam_dir = dir.path().to_string_lossy().into_owned();
    let file = g_opensteamtool::services::channels::generate_channel(
        steam_dir.clone(),
        "pattern".to_string(),
        "steamui".to_string(),
    )
    .expect("generate_channel");

    assert!(file.present, "generated file must be picked up by inspection");
    assert_eq!(file.entries, 10, "steamui hook count");
    assert_eq!(file.expected_name.len(), 64, "file is named after the dll sha256");

    let written = std::fs::read_to_string(dir.path().join(
        "opensteamtool/pattern/steamui/".to_string() + &file.expected_name + ".toml",
    ))
    .expect("read generated toml");
    assert!(written.contains("name = \"AddProtobufAsBinary\""));

    if let Ok(golden_path) = std::env::var("OST_TEST_STEAMUI_GOLDEN") {
        let golden = std::fs::read_to_string(golden_path).expect("golden toml");
        assert_eq!(
            strip_comments(&written),
            strip_comments(&golden),
            "in-app output differs from the python pipeline"
        );
    }
}

#[test]
fn ipc_channel_is_reported_as_unsupported() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("steam.exe"), b"fake").unwrap();
    let err = g_opensteamtool::services::channels::generate_channel(
        dir.path().to_string_lossy().into_owned(),
        "ipc".to_string(),
        "steamclient".to_string(),
    )
    .unwrap_err();
    assert!(err.contains("IPCMessages"), "unexpected error: {err}");
}