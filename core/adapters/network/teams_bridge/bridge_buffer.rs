//! Bounded, drop-oldest buffer between the Teams WebSocket reader and the meeting pump.
//!
//! The reader must never block on transcription, so it pushes decoded events here and returns to
//! the socket immediately. Audio is bounded by duration (`NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS`,
//! summed across participants) and slides by count; on overflow the oldest entries of that kind
//! are dropped and a rate-limited warning is logged. Meeting starts, meeting ends and documents
//! are never dropped, and events keep their arrival order so audio always follows the metadata
//! it belongs to.

use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use tracing::warn;

use crate::adapters::network::teams_bridge::message_decoder::BridgeEvent;

/// Most slides held before the oldest is dropped.
pub const MAXIMUM_BUFFERED_SLIDES: usize = 16;
/// Minimum spacing between two overflow warnings.
const OVERFLOW_WARNING_INTERVAL: Duration = Duration::from_secs(5);

/// What a push had to discard to stay within bounds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PushOutcome {
    /// Older audio chunks dropped by this push.
    pub dropped_audio_chunks: usize,
    /// Older slides dropped by this push.
    pub dropped_slides: usize,
}

/// Mutable buffer state guarded by the mutex.
#[derive(Default)]
struct BufferState {
    /// Queued events in arrival order.
    queued_events: VecDeque<BridgeEvent>,
    /// Total audio duration currently queued.
    buffered_audio_ms: u64,
    /// Number of slides currently queued.
    buffered_slides: usize,
    /// Whether the buffer stopped accepting events.
    closed: bool,
    /// Drops not yet reported by a warning.
    unreported_drops: PushOutcome,
    /// When the last overflow warning was logged.
    last_overflow_warning: Option<Instant>,
}

/// Shared state plus the consumer wake-up signal.
struct SharedBuffer {
    state: Mutex<BufferState>,
    event_available: Condvar,
    audio_capacity_ms: u64,
}

/// A cloneable handle; all clones share one queue.
#[derive(Clone)]
pub struct BridgeBuffer {
    shared: Arc<SharedBuffer>,
}

impl BridgeBuffer {
    /// Creates an empty buffer holding at most `audio_capacity` of audio.
    ///
    /// Inputs: The audio bound (`NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS`).
    /// Outputs: The buffer handle.
    /// Side effects: None.
    pub fn new(audio_capacity: Duration) -> Self {
        Self {
            shared: Arc::new(SharedBuffer {
                state: Mutex::new(BufferState::default()),
                event_available: Condvar::new(),
                audio_capacity_ms: u64::try_from(audio_capacity.as_millis()).unwrap_or(u64::MAX),
            }),
        }
    }

    /// Queues `event`, dropping the oldest audio or slides if a bound is exceeded.
    ///
    /// Inputs: A decoded event; camera frames are discarded, and pushes after `close` are ignored.
    /// Outputs: What had to be dropped.
    /// Side effects: Wakes the consumer; may log a rate-limited overflow warning (counts only,
    /// never media).
    pub fn push(&self, event: BridgeEvent) -> PushOutcome {
        let mut state = self.lock_state();
        if state.closed || matches!(event, BridgeEvent::IgnoredVideo) {
            return PushOutcome::default();
        }
        match &event {
            BridgeEvent::Audio(audio) => state.buffered_audio_ms += audio.duration_ms(),
            BridgeEvent::Slide(_) => state.buffered_slides += 1,
            BridgeEvent::MeetingStarted(_)
            | BridgeEvent::IgnoredVideo
            | BridgeEvent::Document(_)
            | BridgeEvent::MeetingEnded(_) => {}
        }
        state.queued_events.push_back(event);
        let outcome = PushOutcome {
            dropped_audio_chunks: drop_oldest_audio(&mut state, self.shared.audio_capacity_ms),
            dropped_slides: drop_oldest_slides(&mut state),
        };
        if outcome != PushOutcome::default() {
            self.record_overflow(&mut state, outcome);
        }
        drop(state);
        self.shared.event_available.notify_one();
        outcome
    }

