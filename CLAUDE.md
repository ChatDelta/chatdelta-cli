# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Development Commands

```bash
# Build the project
cargo build --release

# Run tests
cargo test

# Format code
cargo fmt

# Check for linting issues
cargo clippy

# Run the CLI with a prompt
./target/release/chatdelta "Your prompt here"

# Read prompt from stdin
echo "Your prompt" | ./target/release/chatdelta -

# Read prompt from file
./target/release/chatdelta --prompt-file prompt.txt

# Check API key configuration
./target/release/chatdelta --doctor

# Test API connections
./target/release/chatdelta --test

# List available models
./target/release/chatdelta --list-models

# Save individual model responses
./target/release/chatdelta "prompt" --save-responses ./responses/

# Raw output mode (no formatting)
./target/release/chatdelta "prompt" --raw

# Use different retry strategies (new in v0.3.0)
./target/release/chatdelta "prompt" --retry-strategy exponential  # default
./target/release/chatdelta "prompt" --retry-strategy linear
./target/release/chatdelta "prompt" --retry-strategy fixed

# Interactive conversation mode (new in v0.3.0)
./target/release/chatdelta --conversation

# Conversation mode with system prompt
./target/release/chatdelta --conversation --system-prompt "You are a helpful coding assistant"

# Save/load conversation history
./target/release/chatdelta --conversation --save-conversation chat.json
./target/release/chatdelta --conversation --load-conversation chat.json
```

## Architecture Overview

ChatDelta CLI is a Rust-based command-line tool that queries multiple AI APIs (OpenAI, Google Gemini, Anthropic Claude) in parallel and optionally summarizes their responses.

### Core Structure

- **main.rs**: Entry point and orchestration logic
  - Handles client creation based on environment variables (OPENAI_API_KEY, GEMINI_API_KEY, ANTHROPIC_API_KEY)
  - Executes parallel queries to all enabled AI models via `execute_parallel()`
  - Generates optional summary using fallback order: Gemini → Claude → OpenAI
  - Implements conversation mode with `ChatSession` for interactive chat
  - Coordinates output formatting and comprehensive logging

- **cli.rs**: Command-line argument parsing and validation
  - Uses clap derive macros for argument parsing
  - Validates conflicting flags (--only/--exclude, --verbose/--quiet)
  - Validates input constraints (temperature 0.0-2.0, timeout > 0, prompt length < 100K)
  - Provides `should_use_ai()` method to determine which AIs to query based on flags

- **output.rs**: Output formatting (text, JSON, Markdown)
  - Text mode: single response prints directly, multiple responses shown with headers
  - JSON mode: structured output with prompt, responses object, and optional summary
  - Markdown mode: formatted with headers for each model and summary section
  - Separate `log_interaction()` for legacy simple file logging

- **logging.rs**: Comprehensive structured logging system
  - Session-based logging with UUIDs for tracking related interactions
  - Records per-model responses, timings, token usage, and errors
  - Multiple log formats: simple (human-readable), JSON, structured
  - Default log location: ~/.chatdelta/logs
  - Tracks performance metrics and generates session summaries

- **metrics_display.rs**: Performance metrics tracking and display
  - Uses `ClientMetrics` from core library for consistent tracking
  - Records success/failure rates, latencies, token usage per provider
  - Provides session summaries with per-provider breakdown
  - Exports metrics as JSON or displays formatted tables

### Key Dependencies

The project depends on the `chatdelta` crate (v0.6.0) which provides:
- `AiClient` trait for unified API interaction
- `ClientConfig` builder for configuration with retry strategies
- `create_client()` factory function supporting "openai", "gemini", "claude", "anthropic" providers
- `execute_parallel()` for concurrent API queries returning Vec<(String, Result<String>)>
- `generate_summary()` for response summarization
- `ChatSession` for conversation management with message history
- `RetryStrategy` enum: Exponential, Linear, Fixed
- `ClientMetrics` for performance tracking

### API Integration Pattern

Each AI client is created conditionally based on:
1. Environment variable presence:
   - OpenAI: `OPENAI_API_KEY` or `CHATGPT_API_KEY`
   - Gemini: `GEMINI_API_KEY`
   - Anthropic: `ANTHROPIC_API_KEY` or `CLAUDE_API_KEY`
2. User selection flags (--only, --exclude) checked via `should_use_ai()`
3. Model specification (--gpt-model, --gemini-model, --claude-model)

All clients share common configuration (timeout, retries, temperature, max_tokens, retry_strategy) through `ClientConfig::builder()`. Client creation errors are handled gracefully with warnings rather than hard failures.

### Doctor Command

The `--doctor` flag (new in v0.3.0) checks for API key configuration and provides helpful setup guidance:
- Checks all environment variable variants (OPENAI_API_KEY/CHATGPT_API_KEY, etc.)
- Displays which environment variable name is being used
- Provides direct links to obtain API keys from each provider
- Shows example export commands for setting up environment variables

## Testing

The project includes integration tests in main.rs that verify:
- Argument parsing and validation
- Error handling for missing API keys
- Command validation logic

Note: Tests expect the `chatdelta` crate to be available as a dependency from crates.io.