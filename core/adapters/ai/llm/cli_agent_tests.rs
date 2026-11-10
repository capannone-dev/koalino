//! Specifies the Claude Code and Codex engines (#105) against a fake runner: the command line each
//! tool gets (no tools, no saved session, a read-only sandbox, the prompt on standard input), the
//! environment that keeps API keys from overriding the subscription login, how answers and
//! failures are read, and that the private working folder is removed. No tool is run.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::json;

use super::cli_agent::{CliAgentEngine, CliInvocation, CliOutput, CliRunner, REMOVED_ENVIRONMENT};
use crate::{
    domain::{
        errors::LlmError,
        llm::{LlmConnection, LlmImage, LlmKind, LlmRequest, LlmRole, LlmTurn},
    },
    ports::ai::LlmEngine,
};

/// What the fake saw: the invocation, and the folder's files while the tool ran.
type Seen = (CliInvocation, Vec<String>);
/// How the fake answers a run.
type Answer = dyn Fn(&CliInvocation) -> Result<CliOutput, LlmError> + Send + Sync;

/// Answers like a tool would and keeps what it was asked.
struct FakeRunner {
    answer: Box<Answer>,
    seen: Mutex<Vec<Seen>>,
}

impl FakeRunner {
    fn new(
        answer: impl Fn(&CliInvocation) -> Result<CliOutput, LlmError> + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            answer: Box::new(answer),
            seen: Mutex::new(Vec::new()),
        })
    }

    fn only(&self) -> Seen {
        let seen = self
            .seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default();
        assert_eq!(seen.len(), 1, "one run");
        seen.into_iter().next().unwrap_or_else(|| panic!("one run"))
    }
}

