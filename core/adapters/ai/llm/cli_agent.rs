//! Answers through Claude Code or Codex installed and signed in on this computer (#105), so a
//! Claude or ChatGPT subscription writes summaries and chat answers instead of API credit.
//!
//! Each request runs the tool once, headlessly, in a new private folder that is removed
//! afterwards:
//!
//! - **Claude Code:** `claude -p --output-format json --tools "" --no-session-persistence
//!   --strict-mcp-config [--model M]`. With every tool off, meeting text can't make it run
//!   commands or read files; it only writes an answer.
//! - **Codex:** `codex exec --ephemeral --sandbox read-only --skip-git-repo-check --color never
//!   -C <folder> -o <folder>/answer.md [-m M] [--image=<file>…] -`. The read-only sandbox keeps
//!   it from changing anything; the answer is read from the `-o` file, since standard output
//!   echoes the prompt.
//!
//! The prompt (instructions, transcript, notes) goes on standard input, never on the command
//! line, which other processes can read. API key variables are removed from the tool's
//! environment, so the subscription login answers rather than a key that bills API credit.
//! Nothing the tool prints is logged; a failure keeps one short line for the owner.

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;

use super::one_short_line;
use crate::{
    adapters::child_process::reaped_elsewhere,
    domain::{
        errors::LlmError,
        llm::{LlmCompletion, LlmConnection, LlmKind, LlmRequest, LlmRole},
    },
    ports::ai::LlmEngine,
};

/// Environment variables removed before a tool runs: API keys would bill API credit instead of
/// the signed-in subscription, and `CLAUDECODE` marks a nested Claude Code session.
pub const REMOVED_ENVIRONMENT: [&str; 5] = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "OPENAI_API_KEY",
    "CODEX_API_KEY",
    "CLAUDECODE",
];

/// How long one answer may take; a long meeting on a busy plan can be slow.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// How often a running tool is checked for having finished.
const POLL_INTERVAL: Duration = Duration::from_millis(200);
/// The system prompt given to Claude Code; the owner's instructions come on standard input.
const CLAUDE_SYSTEM_PROMPT: &str = "You write what the owner asks for about their own meetings \
and notes. Follow the instructions in the input exactly. Answer with the requested text only, \
in Markdown. Do not ask questions and do not mention tools.";
/// Codex's answer file inside the private folder.
const CODEX_ANSWER_FILE: &str = "answer.md";

/// One run of a tool.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CliInvocation {
    /// The program to start.
    pub program: PathBuf,
    /// Its arguments; never meeting content.
    pub args: Vec<String>,
    /// What is written to its standard input: the whole prompt.
    pub stdin: String,
    /// The private folder it runs in.
    pub working_dir: PathBuf,
    /// Environment variables removed before it starts.
    pub removed_env: Vec<&'static str>,
}

/// What a tool printed and whether it exited successfully.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CliOutput {
    /// Whether the exit status was success.
    pub success: bool,
    /// Standard output.
    pub stdout: String,
    /// Standard error.
    pub stderr: String,
}

/// Runs a tool, so tests can answer without one.
pub trait CliRunner: Send + Sync {
    /// Starts the tool, feeds its input and waits for it.
    ///
    /// Inputs: The invocation and the longest wait. Outputs: What it printed, or `ToolFailed`
    /// when it could not start or ran too long. Side effects: Starts a process.
    fn run(&self, invocation: &CliInvocation, timeout: Duration) -> Result<CliOutput, LlmError>;
}

/// Runs tools with `std::process`.
pub struct ProcessRunner;

impl CliRunner for ProcessRunner {
    fn run(&self, invocation: &CliInvocation, timeout: Duration) -> Result<CliOutput, LlmError> {
        let mut command = command_for(&invocation.program);
        command
            .args(&invocation.args)
            .current_dir(&invocation.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for name in &invocation.removed_env {
            command.env_remove(name);
        }
        hide_console_window(&mut command);
        let mut child = command.spawn().map_err(|_| LlmError::ToolFailed {
            reason: Some("the tool could not be started".to_owned()),
        })?;
        // Feed and drain on their own threads, so a full pipe never blocks the tool.
        let stdin_text = invocation.stdin.clone();
        let feeder = child.stdin.take().map(|mut stdin| {
            thread::spawn(move || {
                let _ = stdin.write_all(stdin_text.as_bytes());
            })
        });
        let stdout_reader = child.stdout.take().map(read_all);
        let stderr_reader = child.stderr.take().map(read_all);
        let started = Instant::now();
        // `None` once the Flutter engine collected the status first (`child_process`): the tool
        // has ended, and its answer is judged by its output alone.
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) if started.elapsed() < timeout => thread::sleep(POLL_INTERVAL),
                Err(error) if reaped_elsewhere(&error) => break None,
                Ok(None) | Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(LlmError::ToolFailed {
                        reason: Some(format!(
                            "the tool gave no answer within {} minutes",
                            timeout.as_secs() / 60
                        )),
                    });
                }
            }
        };
        if let Some(feeder) = feeder {
            let _ = feeder.join();
        }
        let joined = |reader: Option<thread::JoinHandle<String>>| {
            reader
                .and_then(|reader| reader.join().ok())
                .unwrap_or_default()
        };
        let stdout = joined(stdout_reader);
        Ok(CliOutput {
            success: status.map_or(!stdout.trim().is_empty(), |status| status.success()),
            stdout,
            stderr: joined(stderr_reader),
        })
    }
}

