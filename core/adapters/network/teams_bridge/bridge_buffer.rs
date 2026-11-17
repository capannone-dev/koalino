//! Bounded buffer between the Teams WebSocket reader and the meeting pump.
//!
//! The reader must never block on transcription, so it pushes decoded events here and returns to
//! the socket immediately. Audio held in memory is bounded by duration
//! (`NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS`, summed across participants) and slides by count.
//! With an `AudioSpool` (#133), audio beyond the memory bound waits in the spool, a sealed file
//! per chunk, and comes back when the pump reaches it, so transcription slower than real time
//! falls behind instead of losing speech. Without a spool, or when the spool refuses, the oldest
//! audio in memory is dropped; slides over their cap are always dropped. Drops log a rate-limited
//! warning. Meeting starts, meeting ends and documents are never dropped, and events keep their
//! arrival order so audio always follows the metadata it belongs to.

use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use tracing::warn;
use uuid::Uuid;

use crate::adapters::network::teams_bridge::message_decoder::BridgeEvent;

/// Most slides held before the oldest is dropped.
pub const MAXIMUM_BUFFERED_SLIDES: usize = 16;
/// Minimum spacing between two overflow warnings.
const OVERFLOW_WARNING_INTERVAL: Duration = Duration::from_secs(5);

/// Names one chunk's audio waiting in an `AudioSpool`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpoolTicket(Uuid);

impl SpoolTicket {
    /// Creates a fresh ticket. Side effects: None.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// The ticket's ID, which a spool may use as a file name. Side effects: None.
    pub fn id(self) -> Uuid {
        self.0
    }
}

impl Default for SpoolTicket {
    fn default() -> Self {
        Self::new()
    }
}

/// Keeps chunk audio out of memory until the pump needs it (#133).
pub trait AudioSpool: Send + Sync {
    /// Stores one chunk's raw audio.
    ///
    /// Inputs: The audio; never log it. Outputs: Its ticket, or `None` when it can't be kept
    /// (the caller then drops audio as before). Side effects: Writes the audio, sealed.
    fn store(&self, audio_bytes: &[u8]) -> Option<SpoolTicket>;

    /// Takes a chunk's audio back, removing it from the spool.
    ///
    /// Outputs: The audio, or `None` when it was lost. Side effects: Deletes the stored copy.
    fn take(&self, ticket: &SpoolTicket) -> Option<Vec<u8>>;
}

/// What a push had to discard to stay within bounds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PushOutcome {
    /// Older audio chunks dropped by this push.
    pub dropped_audio_chunks: usize,
    /// Older slides dropped by this push.
    pub dropped_slides: usize,
}

/// One queued event; spooled audio carries its ticket and duration, its bytes being on disk.
struct QueuedEvent {
    event: BridgeEvent,
    spooled: Option<(SpoolTicket, u64)>,
}

impl QueuedEvent {
    /// Audio duration held in memory by this entry.
    fn memory_audio_ms(&self) -> u64 {
        match (&self.event, self.spooled) {
            (BridgeEvent::Audio(audio), None) => audio.duration_ms(),
            _ => 0,
        }
    }

    /// Audio duration of this entry, in memory or spooled.
    fn audio_ms(&self) -> u64 {
        self.spooled
            .map_or_else(|| self.memory_audio_ms(), |(_, duration_ms)| duration_ms)
    }
}

