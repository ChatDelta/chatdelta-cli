# ChatDelta CLI

Query OpenAI, Gemini, and Claude in parallel and compare their responses — from a single command.

## Installation

Requires a recent stable Rust toolchain:

```bash
cargo build --release
```

The binary lands at `target/release/chatdelta`.

## Setup

Export whichever API keys you have — at least one is required:

```bash
export OPENAI_API_KEY=<key>       # or CHATGPT_API_KEY
export GEMINI_API_KEY=<key>
export ANTHROPIC_API_KEY=<key>    # or CLAUDE_API_KEY
```

Verify your configuration:

```bash
./chatdelta --doctor
```

## Usage

```bash
./chatdelta "How do I implement a binary search in Rust?"
```

### Common flags

| Flag | Description |
|------|-------------|
| `--only gpt,gemini` | Query only the listed models |
| `--exclude claude` | Skip the listed models |
| `--system-prompt <text>` | Set a system prompt for all models |
| `--no-summary` | Skip the summary; show raw responses only |
| `--show-usage` | Print a token / latency table after responses |
| `--stream` | Stream tokens as they arrive (single-model; use with `--only`) |
| `--format text\|json\|markdown` | Output format (default: `text`) |
| `--log <path>` | Append the full exchange to a file |
| `--test` | Test API connectivity without sending a prompt |
| `--list-models` | Print available model names and exit |

### --show-usage

Appends a per-model token count and latency table after the response:

```bash
./chatdelta --show-usage "What is a monad?"
```

```
Model                  Tokens     Latency
──────────────────────────────────────────
ChatGPT                  1024      834ms
Gemini                    718      412ms
Claude                    892      612ms
```

### --system-prompt

```bash
./chatdelta --system-prompt "You are a senior Rust engineer. Be concise." \
  "What are the trade-offs between Arc and Rc?"
```

Works in conversation mode too — see below.

### --stream

Stream tokens from a single model as they arrive:

```bash
./chatdelta --stream --only claude "Explain monads in plain English."
```

If multiple models are selected, `--stream` falls back to parallel mode with a warning. When `--stream` and `--show-usage` are both set, streaming is skipped in favour of a metadata-bearing response so the usage table can be shown.

## Conversation Mode

Start an interactive multi-turn session:

```bash
./chatdelta --conversation           # or -c
./chatdelta -c --system-prompt "You are a Socratic tutor."
```

| Command | Action |
|---------|--------|
| `save`  | Write history to the `--save-conversation` path |
| `clear` | Reset conversation history |
| `exit` / `quit` | End the session (auto-saves if `--save-conversation` is set) |

Save and resume across runs:

```bash
./chatdelta -c --save-conversation session.json
./chatdelta -c --load-conversation session.json --save-conversation session.json
```

## Debate Mode

Run a structured deliberation between two models on a proposition. A third model acts as moderator and produces a report covering the strongest point from each side, shared conclusions, unresolved disagreements, and factual claims worth verifying.

```bash
chatdelta debate \
  --model-a openai:gpt-4o \
  --model-b anthropic:claude-sonnet-4-6 \
  --moderator google:gemini-2.5-flash \
  --prompt "Microservices architecture improves long-term maintainability for most teams."
```

Models are specified as `provider:model`. Supported providers: `openai`, `anthropic` / `claude`, `google` / `gemini`. If `--moderator` is omitted, one is auto-selected from your available keys (Gemini → Claude → OpenAI).

Pipe from a file and export the full transcript:

```bash
cat proposition.txt | chatdelta debate \
  --model-a openai:gpt-4o \
  --model-b anthropic:claude-sonnet-4-6 \
  --rounds 2 \
  --export debate-output.md
```

`deliberate` is an alias for `debate`. Note: `--stream`, `--system-prompt`, `--show-usage`, and other top-level query flags do not apply to `debate`; use the debate-specific flags below.

### Debate flags

| Flag | Default | Description |
|------|---------|-------------|
| `--model-a` | required | Model A: `provider:model` |
| `--model-b` | required | Model B: `provider:model` |
| `--moderator` | auto | Moderator: `provider:model` |
| `--prompt` | required | Proposition text (or `--prompt-file`, or pipe via stdin) |
| `--rounds` | `1` | Rebuttal pairs after the opening exchange |
| `--export` | — | Write transcript + moderator report to a markdown file |
| `--max-turn-chars` | `2000` | Character guideline per turn |
| `--quiet` | — | Suppress progress output |

## Development

```bash
cargo test
cargo test test_args_parsing    # run a single test
```

Tests use `chatdelta v0.8.2` from crates.io. The `mock` feature is enabled in `[dev-dependencies]` so no live API keys are needed to run the suite.

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines. This project follows our [Code of Conduct](CODE_OF_CONDUCT.md).

## License

[MIT](LICENSE)
