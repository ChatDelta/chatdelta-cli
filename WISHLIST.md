# ChatDelta Crate Wishlist & Implementation Status

*Last Updated: After integrating chatdelta v0.3.0*

## 🎉 Successfully Implemented in v0.3.0

Thank you for implementing these features! They're working great in the CLI:

### ✅ RetryStrategy (Wishlist #6)
- **Status**: Fully implemented and integrated
- **What we got**: `RetryStrategy::Exponential(Duration)`, `Linear(Duration)`, `Fixed(Duration)`
- **CLI Integration**: Added `--retry-strategy` flag allowing users to choose strategy
- **Feedback**: Works perfectly! The Duration parameter for base delay is intuitive

### ✅ ChatSession (Wishlist #3) 
- **Status**: Partially implemented
- **What we got**: `ChatSession::new(client)` and `session.send(message)` 
- **CLI Integration**: Added `--conversation` mode for interactive chat
- **Current Limitations**: 
  - Can't extract client from session for resetting
  - No built-in serialization for saving/loading history
  - No apparent methods for accessing conversation history
- **Suggestions for v0.4.0**:
  ```rust
  impl ChatSession {
      pub fn clear(&mut self) // Reset history keeping same client
      pub fn get_history(&self) -> &[Message] // Access messages
      pub fn load_history(&mut self, messages: Vec<Message>)
      pub fn set_system_prompt(&mut self, prompt: &str)
  }
  ```

## 🚀 High Priority for Next Release (v0.4.0)

Based on real-world CLI usage, these would have the most impact:

### 1. Streaming Response Support ⭐⭐⭐⭐⭐
- **Current Pain Point**: Users wait with no feedback for long responses
- **Ideal API**:
  ```rust
  async fn send_prompt_streaming(&self, prompt: &str) -> Result<impl Stream<Item = Result<String>>>
  ```
- **CLI Use Case**: Show responses as they arrive, much better UX

### 2. Response Metadata ⭐⭐⭐⭐⭐
- **Current Pain Point**: Can't access token usage, model info, or finish reasons
- **Ideal API**:
  ```rust
  pub struct Response {
      pub content: String,
      pub tokens_used: Option<TokenUsage>,
      pub model: String,
      pub finish_reason: Option<String>,
  }
  ```
- **CLI Use Case**: Track costs, show usage stats, handle different finish reasons

### 3. System Prompt Support ⭐⭐⭐⭐
- **Current State**: No way to set system prompts
- **Ideal API**: 
  ```rust
  ClientConfig::builder().system_prompt("You are...")
  // OR
  client.set_system_prompt("You are...")
  ```
- **CLI Use Case**: Already have `--system-prompt` flag ready to use

## 📊 Medium Priority Features

### 4. Better Error Types ⭐⭐⭐
- **Current Issue**: String errors make it hard to handle specific cases
- **Ideal Solution**:
  ```rust
  pub enum ChatDeltaError {
      RateLimited { retry_after: Option<Duration> },
      InvalidApiKey,
      ModelNotFound(String),
      NetworkError(String),
      // etc.
  }
  ```

### 5. Model Capability Discovery ⭐⭐⭐
- **Use Case**: Dynamically adjust parameters based on model
- **Ideal API**:
  ```rust
  client.get_capabilities() -> ModelCapabilities {
      max_tokens: usize,
      supports_streaming: bool,
      supports_functions: bool,
      // etc.
  }
  ```

### 6. Custom API Endpoints ⭐⭐⭐
- **Use Case**: Azure OpenAI, local models, proxies
- **Current Workaround**: None
- **Ideal**: `create_client("openai", key, model, config, Some(base_url))`

## 💭 Lower Priority (Nice to Have)

### 7. Progress Callbacks
- For long operations, ability to show progress

### 8. Request Cancellation  
- Ability to abort in-flight requests

### 9. Mock Client for Testing
- Would help with CLI testing

## 🐛 Issues/Observations in v0.3.0

1. **ChatSession Constructor**: Takes client by value, making it impossible to reuse the client or reset the session without recreating everything

2. **Missing re-exports**: Would be helpful to re-export common types at crate root

3. **Documentation**: More examples would help, especially for ChatSession usage patterns

## 💡 Implementation Suggestions

### For Streaming (High Priority)
Consider using `tokio_stream` and async generators:
```rust
use tokio_stream::Stream;

pub trait AiClient {
    fn send_prompt_streaming(&self, prompt: &str) 
        -> impl Stream<Item = Result<String>>;
}
```

### For Response Metadata
A non-breaking approach could be:
```rust
// Keep existing method for compatibility
async fn send_prompt(&self, prompt: &str) -> Result<String>;

// Add new method that returns Response
async fn send_prompt_detailed(&self, prompt: &str) -> Result<Response>;
```

## 🎯 Summary for v0.4.0

**Top 3 Priorities** (would immediately improve CLI):
1. Streaming responses
2. Response metadata (especially token usage)
3. System prompt support

**Quick Wins**:
- Add `ChatSession::clear()` method
- Export `Message` type for conversation history
- Better error types enum

## 🙏 Thank You!

The v0.3.0 release with RetryStrategy and ChatSession has been fantastic! The retry strategies in particular have made the CLI much more robust. Looking forward to continued collaboration!

---
*Note: This wishlist is actively maintained by the chatdelta-cli project as a communication channel with the upstream crate.*