/// Reads a pipe to its end on a new thread.
fn read_all(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// Builds the command; Windows runs npm's `.cmd` shims through `cmd.exe`.
fn command_for(program: &Path) -> Command {
    let is_batch = program
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("cmd"));
    if cfg!(windows) && is_batch {
        let mut command = Command::new("cmd.exe");
        command.arg("/D").arg("/C").arg(program);
        command
    } else {
        Command::new(program)
    }
}

/// Keeps a console window from flashing up on Windows.
#[cfg(windows)]
fn hide_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    /// `CREATE_NO_WINDOW`.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

/// Nothing to hide elsewhere.
#[cfg(not(windows))]
fn hide_console_window(_command: &mut Command) {}

/// A private folder removed when dropped.
struct PrivateFolder(PathBuf);

impl PrivateFolder {
    /// Creates `<temp>/note-taker-<tool>-<random>`, readable only by this user on Unix.
    ///
    /// Outputs: The folder, or `ToolFailed`. Side effects: Creates a directory.
    fn create(tool: &str) -> Result<Self, LlmError> {
        let path = std::env::temp_dir().join(format!(
            "note-taker-{tool}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        #[cfg_attr(not(unix), allow(unused_mut))] // Only Unix sets the folder's mode.
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&path)
            .map(|()| Self(path))
            .map_err(|_| LlmError::ToolFailed {
                reason: Some("a private working folder could not be created".to_owned()),
            })
    }
}

impl Drop for PrivateFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// An engine for one Claude Code or Codex connection.
pub struct CliAgentEngine {
    kind: LlmKind,
    model: Option<String>,
    program: Option<PathBuf>,
    runner: Arc<dyn CliRunner>,
}

impl CliAgentEngine {
    /// Creates the engine.
    ///
    /// Inputs: The connection (Claude Code or Codex), the tool's program if it was found, and
    /// the runner. Outputs: The engine. Side effects: None.
    pub fn new(
        connection: &LlmConnection,
        program: Option<PathBuf>,
        runner: Arc<dyn CliRunner>,
    ) -> Self {
        Self {
            kind: connection.kind(),
            model: Some(connection.model().to_owned()).filter(|model| !model.is_empty()),
            program,
            runner,
        }
    }

    /// Creates the engine for the tool installed on this computer, run as a process.
    ///
    /// Inputs: The connection. Outputs: The engine; a missing tool is reported when it is used.
    /// Side effects: Searches `PATH` and the usual install folders.
    pub fn installed(connection: &LlmConnection) -> Self {
        let program = find_tool(connection.kind());
        Self::new(connection, program, Arc::new(ProcessRunner))
    }

    /// The tool's name for messages.
    fn tool_name(&self) -> &'static str {
        match self.kind {
            LlmKind::Codex => "Codex",
            _ => "Claude Code",
        }
    }

    /// Runs Claude Code and reads its JSON result.
    fn run_claude(
        &self,
        program: PathBuf,
        prompt: String,
        folder: &Path,
    ) -> Result<LlmCompletion, LlmError> {
        let mut args: Vec<String> = [
            "-p",
            "--output-format",
            "json",
            "--tools",
            "",
            "--no-session-persistence",
            "--strict-mcp-config",
            "--system-prompt",
            CLAUDE_SYSTEM_PROMPT,
        ]
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect();
        if let Some(model) = &self.model {
            args.extend(["--model".to_owned(), model.clone()]);
        }
        let output = self.runner.run(
            &CliInvocation {
                program,
                args,
                stdin: prompt,
                working_dir: folder.to_path_buf(),
                removed_env: REMOVED_ENVIRONMENT.to_vec(),
            },
            ANSWER_TIMEOUT,
        )?;
        let Ok(result) = serde_json::from_str::<Value>(output.stdout.trim()) else {
            return Err(failure(&output));
        };
        let text = result
            .get("result")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if result.get("is_error").and_then(Value::as_bool) == Some(true) || !output.success {
            return Err(LlmError::ToolFailed {
                reason: one_short_line(text).or_else(|| last_words(&output.stderr)),
            });
        }
        if text.trim().is_empty() {
            return Err(LlmError::InvalidResponse);
        }
        Ok(LlmCompletion {
            text: text.trim().to_owned(),
            truncated: result.get("stop_reason").and_then(Value::as_str) == Some("max_tokens"),
        })
    }

