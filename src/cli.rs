//! Command-line interface for ChatDelta

use clap::Parser;
use std::path::PathBuf;

/// Command line arguments for chatdelta
#[derive(Parser, Debug)]
#[command(version, about = "Query multiple AIs and connect their responses", long_about = None)]
pub struct Args {
    /// Prompt to send to the AIs (use '-' to read from stdin)
    pub prompt: Option<String>,

    /// Read prompt from a file
    #[arg(long, short = 'F', conflicts_with = "prompt")]
    pub prompt_file: Option<PathBuf>,

    /// Optional path to log the full interaction
    #[arg(long, short)]
    pub log: Option<PathBuf>,

    /// Log directory for structured logging (default: ~/.chatdelta/logs)
    #[arg(long)]
    pub log_dir: Option<PathBuf>,

    /// Log format: simple, json, structured
    #[arg(long, default_value = "simple")]
    pub log_format: String,

    /// Enable performance metrics logging
    #[arg(long)]
    pub log_metrics: bool,

    /// Enable detailed error logging
    #[arg(long)]
    pub log_errors: bool,

    /// Log session ID for tracking related interactions
    #[arg(long)]
    pub session_id: Option<String>,

    /// Verbose output - show detailed progress and API responses
    #[arg(long, short)]
    pub verbose: bool,

    /// Quiet mode - suppress progress indicators
    #[arg(long, short)]
    pub quiet: bool,

    /// Output format: text, json, markdown
    #[arg(long, short = 'f', default_value = "text")]
    pub format: String,

    /// Skip summary generation - just show individual responses
    #[arg(long)]
    pub no_summary: bool,

    /// Only query specific AIs (comma-separated: gpt,gemini,claude)
    #[arg(long, value_delimiter = ',')]
    pub only: Vec<String>,

    /// Exclude specific AIs (comma-separated: gpt,gemini,claude)
    #[arg(long, value_delimiter = ',')]
    pub exclude: Vec<String>,

    /// Timeout for API requests in seconds
    #[arg(long, default_value = "30")]
    pub timeout: u64,

    /// Number of retry attempts for failed requests
    #[arg(long, default_value = "0")]
    pub retries: u32,

    /// OpenAI model to use
    #[arg(long, default_value = "gpt-4o")]
    pub gpt_model: String,

    /// Gemini model to use
    #[arg(long, default_value = "gemini-1.5-pro-latest")]
    pub gemini_model: String,

    /// Claude model to use
    #[arg(long, default_value = "claude-3-5-sonnet-20241022")]
    pub claude_model: String,

    /// Maximum tokens for Claude responses
    #[arg(long, default_value = "1024")]
    pub max_tokens: u32,

    /// Temperature for AI responses (0.0-2.0)
    #[arg(long)]
    pub temperature: Option<f32>,

    /// Show available models and exit
    #[arg(long)]
    pub list_models: bool,

    /// Test API connections and exit
    #[arg(long)]
    pub test: bool,

    /// Save individual model responses to separate files
    #[arg(long)]
    pub save_responses: Option<PathBuf>,

    /// Show progress spinner for long operations
    #[arg(long, default_value = "true")]
    pub progress: bool,

    /// Output raw responses without any formatting
    #[arg(long)]
    pub raw: bool,

    /// Retry strategy: exponential, linear, fixed
    #[arg(long, default_value = "exponential")]
    pub retry_strategy: String,

    /// Enable conversation mode (interactive chat)
    #[arg(long, short = 'c')]
    pub conversation: bool,

    /// System prompt to set context for the AI
    #[arg(long)]
    pub system_prompt: Option<String>,

    /// Load conversation history from file
    #[arg(long)]
    pub load_conversation: Option<PathBuf>,

    /// Save conversation history to file
    #[arg(long)]
    pub save_conversation: Option<PathBuf>,
}

impl Args {
    /// Validate the arguments and handle conflicts
    pub fn validate(&self) -> Result<(), String> {
        // Prompt is required unless using special commands, prompt file, or conversation mode
        if self.prompt.is_none()
            && self.prompt_file.is_none()
            && !self.list_models
            && !self.test
            && !self.conversation
        {
            return Err(
                "Prompt is required unless using --prompt-file, --list-models, --test, or --conversation"
                    .to_string(),
            );
        }

        if let Some(prompt) = &self.prompt {
            if prompt.is_empty() {
                return Err("Prompt cannot be empty".to_string());
            }
        }

        if self.verbose && self.quiet {
            return Err("Cannot use both --verbose and --quiet flags".to_string());
        }

        if !matches!(self.format.as_str(), "text" | "json" | "markdown") {
            return Err("Output format must be one of: text, json, markdown".to_string());
        }

        if !matches!(
            self.retry_strategy.as_str(),
            "exponential" | "linear" | "fixed"
        ) {
            return Err("Retry strategy must be one of: exponential, linear, fixed".to_string());
        }

        if !matches!(self.log_format.as_str(), "simple" | "json" | "structured") {
            return Err("Log format must be one of: simple, json, structured".to_string());
        }

        if !self.only.is_empty() && !self.exclude.is_empty() {
            return Err("Cannot use both --only and --exclude flags".to_string());
        }

        for ai in &self.only {
            if !matches!(ai.as_str(), "gpt" | "gemini" | "claude") {
                return Err(format!(
                    "Unknown AI '{}'. Valid options: gpt, gemini, claude",
                    ai
                ));
            }
        }

        for ai in &self.exclude {
            if !matches!(ai.as_str(), "gpt" | "gemini" | "claude") {
                return Err(format!(
                    "Unknown AI '{}'. Valid options: gpt, gemini, claude",
                    ai
                ));
            }
        }

        if let Some(temp) = self.temperature {
            if !(0.0..=2.0).contains(&temp) {
                return Err("Temperature must be between 0.0 and 2.0".to_string());
            }
        }

        if self.timeout == 0 {
            return Err("Timeout must be greater than 0".to_string());
        }

        Ok(())
    }

    /// Check if a specific AI should be used based on --only and --exclude flags
    pub fn should_use_ai(&self, ai_name: &str) -> bool {
        if !self.only.is_empty() {
            return self.only.contains(&ai_name.to_string());
        }
        if !self.exclude.is_empty() {
            return !self.exclude.contains(&ai_name.to_string());
        }
        true
    }
}
