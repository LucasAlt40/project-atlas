//! Telling "the agent is waiting for a person" apart from "the agent finished".
//!
//! Detection goes from the most to the least reliable signal and stops at the first that
//! speaks:
//!
//! 1. a structured signal the runtime adapter reports itself ([`crate::application::runtimes::ModelRuntime::detect_interaction`]);
//! 2. the `atlas-interaction` block an agent is asked to end its answer with when it cannot go on
//!    ([`structured`]), which works with every runtime;
//! 3. a conservative text analysis of the end of the answer ([`heuristic`]).
//!
//! No model is asked to classify output: that fallback is deliberately not built (it would cost
//! a call per execution and its answer would be one more piece of untrusted text). A detection
//! below [`MIN_CONFIDENCE`] never pauses anything.
//!
//! Everything read here is untrusted. A detection can only *pause* a step so a person looks; it
//! cannot approve, allow or change anything.

mod heuristic;
mod structured;

use crate::domain::interaction::{DetectionSource, InteractionDetection};

pub use structured::INTERACTION_FENCE;

/// Below this a detection is ignored: pausing a workflow for a rhetorical question is worse
/// than letting a doubtful one through to the result contract.
pub const MIN_CONFIDENCE: u8 = 70;

/// What an execution produced that the detector may look at.
#[derive(Debug, Clone, Copy)]
pub struct InteractionSignals<'a> {
    /// The runtime's final answer.
    pub text: &'a str,
}

/// Port: decides whether an output is a request for a person.
pub trait InteractionDetector: Send + Sync {
    fn detect(&self, signals: &InteractionSignals<'_>) -> InteractionDetection;
}

/// Structured block first, text analysis after.
#[derive(Debug, Default, Clone, Copy)]
pub struct LayeredDetector;

impl InteractionDetector for LayeredDetector {
    fn detect(&self, signals: &InteractionSignals<'_>) -> InteractionDetection {
        match structured::detect(signals.text) {
            structured::Block::Interaction(found) => found,
            // The agent ended with a result: it finished, whatever its text asks.
            structured::Block::Concluded => InteractionDetection::none(DetectionSource::Structured),
            structured::Block::Absent => heuristic::detect(signals.text),
        }
    }
}

/// A step ended without a valid result and its message ends on a question: that question is
/// for the person, even if it is not worded as one the strict detector knows. `None` when the
/// message does not end on a question.
pub fn detect_unfinished_question(text: &str) -> Option<InteractionDetection> {
    let found = heuristic::detect_unfinished(text);
    (found.detected && found.kind.is_some()).then_some(found)
}

/// Whether the detection is reliable enough to pause for.
pub fn should_pause(detection: &InteractionDetection) -> bool {
    detection.detected && detection.kind.is_some() && detection.confidence >= MIN_CONFIDENCE
}

/// Shortens text on a character boundary.
pub(crate) fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut clipped: String = text.chars().take(max.saturating_sub(1)).collect();
    clipped.push('…');
    clipped
}

#[cfg(test)]
mod tests;
