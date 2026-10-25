<!-- Documents the local adapter that implements the portable audio-capture port. -->

# Local audio adapter

`local_recorder.rs` implements the `AudioCapture` port with a deterministic, in-memory stream seam. It owns a terminal state machine: `Idle → Recording → Paused → Recording`, with `Recording`, `Paused`, or `Interrupted` allowed to transition to `Stopped`. An OS interruption moves `Recording → Interrupted`, returns `AudioError::Interrupted`, emits a structured warning, and can resume through Start. Invalid commands return `AudioError::InvalidState`; a stopped recorder is not reusable.

Subscribers receive adapter-neutral signed 16-bit PCM `AudioChunk` values only while the recorder is in `Recording`. The adapter never logs raw samples. Platform integrations may feed their OS capture callbacks into this adapter boundary without leaking platform types into ports or application code. A later P2P streaming adapter can implement `AudioCapture` directly and expose the same chunk/state subscriptions.

The Flutter UI, rather than this Rust adapter, is responsible for permission prompts, foreground/background services, wake-locks, and lock-screen controls. Those UI facilities invoke the port lifecycle methods; they do not alter this state machine. OS integrations report suspended audio through `handle_interruption` and use Start to resume after wake.
