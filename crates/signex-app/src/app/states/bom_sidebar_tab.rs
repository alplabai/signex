/// Which sidebar tab is currently shown inside the BOM preview's
/// Properties panel — Altium-style General / Columns split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BomSidebarTab {
    General,
    Columns,
}
