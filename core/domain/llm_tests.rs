//! Specifies LLM connections (#95): which endpoints are allowed, how a connection is checked, and
//! how the summary prompt template is validated and filled.

use super::llm::{
    DEFAULT_SUMMARY_TEMPLATE, LlmConnection, LlmConnectionError, LlmKind, PromptTemplate,
    PromptTemplateError, PromptValues, check_endpoint,
};

#[test]
fn https_is_always_allowed_and_plain_http_only_on_the_local_network() {
    for allowed in [
        "https://api.openai.com/v1",
        "https://api.mistral.ai/v1",
        "http://localhost:11434/v1",
        "http://127.0.0.1:8080/v1",
        "http://192.168.1.20:11434/v1",
        "http://10.0.0.5/v1",
        "http://172.20.1.1/v1",
        "http://[::1]:8000/v1",
        "http://[fd12:3456::1]/v1",
        "http://gpu-node.local:11434/v1",
        "http://gpu.lan/v1",
    ] {
        assert!(
            check_endpoint(allowed).is_ok(),
            "{allowed} should be allowed"
        );
    }
    for refused in [
        "http://api.openai.com/v1",
        "http://8.8.8.8/v1",
        "http://172.32.0.1/v1",
        "ftp://192.168.1.2/",
        "https://user:secret@example.com/v1",
        "not a url",
        "https://",
    ] {
        assert!(
            check_endpoint(refused).is_err(),
            "{refused} should be refused"
        );
    }
}

#[test]
fn an_endpoint_is_normalised_without_a_trailing_slash() {
    assert_eq!(
        check_endpoint(" https://api.mistral.ai/v1/ ")
            .ok()
            .as_deref(),
        Some("https://api.mistral.ai/v1")
    );
    assert_eq!(
        check_endpoint("http://localhost:11434/v1").map(|endpoint| endpoint.is_plain_http()),
        Ok(true)
    );
}

