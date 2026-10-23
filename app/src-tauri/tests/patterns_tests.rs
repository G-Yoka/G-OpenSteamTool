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
        None,
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
fn ipc_on_a_non_pe_file_fails_without_panicking() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("steam.exe"), b"fake").unwrap();
    std::fs::write(dir.path().join("steamclient64.dll"), b"not a pe image").unwrap();
    let err = g_opensteamtool::services::channels::generate_channel(
        dir.path().to_string_lossy().into_owned(),
        "ipc".to_string(),
        "steamclient".to_string(),
        None,
    )
    .unwrap_err();
    assert!(!err.is_empty());
}

#[test]
fn generate_ipc_channel_for_real_steamclient() {
    let (Ok(dll), Ok(idl)) = (
        std::env::var("OST_TEST_STEAMCLIENT"),
        std::env::var("OST_TEST_IDL"),
    ) else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("steam.exe"), b"fake").unwrap();
    std::fs::copy(Path::new(&dll), dir.path().join("steamclient64.dll")).unwrap();

    let file = g_opensteamtool::services::channels::generate_channel(
        dir.path().to_string_lossy().into_owned(),
        "ipc".to_string(),
        "steamclient".to_string(),
        Some(idl),
    )
    .expect("generate ipc");

    assert!(file.present);
    let written = std::fs::read_to_string(
        dir.path()
            .join("opensteamtool/ipc/steamclient/".to_string() + &file.expected_name + ".toml"),
    )
    .expect("read ipc toml");
    assert!(written.contains("[IClientUser]"));
    assert!(written.contains("funcHash = \"0xD6FC3200\""));

    if let Ok(golden_path) = std::env::var("OST_TEST_IPC_GOLDEN") {
        let golden = std::fs::read_to_string(golden_path).expect("golden ipc toml");
        assert_eq!(
            strip_comments(&written),
            strip_comments(&golden),
            "in-app ipc output differs from the python pipeline"
        );
    }
}

/// The embedded upstream IDL must be enough on its own.
#[test]
fn ipc_uses_the_embedded_idl_when_no_override_is_given() {
    let Ok(client) = std::env::var("OST_TEST_STEAMCLIENT") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("steam.exe"), b"fake").unwrap();
    std::fs::copy(Path::new(&client), dir.path().join("steamclient64.dll")).unwrap();

    let file = g_opensteamtool::services::channels::generate_channel(
        dir.path().to_string_lossy().into_owned(),
        "ipc".to_string(),
        "steamclient".to_string(),
        None,
    )
    .expect("ipc with embedded idl");

    assert!(file.present);
    assert!(!file.expected_name.is_empty());
}

#[test]
fn generate_all_writes_pattern_channels_and_reports_skips() {
    let Ok(ui) = std::env::var("OST_TEST_STEAMUI") else {
        return;
    };
    let Ok(client) = std::env::var("OST_TEST_STEAMCLIENT") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("steam.exe"), b"fake").unwrap();
    std::fs::copy(Path::new(&ui), dir.path().join("steamui.dll")).unwrap();
    std::fs::copy(Path::new(&client), dir.path().join("steamclient64.dll")).unwrap();
    let steam_dir = dir.path().to_string_lossy().into_owned();

    // Without an IDL the two pattern channels are still written.
    let report = g_opensteamtool::services::channels::generate_all(steam_dir.clone(), None)
        .expect("generate_all");
    let written: Vec<String> = report
        .files
        .iter()
        .map(|f| format!("{}/{}", f.channel, f.component))
        .collect();
    assert!(written.contains(&"pattern/steamui".to_string()));
    assert!(written.contains(&"pattern/steamclient".to_string()));
    assert!(
        written.contains(&"ipc/steamclient".to_string()),
        "embedded IDL must make ipc zero-input: {written:?} skipped={:?}",
        report.skipped
    );

    // With an IDL the same call also writes ipc.
    if let Ok(idl) = std::env::var("OST_TEST_IDL") {
        let report = g_opensteamtool::services::channels::generate_all(steam_dir, Some(idl))
            .expect("generate_all with idl");
        assert_eq!(report.files.len(), 3, "three channels expected");
        assert!(report.skipped.is_empty());
        assert!(report.files.iter().any(|f| f.channel == "ipc" && f.present));
    }
}