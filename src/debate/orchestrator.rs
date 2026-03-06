//! Debate orchestrator: executes turns in protocol order, builds transcript.

use std::collections::{HashMap, HashSet};
use std::env;

use chatdelta::{create_client, AiClient, ClientConfig};

use super::prompts;
use super::protocol::{turn_sequence, TurnSpec};
use super::renderer::DebateRenderer;
use super::types::{
    ConfidenceLevel, DebateConfig, DebateTurn, DebateTranscript, ModelSpec, ModeratorReport,
    ParticipantRole, TurnStage,
};

pub struct Orchestrator {
    config: DebateConfig,
    client_a: Box<dyn AiClient>,
    client_b: Box<dyn AiClient>,
    moderator_client: Option<Box<dyn AiClient>>,
    renderer: DebateRenderer,
}

impl Orchestrator {
    pub fn new(
        config: DebateConfig,
        client_a: Box<dyn AiClient>,
        client_b: Box<dyn AiClient>,
        moderator_client: Option<Box<dyn AiClient>>,
        quiet: bool,
    ) -> Self {
        Orchestrator {
            config,
            client_a,
            client_b,
            moderator_client,
            renderer: DebateRenderer::new(quiet),
        }
    }

    /// Run the full debate and return the completed transcript.
    pub async fn run(&mut self) -> Result<DebateTranscript, Box<dyn std::error::Error>> {
        let mut transcript = DebateTranscript::new(self.config.clone());
        let turns = turn_sequence(&self.config.protocol, self.config.rounds);

        self.renderer.print_debate_header(&self.config);

        for turn_spec in &turns {
            if turn_spec.role == ParticipantRole::Moderator {
                self.run_moderator_turn(&mut transcript).await?;
            } else {
                self.run_participant_turn(turn_spec, &mut transcript).await?;
            }
        }

        transcript.finalize();
        Ok(transcript)
    }

    async fn run_participant_turn(
        &mut self,
        spec: &TurnSpec,
        transcript: &mut DebateTranscript,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (role_label, opponent_label) = match spec.role {
            ParticipantRole::ModelA => (
                self.config.model_a.to_string(),
                self.config.model_b.to_string(),
            ),
            ParticipantRole::ModelB => (
                self.config.model_b.to_string(),
                self.config.model_a.to_string(),
            ),
            ParticipantRole::Moderator => unreachable!(),
        };

        let context = transcript.format_context();
        let prompt = build_turn_prompt(spec, &self.config, &context, &role_label, &opponent_label);

        self.renderer
            .print_turn_header(&spec.role, &spec.stage, &role_label);

        let response = match spec.role {
            ParticipantRole::ModelA => self.client_a.send_prompt(&prompt).await,
            ParticipantRole::ModelB => self.client_b.send_prompt(&prompt).await,
            ParticipantRole::Moderator => unreachable!(),
        }
        .map_err(|e| format!("{} ({}) failed: {}", spec.role, role_label, e))?;

        // Simple repetition heuristic: compare with this model's last turn
        if let Some(prior) = transcript
            .turns
            .iter()
            .rev()
            .find(|t| t.role == spec.role)
        {
            if is_repetitive(&prior.content, &response) {
                self.renderer.print_repetition_warning(&spec.role);
            }
        }

        self.renderer.print_turn_response(&response);

        transcript.add_turn(DebateTurn {
            role: spec.role.clone(),
            stage: spec.stage.clone(),
            model_label: role_label,
            content: response,
        });

        Ok(())
    }

    async fn run_moderator_turn(
        &mut self,
        transcript: &mut DebateTranscript,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(ref moderator) = self.moderator_client else {
            return Ok(());
        };

        let context = transcript.format_context();
        let prompt = prompts::moderator_prompt(
            &self.config.proposition,
            &self.config.model_a.to_string(),
            &self.config.model_b.to_string(),
            &context,
        );

        self.renderer
            .print_turn_header(&ParticipantRole::Moderator, &TurnStage::ModeratorReport, "Moderator");

        let raw = moderator
            .send_prompt(&prompt)
            .await
            .map_err(|e| format!("Moderator failed: {}", e))?;

        let report = parse_moderator_report(&raw);
        self.renderer.print_moderator_report(&report);
        transcript.set_moderator_report(report);

        Ok(())
    }
}

fn build_turn_prompt(
    spec: &TurnSpec,
    config: &DebateConfig,
    context: &str,
    role_label: &str,
    opponent_label: &str,
) -> String {
    match &spec.stage {
        TurnStage::Opening => prompts::opening_prompt(&config.proposition, config.max_turn_chars),
        TurnStage::InitialResponse => {
            prompts::response_prompt(&config.proposition, context, config.max_turn_chars)
        }
        TurnStage::Rebuttal(round) => prompts::rebuttal_prompt(
            &config.proposition,
            context,
            role_label,
            opponent_label,
            *round,
            config.max_turn_chars,
        ),
        TurnStage::ModeratorReport => unreachable!(),
    }
}

