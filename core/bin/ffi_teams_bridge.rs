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
//! there.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    adapters::keychain::bridge_secret::bridge_secret_keychain,
    domain::models::DeviceId,
    ports::crypto::EmbeddingCipher,
    teams_bridge_composition::{
        BridgePipeline, PipelineHandles, app_bridge_config, start_teams_bridge_with_pipeline,
    },
};

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
    let database_path: PathBuf = database_path.to_path_buf();
    let spawned = std::thread::Builder::new()
        .name("teams-bridge".to_owned())
        .spawn(move || {
            // Here, not on the caller's thread: the keychain may be slow to answer.
            let Some(config) = app_bridge_config(&setting, &bridge_secret_keychain()) else {
                return;
            };
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
