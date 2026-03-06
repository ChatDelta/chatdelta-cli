//! Terminal and markdown rendering for debate transcripts

use std::path::Path;

use super::types::{DebateConfig, DebateTranscript, ModeratorReport, ParticipantRole, TurnStage};

const WIDE: usize = 60;

pub struct DebateRenderer {
    quiet: bool,
}

impl DebateRenderer {
    pub fn new(quiet: bool) -> Self {
        DebateRenderer { quiet }
    }

    pub fn print_debate_header(&self, config: &DebateConfig) {
        if self.quiet {
            return;
        }
        println!();
        println!("╔{}╗", "═".repeat(WIDE - 2));
        println!(
            "║{:^width$}║",
            "ChatDelta — Debate Mode",
            width = WIDE - 2
        );
        println!("╚{}╝", "═".repeat(WIDE - 2));
        println!();
        println!("  Protocol  : {}", config.protocol);
        println!("  Model A   : {}", config.model_a);
        println!("  Model B   : {}", config.model_b);
        match &config.moderator {
            Some(m) => println!("  Moderator : {}", m),
            None => println!("  Moderator : auto-detected"),
        }
        println!("  Rounds    : {}", config.rounds);
        println!();
        println!("  Proposition:");
        println!("  {}", config.proposition);
        println!();
        println!("{}", "─".repeat(WIDE));
    }

    pub fn print_turn_header(&self, role: &ParticipantRole, stage: &TurnStage, model_label: &str) {
        if self.quiet {
            return;
        }
        println!();
        println!("┌─ {} — {}  [{}]", role, stage, model_label);
        println!();
    }

    pub fn print_turn_response(&self, content: &str) {
        if self.quiet {
            return;
        }
        println!("{}", content);
        println!();
        println!("{}", "─".repeat(WIDE));
    }

    pub fn print_repetition_warning(&self, role: &ParticipantRole) {
        if self.quiet {
            return;
        }
        println!();
        println!(
            "  ⚠  {}'s response may repeat prior arguments (>60% word overlap).",
            role
        );
    }

    pub fn print_moderator_report(&self, report: &ModeratorReport) {
        if self.quiet {
            return;
        }
        println!();
        println!("╔{}╗", "═".repeat(WIDE - 2));
        println!("║{:^width$}║", "Moderator's Report", width = WIDE - 2);
        println!("╚{}╝", "═".repeat(WIDE - 2));
        println!();

        Self::print_section("Strongest Point from Model A", &report.strongest_point_a);
        Self::print_section("Strongest Point from Model B", &report.strongest_point_b);
        Self::print_section("Shared Conclusions", &report.shared_conclusions);
        Self::print_section("Unresolved Disagreements", &report.unresolved_disagreements);

        println!("▸ Claims Requiring Verification:");
        if report.verification_flags.is_empty() {
            println!("    None identified.");
        } else {
            for flag in &report.verification_flags {
                println!("    • {}", flag);
            }
        }
        println!();

        Self::print_section("Final Takeaway", &report.final_takeaway);
        println!("▸ Confidence: {}", report.confidence);
        println!();
        println!("{}", "═".repeat(WIDE));
        println!();
    }

    fn print_section(title: &str, content: &str) {
        println!("▸ {}:", title);
        if content.is_empty() {
            println!("    (not provided)");
        } else {
            for line in content.lines() {
                println!("    {}", line);
            }
        }
        println!();
    }

