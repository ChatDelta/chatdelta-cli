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
  - Executes parallel queries to all enabled AI models
  - Generates optional summary using Gemini (if available)
  - Coordinates output formatting and logging

- **cli.rs**: Command-line argument parsing and validation
  - Uses clap for argument parsing
  - Supports model selection (--only, --exclude), output formats, logging options
  - Validates argument combinations and ranges

- **output.rs**: Output formatting (text, JSON, Markdown)
  - Handles different output formats based on --format flag
  - Manages simple file-based logging with --log flag

- **logging.rs**: Comprehensive structured logging system
  - Supports session tracking, performance metrics, error logging
  - Multiple log formats: simple, JSON, structured
  - Default log location: ~/.chatdelta/logs

### New Features in v0.3.0

ChatDelta CLI now leverages the new features from chatdelta crate v0.3.0:

- **RetryStrategy**: Configurable retry strategies (exponential, linear, fixed) for handling API failures
- **ChatSession**: Interactive conversation mode with message history
- **Improved Error Handling**: Better retry logic and error recovery

### Key Dependencies

The project depends on the `chatdelta` crate (v0.3.0) which provides:
- `AiClient` trait for unified API interaction
- `ClientConfig` builder for configuration
- `create_client()` factory function for client instantiation
- `execute_parallel()` for concurrent API queries
- `generate_summary()` for response summarization
- `ChatSession` for conversation management (new in v0.3.0)
- `RetryStrategy` enum for configurable retry logic (new in v0.3.0)

### API Integration Pattern

Each AI client is created conditionally based on:
1. Environment variable presence (API key)
2. User selection flags (--only, --exclude)
3. Model specification (--gpt-model, --gemini-model, --claude-model)

All clients share common configuration (timeout, retries, temperature, max_tokens) through `ClientConfig`.

## Testing

The project includes integration tests in main.rs that verify:
- Argument parsing and validation
- Error handling for missing API keys
- Command validation logic

Note: Tests expect the `chatdelta` crate to be available as a dependency from crates.io.