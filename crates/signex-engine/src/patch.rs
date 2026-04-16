use signex_types::schematic::{SelectedItem, SelectedKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticPatch {
    DocumentReplaced,
    TextUpdated,
    LabelsMutated,
    SymbolFieldsUpdated,
    SelectionDeleted,
    SelectionMoved,
    SelectionRotated,
    SelectionMirrored,
    ObjectPlaced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentPatch(u16);

impl DocumentPatch {
    pub const NONE: Self = Self(0);
    pub const SYMBOLS: Self = Self(1 << 0);
    pub const WIRES: Self = Self(1 << 1);
    pub const LABELS: Self = Self(1 << 2);
    pub const TEXT_NOTES: Self = Self(1 << 3);
    pub const BUSES: Self = Self(1 << 4);
    pub const BUS_ENTRIES: Self = Self(1 << 5);
    pub const JUNCTIONS: Self = Self(1 << 6);
    pub const NO_CONNECTS: Self = Self(1 << 7);
    pub const CHILD_SHEETS: Self = Self(1 << 8);
    pub const DRAWINGS: Self = Self(1 << 9);
    pub const LIB_SYMBOLS: Self = Self(1 << 10);
    pub const PAPER: Self = Self(1 << 11);
    pub const FULL: Self = Self(u16::MAX);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    pub fn from_selected_kind(kind: SelectedKind) -> Self {
        match kind {
            SelectedKind::Symbol
            | SelectedKind::SymbolRefField
            | SelectedKind::SymbolValField => Self::SYMBOLS,
            SelectedKind::Wire => Self::WIRES,
            SelectedKind::Bus => Self::BUSES,
            SelectedKind::BusEntry => Self::BUS_ENTRIES,
            SelectedKind::Junction => Self::JUNCTIONS,
            SelectedKind::NoConnect => Self::NO_CONNECTS,
            SelectedKind::Label => Self::LABELS,
            SelectedKind::TextNote => Self::TEXT_NOTES,
            SelectedKind::ChildSheet => Self::CHILD_SHEETS,
            SelectedKind::Drawing => Self::DRAWINGS,
        }
    }

    pub fn from_selected_items(items: &[SelectedItem]) -> Self {
        let mut patch = Self::NONE;
        for item in items {
            patch |= Self::from_selected_kind(item.kind);
        }

        if patch == Self::NONE {
            Self::FULL
        } else {
            patch
        }
    }
}

impl std::ops::BitOr for DocumentPatch {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for DocumentPatch {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchPair {
    pub semantic: SemanticPatch,
    pub document: DocumentPatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub changed: bool,
    pub patch_pair: Option<PatchPair>,
}

impl CommandResult {
    pub fn changed(patch_pair: PatchPair) -> Self {
        Self {
            changed: true,
            patch_pair: Some(patch_pair),
        }
    }

    pub fn unchanged() -> Self {
        Self {
            changed: false,
            patch_pair: None,
        }
    }
}