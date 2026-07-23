/// Selects the matrix entries stored for each network frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MatrixFormat {
    Full,
    Lower,
    Upper,
}
