use super::{MatrixFormat, TwoPortDataOrder};

/// Stores the keyword metadata preceding Version 2.x network data.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Version2Header {
    pub(super) port_count: usize,
    pub(super) frequency_count: usize,
    pub(super) noise_frequency_count: Option<usize>,
    pub(super) reference_impedances_ohm: Vec<f64>,
    pub(super) matrix_format: MatrixFormat,
    pub(super) two_port_data_order: Option<TwoPortDataOrder>,
}
