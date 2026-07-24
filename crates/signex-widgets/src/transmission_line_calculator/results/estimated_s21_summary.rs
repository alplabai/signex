/// Summarizes the estimated peak S21 value and its bandwidth.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EstimatedS21Summary {
    pub(crate) max_db: f64,
    pub(crate) frequency_hz: f64,
    pub(crate) bandwidth_hz: Option<f64>,
}
