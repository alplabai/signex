use super::PlotTrack;

/// Supplies a frequency-domain result series to the plotting canvas.
pub(crate) struct FrequencyPlotCanvas {
    pub(crate) tracks: Vec<PlotTrack>,
}