/// Mutable buffer state guarded by the mutex.
#[derive(Default)]
struct BufferState {
    /// Queued events in arrival order.
    queued_events: VecDeque<QueuedEvent>,
    /// Total audio duration currently queued, in memory or spooled.
    buffered_audio_ms: u64,
    /// Audio duration currently held in memory.
    memory_audio_ms: u64,
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
    /// Where audio beyond the memory bound waits; `None` drops it instead.
    spool: Option<Arc<dyn AudioSpool>>,
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
        Self::build(audio_capacity, None)
    }

    /// Creates an empty buffer that holds at most `audio_capacity` of audio in memory and spools
    /// the rest (#133).
    ///
    /// Inputs: The memory bound and the spool. Outputs: The buffer. Side effects: None.
    pub fn with_spool(audio_capacity: Duration, spool: Arc<dyn AudioSpool>) -> Self {
        Self::build(audio_capacity, Some(spool))
    }

    /// Creates the shared state. Side effects: None.
    fn build(audio_capacity: Duration, spool: Option<Arc<dyn AudioSpool>>) -> Self {
        Self {
            shared: Arc::new(SharedBuffer {
                state: Mutex::new(BufferState::default()),
                event_available: Condvar::new(),
                audio_capacity_ms: u64::try_from(audio_capacity.as_millis()).unwrap_or(u64::MAX),
                spool,
            }),
        }
    }

    /// Queues `event`, spooling audio beyond the memory bound, or dropping the oldest audio or
    /// slides when a bound is exceeded and nothing can hold them.
    ///
    /// Inputs: A decoded event; camera frames are discarded, and pushes after `close` are ignored.
    /// Outputs: What had to be dropped.
    /// Side effects: May write the chunk's audio to the spool (sealed); wakes the consumer; may log
    /// a rate-limited overflow warning (counts only, never media).
    pub fn push(&self, mut event: BridgeEvent) -> PushOutcome {
        let mut state = self.lock_state();
        if state.closed || matches!(event, BridgeEvent::IgnoredVideo) {
            return PushOutcome::default();
        }
        let mut spooled = None;
        match &mut event {
            BridgeEvent::Audio(audio) => {
                let duration_ms = audio.duration_ms();
                state.buffered_audio_ms += duration_ms;
                let memory_full =
                    state.memory_audio_ms + duration_ms > self.shared.audio_capacity_ms;
                // Audio already waiting on disk keeps later audio there too, so the memory
                // always holds the oldest chunks and order needs no bookkeeping.
                let spooling = memory_full
                    || state
                        .queued_events
                        .iter()
                        .any(|queued| queued.spooled.is_some());
                if let Some(spool) = self.shared.spool.as_ref().filter(|_| spooling) {
                    let audio_bytes = audio.take_audio();
                    match spool.store(&audio_bytes) {
                        Some(ticket) => spooled = Some((ticket, duration_ms)),
                        None => audio.restore_audio(audio_bytes),
                    }
                }
                if spooled.is_none() {
                    state.memory_audio_ms += duration_ms;
                }
            }
            BridgeEvent::Slide(_) => state.buffered_slides += 1,
            BridgeEvent::MeetingStarted(_)
            | BridgeEvent::IgnoredVideo
            | BridgeEvent::Document(_)
            | BridgeEvent::MeetingEnded(_) => {}
        }
        state
            .queued_events
            .push_back(QueuedEvent { event, spooled });
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
            if let Some(queued) = state.queued_events.pop_front() {
                state.buffered_audio_ms -= queued.audio_ms();
                state.memory_audio_ms -= queued.memory_audio_ms();
                if matches!(queued.event, BridgeEvent::Slide(_)) {
                    state.buffered_slides -= 1;
                }
                let Some((ticket, _)) = queued.spooled else {
                    return Some(queued.event);
                };
                // Read the spooled audio without holding the lock, so the reader isn't kept.
                drop(state);
                let mut event = queued.event;
                let restored = self
                    .shared
                    .spool
                    .as_ref()
                    .and_then(|spool| spool.take(&ticket));
                match (&mut event, restored) {
                    (BridgeEvent::Audio(audio), Some(audio_bytes)) => {
                        audio.restore_audio(audio_bytes);
                        return Some(event);
                    }
                    _ => {
                        warn!("Teams bridge lost a spooled audio chunk");
                        state = self.lock_state();
                        continue;
                    }
                }
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

/// Drops the oldest audio held in memory until it is within `audio_capacity_ms`, always keeping
/// the newest chunk. Spooled audio is never dropped.
fn drop_oldest_audio(state: &mut BufferState, audio_capacity_ms: u64) -> usize {
    let in_memory = |queued: &QueuedEvent| queued.memory_audio_ms() > 0;
    let mut dropped_chunks = 0;
    while state.memory_audio_ms > audio_capacity_ms {
        let oldest_audio_position = state.queued_events.iter().position(in_memory);
        let newest_audio_position = state.queued_events.iter().rposition(in_memory);
        let Some(position) =
            oldest_audio_position.filter(|&oldest| Some(oldest) != newest_audio_position)
        else {
            break;
        };
        if let Some(queued) = state.queued_events.remove(position) {
            state.memory_audio_ms -= queued.memory_audio_ms();
            state.buffered_audio_ms -= queued.audio_ms();
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
            .position(|queued| matches!(queued.event, BridgeEvent::Slide(_)))
        else {
            break;
        };
        state.queued_events.remove(position);
        state.buffered_slides -= 1;
        dropped_slides += 1;
    }
    dropped_slides
}
