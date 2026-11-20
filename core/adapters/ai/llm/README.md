# `core/adapters/ai/llm`

Language model adapters behind the `LlmEngine` port (`core/ports/ai.rs`), used for meeting
summaries (#95) and, later, chat (#97).

| File | Purpose |
| --- | --- |
| `mod.rs` | `engine_for(connection)` picks the adapter; HTTP status mapping; `provider_reason` and `one_short_line` for failure reasons; URL joining (`/v1` is added when the base URL has no path). |
| `openai_compatible.rs` | Chat Completions (`POST …/chat/completions`) for servers on this device or the local network (Ollama, llama.cpp, LM Studio, vLLM) and online services (Gemini, DeepSeek, Qwen, Kimi, Mistral, OpenRouter, #138). A saved API key goes as `Authorization: Bearer`, but only over HTTPS or to this computer, never in clear across a network. Images go as `data:` URLs on the first user turn. A leading `<think>` block from local reasoning models is dropped. No length limit is sent, since some reasoning models refuse `max_tokens`. |
| `cli_agent.rs` | Claude Code and Codex installed and signed in on this computer (#105), so a Claude or ChatGPT plan answers instead of API credit. One headless run per request in a private temporary folder (mode 0700, removed afterwards). Claude Code: `-p --output-format json --tools "" --no-session-persistence --strict-mcp-config`, so it can't run commands or read files; it reads no images. Codex: `exec --ephemeral --sandbox read-only --skip-git-repo-check`, images via `--image=`, the answer read from its `-o` file. The prompt goes on standard input, never the command line; API key variables are removed from the tool's environment so the subscription login answers. `find_tool` also looks in the usual install folders, since a desktop app's `PATH` is short. A failure keeps one short line of what the tool said (`LlmError::ToolFailed`). |
| `cli_agent_tests.rs` | The command lines, environment, answers and failures against a fake runner, plus an ignored test that runs the real tools with a made-up meeting. |
| `transport.rs` | Blocking reqwest client: 10-minute answer timeout, 15-second connect timeout, no redirects (a key is never sent to another host). An error reply's body is read (up to 64 KiB) only so `provider_reason` can take its `error` type and message, one line of at most 300 characters, into `LlmError::Rejected` (#104); nothing else of it is kept or logged. |
| `base64.rs` | Standard base64 for inline images, so no new dependency is needed. |
| `llm_adapters_tests.rs` | Request and answer shapes against a recording transport, plus a real round trip with a local axum server. Nothing leaves the machine. |

The engines block, so callers run them on a background thread, never on the Flutter UI thread or
the async runtime. Keys are passed as `Zeroizing<String>` and never logged.
