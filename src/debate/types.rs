//! Core types for ChatDelta Debate Mode

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

/// Parsed model specification from "provider:model" strings (e.g. "openai:gpt-4o")
#[derive(Debug, Clone)]
pub struct ModelSpec {
    pub provider: String,
    pub model: String,
}

impl FromStr for ModelSpec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once(':') {
            Some((provider, model)) if !model.is_empty() => {
                let provider = normalize_provider(provider)?;
                Ok(ModelSpec {
                    provider,
                    model: model.to_string(),
                })
            }
            Some(_) => Err(format!("Model name is empty in '{s}'")),
            None => Err(format!(
                "Invalid model spec '{s}'. Expected format: provider:model (e.g. openai:gpt-4o)"
            )),
        }
    }
}

impl fmt::Display for ModelSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.provider, self.model)
    }
}

fn normalize_provider(p: &str) -> Result<String, String> {
    match p.to_lowercase().as_str() {
        "openai" | "gpt" | "chatgpt" => Ok("openai".to_string()),
        "anthropic" | "claude" => Ok("claude".to_string()),
        "google" | "gemini" => Ok("gemini".to_string()),
        other => Err(format!(
            "Unknown provider '{other}'. Valid: openai/gpt, anthropic/claude, google/gemini"
        )),
    }
}

/// Full configuration for a debate session
#[derive(Debug, Clone)]
pub struct DebateConfig {
    pub proposition: String,
    pub model_a: ModelSpec,
    pub model_b: ModelSpec,
    /// None means auto-detect from available API keys
    pub moderator: Option<ModelSpec>,
    /// Number of rebuttal pairs (each pair = one turn for A + one for B)
    pub rounds: u32,
    pub protocol: DebateProtocol,
    pub max_turn_chars: usize,
    pub export_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DebateProtocol {
    ModeratedDebate,
}

impl fmt::Display for DebateProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DebateProtocol::ModeratedDebate => write!(f, "moderated-debate"),
        }
    }
}

impl FromStr for DebateProtocol {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "moderated-debate" | "moderated_debate" => Ok(DebateProtocol::ModeratedDebate),
            other => Err(format!(
                "Unknown protocol '{other}'. Valid protocols: moderated-debate"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParticipantRole {
    ModelA,
    ModelB,
    Moderator,
}

impl fmt::Display for ParticipantRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParticipantRole::ModelA => write!(f, "Model A"),
            ParticipantRole::ModelB => write!(f, "Model B"),
            ParticipantRole::Moderator => write!(f, "Moderator"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TurnStage {
    Opening,
    InitialResponse,
    /// round number starting at 1
    Rebuttal(u32),
    ModeratorReport,
}

impl fmt::Display for TurnStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TurnStage::Opening => write!(f, "Opening Statement"),
            TurnStage::InitialResponse => write!(f, "Response"),
            TurnStage::Rebuttal(n) => write!(f, "Rebuttal (Round {n})"),
            TurnStage::ModeratorReport => write!(f, "Moderator's Report"),
        }
    }
}

/// A single recorded turn in the debate
#[derive(Debug, Clone)]
pub struct DebateTurn {
    pub role: ParticipantRole,
    pub stage: TurnStage,
    /// Human-readable label, e.g. "openai:gpt-4o"
    pub model_label: String,
    pub content: String,
}

/// Parsed output of the moderator's synthesis
#[derive(Debug, Clone)]
pub struct ModeratorReport {
    pub strongest_point_a: String,
    pub strongest_point_b: String,
    pub shared_conclusions: String,
    pub unresolved_disagreements: String,
    pub verification_flags: Vec<String>,
    pub final_takeaway: String,
    pub confidence: ConfidenceLevel,
    /// Full raw moderator response, preserved for export
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
}

impl fmt::Display for ConfidenceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfidenceLevel::High => write!(f, "High"),
            ConfidenceLevel::Medium => write!(f, "Medium"),
            ConfidenceLevel::Low => write!(f, "Low"),
        }
    }
}

/// The accumulated record of a complete debate
#[derive(Debug)]
pub struct DebateTranscript {
    pub config: DebateConfig,
    pub turns: Vec<DebateTurn>,
    pub moderator_report: Option<ModeratorReport>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl DebateTranscript {
    pub fn new(config: DebateConfig) -> Self {
        DebateTranscript {
            config,
            turns: Vec::new(),
            moderator_report: None,
            started_at: Utc::now(),
            finished_at: None,
        }
    }

    pub fn add_turn(&mut self, turn: DebateTurn) {
        self.turns.push(turn);
    }

    pub fn set_moderator_report(&mut self, report: ModeratorReport) {
        self.moderator_report = Some(report);
    }

    pub fn finalize(&mut self) {
        self.finished_at = Some(Utc::now());
    }

    /// Format all recorded turns as readable context for the next model prompt
    pub fn format_context(&self) -> String {
        let mut context = String::new();
        for turn in &self.turns {
            context.push_str(&format!(
                "\n--- {} [{}] — {} ---\n{}\n",
                turn.role, turn.model_label, turn.stage, turn.content
            ));
        }
        context
    }
}