impl CliRunner for FakeRunner {
    fn run(&self, invocation: &CliInvocation, _timeout: Duration) -> Result<CliOutput, LlmError> {
        let mut files: Vec<String> = std::fs::read_dir(&invocation.working_dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        if let Ok(mut seen) = self.seen.lock() {
            seen.push((invocation.clone(), files));
        }
        (self.answer)(invocation)
    }
}

fn ok(stdout: &str) -> Result<CliOutput, LlmError> {
    Ok(CliOutput {
        success: true,
        stdout: stdout.to_owned(),
        stderr: String::new(),
    })
}

fn connection(kind: LlmKind, model: &str) -> LlmConnection {
    LlmConnection::with_id("c1", "Pro", kind, "", model, 200_000)
        .unwrap_or_else(|error| panic!("valid connection: {error}"))
}

fn request() -> LlmRequest {
    LlmRequest {
        system: "Summarize the meeting \"Budget\".".to_owned(),
        turns: vec![LlmTurn {
            role: LlmRole::User,
            text: "Anna: we approve the budget.".to_owned(),
        }],
        images: LlmImage::new("image/png", vec![1, 2, 3])
            .into_iter()
            .collect(),
        max_output_tokens: 4_000,
    }
}

fn engine(kind: LlmKind, model: &str, runner: Arc<FakeRunner>) -> CliAgentEngine {
    CliAgentEngine::new(
        &connection(kind, model),
        Some(PathBuf::from("/opt/tools/the-cli")),
        runner,
    )
}

fn claude_result(result: &str) -> String {
    json!({"type": "result", "subtype": "success", "is_error": false, "result": result,
           "stop_reason": "end_turn"})
    .to_string()
}

#[test]
fn claude_code_runs_without_tools_or_a_saved_session_and_reads_the_prompt_from_stdin() {
    let runner = FakeRunner::new(|_| ok(&claude_result("## Overview\nBudget approved.")));
    let engine = engine(
        LlmKind::ClaudeCode,
        "claude-sonnet-5-5",
        Arc::clone(&runner),
    );

    let answer = engine.complete(&request());

    assert_eq!(
        answer.map(|answer| answer.text),
        Ok("## Overview\nBudget approved.".to_owned())
    );
    let (invocation, files) = runner.only();
    assert_eq!(invocation.program, PathBuf::from("/opt/tools/the-cli"));
    let args = invocation.args.join(" ");
    for expected in [
        "-p",
        "--output-format json",
        "--no-session-persistence",
        "--strict-mcp-config",
        "--model claude-sonnet-5-5",
    ] {
        assert!(args.contains(expected), "{expected} in {args}");
    }
    let tools = invocation.args.iter().position(|arg| arg == "--tools");
    assert_eq!(
        tools
            .and_then(|index| invocation.args.get(index + 1))
            .map(String::as_str),
        Some(""),
        "every tool is off, so meeting text can't make it act"
    );
    assert!(
        !args.contains("Budget") && !args.contains("Anna"),
        "meeting text stays off the command line, which other processes can read"
    );
    assert!(
        invocation
            .stdin
            .contains("Summarize the meeting \"Budget\".")
    );
    assert!(invocation.stdin.contains("Anna: we approve the budget."));
    assert!(files.is_empty(), "it runs in an empty folder: {files:?}");
    assert!(!engine.reads_images());
}

#[test]
fn api_keys_are_kept_from_the_tools_so_the_subscription_login_answers() {
    for name in [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
        "CLAUDECODE",
    ] {
        assert!(REMOVED_ENVIRONMENT.contains(&name), "{name}");
    }
    let runner = FakeRunner::new(|_| ok(&claude_result("Done.")));
    let _ = engine(LlmKind::ClaudeCode, "", Arc::clone(&runner)).complete(&request());
    let (invocation, _) = runner.only();
    assert_eq!(invocation.removed_env, REMOVED_ENVIRONMENT.to_vec());
    assert!(
        !invocation.args.iter().any(|arg| arg == "--model"),
        "no model leaves the choice to the tool"
    );
}

#[test]
fn claude_codes_own_failure_is_shown_and_a_cut_answer_is_marked() {
    let runner = FakeRunner::new(|_| {
        Ok(CliOutput {
            success: false,
            stdout: json!({"type": "result", "subtype": "success", "is_error": true,
                           "result": "Not logged in · Please run /login"})
            .to_string(),
            stderr: String::new(),
        })
    });
    assert_eq!(
        engine(LlmKind::ClaudeCode, "", runner).complete(&request()),
        Err(LlmError::ToolFailed {
            reason: Some("Not logged in · Please run /login".to_owned())
        })
    );

    let cut = FakeRunner::new(|_| {
        ok(
            &json!({"type": "result", "subtype": "success", "is_error": false,
                   "result": "Partial", "stop_reason": "max_tokens"})
            .to_string(),
        )
    });
    assert_eq!(
        engine(LlmKind::ClaudeCode, "", cut)
            .complete(&request())
            .map(|answer| answer.truncated),
        Ok(true)
    );

    let garbled = FakeRunner::new(|_| {
        Ok(CliOutput {
            success: false,
            stdout: String::new(),
            stderr: "boom\nerror: unknown option '--tools'\n".to_owned(),
        })
    });
    assert_eq!(
        engine(LlmKind::ClaudeCode, "", garbled).complete(&request()),
        Err(LlmError::ToolFailed {
            reason: Some("boom error: unknown option '--tools'".to_owned())
        })
    );
}

#[test]
fn codex_runs_read_only_and_ephemeral_with_images_and_its_answer_comes_from_a_file() {
    let runner = FakeRunner::new(|invocation| {
        let answer = output_file(&invocation.args).unwrap_or_else(|| panic!("an -o file"));
        std::fs::write(answer, "## Overview\nBudget approved.\n")
            .unwrap_or_else(|error| panic!("{error}"));
        Ok(CliOutput {
            success: true,
            stdout: "user\nAnna: we approve the budget.\ncodex\n## Overview".to_owned(),
            stderr: String::new(),
        })
    });
    let engine = engine(LlmKind::Codex, "gpt-5.5", Arc::clone(&runner));

    let answer = engine.complete(&request());

    assert_eq!(
        answer.map(|answer| answer.text),
        Ok("## Overview\nBudget approved.".to_owned())
    );
    let (invocation, files) = runner.only();
    assert_eq!(invocation.args.first().map(String::as_str), Some("exec"));
    let args = invocation.args.join(" ");
    for expected in [
        "--ephemeral",
        "--sandbox read-only",
        "--skip-git-repo-check",
        "--color never",
        "-m gpt-5.5",
    ] {
        assert!(args.contains(expected), "{expected} in {args}");
    }
    assert_eq!(
        invocation.args.last().map(String::as_str),
        Some("-"),
        "the prompt is read from standard input"
    );
    assert_eq!(files, vec!["image-1.png".to_owned()]);
    assert!(args.contains("--image="), "{args}");
    assert!(!args.contains("Budget"));
    assert!(invocation.stdin.contains("Anna: we approve the budget."));
    assert!(engine.reads_images());
}

#[test]
fn a_codex_failure_shows_its_last_words_and_an_empty_answer_is_unreadable() {
    let failed = FakeRunner::new(|_| {
        Ok(CliOutput {
            success: false,
            stdout: "user\nsecret meeting text\n".to_owned(),
            stderr: "\nERROR: You've hit your usage limit.\n".to_owned(),
        })
    });
    assert_eq!(
        engine(LlmKind::Codex, "", failed).complete(&request()),
        Err(LlmError::ToolFailed {
            reason: Some("ERROR: You've hit your usage limit.".to_owned())
        })
    );
    let silent = FakeRunner::new(|_| ok(""));
    assert_eq!(
        engine(LlmKind::Codex, "", silent).complete(&request()),
        Err(LlmError::InvalidResponse)
    );
}

#[test]
fn a_tool_that_is_not_installed_is_reported_before_anything_runs() {
    let runner = FakeRunner::new(|_| ok(""));
    let engine = CliAgentEngine::new(
        &connection(LlmKind::Codex, ""),
        None,
        Arc::clone(&runner) as Arc<dyn CliRunner>,
    );
    assert_eq!(
        engine.complete(&request()),
        Err(LlmError::ToolMissing { tool: "Codex" })
    );
    assert!(
        runner
            .seen
            .lock()
            .map(|seen| seen.is_empty())
            .unwrap_or(false)
    );
}

#[test]
fn the_private_folder_is_removed_after_each_run() {
    let folder: Arc<Mutex<Option<PathBuf>>> = Arc::default();
    let kept = Arc::clone(&folder);
    let runner = FakeRunner::new(move |invocation| {
        if let Ok(mut slot) = kept.lock() {
            *slot = Some(invocation.working_dir.clone());
        }
        Err(LlmError::ToolFailed { reason: None })
    });
    let _ = engine(LlmKind::Codex, "", runner).complete(&request());
    let folder = folder.lock().ok().and_then(|slot| slot.clone());
    let folder = folder.unwrap_or_else(|| panic!("the tool ran"));
    assert!(!folder.exists(), "{} is gone", folder.display());
    #[cfg(unix)]
    assert!(folder.starts_with(std::env::temp_dir()));
}

/// The path after Codex's `-o`.
fn output_file(args: &[String]) -> Option<&Path> {
    let index = args.iter().position(|arg| arg == "-o")?;
    args.get(index + 1).map(Path::new)
}

/// Runs the real tools installed on this computer with a made-up meeting. Ignored: it needs
/// Claude Code and Codex signed in, and uses their plans. Run it by hand with
/// `cargo test --lib real_tools -- --ignored --nocapture`.
#[test]
#[ignore = "runs the installed Claude Code and Codex"]
fn real_tools_summarize_a_made_up_meeting() {
    let made_up = LlmRequest {
        system: "Summarize this made-up meeting in at most three Markdown bullet points."
            .to_owned(),
        turns: vec![LlmTurn {
            role: LlmRole::User,
            text: "[00:00] Anna: Let's move the demo to Friday.\n\
                   [00:10] Marco: Fine, I'll book the room."
                .to_owned(),
        }],
        images: Vec::new(),
        max_output_tokens: 1_000,
    };
    for kind in [LlmKind::ClaudeCode, LlmKind::Codex] {
        let engine = CliAgentEngine::installed(&connection(kind, ""));
        let started = std::time::Instant::now();
        let answer = engine.complete(&made_up);
        println!("{kind:?} in {:?}: {answer:?}", started.elapsed());
        let text = answer.map(|answer| answer.text).unwrap_or_default();
        assert!(text.to_lowercase().contains("friday"), "{kind:?}: {text}");
    }
}
