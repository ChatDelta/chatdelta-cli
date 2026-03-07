# ChatDelta Crate Wishlist & Implementation Status

*Last Updated: 2026-03-07 — synced against chatdelta-rs v0.7.0, CLI currently pinned to v0.6.0*

> **Note:** The CLI's `Cargo.toml` is currently pinned to `chatdelta = "0.6.0"`. Several features
> delivered in v0.4.0–v0.7.0 are available in the crate but **not yet wired up in the CLI**.
> See the "Available but Not Yet Integrated" section below.

---

## ✅ Delivered & Integrated (working in CLI)

### RetryStrategy — delivered v0.3.0
- `RetryStrategy::Exponential(Duration)`, `Linear(Duration)`, `Fixed(Duration)`
- CLI exposes `--retry-strategy` flag
- Working great in production use

### ChatSession (partial) — delivered v0.3.0
- `ChatSession::new(client)` and `session.send(message)` integrated
- CLI exposes `--conversation` mode for interactive chat
- Known limitation: client is consumed by value; can't reset session without full rebuild

---

## ✅ Delivered in Crate — Not Yet Wired Up in CLI

These features exist in `chatdelta-rs` but the CLI hasn't been updated to use them.
The CLI needs to bump its dependency and implement the integration.

### Streaming Responses — delivered v0.4.0 / v0.7.0
- **What we got:** BoxStream via `stream_prompt()` (v0.4.0) + channel-based `send_prompt_streaming(tx)` (v0.7.0)
- **CLI status:** ❌ Not integrated — users still wait with no feedback for long responses
- **What to do:** Wire up `send_prompt_streaming` with an `mpsc::unbounded_channel` and print chunks as they arrive. The channel-based API (v0.7.0) is the simpler path for CLI use.
- **Impact:** High — especially noticeable in debate mode where moderator reports are long

### System Prompt Support — delivered v0.4.0
- **What we got:** `ClientConfig::builder().system_message("You are...")`
- **CLI status:** ❌ `--system-prompt` flag exists in CLI but is not connected to anything
- **What to do:** Pass `--system-prompt` value into `ClientConfig::builder().system_message()`
- **Impact:** High — debate mode role framing currently relies on prepending text to turn content

### Structured Error Handling — delivered v0.4.1+
- **What we got:** `ClientError` enum with `Network`, `Api`, `Authentication`, `Configuration`, `Parse`, `Stream` variants; `ApiErrorType::RateLimit`, `ApiErrorType::QuotaExceeded`, etc.
- **CLI status:** ❌ CLI still treats all errors as strings
- **What to do:** Match on `ClientError` variants to give users actionable messages (e.g. "Rate limit hit — retry after Xs" vs "Invalid API key — check your env vars")
- **Impact:** Medium — significantly better DX when things go wrong

### `ModelCapabilities` — delivered v0.6.0
- **What we got:** `ModelCapabilities` struct with `supports_streaming`, `max_tokens`, etc. via the `orchestration` feature flag
- **CLI status:** ❌ Not integrated
- **What to do:** Use `client.get_capabilities()` to guard streaming paths and set sensible defaults per model
- **Impact:** Medium — prevents silent failures when streaming is attempted on non-supporting models

---

## 🚀 New Wishes for v0.8.0+

Based on current CLI needs and what v0.6.0 orchestration features unlock:

### 1. Response Metadata ⭐⭐⭐⭐⭐
- **Pain point:** Can't track token usage or cost across a debate session
- **Ideal API:**
  ```rust
  pub struct Response {
      pub content: String,
      pub tokens_used: Option<TokenUsage>,
      pub model: String,
      pub finish_reason: Option<String>,
  }

  async fn send_prompt_detailed(&self, prompt: &str) -> Result<Response>;
  ```
- **CLI use case:** `--show-usage` flag to display token counts and estimated cost per turn; especially useful for multi-round debates

### 2. Debate / Orchestration Integration ⭐⭐⭐⭐
- **What exists:** v0.6.0 ships `AiOrchestrator`, consensus strategies, response fusion, confidence scoring
- **Pain point:** The CLI's debate mode is hand-rolled on top of raw `send_prompt` calls; it doesn't use the orchestration layer at all
- **Ideal:** Expose a `DebateSession` or `ModeratedDebate` struct in the crate that the CLI can drive, so debate protocol logic lives in the library and is reusable by non-CLI consumers
- **CLI use case:** Cleaner debate implementation, configurable strategies per round

### 3. Custom API Endpoints ⭐⭐⭐
- **Use case:** Azure OpenAI, local Ollama instances, LiteLLM proxies
- **Still missing:** No way to pass a `base_url` override to `create_client`
- **Ideal:** `create_client("openai", key, model, config, Some("https://my-proxy/v1"))`

### 4. `ChatSession::clear()` and `get_history()` ⭐⭐⭐
- Requested since v0.3.0, still outstanding
- Without `clear()`, conversation mode can't offer a "fresh start" without rebuilding the client
- Without `get_history()`, the CLI can't serialize/save/resume sessions
  ```rust
  impl ChatSession {
      pub fn clear(&mut self);
      pub fn get_history(&self) -> &[Message];
      pub fn load_history(&mut self, messages: Vec<Message>);
  }
  ```

---

## 📊 Lower Priority

### Mock Client for Testing
- Still needed for CLI integration tests
- A `MockAiClient` in the crate that returns canned responses would allow testing debate logic without live API calls

### Request Cancellation
- Ability to abort in-flight requests (e.g. Ctrl+C during a streaming response)

### Progress Callbacks for Parallel Execution
- When running `execute_parallel` across 3 models, the CLI has no way to show per-model progress
- A callback or channel that fires per-model completion would allow a live progress display

---

## 🐛 Ongoing Issues

1. **ChatSession takes client by value** — still can't reset session without full rebuild (v0.3.0 issue, still present)
2. **`Conversation` vs `ChatSession`** — the crate now has both. The README examples use `Conversation` with `add_user`/`add_assistant`, but the CLI uses `ChatSession`. Clarify which is canonical for stateful multi-turn use.

---

## 🎯 Immediate Action Items for CLI

**Bump `chatdelta` dependency to `"0.7"` and:**
1. Wire up `send_prompt_streaming` for debate mode and parallel queries
2. Connect `--system-prompt` flag to `ClientConfig::builder().system_message()`
3. Replace string error handling with `ClientError` variant matching

These three changes are purely CLI work — the crate already supports all of them.

---

*This wishlist is maintained by the chatdelta-cli project as a communication channel with chatdelta-rs.*
*For the reverse channel (crate → CLI), see `WHATSNEW.md` in chatdelta-rs (proposed).*
