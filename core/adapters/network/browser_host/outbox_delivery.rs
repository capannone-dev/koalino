//! Delivers outbox sessions whose browser host has gone (#135): the browser closed, or the host
//! stopped, before Koalino took the meeting.
//!
//! Koalino runs `run_outbox_importer`, which every `IMPORT_INTERVAL` takes the delivery lock,
//! claims the sessions left behind, and replays each one, in the order they happened, to its own
//! Teams bridge over the loopback WebSocket, exactly as the host would have sent them. A browser
//! host that takes the lock delivers left-behind sessions too, before its own frames, so meetings
//! always reach Koalino in order. Each frame is deleted once sent; a session that can't be
//! finished stays claimed and is replayed from where it stopped next time.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use futures::SinkExt;
use tokio_tungstenite::tungstenite::Message;

use super::{
    bridge_client::{BridgeClientConfig, BridgeSocket, connect},
    outbox::{
        OutboxKey, OutboxSession, STALE_SESSION_AFTER, claim_stale_sessions, claimed_sessions,
        try_delivery_lock,
    },
};

/// How often Koalino looks for sessions left behind.
pub const IMPORT_INTERVAL: Duration = Duration::from_secs(30);

/// Sessions to replay, oldest first: those a stopped reader had claimed, then those whose host
/// stopped at least `stale_after` ago (claimed now).
///
/// Call it only while holding the delivery lock. Side effects: Renames folders.
pub(crate) fn left_behind_sessions(root: &Path, stale_after: Duration) -> Vec<PathBuf> {
    let mut sessions = claimed_sessions(root);
    sessions.extend(claim_stale_sessions(root, stale_after));
    sessions
}

/// Sends the sessions' frames in order over `socket`.
///
/// Outputs: The frames sent, and whether all were (`false` when the connection failed: the rest
/// stays on disk). Side effects: Reads and deletes frame files and emptied folders.
pub(crate) async fn send_sessions(
    socket: &mut BridgeSocket,
    sessions: Vec<PathBuf>,
    key: &OutboxKey,
) -> (usize, bool) {
    let mut sent = 0;
    for directory in sessions {
        let Ok(mut session) = OutboxSession::adopt(directory, key.clone()) else {
            continue;
        };
        while let Some(frame) = session.front() {
            if socket.send(Message::binary(frame)).await.is_err() {
                return (sent, false);
            }
            session.remove_front();
            sent += 1;
        }
        session.finish();
    }
    (sent, true)
}

/// One replay pass: takes the delivery lock, claims what was left behind and sends it to the
/// bridge at `config`.
///
/// Inputs: The outbox root, the key, the bridge to deliver to, and how long a session must have
/// been silent to count as left behind (`STALE_SESSION_AFTER` outside tests).
/// Outputs: The frames delivered (0 when another program is delivering, nothing waits, or the
/// bridge can't be reached).
/// Side effects: Connects to 127.0.0.1; reads, renames and deletes outbox files; logs counts.
pub async fn deliver_waiting_sessions(
    root: &Path,
    key: &OutboxKey,
    config: &BridgeClientConfig,
    stale_after: Duration,
) -> usize {
    let Some(_delivery_lock) = try_delivery_lock(root) else {
        return 0;
    };
    let sessions = left_behind_sessions(root, stale_after);
    if sessions.is_empty() {
        return 0;
    }
    let mut socket = match connect(config).await {
        Ok(socket) => socket,
        Err(reason) => {
            tracing::debug!(
                reason,
                "Kept meetings wait: the Teams bridge is not reachable"
            );
            return 0;
        }
    };
    let (sent, finished) = send_sessions(&mut socket, sessions, key).await;
    let _ = socket.close(None).await;
    if sent > 0 {
        tracing::info!(
            delivered_frames = sent,
            finished,
            "Meeting data kept by the browser host was taken over"
        );
    }
    sent
}

/// Replays left-behind sessions now and every `IMPORT_INTERVAL`, for the life of the bridge.
///
/// Side effects: See `deliver_waiting_sessions`.
pub async fn run_outbox_importer(root: PathBuf, key: OutboxKey, config: BridgeClientConfig) {
    loop {
        deliver_waiting_sessions(&root, &key, &config, STALE_SESSION_AFTER).await;
        tokio::time::sleep(IMPORT_INTERVAL).await;
    }
}