    /// Export the full debate transcript and moderator report to a markdown file
    pub fn export_markdown(
        transcript: &DebateTranscript,
        path: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut md = String::new();

        md.push_str("# ChatDelta Debate\n\n");

        // Metadata
        md.push_str("## Metadata\n\n");
        md.push_str(&format!("- **Model A:** {}\n", transcript.config.model_a));
        md.push_str(&format!("- **Model B:** {}\n", transcript.config.model_b));
        match &transcript.config.moderator {
            Some(m) => md.push_str(&format!("- **Moderator:** {}\n", m)),
            None => md.push_str("- **Moderator:** auto-detected\n"),
        }
        md.push_str(&format!(
            "- **Protocol:** {}\n",
            transcript.config.protocol
        ));
        md.push_str(&format!("- **Rounds:** {}\n", transcript.config.rounds));
        md.push_str(&format!(
            "- **Proposition:** {}\n",
            transcript.config.proposition
        ));
        md.push_str(&format!(
            "- **Started:** {}\n",
            transcript.started_at.format("%Y-%m-%d %H:%M UTC")
        ));
        if let Some(finished) = transcript.finished_at {
            md.push_str(&format!(
                "- **Finished:** {}\n",
                finished.format("%Y-%m-%d %H:%M UTC")
            ));
        }
        md.push('\n');

        // Transcript
        md.push_str("## Transcript\n\n");
        for turn in &transcript.turns {
            md.push_str(&format!(
                "### {} — {} [{}]\n\n",
                turn.role, turn.stage, turn.model_label
            ));
            md.push_str(&turn.content);
            md.push_str("\n\n---\n\n");
        }

        // Moderator report
        if let Some(ref report) = transcript.moderator_report {
            md.push_str("## Moderator Report\n\n");

            md.push_str("### Strongest Point from Model A\n\n");
            md.push_str(&report.strongest_point_a);
            md.push_str("\n\n");

            md.push_str("### Strongest Point from Model B\n\n");
            md.push_str(&report.strongest_point_b);
            md.push_str("\n\n");

            md.push_str("### Shared Conclusions\n\n");
            md.push_str(&report.shared_conclusions);
            md.push_str("\n\n");

            md.push_str("### Unresolved Disagreements\n\n");
            md.push_str(&report.unresolved_disagreements);
            md.push_str("\n\n");

            md.push_str("### Claims Requiring Verification\n\n");
            if report.verification_flags.is_empty() {
                md.push_str("None identified.\n\n");
            } else {
                for flag in &report.verification_flags {
                    md.push_str(&format!("- {}\n", flag));
                }
                md.push('\n');
            }

            md.push_str("### Final Takeaway\n\n");
            md.push_str(&report.final_takeaway);
            md.push_str("\n\n");

            md.push_str("### Confidence\n\n");
            md.push_str(&format!("{}\n\n", report.confidence));
        }

        md.push_str("---\n\n*Generated by [ChatDelta](https://github.com/ChatDelta/chatdelta-cli) Debate Mode*\n");

        std::fs::write(path, &md)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::debate::types::{
        DebateConfig, DebateProtocol, DebateTurn, ModelSpec, ParticipantRole, TurnStage,
    };
    use std::str::FromStr;

    fn make_config() -> DebateConfig {
        DebateConfig {
            proposition: "Test proposition".to_string(),
            model_a: ModelSpec::from_str("openai:gpt-4o").unwrap(),
            model_b: ModelSpec::from_str("anthropic:claude-3-5-sonnet-20241022").unwrap(),
            moderator: None,
            rounds: 1,
            protocol: DebateProtocol::ModeratedDebate,
            max_turn_chars: 2000,
            export_path: None,
        }
    }

    #[test]
    fn export_markdown_contains_sections() {
        let config = make_config();
        let mut transcript = DebateTranscript::new(config);

        transcript.add_turn(DebateTurn {
            role: ParticipantRole::ModelA,
            stage: TurnStage::Opening,
            model_label: "openai:gpt-4o".to_string(),
            content: "Model A opening content.".to_string(),
        });

        transcript.add_turn(DebateTurn {
            role: ParticipantRole::ModelB,
            stage: TurnStage::InitialResponse,
            model_label: "anthropic:claude-3-5-sonnet-20241022".to_string(),
            content: "Model B response content.".to_string(),
        });

        transcript.finalize();

        let dir = std::env::temp_dir();
        let path = dir.join("test_debate_export.md");
        DebateRenderer::export_markdown(&transcript, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# ChatDelta Debate"));
        assert!(content.contains("## Metadata"));
        assert!(content.contains("## Transcript"));
        assert!(content.contains("Model A opening content."));
        assert!(content.contains("Test proposition"));

        let _ = std::fs::remove_file(&path);
    }
}
