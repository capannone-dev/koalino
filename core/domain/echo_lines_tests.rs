//! Specifies which microphone lines are echoes of the computer's sound (#108). The cases are
//! ported from Yogurt's `app/test/echo.test.ts` (FrigadeHQ/yogurt, MIT License, Copyright (c)
//! 2026 Frigade), with Italian ones added.

use super::echo_lines::{EchoLine, echo_positions, token_overlap, tokens};

fn line(text: &str, start_ms: u64) -> EchoLine<'_> {
    EchoLine {
        start_ms,
        end_ms: start_ms + 2_000,
        text,
    }
}

fn spanning(text: &str, start_ms: u64, end_ms: u64) -> EchoLine<'_> {
    EchoLine {
        start_ms,
        end_ms,
        text,
    }
}

/// The positions dropped with echo heard and the microphone echoing everywhere.
fn dropped(mine: &[EchoLine<'_>], theirs: &[EchoLine<'_>]) -> Vec<usize> {
    echo_positions(mine, theirs, true, &|_| true)
}

#[test]
fn a_copy_a_moment_later_is_dropped_and_different_words_stay() {
    let theirs = [line("we should move the launch", 10_000)];
    assert_eq!(
        dropped(&[line("we should move the launch", 10_200)], &theirs),
        vec![0]
    );
    assert!(dropped(&[line("I can send the notes tonight", 10_200)], &theirs).is_empty());
    assert!(
        dropped(&[line("we should move the launch", 60_000)], &theirs).is_empty(),
        "far away in time"
    );
}

#[test]
fn case_punctuation_and_a_few_missing_words_do_not_matter() {
    let theirs = [line("Okay, we should move the launch to Friday.", 9_500)];
    assert_eq!(
        dropped(&[line("We should move... the launch!", 10_000)], &theirs),
        vec![0]
    );
}

#[test]
fn a_copy_must_begin_before_their_line_and_the_speakers_tail_end() {
    let theirs = [spanning("we should move the launch", 10_000, 12_000)];
    assert_eq!(
        dropped(
            &[spanning("we should move the launch", 12_400, 14_000)],
            &theirs
        ),
        vec![0]
    );
    assert!(
        dropped(
            &[spanning("we should move the launch", 12_600, 14_000)],
            &theirs
        )
        .is_empty()
    );
}

#[test]
fn short_replies_stay_and_scraps_and_fillers_go() {
    assert!(dropped(&[line("okay", 10_000)], &[line("okay", 10_000)]).is_empty());
    let theirs = [spanning(
        "the main responsibility is user interface and my role is to design a remote",
        28_000,
        32_000,
    )];
    assert_eq!(
        dropped(&[spanning("role is", 29_000, 29_600)], &theirs),
        vec![0]
    );
    assert!(dropped(&[spanning("role is", 40_000, 40_600)], &theirs).is_empty());
    assert!(dropped(&[spanning("okay sure", 29_000, 29_600)], &theirs).is_empty());
    let brief = [spanning("so that is kind of our brief", 9_000, 12_000)];
    assert_eq!(
        dropped(&[spanning("Um...", 10_000, 10_500)], &brief),
        vec![0]
    );
    assert!(dropped(&[spanning("Um...", 50_000, 50_500)], &brief).is_empty());
}

#[test]
fn a_line_spread_over_two_of_theirs_is_an_echo_but_a_few_shared_words_are_not() {
    let theirs = [
        spanning("okay so I think we will probably move the", 9_000, 11_000),
        spanning(
            "launch to next friday if that works for everyone",
            11_000,
            13_500,
        ),
    ];
    assert_eq!(
        dropped(
            &[spanning(
                "I think we should move the launch to next friday",
                10_100,
                12_600
            )],
            &theirs
        ),
        vec![0]
    );
    assert!(
        dropped(
            &[spanning(
                "I think we should wait for more data first",
                10_100,
                12_600
            )],
            &theirs
        )
        .is_empty()
    );
}

#[test]
fn nothing_is_dropped_without_echo_heard_or_where_the_microphone_did_not_echo() {
    let theirs = [line("we should move the launch", 10_000)];
    let mine = [line("we should move the launch", 10_200)];
    assert!(echo_positions(&mine, &theirs, false, &|_| true).is_empty());
    assert!(echo_positions(&mine, &theirs, true, &|_| false).is_empty());
}

#[test]
fn real_replies_stay_with_echo_heard() {
    let theirs_a = [spanning(
        "yeah I think we can ship it on Friday if design is right",
        10_000,
        16_000,
    )];
    let theirs_b = [
        spanning("so I think we should do that now", 9_000, 11_000),
        spanning("and yes I can send it so you have it", 11_200, 14_000),
    ];
    for reply in ["Yeah.", "Right.", "Okay."] {
        assert!(
            dropped(&[spanning(reply, 12_000, 12_500)], &theirs_a).is_empty(),
            "{reply}"
        );
    }
    for reply in [
        "Yes, I think so.",
        "I can do that.",
        "Sounds good, I can send it.",
    ] {
        assert!(
            dropped(&[spanning(reply, 11_000, 12_500)], &theirs_b).is_empty(),
            "{reply}"
        );
    }
    let both: Vec<EchoLine<'_>> = theirs_a.iter().chain(theirs_b.iter()).copied().collect();
    for copy in [
        "I think we can ship it on Friday",
        "ship it on Friday",
        "and yes I can send it so you have it",
        "we should do that now",
    ] {
        assert_eq!(
            dropped(&[spanning(copy, 11_000, 14_000)], &both),
            vec![0],
            "{copy}"
        );
    }
}

#[test]
fn italian_replies_stay_and_copies_go() {
    let theirs = [spanning(
        "allora direi di spostare il lancio a venerdì prossimo",
        10_000,
        14_000,
    )];
    assert!(dropped(&[spanning("Sì, va bene.", 12_000, 12_800)], &theirs).is_empty());
    assert_eq!(
        dropped(
            &[spanning(
                "direi di spostare il lancio a venerdì",
                10_300,
                13_500
            )],
            &theirs
        ),
        vec![0]
    );
}

#[test]
fn words_are_compared_without_case_or_punctuation_and_cjk_by_character() {
    assert_eq!(tokens("We, the PEOPLE!"), vec!["we", "the", "people"]);
    assert_eq!(tokens("明日の会議").len(), 5);
    assert!((token_overlap("a b c d", "a b c d e f") - 1.0).abs() < f32::EPSILON);
    assert!((token_overlap("a b c d", "a b x y") - 0.5).abs() < f32::EPSILON);
    assert!(token_overlap("", "a").abs() < f32::EPSILON);
    assert_eq!(
        dropped(
            &[line("明日の会議は十時からです", 5_000)],
            &[line("明日の会議は十時からです", 4_800)]
        ),
        vec![0]
    );
}
