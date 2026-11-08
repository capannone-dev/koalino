# `core/adapters/ai/llm`

Language model adapters behind the `LlmEngine` port (`core/ports/ai.rs`), used for meeting
summaries (#95) and, later, chat (#97).

| File | Purpose |
| --- | --- |
| `mod.rs` | `engine_for(connection, key)` picks the adapter; HTTP status mapping; URL joining (`/v1` is added when the base URL has no path). |
| `openai_compatible.rs` | Chat Completions (`POST …/chat/completions`) for OpenAI, Mistral and local servers (Ollama, llama.cpp, LM Studio, vLLM). Images go as `data:` URLs on the first user turn. A leading `<think>` block from local reasoning models is dropped. No length limit is sent, since OpenAI's reasoning models refuse `max_tokens`. |
| `anthropic_messages.rs` | Messages API (`POST /v1/messages`, `anthropic-version: 2026-10-21`). Images go as base64 blocks before the text. On `api.anthropic.com`, Fable 5.1, Opus 5.5, Opus 5 and Sonnet 5.5 requests carry `fallbacks: "default"` (beta `server-side-fallback-2026-07-01`), so a declined request is retried on a fallback model. `stop_reason: "refusal"` becomes `LlmError::Refused`. |
| `transport.rs` | Blocking reqwest client: 10-minute answer timeout, 15-second connect timeout, no redirects (a key is never sent to another host). Error bodies are not read, since they can echo meeting content. |
| `base64.rs` | Standard base64 for inline images, so no new dependency is needed. |
| `llm_adapters_tests.rs` | Request and answer shapes against a recording transport, plus a real round trip with a local axum server. Nothing leaves the machine. |

The engines block, so callers run them on a background thread, never on the Flutter UI thread or
the async runtime. Keys are passed as `Zeroizing<String>` and never logged.
