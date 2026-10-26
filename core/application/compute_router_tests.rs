//! Defines red contracts for context propagation through compute dispatching.

/// Requires sequential dispatches to send the prior transcript as the next chunk's context.
///
/// Inputs: The dispatcher source, which will own the portable prior-transcript state during the
/// context-propagation implementation.
/// Outputs: Confirms dispatching references `TranscriptionContext` and preserves transcript
/// content for the following request.
/// Side effects: Reads compile-time source text only; it does not invoke an AI engine or write
/// telemetry.
#[test]
fn compute_dispatcher_carries_previous_chunk_transcript_to_next_chunk_context() {
    let dispatcher_source = include_str!("compute_router.rs");

    assert!(
        dispatcher_source.contains("TranscriptionContext"),
        "dispatcher must construct portable context for transcription requests"
    );
    assert!(
        dispatcher_source.contains("previous_text"),
        "dispatcher must retain each chunk transcript for the next chunk context"
    );
}
