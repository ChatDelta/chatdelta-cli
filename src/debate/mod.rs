//! Debate Mode for ChatDelta CLI
//!
//! Provides a structured multi-model deliberation workflow:
//!   1. Model A gives an opening statement
//!   2. Model B responds
//!   3. Alternating rebuttal rounds (configurable)
//!   4. A moderator synthesizes the exchange
//!
//! Entry point from main.rs: call `run_debate(DebateArgs)`.

pub mod orchestrator;
pub mod prompts;
pub mod protocol;
pub mod renderer;
pub mod types;

pub use orchestrator::{resolve_auto_moderator, resolve_client, Orchestrator};
pub use renderer::DebateRenderer;
pub use types::{DebateConfig, DebateProtocol, ModelSpec};
