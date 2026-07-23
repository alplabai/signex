/// Identifies an editable value field in a circuit component row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitComponentField {
    Primary,
    Secondary,
    Tertiary,
    Tolerance,
}
