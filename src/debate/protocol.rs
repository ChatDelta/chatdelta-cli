//! Debate protocol definitions and turn sequencing.
//!
//! A protocol determines the ordered sequence of turns for a debate.
//! New protocols can be added by implementing `turn_sequence` for a new variant.

use super::types::{DebateProtocol, ParticipantRole, TurnStage};

/// Specification for a single debate turn (who speaks and in what stage)
#[derive(Debug, Clone)]
pub struct TurnSpec {
    pub role: ParticipantRole,
    pub stage: TurnStage,
}

/// Generate the ordered sequence of turns for a given protocol and round count.
///
/// `rounds` is the number of rebuttal pairs after the initial opening + response.
/// Each pair consists of one rebuttal from Model A and one from Model B.
/// The moderator report is always the final turn.
pub fn turn_sequence(protocol: &DebateProtocol, rounds: u32) -> Vec<TurnSpec> {
    match protocol {
        DebateProtocol::ModeratedDebate => moderated_debate_sequence(rounds),
    }
}

/// moderated-debate sequence:
///   Opening (A) → Response (B) → [Rebuttal A, Rebuttal B] × rounds → Moderator
fn moderated_debate_sequence(rounds: u32) -> Vec<TurnSpec> {
    let mut turns = vec![
        TurnSpec {
            role: ParticipantRole::ModelA,
            stage: TurnStage::Opening,
        },
        TurnSpec {
            role: ParticipantRole::ModelB,
            stage: TurnStage::InitialResponse,
        },
    ];

    for r in 1..=rounds {
        turns.push(TurnSpec {
            role: ParticipantRole::ModelA,
            stage: TurnStage::Rebuttal(r),
        });
        turns.push(TurnSpec {
            role: ParticipantRole::ModelB,
            stage: TurnStage::Rebuttal(r),
        });
    }

    turns.push(TurnSpec {
        role: ParticipantRole::Moderator,
        stage: TurnStage::ModeratorReport,
    });

    turns
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_rounds_zero() {
        let turns = turn_sequence(&DebateProtocol::ModeratedDebate, 0);
        // Opening, Response, Moderator
        assert_eq!(turns.len(), 3);
        assert_eq!(turns[0].role, ParticipantRole::ModelA);
        assert_eq!(turns[0].stage, TurnStage::Opening);
        assert_eq!(turns[1].role, ParticipantRole::ModelB);
        assert_eq!(turns[1].stage, TurnStage::InitialResponse);
        assert_eq!(turns[2].role, ParticipantRole::Moderator);
        assert_eq!(turns[2].stage, TurnStage::ModeratorReport);
    }

    #[test]
    fn sequence_rounds_one() {
        let turns = turn_sequence(&DebateProtocol::ModeratedDebate, 1);
        // Opening, Response, Rebuttal-A-1, Rebuttal-B-1, Moderator
        assert_eq!(turns.len(), 5);
        assert_eq!(turns[2].role, ParticipantRole::ModelA);
        assert_eq!(turns[2].stage, TurnStage::Rebuttal(1));
        assert_eq!(turns[3].role, ParticipantRole::ModelB);
        assert_eq!(turns[3].stage, TurnStage::Rebuttal(1));
    }

    #[test]
    fn sequence_rounds_two() {
        let turns = turn_sequence(&DebateProtocol::ModeratedDebate, 2);
        // Opening, Response, R-A-1, R-B-1, R-A-2, R-B-2, Moderator
        assert_eq!(turns.len(), 7);
        assert_eq!(turns[4].stage, TurnStage::Rebuttal(2));
        assert_eq!(turns[4].role, ParticipantRole::ModelA);
        assert_eq!(turns[5].stage, TurnStage::Rebuttal(2));
        assert_eq!(turns[5].role, ParticipantRole::ModelB);
    }

    #[test]
    fn moderator_always_last() {
        for rounds in 0..=4 {
            let turns = turn_sequence(&DebateProtocol::ModeratedDebate, rounds);
            let last = turns.last().unwrap();
            assert_eq!(last.role, ParticipantRole::Moderator);
            assert_eq!(last.stage, TurnStage::ModeratorReport);
        }
    }

    #[test]
    fn turn_count_formula() {
        // total = 2 (base) + rounds * 2 (rebuttal pairs) + 1 (moderator)
        for rounds in 0..=5 {
            let turns = turn_sequence(&DebateProtocol::ModeratedDebate, rounds);
            assert_eq!(turns.len(), (3 + rounds * 2) as usize);
        }
    }
}
