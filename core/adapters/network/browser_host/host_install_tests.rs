//! Specifies how the host registers itself with each browser.

use std::path::{Path, PathBuf};

use super::host_install::{
    Browser, CHROMIUM_EXTENSION_ID, FIREFOX_EXTENSION_ID, HOST_NAME, bridge_secret_report,
    host_manifest, install_linux, linux_manifest_path, windows_registry_key,
    without_verbatim_prefix,
};

/// A unique temporary home directory, removed on drop.
struct TemporaryHome(PathBuf);

impl Drop for TemporaryHome {
    /// Removes the directory tree.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn firefox_allows_only_the_note_taker_extension() {
    let manifest = host_manifest(Browser::Firefox, Path::new("/opt/nt/browser-host"));
    assert_eq!(manifest["name"], HOST_NAME);
    assert_eq!(manifest["type"], "stdio");
    assert_eq!(manifest["path"], "/opt/nt/browser-host");
    assert_eq!(manifest["allowed_extensions"][0], FIREFOX_EXTENSION_ID);
    assert!(manifest.get("allowed_origins").is_none());
}

#[test]
fn chromium_browsers_allow_only_the_note_taker_origin() {
    for browser in [Browser::Chrome, Browser::Chromium, Browser::Edge] {
        let manifest = host_manifest(browser, Path::new("/opt/nt/browser-host"));
        assert_eq!(
            manifest["allowed_origins"][0],
            format!("chrome-extension://{CHROMIUM_EXTENSION_ID}/")
        );
        assert!(manifest.get("allowed_extensions").is_none());
    }
}

#[test]
fn linux_manifests_go_where_each_browser_looks() {
    let home = Path::new("/home/ada");
    for (browser, expected) in [
        (
            Browser::Firefox,
            "/home/ada/.mozilla/native-messaging-hosts",
        ),
        (
            Browser::Chrome,
            "/home/ada/.config/google-chrome/NativeMessagingHosts",
        ),
        (
            Browser::Chromium,
            "/home/ada/.config/chromium/NativeMessagingHosts",
        ),
        (
            Browser::Edge,
            "/home/ada/.config/microsoft-edge/NativeMessagingHosts",
        ),
    ] {
        assert_eq!(
            linux_manifest_path(browser, home),
            Path::new(expected).join(format!("{HOST_NAME}.json"))
        );
    }
}

#[test]
fn windows_registry_keys_name_the_host() {
    assert_eq!(
        windows_registry_key(Browser::Firefox),
        format!(r"HKCU\Software\Mozilla\NativeMessagingHosts\{HOST_NAME}")
    );
    assert_eq!(
        windows_registry_key(Browser::Chrome),
        format!(r"HKCU\Software\Google\Chrome\NativeMessagingHosts\{HOST_NAME}")
    );
    assert_eq!(
        windows_registry_key(Browser::Edge),
        format!(r"HKCU\Software\Microsoft\Edge\NativeMessagingHosts\{HOST_NAME}")
    );
}

#[test]
fn installing_on_linux_writes_firefox_and_the_browsers_present() {
    let home = TemporaryHome(
        std::env::temp_dir().join(format!("note-taker-host-home-{}", uuid::Uuid::new_v4())),
    );
    std::fs::create_dir_all(home.0.join(".config/chromium"))
        .unwrap_or_else(|error| panic!("home should be created: {error}"));

    let written = install_linux(Path::new("/opt/nt/browser-host"), &home.0)
        .unwrap_or_else(|error| panic!("install should succeed: {error}"));

    assert_eq!(
        written,
        vec![
            linux_manifest_path(Browser::Firefox, &home.0),
            linux_manifest_path(Browser::Chromium, &home.0),
        ],
        "Chrome and Edge are not installed in this home"
    );
    let chromium: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&written[1])
            .unwrap_or_else(|error| panic!("manifest should exist: {error}")),
    )
    .unwrap_or_else(|error| panic!("manifest should be JSON: {error}"));
    assert_eq!(chromium["path"], "/opt/nt/browser-host");
}

#[test]
fn windows_paths_lose_the_verbatim_prefix_browsers_reject() {
    // `canonicalize` on Windows returns `\\?\` paths; Chrome finds neither the manifest nor the
    // host through them (seen on Windows 11 with Chrome 157, #102).
    assert_eq!(
        without_verbatim_prefix(Path::new(r"\\?\C:\Users\me\kx\browser-host.exe")),
        PathBuf::from(r"C:\Users\me\kx\browser-host.exe")
    );
    assert_eq!(
        without_verbatim_prefix(Path::new(r"\\?\UNC\server\share\browser-host.exe")),
        PathBuf::from(r"\\server\share\browser-host.exe")
    );
}

#[test]
fn ordinary_paths_are_kept_as_they_are() {
    for path in [
        r"C:\Program Files\Koalino\browser-host.exe",
        "/opt/nt/browser-host",
        r"\\?\Volume{0}\browser-host.exe",
    ] {
        assert_eq!(
            without_verbatim_prefix(Path::new(path)),
            PathBuf::from(path)
        );
    }
}

/// Verifies the install report names where the secret is kept when it could be created.
#[test]
fn the_report_names_the_keychain_entry_when_the_secret_is_kept() {
    let report = bridge_secret_report(None);
    assert!(report.contains("in the keychain"), "{report}");
    assert!(report.contains("teams-bridge-secret"), "{report}");
}

/// Verifies the report tells the app and the daemon apart when the keychain refused (#125): the
/// app keeps its bridge off (or creates the secret when it starts), while only a daemon without a
/// configured secret accepts any local program.
#[test]
fn the_report_without_a_secret_says_what_the_app_and_the_daemon_do() {
    let report = bridge_secret_report(Some("store locked"));
    assert!(report.contains("not created (store locked)"), "{report}");
    assert!(report.contains("Koalino app"), "{report}");
    assert!(
        report.contains("NOTE_TAKER_TEAMS_BRIDGE_SECRET"),
        "{report}"
    );
    assert!(
        !report.starts_with("Bridge secret: not created (store locked); the daemon's bridge then accepts any local program"),
        "{report}"
    );
}
