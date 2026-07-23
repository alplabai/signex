use super::ResultDiagramKind;

/// Stores validated user input for a frequency-domain CSV export.
#[derive(Debug, Clone)]
pub(crate) struct CsvExportConfiguration {
    pub(crate) kind: ResultDiagramKind,
    pub(crate) start_frequency_mhz: String,
    pub(crate) stop_frequency_mhz: String,
    pub(crate) samples: String,
    pub(crate) error: Option<String>,
}

impl CsvExportConfiguration {
    /// Creates an export configuration with display-ready frequency values.
    pub(crate) fn new(
        kind: ResultDiagramKind,
        start_frequency_mhz: f64,
        stop_frequency_mhz: f64,
        samples: usize,
    ) -> Self {
        Self {
            kind,
            start_frequency_mhz: format_frequency(start_frequency_mhz),
            stop_frequency_mhz: format_frequency(stop_frequency_mhz),
            samples: samples.to_string(),
            error: None,
        }
    }
}

/// Formats frequency for display or serialization.
fn format_frequency(value: f64) -> String {
    let formatted = format!("{value:.6}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}
