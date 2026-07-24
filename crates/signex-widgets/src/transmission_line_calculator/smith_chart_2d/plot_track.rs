use iced::Color;

/// Stores a labelled frequency-domain plot series and its line color.
#[derive(Debug, Clone)]
pub(crate) struct PlotTrack {
    pub(crate) label: String,
    pub(crate) points: Vec<(f64, f64)>,
    pub(crate) color: Color,
}
