//! Specifies the owner's prompts shared by paired devices (#145): the chat prompt's checks and
//! filling, and the rule that makes every device keep the newest edit of each prompt whatever
//! order the devices hear of it in.

use super::{
    llm::PromptTemplateError,
    shared_prompts::{ChatPrompt, DEFAULT_CHAT_PROMPT, PromptVersion, SharedPrompts},
};

fn version(text: Option<&str>, changed_at_ms: u64) -> PromptVersion {
    PromptVersion {
        text: text.map(str::to_owned),
        changed_at_ms,
    }
}

#[test]
fn the_chat_prompt_fills_in_what_is_asked_about() {
    let prompt = ChatPrompt::parse("Answer about {{scope}} briefly.")
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        prompt.render("this meeting"),
        "Answer about this meeting briefly."
    );
    assert!(
        ChatPrompt::default()
            .render("all notes")
            .contains("all notes")
    );
    assert!(DEFAULT_CHAT_PROMPT.contains("{{scope}}"));
}

#[test]
fn a_chat_prompt_is_checked() {
    assert_eq!(ChatPrompt::parse("  "), Err(PromptTemplateError::Empty));
    assert_eq!(
        ChatPrompt::parse("About {{title}}"),
        Err(PromptTemplateError::UnknownPlaceholder("title".to_owned()))
    );
    assert_eq!(
        ChatPrompt::parse("About {{scope"),
        Err(PromptTemplateError::Unclosed)
    );
    assert_eq!(
        ChatPrompt::parse(&"x".repeat(8_001)),
        Err(PromptTemplateError::TooLong)
    );
}

#[test]
fn the_newest_edit_of_each_prompt_wins() {
    let mut laptop = SharedPrompts {
        summary: version(Some("Laptop summary"), 200),
        chat: version(Some("Laptop chat about {{scope}}"), 100),
    };
    let phone = SharedPrompts {
        summary: version(Some("Phone summary"), 150),
        chat: version(None, 300),
    };

    assert!(laptop.merge(&phone));
    assert_eq!(laptop.summary, version(Some("Laptop summary"), 200));
    // Going back to the built-in prompt is an edit too.
    assert_eq!(laptop.chat, version(None, 300));
    // Hearing the same again changes nothing, so devices do not echo each other.
    assert!(!laptop.merge(&phone));
}

#[test]
fn two_devices_end_with_the_same_prompts_in_either_order() {
    let first = SharedPrompts {
        summary: version(Some("One"), 500),
        chat: version(Some("A {{scope}}"), 0),
    };
    let second = SharedPrompts {
        summary: version(Some("Two"), 500),
        chat: version(None, 0),
    };
    let mut one_way = first.clone();
    one_way.merge(&second);
    let mut other_way = second.clone();
    other_way.merge(&first);
    assert_eq!(one_way, other_way);
}
