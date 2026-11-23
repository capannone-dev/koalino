//! Registers the native messaging host with the browsers, so the Koalino extension (and only
//! it) can start the host.
//!
//! Each browser reads a JSON manifest naming the host, its executable and the extensions allowed
//! to start it. On Linux the manifest is a file in a per-browser directory; on Windows it is a
//! file next to the host, found through a registry key under `HKCU`. Firefox is always
//! registered; Chrome, Chromium and Edge only when their profile directory exists (Linux) or
//! always (Windows, where an unused key is harmless).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::json;

/// The host's name, as the extension passes it to `connectNative`.
pub const HOST_NAME: &str = "com.note_taker.browser_bridge";
/// The Firefox add-on ID (`browser_specific_settings.gecko.id` in the extension manifest).
pub const FIREFOX_EXTENSION_ID: &str = "browser-bridge@note-taker.local";
/// The Chromium extension ID, fixed by the public `key` in the extension manifest.
pub const CHROMIUM_EXTENSION_ID: &str = "dgmaacilnokmfecfdcajdjadeboemnpd";
/// What the browsers show for the host.
const HOST_DESCRIPTION: &str = "Koalino: sends online meetings to the local Koalino app";

/// A browser the host can be registered with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Browser {
    /// Mozilla Firefox (including ESR).
    Firefox,
    /// Google Chrome.
    Chrome,
    /// Chromium.
    Chromium,
    /// Microsoft Edge.
    Edge,
}

impl Browser {
    /// Every supported browser.
    pub const ALL: [Self; 4] = [Self::Firefox, Self::Chrome, Self::Chromium, Self::Edge];

    /// A short lowercase name, used for the Windows manifest file. Side effects: None.
    pub fn short_name(self) -> &'static str {
        match self {
            Self::Firefox => "firefox",
            Self::Chrome => "chrome",
            Self::Chromium => "chromium",
            Self::Edge => "edge",
        }
    }
}

/// Builds the host manifest for `browser`.
///
/// Inputs: The browser and the absolute path of the host executable.
/// Outputs: The manifest JSON. Side effects: None.
pub fn host_manifest(browser: Browser, host_path: &Path) -> serde_json::Value {
    let mut manifest = json!({
        "name": HOST_NAME,
        "description": HOST_DESCRIPTION,
        "path": host_path.to_string_lossy(),
        "type": "stdio",
    });
    match browser {
        Browser::Firefox => {
            manifest["allowed_extensions"] = json!([FIREFOX_EXTENSION_ID]);
        }
        Browser::Chrome | Browser::Chromium | Browser::Edge => {
            manifest["allowed_origins"] =
                json!([format!("chrome-extension://{CHROMIUM_EXTENSION_ID}/")]);
        }
    }
    manifest
}

/// Where `browser` looks for host manifests on Linux.
///
/// Inputs: The browser and the user's home directory.
/// Outputs: The manifest file path. Side effects: None.
pub fn linux_manifest_path(browser: Browser, home: &Path) -> PathBuf {
    let directory = match browser {
        Browser::Firefox => home.join(".mozilla").join("native-messaging-hosts"),
        Browser::Chrome => home.join(".config/google-chrome/NativeMessagingHosts"),
        Browser::Chromium => home.join(".config/chromium/NativeMessagingHosts"),
        Browser::Edge => home.join(".config/microsoft-edge/NativeMessagingHosts"),
    };
    directory.join(format!("{HOST_NAME}.json"))
}

/// Where `browser` looks for host manifests on macOS (#151).
///
/// Inputs: The browser and the user's home directory.
/// Outputs: The manifest file path, under `~/Library/Application Support`. Side effects: None.
pub fn macos_manifest_path(browser: Browser, home: &Path) -> PathBuf {
    let support = home.join("Library").join("Application Support");
    let directory = match browser {
        Browser::Firefox => support.join("Mozilla"),
        Browser::Chrome => support.join("Google").join("Chrome"),
        Browser::Chromium => support.join("Chromium"),
        Browser::Edge => support.join("Microsoft Edge"),
    };
    directory
        .join("NativeMessagingHosts")
        .join(format!("{HOST_NAME}.json"))
}

/// The registry key through which `browser` finds the host on Windows.
///
/// Outputs: The key, under `HKCU`. Side effects: None.
pub fn windows_registry_key(browser: Browser) -> String {
    let vendor = match browser {
        Browser::Firefox => r"Mozilla",
        Browser::Chrome => r"Google\Chrome",
        Browser::Chromium => r"Chromium",
        Browser::Edge => r"Microsoft\Edge",
    };
    format!(r"HKCU\Software\{vendor}\NativeMessagingHosts\{HOST_NAME}")
}

/// Writes the Linux manifests.
///
/// Inputs: The absolute host path and the user's home directory.
/// Outputs: The manifest files written: Firefox's always, the others when the browser's
/// directory (for example `~/.config/chromium`) exists.
/// Side effects: Creates directories and writes files.
pub fn install_linux(host_path: &Path, home: &Path) -> Result<Vec<PathBuf>> {
    install_in_home(host_path, home, linux_manifest_path)
}

