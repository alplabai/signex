use crate::transmission_line_calculator::ScalarUnit;

use super::ResultDiagramKind;

pub(crate) const CSV_FREQUENCY_UNITS: [ScalarUnit; 4] = [
    ScalarUnit::Hertz,
    ScalarUnit::KiloHertz,
    ScalarUnit::MegaHertz,
    ScalarUnit::GigaHertz,
];

/// Stores validated user input for a frequency-domain CSV export.
#[derive(Debug, Clone)]
pub(crate) struct CsvExportConfiguration {
    pub(crate) kind: ResultDiagramKind,
    pub(crate) start_frequency_unit: ScalarUnit,
    pub(crate) stop_frequency_unit: ScalarUnit,
    pub(crate) output_frequency_unit: ScalarUnit,
    pub(crate) start_frequency: String,
    pub(crate) stop_frequency: String,
    pub(crate) samples: String,
    pub(crate) error: Option<String>,
}

impl CsvExportConfiguration {
    /// Creates an export configuration with display-ready frequency values.
    pub(crate) fn new(
        kind: ResultDiagramKind,
        start_frequency_hz: f64,
        stop_frequency_hz: f64,
        samples: usize,
    ) -> Self {
        Self {
            kind,
            start_frequency_unit: ScalarUnit::Hertz,
            stop_frequency_unit: ScalarUnit::Hertz,
            output_frequency_unit: ScalarUnit::Hertz,
            start_frequency: format_frequency(start_frequency_hz),
            stop_frequency: format_frequency(stop_frequency_hz),
            samples: samples.to_string(),
            error: None,
        }
    }

    /// Changes the start-frequency unit while preserving its physical value.
    pub(crate) fn set_start_frequency_unit(&mut self, frequency_unit: ScalarUnit) {
        if self.start_frequency_unit == frequency_unit {
            return;
        }

        let old_multiplier = self.start_frequency_unit.multiplier();
        let new_multiplier = frequency_unit.multiplier();
        convert_frequency(&mut self.start_frequency, old_multiplier, new_multiplier);
        self.start_frequency_unit = frequency_unit;
        self.error = None;
    }

    /// Changes the stop-frequency unit while preserving its physical value.
    pub(crate) fn set_stop_frequency_unit(&mut self, frequency_unit: ScalarUnit) {
        if self.stop_frequency_unit == frequency_unit {
            return;
        }

        let old_multiplier = self.stop_frequency_unit.multiplier();
        let new_multiplier = frequency_unit.multiplier();
        convert_frequency(&mut self.stop_frequency, old_multiplier, new_multiplier);
        self.stop_frequency_unit = frequency_unit;
        self.error = None;
    }

    /// Changes the frequency unit written to the CSV file.
    pub(crate) fn set_output_frequency_unit(&mut self, frequency_unit: ScalarUnit) {
        self.output_frequency_unit = frequency_unit;
        self.error = None;
    }
}

/// Formats frequency for display or serialization.
fn format_frequency(value: f64) -> String {
    let formatted = format!("{value:.12}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// Converts a valid frequency field between engineering units.
fn convert_frequency(value: &mut String, old_multiplier: f64, new_multiplier: f64) {
    let Ok(parsed) = value.trim().parse::<f64>() else {
        return;
    };
    if !parsed.is_finite() {
        return;
    }

    *value = format_frequency(parsed * old_multiplier / new_multiplier);
}
