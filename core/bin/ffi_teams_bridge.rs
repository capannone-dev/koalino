//! Hosts the Teams bridge in the desktop app (#123), so the browser extension (#102) records
//! meetings joined in the browser into the app's own database, without a daemon.
//!
//! On Linux and Windows the app starts the loopback bridge when its runtime opens, unless
//! `NOTE_TAKER_TEAMS_BRIDGE_ENABLED=false`. The bridge needs a secret: the app creates it in the
//! keychain on first use (`teams-bridge-secret`, the one the browser host reads), or takes
//! `NOTE_TAKER_TEAMS_BRIDGE_SECRET`. Without either it stays off. The meeting pipeline is the
//! app's own, built when the first browser meeting starts, so an app that never records from the
//! browser loads no model for it. The bridge runs on its own thread with its own Tokio runtime
//! for the life of the process. Android has no browser extension, so the bridge never starts
//! there. While the app runs it holds the browser outbox's running lock, so the browser host
//! knows not to start it again, and the bridge replays meetings the host kept for it (#135).

use std::{
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use crate::{
    adapters::{
        keychain::bridge_secret::bridge_secret_keychain,
        network::browser_host::outbox::{ExclusiveLock, browser_outbox_root, hold_app_running},
    },
    domain::models::DeviceId,
    ports::crypto::EmbeddingCipher,
    teams_bridge_composition::{
        BridgePipeline, PipelineHandles, app_bridge_config, start_teams_bridge_with_pipeline,
    },
};

/// Tells the browser host that Koalino runs, for the life of the process (#135), so it never
/// starts a second one.
///
/// Side effects: Creates and locks `app-running.lock` in the browser outbox folder; the OS
/// releases the lock when the process ends.
fn hold_app_running_lock() {
    static RUNNING_LOCK: OnceLock<Option<ExclusiveLock>> = OnceLock::new();
    RUNNING_LOCK.get_or_init(|| {
        let root = browser_outbox_root(|name| std::env::var(name).ok())?;
        let lock = hold_app_running(&root);
        if lock.is_none() {
            tracing::warn!("Another Koalino holds the running lock");
        }
        lock
    });
}

/// Starts the app's Teams bridge on a thread of its own, on desktops only.
///
/// Inputs: The app's database path, its device ID and the unlocked cipher; `setting` reads the
/// app's configuration (`NOTE_TAKER_TEAMS_BRIDGE_*`); `pipeline` returns the app's pipeline
/// worker and audio store, building them on first use.
/// Outputs: None; a bridge that is off or fails to start is logged without values.
/// Side effects: May create the keychain secret; spawns the `teams-bridge` thread, which listens
/// on 127.0.0.1 (port 8765 unless configured) until the process exits.
pub fn start_app_teams_bridge(
    database_path: &Path,
    device_id: DeviceId,
    embedding_cipher: Arc<dyn EmbeddingCipher>,
    setting: fn(&str) -> Option<String>,
    pipeline: fn() -> anyhow::Result<PipelineHandles>,
) {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return;
    }
    hold_app_running_lock();
    let database_path: PathBuf = database_path.to_path_buf();
    let spawned = std::thread::Builder::new()
        .name("teams-bridge".to_owned())
        .spawn(move || {
            // Here, not on the caller's thread: the keychain may be slow to answer.
            let Some(config) = app_bridge_config(&setting, &bridge_secret_keychain()) else {
                return;
            };
            // The extension may set Teams' spoken language to Koalino's (#141).
            let config = config.with_spoken_language_from(setting);
            // The extension may set Teams' spoken language to the app's (#141).
            let config = config.with_spoken_language_from(setting);
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    tracing::warn!(error = %error, "Teams bridge unavailable: no async runtime");
                    return;
                }
            };
            runtime.block_on(async move {
                let service = start_teams_bridge_with_pipeline(
                    &config,
                    &database_path,
                    device_id,
                    embedding_cipher,
                    BridgePipeline::on_first_meeting(Box::new(pipeline)),
                )
                .await;
                if service.is_some() {
                    // Serve until the process exits; dropping the service would stop it.
                    std::future::pending::<()>().await;
                }
                drop(service);
            });
        });
    if let Err(error) = spawned {
        tracing::warn!(error = %error, "Teams bridge thread could not start");
    }
}