/// Writes the macOS manifests (#151).
///
/// Inputs: The absolute host path (inside `Koalino.app/Contents/MacOS`) and the home directory.
/// Outputs: The manifest files written: Firefox's always, the others when the browser's folder
/// in `~/Library/Application Support` (for example `Google/Chrome`) exists.
/// Side effects: Creates directories and writes files.
pub fn install_macos(host_path: &Path, home: &Path) -> Result<Vec<PathBuf>> {
    install_in_home(host_path, home, macos_manifest_path)
}

/// Writes a manifest for Firefox and for each other browser whose folder exists.
///
/// Inputs: The host path, the home directory, and where each browser looks for its manifest
/// (`<browser folder>/<hosts folder>/<name>.json`). Outputs: The manifest files written.
/// Side effects: Creates directories and writes files.
fn install_in_home(
    host_path: &Path,
    home: &Path,
    manifest_path_for: fn(Browser, &Path) -> PathBuf,
) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for browser in Browser::ALL {
        let manifest_path = manifest_path_for(browser, home);
        let Some(host_directory) = manifest_path.parent() else {
            continue;
        };
        let installed =
            browser == Browser::Firefox || host_directory.parent().is_some_and(Path::is_dir);
        if !installed {
            continue;
        }
        write_manifest(&manifest_path, &host_manifest(browser, host_path))?;
        written.push(manifest_path);
    }
    Ok(written)
}

/// Drops the `\\?\` verbatim prefix that `canonicalize` gives Windows paths.
///
/// Chrome finds neither a manifest nor a host through such a path, so the registry value and the
/// manifest's `path` must be ordinary ones (#102).
///
/// Inputs: A path. Outputs: `\\?\C:\…` as `C:\…` and `\\?\UNC\server\…` as `\\server\…`;
/// any other path (including verbatim volume paths, which have no ordinary form) unchanged.
/// Side effects: None.
pub fn without_verbatim_prefix(path: &Path) -> PathBuf {
    let Some(text) = path.to_str() else {
        return path.to_path_buf();
    };
    if let Some(share) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{share}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) if is_drive_path(rest) => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

/// Whether text starts like `C:\`.
fn is_drive_path(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\'
}

/// Writes the Windows manifests next to the host and points each browser's registry key at them.
///
/// Inputs: The absolute host path (a `\\?\` prefix is dropped first).
/// Outputs: The manifest files written.
/// Side effects: Writes files beside the host; runs `reg add` for each browser.
pub fn install_windows(host_path: &Path) -> Result<Vec<PathBuf>> {
    let host_path = without_verbatim_prefix(host_path);
    let host_path = host_path.as_path();
    let host_directory = host_path
        .parent()
        .context("the host path has no directory")?;
    let mut written = Vec::new();
    for browser in Browser::ALL {
        let manifest_path =
            host_directory.join(format!("{HOST_NAME}.{}.json", browser.short_name()));
        write_manifest(&manifest_path, &host_manifest(browser, host_path))?;
        let status = std::process::Command::new("reg")
            .args([
                "add",
                &windows_registry_key(browser),
                "/ve",
                "/t",
                "REG_SZ",
                "/d",
            ])
            .arg(&manifest_path)
            .arg("/f")
            .status()
            .context("unable to run reg")?;
        if !status.success() {
            bail!("reg add failed for {}", browser.short_name());
        }
        written.push(manifest_path);
    }
    Ok(written)
}

/// Writes one manifest, creating its directory.
fn write_manifest(manifest_path: &Path, manifest: &serde_json::Value) -> Result<()> {
    if let Some(directory) = manifest_path.parent() {
        std::fs::create_dir_all(directory)
            .with_context(|| format!("unable to create {}", directory.display()))?;
    }
    let text = serde_json::to_string_pretty(manifest).context("manifest is not serializable")?;
    std::fs::write(manifest_path, text)
        .with_context(|| format!("unable to write {}", manifest_path.display()))
}

/// The line `browser-host install` prints about the bridge secret.
///
/// Inputs: `None` when the secret is in the keychain, or why it could not be created there.
/// Outputs: One line for the owner. Without a secret it says what each receiver does (#125): the
/// Koalino app creates the secret when it starts, or keeps its bridge off; only a daemon with no
/// `NOTE_TAKER_TEAMS_BRIDGE_SECRET` accepts any local program. Side effects: None.
pub fn bridge_secret_report(keychain_error: Option<&str>) -> String {
    match keychain_error {
        None => "Bridge secret: in the keychain (note-taker / teams-bridge-secret); the app and the daemon read it too".to_owned(),
        Some(error) => format!(
            "Bridge secret: not created ({error}). The Koalino app creates it when it starts, and \
             keeps its bridge off if the keychain cannot hold one. A daemon needs \
             NOTE_TAKER_TEAMS_BRIDGE_SECRET set for itself and for the browser; without it, its \
             bridge accepts any local program."
        ),
    }
}
