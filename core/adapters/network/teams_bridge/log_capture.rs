//! Test-only capture of `tracing` output, so tests can assert that warnings were logged.
//!
//! A single global subscriber is installed once (scoped thread-local subscribers race with
//! `tracing`'s global callsite-interest cache when tests run in parallel). It writes each event
//! into the capture buffer registered by the emitting thread, so a test only sees events logged
//! on its own thread (use a current-thread Tokio runtime for async tests).

use std::{
    cell::RefCell,
    io::Write,
    sync::{Arc, Mutex, Once, PoisonError},
};

use tracing_subscriber::fmt::MakeWriter;

/// Bytes captured for one test thread.
type CaptureTarget = Arc<Mutex<Vec<u8>>>;

thread_local! {
    /// The capture buffer of the current test thread, if it is capturing.
    static THREAD_CAPTURE_TARGET: RefCell<Option<CaptureTarget>> = const { RefCell::new(None) };
}

/// Installs the global subscriber exactly once.
static INSTALL_GLOBAL_SUBSCRIBER: Once = Once::new();

/// Logs captured by one test; capturing stops when this value drops.
pub(crate) struct CapturedLogs {
    captured_bytes: CaptureTarget,
}

impl CapturedLogs {
    /// Returns everything logged on this thread since capturing started.
    pub(crate) fn text(&self) -> String {
        let captured_bytes = self
            .captured_bytes
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&captured_bytes).into_owned()
    }
}

impl Drop for CapturedLogs {
    /// Unregisters the thread's capture buffer.
    fn drop(&mut self) {
        THREAD_CAPTURE_TARGET.with(|target| target.borrow_mut().take());
    }
}

/// Routes formatted events to the emitting thread's capture buffer.
struct ThreadRoutedWriterFactory;

/// Writes into one capture buffer, or discards when the thread is not capturing.
struct ThreadRoutedWriter {
    capture_target: Option<CaptureTarget>,
}

impl Write for ThreadRoutedWriter {
    /// Appends `bytes` to the capture buffer, if any; never fails.
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if let Some(capture_target) = &self.capture_target {
            capture_target
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }

    /// Nothing is buffered, so flushing is a no-op.
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for ThreadRoutedWriterFactory {
    type Writer = ThreadRoutedWriter;

    /// Hands out a writer for the calling thread's capture buffer.
    fn make_writer(&'writer self) -> Self::Writer {
        ThreadRoutedWriter {
            capture_target: THREAD_CAPTURE_TARGET.with(|target| target.borrow().clone()),
        }
    }
}

/// Starts capturing every `tracing` event (all levels, no ANSI) logged on the current thread.
///
/// Outputs: The captured logs; capturing stops when the value drops.
/// Side effects: Installs a global subscriber on first use (ignored if one already exists).
pub(crate) fn capture_logs() -> CapturedLogs {
    INSTALL_GLOBAL_SUBSCRIBER.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(ThreadRoutedWriterFactory)
            .with_ansi(false)
            .with_max_level(tracing::Level::TRACE)
            .finish();
        let _ignored_if_already_set = tracing::subscriber::set_global_default(subscriber);
    });
    let captured_bytes = CaptureTarget::default();
    THREAD_CAPTURE_TARGET.with(|target| *target.borrow_mut() = Some(Arc::clone(&captured_bytes)));
    CapturedLogs { captured_bytes }
}
