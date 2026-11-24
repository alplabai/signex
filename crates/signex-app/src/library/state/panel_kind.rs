//! Which kind the one Library panel shows (mockup row 119): symbols,
//! footprints or parts. It opens on the kind of the active editor tab; a
//! chip pick holds while the tab it was made on stays active, so no
//! tab-switch hook is needed and `view` stays a function of the state.

use std::path::PathBuf;

use super::PanelRows;

/// The three kinds of the Library panel's chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LibraryPanelKind {
    /// The symbols of the `.snxsym` open in the symbol editor.
    Symbols,
    /// The footprints of the `.snxfpt` open in the footprint editor.
    Footprints,
    /// The component rows of every open library.
    Parts,
}

/// The editor context the panel derives its kind from: the symbol or
/// footprint file of the active editor tab, or neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryPanelTab {
    Symbol(PathBuf),
    Footprint(PathBuf),
    Other,
}

impl LibraryPanelTab {
    /// The kind the panel opens on in this context.
    pub fn default_kind(&self) -> LibraryPanelKind {
        match self {
            Self::Symbol(_) => LibraryPanelKind::Symbols,
            Self::Footprint(_) => LibraryPanelKind::Footprints,
            Self::Other => LibraryPanelKind::Parts,
        }
    }
}

/// A kind the user picked with a chip, and the context it was picked in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryPanelPick {
    pub kind: LibraryPanelKind,
    pub tab: LibraryPanelTab,
}

/// The Library panel's own state: the kind pick and the Parts rows that
/// match the filter.
#[derive(Debug, Clone, Default)]
pub struct LibraryPanelState {
    pub pick: Option<LibraryPanelPick>,
    /// Parts rows matching `LibraryState::panel_search`, rebuilt by
    /// `update` (see `panel_rows`), never by `view`.
    pub rows: PanelRows,
}

impl LibraryPanelState {
    /// The kind the panel shows while `tab` is the active context: the
    /// pick made in that context, else the context's own kind.
    pub fn kind_in(&self, tab: &LibraryPanelTab) -> LibraryPanelKind {
        match &self.pick {
            Some(pick) if &pick.tab == tab => pick.kind,
            _ => tab.default_kind(),
        }
    }

    /// Record a chip pick made while `tab` is active.
    pub fn pick(&mut self, kind: LibraryPanelKind, tab: LibraryPanelTab) {
        self.pick = Some(LibraryPanelPick { kind, tab });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbol() -> LibraryPanelTab {
        LibraryPanelTab::Symbol(PathBuf::from("opamps.snxsym"))
    }

    #[test]
    fn each_context_opens_on_its_own_kind() {
        let state = LibraryPanelState::default();
        assert_eq!(state.kind_in(&symbol()), LibraryPanelKind::Symbols);
        let footprint = LibraryPanelTab::Footprint(PathBuf::from("chip.snxfpt"));
        assert_eq!(state.kind_in(&footprint), LibraryPanelKind::Footprints);
        assert_eq!(
            state.kind_in(&LibraryPanelTab::Other),
            LibraryPanelKind::Parts
        );
    }

    #[test]
    fn a_pick_holds_only_in_the_context_it_was_made_in() {
        let mut state = LibraryPanelState::default();
        state.pick(LibraryPanelKind::Parts, symbol());
        assert_eq!(state.kind_in(&symbol()), LibraryPanelKind::Parts);
        let other_symbol = LibraryPanelTab::Symbol(PathBuf::from("logic.snxsym"));
        assert_eq!(state.kind_in(&other_symbol), LibraryPanelKind::Symbols);
        assert_eq!(
            state.kind_in(&LibraryPanelTab::Other),
            LibraryPanelKind::Parts
        );
        // Back on the tab of the pick, the pick shows again.
        assert_eq!(state.kind_in(&symbol()), LibraryPanelKind::Parts);
    }
}
