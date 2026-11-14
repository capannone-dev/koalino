//! Specifies the thread that answers paired devices' chat questions (#107).

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use uuid::Uuid;

use super::paired_chat_host::{PAIRED_CHATS_PER_HOUR, ThreadedPairedChatHost};
use crate::{
    application::{
        p2p_service::{FinishedPairedChat, PairedChatHost},
        paired_chat_exchange::{PairedChatOutcome, PairedChatRequest},
    },
    domain::{chat::ChatScope, models::DeviceId},
};

fn request(number: u128) -> PairedChatRequest {
    PairedChatRequest {
        job_id: Uuid::from_u128(number),
        scope: ChatScope::All,
        question: "What is open?".to_owned(),
        history: Vec::new(),
    }
}

fn declined(code: &str) -> PairedChatOutcome {
    PairedChatOutcome::Declined {
        error_code: code.to_owned(),
    }
}

/// Polls until a finished answer arrives, or two seconds pass.
fn wait(host: &mut ThreadedPairedChatHost) -> Option<FinishedPairedChat> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(finished) = host.poll_finished() {
            return Some(finished);
        }
        std::thread::yield_now();
    }
    None
}

#[test]
fn a_computer_that_does_not_offer_chat_declines_at_once() {
    let mut host = ThreadedPairedChatHost::spawn(|| false, |_, _| declined("unused"))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        host.submit(DeviceId::new("phone"), request(1)),
        Err("not_offered".to_owned())
    );
}

#[test]
fn a_question_is_answered_on_the_worker() {
    let asked: Arc<Mutex<Vec<String>>> = Arc::default();
    let seen = Arc::clone(&asked);
    let mut host = ThreadedPairedChatHost::spawn(
        || true,
        move |peer: &DeviceId, request: PairedChatRequest| {
            if let Ok(mut seen) = seen.lock() {
                seen.push(format!("{}: {}", peer.as_str(), request.question));
            }
            declined("llm_unreachable")
        },
    )
    .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(host.submit(DeviceId::new("phone"), request(2)), Ok(()));
    let finished = wait(&mut host).unwrap_or_else(|| panic!("no answer"));

    assert_eq!(finished.peer, DeviceId::new("phone"));
    assert_eq!(finished.job_id, Uuid::from_u128(2));
    assert_eq!(finished.outcome, declined("llm_unreachable"));
    assert_eq!(
        asked.lock().map(|asked| asked.clone()).unwrap_or_default(),
        vec!["phone: What is open?".to_owned()]
    );
}

#[test]
fn questions_beyond_the_hourly_limit_are_declined() {
    let mut host = ThreadedPairedChatHost::spawn(|| true, |_, _| declined("llm_unreachable"))
        .unwrap_or_else(|error| panic!("{error}"));
    for number in 0..PAIRED_CHATS_PER_HOUR as u128 {
        assert_eq!(host.submit(DeviceId::new("phone"), request(number)), Ok(()));
        let _ = wait(&mut host);
    }
    assert_eq!(
        host.submit(DeviceId::new("phone"), request(999)),
        Err("rate_limited".to_owned())
    );
}
