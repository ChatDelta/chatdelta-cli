# ChatDelta Crate Wishlist

This document contains feature requests and improvements for the `chatdelta` crate based on experience building the CLI tool.

## High Priority Features

### 1. Streaming Response Support
- **Need**: Ability to stream responses as they arrive rather than waiting for complete responses
- **API**: Add `send_prompt_streaming()` method that returns a Stream/AsyncIterator
- **Benefit**: Better user experience for long responses, ability to show progress

### 2. Token Usage Reporting
- **Need**: Consistent token usage reporting across all AI providers
- **API**: Include `tokens_used` field in responses or as metadata
- **Benefit**: Cost tracking, usage optimization, better metrics

### 3. Conversation/Chat Support
- **Need**: Support for multi-turn conversations with message history
- **API**: Add `ChatSession` struct with `add_message()` and `send_messages()` methods
- **Benefit**: Enable chatbot-style interactions, maintain context across queries

### 4. System Prompt Support
- **Need**: Ability to set system prompts for models that support them
- **API**: Add `system_prompt` field to `ClientConfig` or separate method
- **Benefit**: Better control over model behavior and response style

## Medium Priority Features

### 5. Custom API Endpoints
- **Need**: Support for alternative API endpoints (Azure OpenAI, local models, proxies)
- **API**: Add `base_url` parameter to `create_client()` or `ClientConfig`
- **Benefit**: Enterprise deployments, privacy-conscious users, development/testing

### 6. Retry Strategy Customization
- **Need**: More control over retry behavior (exponential backoff, jitter, specific error handling)
- **API**: Add `RetryStrategy` enum with options like `Exponential`, `Linear`, `Fixed`
- **Benefit**: Better handling of rate limits and transient failures

### 7. Response Metadata
- **Need**: Access to response metadata (model version used, finish reason, safety ratings)
- **API**: Return `Response` struct with `content` and `metadata` fields instead of just String
- **Benefit**: Better debugging, safety monitoring, understanding model behavior

### 8. Model Capability Discovery
- **Need**: Programmatic way to discover model capabilities (max tokens, supports streaming, etc.)
- **API**: Add `get_model_info()` or `capabilities()` method to `AiClient`
- **Benefit**: Dynamic UI generation, automatic fallback selection

## Lower Priority Features

### 9. Request Cancellation
- **Need**: Ability to cancel in-flight requests
- **API**: Return `CancellableRequest` with `abort()` method
- **Benefit**: Better resource management, user control

### 10. Batch Processing
- **Need**: Efficient processing of multiple prompts
- **API**: Add `send_batch()` method that optimizes multiple requests
- **Benefit**: Better performance for bulk operations

### 11. Response Caching
- **Need**: Optional caching of responses for identical prompts
- **API**: Add `enable_cache()` to `ClientConfig` with TTL settings
- **Benefit**: Cost savings, faster responses for repeated queries

### 12. Fine-tuned Model Support
- **Need**: Support for custom/fine-tuned models
- **API**: Accept full model IDs/paths in `create_client()`
- **Benefit**: Support specialized use cases

## API Improvements

### 13. Better Error Types
- **Need**: More specific error types for different failure modes
- **Current**: Generic error strings
- **Proposed**: Enum with variants like `RateLimited`, `InvalidApiKey`, `ModelNotFound`, `NetworkError`
- **Benefit**: Better error handling and recovery strategies

### 14. Async Trait Methods
- **Need**: All trait methods should be properly async
- **Benefit**: Better performance, no blocking operations

### 15. Builder Pattern Everywhere
- **Need**: Consistent use of builder pattern for complex configurations
- **Example**: `SummaryRequest::builder().format("bullets").max_length(500).build()`
- **Benefit**: Better API ergonomics, easier to extend

## Quality of Life

### 16. Timeout Granularity
- **Need**: Separate timeouts for connection vs. response generation
- **API**: `connection_timeout` and `response_timeout` in `ClientConfig`
- **Benefit**: Better handling of slow model responses vs. network issues

### 17. Progress Callbacks
- **Need**: Callbacks for long-running operations
- **API**: `on_progress` callback in config or request
- **Benefit**: Better UX for CLI and GUI applications

### 18. Model Aliases
- **Need**: Simpler model selection with aliases like "fast", "balanced", "powerful"
- **API**: `ModelAlias` enum that maps to specific models per provider
- **Benefit**: Easier model selection for users

## Documentation Requests

### 19. Best Practices Guide
- How to handle rate limits effectively
- Recommended timeout values per model
- Cost optimization strategies
- Error recovery patterns

### 20. More Examples
- Streaming responses example
- Error handling patterns
- Building a chatbot
- Parallel processing with progress reporting

## Testing Support

### 21. Mock Client
- **Need**: A mock implementation for testing
- **API**: `MockAiClient` that returns predetermined responses
- **Benefit**: Easier testing of applications using the crate

### 22. Request Recording
- **Need**: Record and replay API interactions for testing
- **API**: `RecordingClient` wrapper
- **Benefit**: Reproducible tests, debugging production issues

## Performance

### 23. Connection Pooling
- **Need**: Reuse HTTP connections across requests
- **Benefit**: Lower latency, reduced connection overhead

### 24. Response Compression
- **Need**: Support for compressed responses
- **Benefit**: Reduced bandwidth usage, faster responses

## Breaking Change Suggestions

If a v0.3.0 or v1.0.0 is planned:

1. **Rename `send_prompt()` to `complete()`** - More accurate naming
2. **Make `AiClient` object-safe** - Allow for `Box<dyn AiClient>` without issues
3. **Standardize model names** - Use provider-agnostic names where possible
4. **Return Result for all operations** - Consistent error handling

## Summary

The most impactful improvements would be:
1. Streaming support (critical for UX)
2. Conversation/chat support (enables new use cases)
3. Better error types (improves reliability)
4. Response metadata (enables better monitoring)
5. Custom endpoints (enables enterprise use)

These changes would make the crate more suitable for production applications while maintaining its current simplicity for basic use cases.