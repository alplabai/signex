/// Selects the ordering of the transfer terms in a two-port record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TwoPortDataOrder {
    S21S12,
    S12S21,
}
