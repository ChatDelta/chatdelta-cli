# ChatDelta CLI

ChatDelta CLI is a command-line tool for querying multiple AI models in parallel and summarizing their responses. It is written in Rust and provides a unified interface to several popular APIs.

## Features

- Connects to OpenAI (ChatGPT), Google Gemini, and Anthropic Claude depending on the API keys provided.
- Runs queries against all enabled models in parallel.
- Optional summarization of results for quick comparison.
- Supports output as plain text, JSON, or Markdown.
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
export OPENAI_API_KEY=<your-openai-key>
export GEMINI_API_KEY=<your-gemini-key>
export ANTHROPIC_API_KEY=<your-anthropic-key>
```

Run the CLI with a prompt:

```bash
./chatdelta "How do I implement a binary search in Rust?"
```

### Common options

- `--log <path>`: save the full conversation to a file
- `--format <text|json|markdown>`: choose output format
- `--only gpt,gemini` or `--exclude claude`: control which AIs are queried
- `--no-summary`: display raw responses without generating a summary
- `--list-models`: print available model names and exit
- `--test`: verify API connectivity without sending a prompt

See `--help` for the full list of flags.

## Development

The project contains integration tests in `src/main.rs`. Run them with:

```bash
cargo test
```

Note that the tests require the dependent `chatdelta` crate which lives in the repository root. Ensure it is checked out alongside this project.

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.
This project adheres to our [Code of Conduct](CODE_OF_CONDUCT.md). By participating you agree to uphold it.

## License

This project is licensed under the [MIT License](LICENSE).

