/// Selects how frequencies are distributed and positioned on result diagrams.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FrequencyScale {
    #[default]
    Linear,
    Logarithmic,
    NaturalLogarithm,
}

impl FrequencyScale {
    pub const ALL: [Self; 3] = [Self::Linear, Self::Logarithmic, Self::NaturalLogarithm];

    /// Returns the user-facing scale label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::Logarithmic => "Log10",
            Self::NaturalLogarithm => "Ln",
        }
    }

    /// Returns a frequency at the supplied normalized position.
    pub(crate) fn frequency_at(self, start_hz: f64, stop_hz: f64, ratio: f64) -> f64 {
        let transformed_start = self.transform(start_hz);
        let transformed_stop = self.transform(stop_hz);
        self.inverse(transformed_start + ratio * (transformed_stop - transformed_start))
    }

    /// Returns the normalized position of a frequency within the supplied range.
    pub(crate) fn normalize(self, frequency_hz: f64, start_hz: f64, stop_hz: f64) -> f64 {
        let transformed_start = self.transform(start_hz);
        let transformed_stop = self.transform(stop_hz);
        (self.transform(frequency_hz) - transformed_start) / (transformed_stop - transformed_start)
    }

    fn transform(self, frequency_hz: f64) -> f64 {
        match self {
            Self::Linear => frequency_hz,
            Self::Logarithmic => frequency_hz.log10(),
            Self::NaturalLogarithm => frequency_hz.ln(),
        }
    }

    fn inverse(self, value: f64) -> f64 {
        match self {
            Self::Linear => value,
            Self::Logarithmic => 10.0_f64.powf(value),
            Self::NaturalLogarithm => value.exp(),
        }
    }
}