    /// Runs Codex and reads its answer file.
    fn run_codex(
        &self,
        program: PathBuf,
        prompt: String,
        request: &LlmRequest,
        folder: &Path,
    ) -> Result<LlmCompletion, LlmError> {
        let answer_file = folder.join(CODEX_ANSWER_FILE);
        let mut args: Vec<String> = vec![
            "exec".to_owned(),
            "--ephemeral".to_owned(),
            "--sandbox".to_owned(),
            "read-only".to_owned(),
            "--skip-git-repo-check".to_owned(),
            "--color".to_owned(),
            "never".to_owned(),
            "-C".to_owned(),
            folder.to_string_lossy().into_owned(),
            "-o".to_owned(),
            answer_file.to_string_lossy().into_owned(),
        ];
        if let Some(model) = &self.model {
            args.extend(["-m".to_owned(), model.clone()]);
        }
        for (index, image) in request.images.iter().enumerate() {
            let extension = image
                .media_type()
                .rsplit('/')
                .next()
                .unwrap_or("png")
                .replace("jpeg", "jpg");
            let path = folder.join(format!("image-{}.{extension}", index + 1));
            std::fs::write(&path, image.bytes()).map_err(|_| LlmError::ToolFailed {
                reason: Some("a picture could not be handed to Codex".to_owned()),
            })?;
            // `--image=` keeps the final `-` from being read as another image.
            args.push(format!("--image={}", path.to_string_lossy()));
        }
        args.push("-".to_owned());
        let output = self.runner.run(
            &CliInvocation {
                program,
                args,
                stdin: prompt,
                working_dir: folder.to_path_buf(),
                removed_env: REMOVED_ENVIRONMENT.to_vec(),
            },
            ANSWER_TIMEOUT,
        )?;
        if !output.success {
            return Err(failure(&output));
        }
        let text = std::fs::read_to_string(&answer_file).unwrap_or_default();
        if text.trim().is_empty() {
            return Err(LlmError::InvalidResponse);
        }
        Ok(LlmCompletion {
            text: text.trim().to_owned(),
            truncated: false,
        })
    }
}

impl LlmEngine for CliAgentEngine {
    fn complete(&self, request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        if request.turns.is_empty() {
            return Err(LlmError::InvalidRequest {
                reason: "the conversation is empty",
            });
        }
        let program = self.program.clone().ok_or(LlmError::ToolMissing {
            tool: self.tool_name(),
        })?;
        let folder = PrivateFolder::create(match self.kind {
            LlmKind::Codex => "codex",
            _ => "claude",
        })?;
        let prompt = prompt_text(request);
        match self.kind {
            LlmKind::Codex => self.run_codex(program, prompt, request, &folder.0),
            _ => self.run_claude(program, prompt, &folder.0),
        }
    }

    fn reads_images(&self) -> bool {
        self.kind == LlmKind::Codex
    }
}

/// Writes the whole request as one text: the instructions, then the conversation.
///
/// Inputs: The request. Outputs: The text for standard input. Side effects: None.
fn prompt_text(request: &LlmRequest) -> String {
    let mut text = String::new();
    if !request.system.trim().is_empty() {
        text.push_str("<instructions>\n");
        text.push_str(request.system.trim());
        text.push_str("\n</instructions>\n\n");
    }
    if let [only] = request.turns.as_slice() {
        text.push_str(&only.text);
        return text;
    }
    text.push_str("The conversation so far; answer the owner's last message.\n");
    for turn in &request.turns {
        let speaker = match turn.role {
            LlmRole::User => "Owner",
            LlmRole::Assistant => "You",
        };
        text.push_str(&format!("\n## {speaker}\n{}\n", turn.text));
    }
    text
}

/// Words a failed run from its last non-empty error lines, or its output's.
fn failure(output: &CliOutput) -> LlmError {
    LlmError::ToolFailed {
        reason: last_words(&output.stderr).or_else(|| last_words(&output.stdout)),
    }
}

/// The last two non-empty lines of a tool's output, as one short line.
fn last_words(text: &str) -> Option<String> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let start = lines.len().saturating_sub(2);
    one_short_line(&lines[start..].join(" "))
}

/// Finds a tool on `PATH` or in the folders its installers use, since an app started from the
/// desktop often has a shorter `PATH` than a terminal.
///
/// Inputs: The kind. Outputs: The program, or `None`. Side effects: Reads the file system.
pub fn find_tool(kind: LlmKind) -> Option<PathBuf> {
    let base = match kind {
        LlmKind::ClaudeCode => "claude",
        LlmKind::Codex => "codex",
        LlmKind::OpenAiCompatible | LlmKind::Anthropic => return None,
    };
    let names: Vec<String> = if cfg!(windows) {
        ["exe", "cmd"]
            .iter()
            .map(|extension| format!("{base}.{extension}"))
            .collect()
    } else {
        vec![base.to_owned()]
    };
    let home =
        std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let mut folders: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    if let Some(home) = &home {
        for relative in [
            ".local/bin",
            ".claude/local",
            ".npm-global/bin",
            ".bun/bin",
            ".volta/bin",
            "bin",
        ] {
            folders.push(home.join(relative));
        }
    }
    if let Some(app_data) = std::env::var_os("APPDATA") {
        folders.push(PathBuf::from(app_data).join("npm"));
    }
    folders.extend(
        ["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin"]
            .iter()
            .map(PathBuf::from),
    );
    folders
        .iter()
        .flat_map(|folder| names.iter().map(move |name| folder.join(name)))
        .find(|candidate| candidate.is_file())
}
