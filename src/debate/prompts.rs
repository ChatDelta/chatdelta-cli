//! Prompt templates for all debate stages.
//!
//! All prompts embed the proposition and prior context inline — the AiClient
//! trait only exposes `send_prompt`, so there is no system-prompt channel.

/// Opening statement prompt for Model A
pub fn opening_prompt(proposition: &str, max_chars: usize) -> String {
    format!(
        r#"You are participating in a structured moderated debate. Your task is to give an opening statement.

PROPOSITION: {proposition}

Instructions:
- State your position on this proposition clearly in your first sentence.
- Support it with 2-3 specific, well-reasoned arguments.
- Be concrete — use examples or evidence where possible.
- Avoid vague generalizations and rhetorical padding; every sentence must add substance.
- Do not claim certainty about facts you cannot verify; hedge appropriately.
- Keep your response under {max_chars} characters.

Give your opening statement now."#
    )
}

/// Initial response prompt for Model B, given the full transcript context so far
/// (which at this point contains only Model A's opening)
pub fn response_prompt(proposition: &str, context: &str, max_chars: usize) -> String {
    format!(
        r#"You are participating in a structured moderated debate. Your task is to respond to your opponent's opening statement.

PROPOSITION: {proposition}

Debate so far:
{context}

Instructions:
- Engage directly with your opponent's specific arguments — do not ignore or talk past them.
- You may agree with some points while challenging others; nuance is valued.
- Identify the weakest assumptions or unsupported claims in their argument.
- Offer your own position on the proposition with supporting reasoning.
- Do not merely restate the proposition or summarize your opponent; add something new.
- Flag any claims in their statement that appear factually questionable or unverified.
- Keep your response under {max_chars} characters.

Give your response now."#
    )
}

/// Rebuttal prompt for either model, given the full transcript and round context
pub fn rebuttal_prompt(
    proposition: &str,
    context: &str,
    role_label: &str,
    opponent_label: &str,
    round: u32,
    max_chars: usize,
) -> String {
    format!(
        r#"You are participating in a structured moderated debate as {role_label}.

PROPOSITION: {proposition}

Full debate transcript so far:
{context}

Instructions for Rebuttal Round {round}:
- Focus on the single strongest unresolved disagreement between you and {opponent_label}.
- Address your opponent's most recent argument directly — do not ignore it.
- Do NOT repeat arguments you have already made. Build on them, refine them, or concede ground.
- If your opponent raised a valid point, acknowledge it honestly rather than deflecting.
- Stay focused on the original proposition — do not drift into tangents.
- Avoid rhetorical flourish; prioritize substance and specificity.
- Keep your response under {max_chars} characters.

Give your rebuttal now."#
    )
}

/// Moderator synthesis prompt, given the full transcript
pub fn moderator_prompt(
    proposition: &str,
    model_a_label: &str,
    model_b_label: &str,
    context: &str,
) -> String {
    format!(
        r#"You are the moderator of a structured debate. Your role is to analyze the exchange impartially and produce a synthesis report. You are NOT a participant — do not advocate for either side.

PROPOSITION: {proposition}

Model A is: {model_a_label}
Model B is: {model_b_label}

Complete debate transcript:
{context}

Produce a structured report using EXACTLY the following section headers (use ## for each header):

## Strongest Point from Model A
Identify the single most compelling argument made by Model A. Be specific — quote or paraphrase directly.

## Strongest Point from Model B
Identify the single most compelling argument made by Model B. Be specific — quote or paraphrase directly.

## Shared Conclusions
List any points of agreement or convergence that emerged during the debate, even if neither side stated them explicitly.

## Unresolved Disagreements
List the key disagreements that remain unresolved after the debate. For each, briefly explain why it is difficult to resolve (e.g., empirical uncertainty, value differences, definitional disagreement).

## Claims Requiring Verification
List specific factual claims made by either side that should be independently verified before acting on them. For each, note which side made the claim. If none, write "None identified."

## Final Takeaway
Provide a concise (2-3 sentence) synthesis for the human reading this debate. What is the most important thing they should understand?

## Confidence Level
Rate your confidence in this synthesis as High, Medium, or Low. Explain in one sentence why.

Additional moderation notes (include inline within sections as appropriate):
- Flag any places where a participant repeated prior arguments without adding substance.
- Flag any places where a participant avoided directly engaging with the opponent's strongest point.
- Note if either side made claims that appeared unsupported by evidence."#
    )
}