#[test]
fn a_connection_needs_a_label_a_model_and_a_sane_context_size() {
    let connection = LlmConnection::new(
        "Local Ollama",
        LlmKind::OpenAiCompatible,
        "http://192.168.1.20:11434/v1",
        "llama3.1:8b",
        8_192,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(connection.label(), "Local Ollama");
    assert_eq!(connection.base_url(), "http://192.168.1.20:11434/v1");
    assert!(!connection.id().is_empty());

    assert_eq!(
        LlmConnection::new(
            " ",
            LlmKind::OpenAiCompatible,
            "https://llm.example/v1",
            "m",
            8_192
        )
        .err(),
        Some(LlmConnectionError::MissingLabel)
    );
    assert_eq!(
        LlmConnection::new(
            "A",
            LlmKind::OpenAiCompatible,
            "https://llm.example/v1",
            "",
            8_192
        )
        .err(),
        Some(LlmConnectionError::MissingModel)
    );
    assert_eq!(
        LlmConnection::new(
            "A",
            LlmKind::OpenAiCompatible,
            "https://llm.example/v1",
            "m",
            100
        )
        .err(),
        Some(LlmConnectionError::ContextSize)
    );
    assert!(matches!(
        LlmConnection::new(
            "A",
            LlmKind::OpenAiCompatible,
            "http://8.8.8.8/v1",
            "m",
            8_192
        ),
        Err(LlmConnectionError::Endpoint(_))
    ));
}

#[test]
fn a_template_accepts_only_known_placeholders() {
    assert!(PromptTemplate::parse(DEFAULT_SUMMARY_TEMPLATE).is_ok());
    assert!(PromptTemplate::parse("Summarise {{title}} from {{date}} for {{project}}.").is_ok());
    assert_eq!(
        PromptTemplate::parse("Hello {{name}}").err(),
        Some(PromptTemplateError::UnknownPlaceholder("name".to_owned()))
    );
    assert_eq!(
        PromptTemplate::parse("Hello {{title").err(),
        Some(PromptTemplateError::Unclosed)
    );
    assert_eq!(
        PromptTemplate::parse("   ").err(),
        Some(PromptTemplateError::Empty)
    );
}

#[test]
fn a_template_is_filled_with_the_meeting_values() {
    let template = PromptTemplate::parse(
        "{{title}} on {{date}} at {{time}} ({{duration}}), {{device}}, project {{project}}, \
         speakers {{speakers}}, terms {{glossary}}, language {{language}}.",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let values = PromptValues {
        title: "Kick-off".to_owned(),
        date: "2026-11-08".to_owned(),
        time: "09:30".to_owned(),
        duration: "45 min".to_owned(),
        device: "laptop".to_owned(),
        project: None,
        speakers: vec!["Anna".to_owned(), "Marco".to_owned()],
        glossary: vec!["OKR".to_owned()],
        language: "it".to_owned(),
    };

    assert_eq!(
        template.render(&values),
        "Kick-off on 2026-11-08 at 09:30 (45 min), laptop, project none, speakers Anna, Marco, \
         terms OKR, language it."
    );
}

#[test]
fn a_connection_id_is_safe_for_a_file_name() {
    use super::llm::{LlmConnection, LlmConnectionError, LlmKind};
    for id in ["", "../key", "a b", "x".repeat(65).as_str()] {
        assert_eq!(
            LlmConnection::with_id(
                id,
                "L",
                LlmKind::OpenAiCompatible,
                "https://llm.example/v1",
                "m",
                8_192
            ),
            Err(LlmConnectionError::InvalidId),
            "{id:?}"
        );
    }
    let fresh = LlmConnection::new(
        "L",
        LlmKind::OpenAiCompatible,
        "https://llm.example/v1",
        "m",
        8_192,
    );
    assert!(fresh.is_ok(), "a generated ID passes");
}

#[test]
fn a_local_cli_connection_needs_no_address_and_may_leave_the_model_to_the_tool() {
    for kind in [LlmKind::ClaudeCode, LlmKind::Codex] {
        assert!(kind.is_local_cli());
        let connection =
            LlmConnection::new("Pro plan", kind, "https://ignored.example", "", 200_000)
                .unwrap_or_else(|error| panic!("{kind:?}: {error}"));
        assert_eq!(connection.base_url(), "", "no address is kept");
        assert_eq!(connection.model(), "", "the tool's own default model");
        assert!(!connection.is_plain_http());
        let revalidated = connection
            .clone()
            .revalidated()
            .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(revalidated, connection);
    }
    assert!(!LlmKind::OpenAiCompatible.is_local_cli());
    let named = LlmConnection::new(
        "Sonnet",
        LlmKind::ClaudeCode,
        "",
        "claude-sonnet-5-5",
        200_000,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(named.model(), "claude-sonnet-5-5");
}

#[test]
fn a_local_cli_model_name_cannot_pass_for_a_command_line_option() {
    for model in [
        "--dangerously-skip-permissions",
        "-c",
        "gpt 5",
        "a;b",
        "$(x)",
    ] {
        assert_eq!(
            LlmConnection::new("A", LlmKind::Codex, "", model, 200_000).err(),
            Some(LlmConnectionError::MissingModel),
            "{model}"
        );
    }
    for model in [
        "gpt-5.5",
        "claude-opus-5-5",
        "opus",
        "claude-sonnet-5-5[1m]",
        "o4:mini",
    ] {
        assert!(
            LlmConnection::new("A", LlmKind::ClaudeCode, "", model, 200_000).is_ok(),
            "{model}"
        );
    }
}

#[test]
fn local_cli_kinds_are_named_in_snake_case() {
    assert_eq!(
        serde_json::to_value(LlmKind::ClaudeCode).ok(),
        Some(serde_json::json!("claude_code"))
    );
    assert_eq!(
        serde_json::to_value(LlmKind::Codex).ok(),
        Some(serde_json::json!("codex"))
    );
}