/// Word-overlap repetition heuristic.
/// Returns true if the current response shares >60% of its content words with the prior response
/// from the same model, suggesting the model is repeating itself.
fn is_repetitive(prior: &str, current: &str) -> bool {
    let stop_words: HashSet<&str> = [
        "the", "a", "an", "is", "it", "in", "of", "and", "or", "to", "that", "this", "with",
        "for", "as", "by", "on", "at", "be", "are", "was", "were", "has", "have", "had", "but",
        "not", "from", "they", "their", "its", "i", "my", "we", "our", "you", "your", "do",
        "did", "will", "would", "can", "could", "should", "may", "might", "also", "more",
        "than", "so", "if", "about", "which", "there", "when", "just", "both",
    ]
    .iter()
    .copied()
    .collect();

    let content_words = |text: &str| -> HashSet<String> {
        text.split_whitespace()
            .map(|w| {
                w.to_lowercase()
                    .trim_matches(|c: char| !c.is_alphabetic())
                    .to_string()
            })
            .filter(|w| w.len() > 4 && !stop_words.contains(w.as_str()))
            .collect()
    };

    let prior_words = content_words(prior);
    let current_words = content_words(current);

    if prior_words.is_empty() || current_words.len() < 10 {
        return false;
    }

    let overlap = current_words.intersection(&prior_words).count();
    let ratio = overlap as f64 / current_words.len() as f64;
    ratio > 0.60
}

/// Parse a structured moderator report from raw LLM output.
/// Splits on `## Header` markers to fill report fields.
/// Gracefully handles missing or malformed sections.
fn parse_moderator_report(raw: &str) -> ModeratorReport {
    let sections = extract_sections(raw);
    let get = |key: &str| -> String {
        sections
            .get(key)
            .cloned()
            .unwrap_or_default()
            .trim()
            .to_string()
    };

    let verification_text = get("Claims Requiring Verification");
    let verification_flags = if verification_text.is_empty()
        || verification_text.to_lowercase().starts_with("none")
    {
        vec![]
    } else {
        verification_text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.trim_start_matches(['-', '*', '•', ' ']).to_string())
            .filter(|l| !l.is_empty())
            .collect()
    };

    let confidence = parse_confidence(&get("Confidence Level"));

    ModeratorReport {
        strongest_point_a: get("Strongest Point from Model A"),
        strongest_point_b: get("Strongest Point from Model B"),
        shared_conclusions: get("Shared Conclusions"),
        unresolved_disagreements: get("Unresolved Disagreements"),
        verification_flags,
        final_takeaway: get("Final Takeaway"),
        confidence,
        raw: raw.to_string(),
    }
}

/// Split text into named sections using `## Header` markers
fn extract_sections(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut current_key: Option<String> = None;
    let mut current_content = String::new();

    for line in text.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            if let Some(key) = current_key.take() {
                map.insert(key, current_content.trim().to_string());
            }
            current_key = Some(header.trim().to_string());
            current_content = String::new();
        } else if current_key.is_some() {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    if let Some(key) = current_key {
        map.insert(key, current_content.trim().to_string());
    }

    map
}

fn parse_confidence(text: &str) -> ConfidenceLevel {
    let lower = text.to_lowercase();
    if lower.starts_with("high") {
        ConfidenceLevel::High
    } else if lower.starts_with("low") {
        ConfidenceLevel::Low
    } else {
        ConfidenceLevel::Medium
    }
}

/// Create an AiClient from a ModelSpec, looking up the API key from environment variables
pub fn resolve_client(
    spec: &ModelSpec,
    config: ClientConfig,
) -> Result<Box<dyn AiClient>, Box<dyn std::error::Error>> {
    let api_key = match spec.provider.as_str() {
        "openai" => env::var("OPENAI_API_KEY").or_else(|_| env::var("CHATGPT_API_KEY")),
        "gemini" => env::var("GEMINI_API_KEY"),
        "claude" => env::var("ANTHROPIC_API_KEY").or_else(|_| env::var("CLAUDE_API_KEY")),
        other => return Err(format!("Unknown provider: {other}").into()),
    }
    .map_err(|_| {
        let env_hint = match spec.provider.as_str() {
            "openai" => "OPENAI_API_KEY or CHATGPT_API_KEY",
            "gemini" => "GEMINI_API_KEY",
            "claude" => "ANTHROPIC_API_KEY or CLAUDE_API_KEY",
            _ => "the relevant API key",
        };
        format!(
            "No API key found for provider '{}'. Set the {} environment variable.",
            spec.provider, env_hint
        )
    })?;

    create_client(&spec.provider, &api_key, &spec.model, config)
        .map_err(|e| format!("Failed to create client for {}: {}", spec, e).into())
}

/// Auto-detect a moderator client from available environment variables.
/// Preference order: Gemini → Claude → OpenAI.
pub fn resolve_auto_moderator(
    config: ClientConfig,
) -> Result<Option<Box<dyn AiClient>>, Box<dyn std::error::Error>> {
    if let Ok(key) = env::var("GEMINI_API_KEY") {
        let client = create_client("gemini", &key, "gemini-2.5-flash", config)?;
        return Ok(Some(client));
    }
    if let Ok(key) = env::var("ANTHROPIC_API_KEY").or_else(|_| env::var("CLAUDE_API_KEY")) {
        let client = create_client("claude", &key, "claude-sonnet-4-6", config)?;
        return Ok(Some(client));
    }
    if let Ok(key) = env::var("OPENAI_API_KEY").or_else(|_| env::var("CHATGPT_API_KEY")) {
        let client = create_client("openai", &key, "gpt-4o", config)?;
        return Ok(Some(client));
    }
    Ok(None)
}
