#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FormulaReference {
    pub(crate) item: &'static str,
    pub(crate) latex: &'static str,
    pub(crate) equation: &'static str,
    pub(crate) notes: &'static str,
}