    /// Takes the oldest event, waiting up to `timeout` for one to arrive.
    ///
    /// Inputs: The longest wait (`Duration::ZERO` polls).
    /// Outputs: The event, or `None` on timeout or once the buffer is closed and empty.
    /// Side effects: Blocks the calling thread; call it from the pump thread, never the runtime.
    pub fn next_event(&self, timeout: Duration) -> Option<BridgeEvent> {
        let deadline = Instant::now() + timeout;
        let mut state = self.lock_state();
        loop {
            if let Some(event) = state.queued_events.pop_front() {
                match &event {
                    BridgeEvent::Audio(audio) => {
                        state.buffered_audio_ms -= audio.duration_ms();
                    }
                    BridgeEvent::Slide(_) => state.buffered_slides -= 1,
                    BridgeEvent::MeetingStarted(_)
                    | BridgeEvent::IgnoredVideo
                    | BridgeEvent::Document(_)
                    | BridgeEvent::MeetingEnded(_) => {}
                }
                return Some(event);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if state.closed || remaining.is_zero() {
                return None;
            }
            state = self
                .shared
                .event_available
                .wait_timeout(state, remaining)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Returns the queued audio duration in milliseconds.
    pub fn buffered_audio_ms(&self) -> u64 {
        self.lock_state().buffered_audio_ms
    }

    /// Stops accepting events and wakes the consumer; queued events can still be drained.
    ///
    /// Side effects: Wakes every waiting consumer.
    pub fn close(&self) {
        self.lock_state().closed = true;
        self.shared.event_available.notify_all();
    }

    /// Returns whether `close` was called.
    pub fn is_closed(&self) -> bool {
        self.lock_state().closed
    }

    /// Locks the state, recovering from a poisoned mutex (the state stays consistent per push).
    fn lock_state(&self) -> MutexGuard<'_, BufferState> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Accumulates drops and logs a warning at most once per `OVERFLOW_WARNING_INTERVAL`.
    fn record_overflow(&self, state: &mut BufferState, outcome: PushOutcome) {
        state.unreported_drops.dropped_audio_chunks += outcome.dropped_audio_chunks;
        state.unreported_drops.dropped_slides += outcome.dropped_slides;
        let now = Instant::now();
        let warning_due = state.last_overflow_warning.is_none_or(|last_warning| {
            now.duration_since(last_warning) >= OVERFLOW_WARNING_INTERVAL
        });
        if warning_due {
            warn!(
                dropped_audio_chunks = state.unreported_drops.dropped_audio_chunks,
                dropped_slides = state.unreported_drops.dropped_slides,
                audio_capacity_ms = self.shared.audio_capacity_ms,
                "Teams bridge buffer overflowed; dropped the oldest media"
            );
            state.unreported_drops = PushOutcome::default();
            state.last_overflow_warning = Some(now);
        }
    }
}

/// Drops the oldest audio until within `audio_capacity_ms`, always keeping the newest chunk.
fn drop_oldest_audio(state: &mut BufferState, audio_capacity_ms: u64) -> usize {
    let mut dropped_chunks = 0;
    while state.buffered_audio_ms > audio_capacity_ms {
        let oldest_audio_position = state
            .queued_events
            .iter()
            .position(|event| matches!(event, BridgeEvent::Audio(_)));
        let newest_audio_position = state
            .queued_events
            .iter()
            .rposition(|event| matches!(event, BridgeEvent::Audio(_)));
        let Some(position) =
            oldest_audio_position.filter(|&oldest| Some(oldest) != newest_audio_position)
        else {
            break;
        };
        if let Some(BridgeEvent::Audio(audio)) = state.queued_events.remove(position) {
            state.buffered_audio_ms -= audio.duration_ms();
            dropped_chunks += 1;
        }
    }
    dropped_chunks
}

/// Drops the oldest slides until at most `MAXIMUM_BUFFERED_SLIDES` remain.
fn drop_oldest_slides(state: &mut BufferState) -> usize {
    let mut dropped_slides = 0;
    while state.buffered_slides > MAXIMUM_BUFFERED_SLIDES {
        let Some(position) = state
            .queued_events
            .iter()
            .position(|event| matches!(event, BridgeEvent::Slide(_)))
        else {
            break;
        };
        state.queued_events.remove(position);
        state.buffered_slides -= 1;
        dropped_slides += 1;
    }
    dropped_slides
}
