/// Order in which symbols are visited during Annotate. Mirrors Altium's
/// "Order of Processing" drop-down (four variants).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotateOrder {
    /// Top-to-bottom within each column, left-to-right across columns.
    UpThenAcross,
    /// Bottom-to-top within each column, left-to-right across columns.
    DownThenAcross,
    /// Left-to-right within each row, top-to-bottom across rows.
    AcrossThenDown,
    /// Left-to-right within each row, bottom-to-top across rows.
    AcrossThenUp,
}
