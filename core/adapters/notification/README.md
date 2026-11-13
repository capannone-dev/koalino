# notification

System notifications the core shows itself. `call_toast.rs` (#111) shows the offer to record a call as a Windows toast (Record / Not now) and waits for the answer through the toast's in-process WinRT events; its XML and answers are portable and tested on every platform. It is exposed as `note_taker_call_toast` (`core/bin/ffi_call_watch.rs`).
