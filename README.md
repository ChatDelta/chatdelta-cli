# ChatDelta CLI

ChatDelta CLI is a command-line tool for querying multiple AI models in parallel and summarizing their responses. It is written in Rust and provides a unified interface to several popular APIs.

## Features

- Connects to OpenAI, Google Gemini, and Anthropic Claude depending on the API keys provided.
- Runs queries against all enabled models in parallel.
- Optional summarization of results for quick comparison.
- Supports output as plain text, JSON, or Markdown.
- **Debate Mode**: structured multi-model deliberation with a moderator that synthesizes the exchange.
- Command line flags for selecting models, adjusting temperature and timeouts, and more.
- Built-in commands to list available models and test API connectivity.

## Installation

This crate requires a recent version of Rust. Clone the repository and build with Cargo:

```bash
cargo build --release
```

The resulting binary will be available in `target/release/chatdelta`.

## Usage

Set your API keys in the environment before running the tool:

```bash
export OPENAI_API_KEY=<your-openai-key>  # or CHATGPT_API_KEY
export GEMINI_API_KEY=<your-gemini-key>
export ANTHROPIC_API_KEY=<your-anthropic-key>  # or CLAUDE_API_KEY
```

Check your API key configuration:

```bash
./chatdelta --doctor
```

Run the CLI with a prompt:

```bash
./chatdelta "How do I implement a binary search in Rust?"
```

### Common options

- `--doctor`: check API key configuration and get setup guidance
- `--test`: verify API connectivity without sending a prompt
- `--list-models`: print available model names and exit
- `--log <path>`: save the full conversation to a file
- `--format <text|json|markdown>`: choose output format
- `--only gpt,gemini` or `--exclude claude`: control which AIs are queried
- `--no-summary`: display raw responses without generating a summary

See `--help` for the full list of flags.

## Debate Mode

Debate Mode runs a structured deliberation between two AI models on a proposition, then brings in a third model as a moderator to evaluate the exchange.

```
chatdelta debate --model-a <provider:model> --model-b <provider:model> --prompt "<proposition>"
```

Models are specified as `provider:model`. Supported providers: `openai`, `anthropic` (or `claude`), `google` (or `gemini`).

### How it works

1. **Model A** gives an opening statement on the proposition.
2. **Model B** responds, engaging directly with Model A's arguments.
3. Each rebuttal round (controlled by `--rounds`) alternates between Model A and Model B.
4. The **Moderator** analyzes the full transcript and produces a structured report covering:
   - Strongest point from each side
   - Shared conclusions
   - Unresolved disagreements
   - Factual claims that should be independently verified
   - Final takeaway and confidence level

The moderator is a referee and synthesizer, not a participant. If `--moderator` is omitted, one is auto-detected from your available API keys (preference order: Gemini → Claude → OpenAI).

### Example

```bash
chatdelta debate \
  --model-a openai:gpt-4o \
  --model-b anthropic:claude-sonnet-4-6 \
  --moderator google:gemini-2.5-flash \
  --rounds 1 \
  --prompt "Microservices architecture improves long-term maintainability for most teams."
```

Pipe a proposition from a file:

```bash
cat proposition.txt | chatdelta debate \
  --model-a openai:gpt-4o \
  --model-b anthropic:claude-sonnet-4-6 \
  --rounds 2 \
  --export debate-output.md
```

The `deliberate` command is an alias for `debate`.

### Debate flags

| Flag | Default | Description |
|------|---------|-------------|
| `--model-a` | required | Model A: `provider:model` |
| `--model-b` | required | Model B: `provider:model` |
| `--moderator` | auto | Moderator model: `provider:model` |
| `--prompt` | required | Proposition text (or use `--prompt-file`, or pipe via stdin) |
| `--rounds` | `1` | Number of rebuttal pairs after the opening exchange |
| `--protocol` | `moderated-debate` | Debate protocol |
| `--export` | — | Write full transcript + report to a markdown file |
| `--max-turn-chars` | `2000` | Character guideline per turn |
| `--quiet` | — | Suppress progress output |

## Development

The project contains integration tests in `src/main.rs`. Run them with:

```bash
cargo test
```

To run a single test:

```bash
cargo test test_args_parsing
```

Note that the tests require the dependent `chatdelta` crate which lives in the repository root. Ensure it is checked out alongside this project.

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
This project adheres to our [Code of Conduct](CODE_OF_CONDUCT.md). By participating you agree to uphold it.

## License

This project is licensed under the [MIT License](LICENSE).
