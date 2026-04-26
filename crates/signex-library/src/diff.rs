//! Visual + parametric diff over two Revisions. Body lands in Phase 1 WS-D.
//! See LIBRARY_PLAN §9.

use crate::component::Revision;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RevisionDiff {
    pub symbol: SymbolDiff,
    pub footprint: FootprintDiff,
    pub parameters: ParameterDiff,
    pub suppliers: SupplierDiff,
    pub lifecycle: LifecycleDiff,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SymbolDiff {
    pub added_pins: Vec<String>,
    pub removed_pins: Vec<String>,
    pub moved_pins: Vec<(String, [f64; 2], [f64; 2])>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FootprintDiff {
    pub added_pads: Vec<String>,
    pub removed_pads: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParameterDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<(String, String, String)>, // key, old, new
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SupplierDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LifecycleDiff {
    pub from: Option<crate::lifecycle::LifecycleState>,
    pub to: Option<crate::lifecycle::LifecycleState>,
}

/// Stub — Phase 1 WS-D ships the real implementation.
pub fn diff_revisions(_a: &Revision, _b: &Revision) -> RevisionDiff {
    RevisionDiff::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_compiles_and_returns_default() {
        // Phase 0 only; real tests in WS-D.
        let d = RevisionDiff::default();
        assert_eq!(d, RevisionDiff::default());
    }
}
