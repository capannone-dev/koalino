//! Specifies how a question goes to each paired computer in turn until one answers (#107).

use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use uuid::Uuid;

use super::{
    paired_chat_exchange::{PairedChatAnswer, PairedChatOutcome, PairedChatResponse},
    paired_chat_relay::{
        ACCEPT_TIMEOUT, ANSWER_TIMEOUT, PairedChatJob, PairedChatRelay, PairedChatResult,
    },
};
use crate::domain::{chat::ChatScope, models::DeviceId};

fn device(name: &str) -> DeviceId {
    DeviceId::new(name)
}

fn job() -> (PairedChatJob, mpsc::Receiver<PairedChatResult>) {
    let (reply, answers) = mpsc::channel();
    (
        PairedChatJob {
            scope: ChatScope::All,
            question: "What is open?".to_owned(),
            history: Vec::new(),
            reply,
        },
        answers,
    )
}

fn answer() -> PairedChatAnswer {
    PairedChatAnswer {
        text: "Two items.".to_owned(),
        included: Vec::new(),
        omitted: Vec::new(),
        sources: Vec::new(),
        connection_label: "Claude Code on Laptop".to_owned(),
        truncated: false,
        images_sent: 0,
    }
}

fn response(job_id: Uuid, outcome: PairedChatOutcome) -> PairedChatResponse {
    PairedChatResponse { job_id, outcome }
}

fn declined(code: &str) -> PairedChatOutcome {
    PairedChatOutcome::Declined {
        error_code: code.to_owned(),
    }
}

#[test]
fn without_a_connected_computer_the_answer_is_no_paired_computer() {
    let mut relay = PairedChatRelay::default();
    let (job, answers) = job();
    assert!(relay.begin(job, Vec::new(), Instant::now()).is_none());
    assert_eq!(answers.try_recv(), Ok(Err("no_paired_computer".to_owned())));
    assert!(relay.is_idle());
}

#[test]
fn the_first_computer_that_answers_ends_the_question() {
    let mut relay = PairedChatRelay::default();
    let (job, answers) = job();
    let now = Instant::now();
    let Some((peer, request)) = relay.begin(job, vec![device("a"), device("b")], now) else {
        panic!("nothing was sent");
    };
    assert_eq!(peer, device("a"));
    assert_eq!(request.question, "What is open?");
    let accepted = response(request.job_id, PairedChatOutcome::Accepted);
    assert!(relay.on_response(&device("a"), accepted, now).is_none());
    let answered = response(request.job_id, PairedChatOutcome::Answered(answer()));
    assert!(relay.on_response(&device("a"), answered, now).is_none());
    assert_eq!(answers.try_recv(), Ok(Ok(answer())));
    assert!(relay.is_idle());
}

#[test]
fn a_computer_that_does_not_offer_it_passes_the_question_on() {
    let mut relay = PairedChatRelay::default();
    let (job, answers) = job();
    let now = Instant::now();
    let Some((_, request)) = relay.begin(job, vec![device("a"), device("b")], now) else {
        panic!("nothing was sent");
    };
    let next = relay.on_response(
        &device("a"),
        response(request.job_id, declined("not_offered")),
        now,
    );
    assert_eq!(next.map(|(peer, _)| peer), Some(device("b")));
    let last = relay.on_response(
        &device("b"),
        response(request.job_id, declined("not_offered")),
        now,
    );
    assert!(last.is_none());
    assert_eq!(answers.try_recv(), Ok(Err("no_paired_computer".to_owned())));
}

#[test]
fn the_most_telling_refusal_is_reported() {
    let mut relay = PairedChatRelay::default();
    let (job, answers) = job();
    let now = Instant::now();
    let Some((_, request)) = relay.begin(job, vec![device("a"), device("b")], now) else {
        panic!("nothing was sent");
    };
    relay.on_response(
        &device("a"),
        response(request.job_id, declined("llm_unreachable")),
        now,
    );
    relay.on_response(
        &device("b"),
        response(request.job_id, declined("not_offered")),
        now,
    );
    assert_eq!(answers.try_recv(), Ok(Err("llm_unreachable".to_owned())));
}

#[test]
fn a_computer_that_stays_silent_is_skipped_after_the_accept_timeout() {
    let mut relay = PairedChatRelay::default();
    let (job, answers) = job();
    let start = Instant::now();
    relay.begin(job, vec![device("a"), device("b")], start);
    assert!(relay.expire(start + ACCEPT_TIMEOUT / 2).is_empty());
    let resent = relay.expire(start + ACCEPT_TIMEOUT + Duration::from_millis(1));
    assert_eq!(
        resent
            .iter()
            .map(|(peer, _)| peer.clone())
            .collect::<Vec<_>>(),
        vec![device("b")]
    );
    assert!(answers.try_recv().is_err());
}

#[test]
fn an_accepted_question_waits_for_the_answer_timeout() {
    let mut relay = PairedChatRelay::default();
    let (job, answers) = job();
    let start = Instant::now();
    let Some((_, request)) = relay.begin(job, vec![device("a")], start) else {
        panic!("nothing was sent");
    };
    relay.on_response(
        &device("a"),
        response(request.job_id, PairedChatOutcome::Accepted),
        start,
    );
    assert!(relay.expire(start + ACCEPT_TIMEOUT * 2).is_empty());
    assert!(answers.try_recv().is_err());
    relay.expire(start + ANSWER_TIMEOUT + Duration::from_millis(1));
    assert_eq!(
        answers.try_recv(),
        Ok(Err("paired_computer_timeout".to_owned()))
    );
}

#[test]
fn a_response_from_another_device_is_ignored() {
    let mut relay = PairedChatRelay::default();
    let (job, answers) = job();
    let now = Instant::now();
    let Some((_, request)) = relay.begin(job, vec![device("a")], now) else {
        panic!("nothing was sent");
    };
    let forged = response(request.job_id, PairedChatOutcome::Answered(answer()));
    assert!(relay.on_response(&device("x"), forged, now).is_none());
    assert!(answers.try_recv().is_err());
    assert!(!relay.is_idle());
}

#[test]
fn a_computer_that_leaves_passes_the_question_on() {
    let mut relay = PairedChatRelay::default();
    let (job, answers) = job();
    let now = Instant::now();
    relay.begin(job, vec![device("a"), device("b")], now);
    let resent = relay.on_disconnected(&device("a"), now);
    assert_eq!(resent.len(), 1);
    assert!(relay.on_disconnected(&device("b"), now).is_empty());
    assert_eq!(
        answers.try_recv(),
        Ok(Err("paired_computer_left".to_owned()))
    );
}

#[test]
fn a_failed_send_moves_on() {
    let mut relay = PairedChatRelay::default();
    let (job, answers) = job();
    let now = Instant::now();
    let Some((_, request)) = relay.begin(job, vec![device("a"), device("b")], now) else {
        panic!("nothing was sent");
    };
    let next = relay.send_failed(request.job_id, now);
    assert_eq!(next.map(|(peer, _)| peer), Some(device("b")));
    assert!(relay.send_failed(request.job_id, now).is_none());
    assert_eq!(answers.try_recv(), Ok(Err("no_paired_computer".to_owned())));
}
