//! A number Atlas reports about context, with where it came from and how far it can be trusted.
//!
//! Two separate questions, never merged: **who said it** ([`FigureSource`]) and **how much it is
//! worth as a measurement** ([`Precision`]). A runtime can report a limit that is itself an
//! approximation (`Reported` is not `Exact`), and a number the user typed is not a measurement
//! (`Configured` is not `Exact`). An estimate always says how it was made ([`EstimationMethod`]).

use serde::{Deserialize, Serialize};

/// Who stated a figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FigureSource {
    /// The runtime or provider said so.
    Reported,
    /// The user set it.
    Configured,
    /// Atlas's own rule or method (for example its token estimate).
    Default,
    /// Nobody did: the figure is absent.
    Unknown,
}

/// How much a figure is worth as a measurement. Ordered from the strongest to the weakest claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Precision {
    /// Counted by the model's own tokenizer, or an exact limit.
    Exact,
    /// An approximation. Never to be shown as a count or as the model's official limit.
    Estimated,
    Unknown,
}

impl Precision {
    fn rank(self) -> u8 {
        match self {
            Self::Exact => 2,
            Self::Estimated => 1,
            Self::Unknown => 0,
        }
    }

    /// What can still be claimed about something made from two figures: the weaker of the two.
    #[must_use]
    pub fn weakest(self, other: Self) -> Self {
        if self.rank() <= other.rank() {
            self
        } else {
            other
        }
    }
}

impl From<crate::domain::optimization::TokenSource> for Precision {
    /// `TokenSource` is the phase-0 name for the same axis (tokens only); its stored value
    /// `unavailable` is today's `unknown`.
    fn from(source: crate::domain::optimization::TokenSource) -> Self {
        use crate::domain::optimization::TokenSource;
        match source {
            TokenSource::Exact => Self::Exact,
            TokenSource::Estimated => Self::Estimated,
            TokenSource::Unavailable => Self::Unknown,
        }
    }
}

/// How an estimate was computed, so a better method can replace it without changing readers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstimationMethod {
    /// `ceil(characters / 4)`: a rough average for English prose and code, not calibrated against
    /// any model's tokenizer.
    CharsDiv4,
}

/// A count of tokens (or `None`: not known), with its source, precision and method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Figure {
    pub value: Option<u64>,
    pub source: FigureSource,
    pub precision: Precision,
    /// Set when `precision` is `Estimated`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<EstimationMethod>,
}

impl Figure {
    /// Nothing is known. Never a made-up number.
    pub const fn unknown() -> Self {
        Self {
            value: None,
            source: FigureSource::Unknown,
            precision: Precision::Unknown,
            method: None,
        }
    }

    /// A value the runtime or provider reported, with the precision it claims.
    pub const fn reported(value: u64, precision: Precision) -> Self {
        Self {
            value: Some(value),
            source: FigureSource::Reported,
            precision,
            method: None,
        }
    }

    /// A value the user set. Not a measurement, so its precision is `Unknown`: Atlas cannot say it
    /// matches what the model really accepts.
    pub const fn configured(value: u64) -> Self {
        Self {
            value: Some(value),
            source: FigureSource::Configured,
            precision: Precision::Unknown,
            method: None,
        }
    }

    /// Tokens Atlas estimated (`chars / 4`), already counted.
    pub const fn estimated(tokens: u64) -> Self {
        Self {
            value: Some(tokens),
            source: FigureSource::Default,
            precision: Precision::Estimated,
            method: Some(EstimationMethod::CharsDiv4),
        }
    }

    /// Tokens Atlas estimated from a character count (`chars / 4`).
    pub fn estimated_from_chars(chars: usize) -> Self {
        Self::estimated(crate::domain::optimization::estimate_tokens(chars))
    }

    pub const fn is_known(&self) -> bool {
        self.value.is_some()
    }

    /// A figure no layer may raise: the runtime reported it, or it is exact.
    pub const fn is_ceiling(&self) -> bool {
        self.value.is_some()
            && (matches!(self.source, FigureSource::Reported)
                || matches!(self.precision, Precision::Exact))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_figure_says_who_stated_it_and_how_far_to_trust_it() {
        let estimate = Figure::estimated_from_chars(16_800);
        assert_eq!(estimate.value, Some(4_200));
        assert_eq!(estimate.source, FigureSource::Default);
        assert_eq!(estimate.precision, Precision::Estimated);
        assert_eq!(estimate.method, Some(EstimationMethod::CharsDiv4));

        // Reported is not exact, and configured is never exact.
        let reported = Figure::reported(8_000, Precision::Estimated);
        assert_eq!(reported.source, FigureSource::Reported);
        assert_eq!(reported.precision, Precision::Estimated);
        assert_eq!(Figure::configured(8_000).precision, Precision::Unknown);

        let unknown = Figure::unknown();
        assert_eq!(unknown.value, None);
        assert_eq!(
            (unknown.source, unknown.precision),
            (FigureSource::Unknown, Precision::Unknown)
        );
    }

    #[test]
    fn only_a_reported_or_exact_figure_is_a_ceiling() {
        assert!(Figure::reported(10, Precision::Estimated).is_ceiling());
        assert!(Figure::reported(10, Precision::Exact).is_ceiling());
        let exact_default = Figure {
            precision: Precision::Exact,
            ..Figure::configured(10)
        };
        assert!(exact_default.is_ceiling());
        assert!(!Figure::configured(10).is_ceiling());
        assert!(!Figure::estimated_from_chars(40).is_ceiling());
        assert!(!Figure::unknown().is_ceiling());
    }

    #[test]
    fn combining_two_figures_keeps_the_weaker_claim() {
        use Precision::{Estimated, Exact, Unknown};
        assert_eq!(Exact.weakest(Exact), Exact);
        assert_eq!(Exact.weakest(Estimated), Estimated);
        assert_eq!(Estimated.weakest(Exact), Estimated);
        assert_eq!(Estimated.weakest(Unknown), Unknown);
    }

    #[test]
    fn the_phase_zero_token_source_maps_onto_precision() {
        use crate::domain::optimization::TokenSource;
        assert_eq!(Precision::from(TokenSource::Exact), Precision::Exact);
        assert_eq!(
            Precision::from(TokenSource::Estimated),
            Precision::Estimated
        );
        assert_eq!(
            Precision::from(TokenSource::Unavailable),
            Precision::Unknown
        );
    }
}
