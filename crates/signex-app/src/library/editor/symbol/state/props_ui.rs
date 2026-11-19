//! What the symbol editor's Properties panel shows besides the symbol: its
//! tab, the Pins tab filter and part chip, and the illustration hover
//! (#122, #123). Session state of one editor; never saved, never undone.

use signex_types::schematic::BodyStyle;
use signex_widgets::property_illustration::{IllustrationHover, IllustrationMsg};

/// The tab the Properties panel shows while nothing is selected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SymbolPropsTab {
    #[default]
    Symbol,
    Pins,
}

/// Which pins the Pins tab lists: every pin, or the pins of one part
/// (`0` is the shared part).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PartChip {
    #[default]
    All,
    Part(u8),
}

/// A press on the Parts stepper of the Symbol tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartStep {
    Previous,
    Next,
    Added,
    Removed,
}

/// What the user did in the symbol editor's Properties panel besides an
/// edit of a field.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolPropsMsg {
    TabPicked(SymbolPropsTab),
    PinFilterChanged(String),
    PartChipPicked(PartChip),
    /// `‹ Pins` over the Pin properties: clear the selection and show the
    /// Pins tab.
    PinsBackPressed,
    /// `+ Pin` on the Pins tab: arm the Add Pin tool.
    AddPinPressed,
    PartStepPressed(PartStep),
    BodyPicked(BodyStyle),
    IllustrationHovered(IllustrationMsg),
}

/// The panel state of one symbol editor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SymbolPropsUi {
    pub tab: SymbolPropsTab,
    pub pin_filter: String,
    pub part_chip: PartChip,
    pub hover: IllustrationHover,
}

impl SymbolPropsUi {
    /// Apply what changes this state. `false` when the message also acts on
    /// the editor (its selection, tool, part or body), which the caller does.
    pub fn apply(&mut self, msg: &SymbolPropsMsg) -> bool {
        match msg {
            SymbolPropsMsg::TabPicked(tab) => self.tab = *tab,
            SymbolPropsMsg::PinFilterChanged(query) => self.pin_filter.clone_from(query),
            SymbolPropsMsg::PartChipPicked(chip) => self.part_chip = *chip,
            SymbolPropsMsg::IllustrationHovered(hover) => self.hover.update(hover.clone()),
            SymbolPropsMsg::PinsBackPressed => {
                self.tab = SymbolPropsTab::Pins;
                return false;
            }
            SymbolPropsMsg::AddPinPressed
            | SymbolPropsMsg::PartStepPressed(_)
            | SymbolPropsMsg::BodyPicked(_) => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_widgets::property_illustration::FieldId;

    #[test]
    fn the_tab_filter_and_chip_are_panel_state() {
        let mut ui = SymbolPropsUi::default();
        assert_eq!(ui.tab, SymbolPropsTab::Symbol);
        assert!(ui.apply(&SymbolPropsMsg::TabPicked(SymbolPropsTab::Pins)));
        assert!(ui.apply(&SymbolPropsMsg::PinFilterChanged("in".into())));
        assert!(ui.apply(&SymbolPropsMsg::PartChipPicked(PartChip::Part(2))));
        assert_eq!(ui.tab, SymbolPropsTab::Pins);
        assert_eq!(ui.pin_filter, "in");
        assert_eq!(ui.part_chip, PartChip::Part(2));
    }

    #[test]
    fn pins_back_shows_the_pins_tab_and_leaves_the_selection_to_the_editor() {
        let mut ui = SymbolPropsUi::default();
        assert!(!ui.apply(&SymbolPropsMsg::PinsBackPressed));
        assert_eq!(ui.tab, SymbolPropsTab::Pins);
    }

    #[test]
    fn editor_actions_are_left_to_the_caller() {
        let mut ui = SymbolPropsUi::default();
        for msg in [
            SymbolPropsMsg::AddPinPressed,
            SymbolPropsMsg::PartStepPressed(PartStep::Next),
            SymbolPropsMsg::BodyPicked(BodyStyle::Alternate),
        ] {
            assert!(!ui.apply(&msg), "{msg:?}");
        }
        assert_eq!(ui, SymbolPropsUi::default());
    }

    #[test]
    fn a_field_hover_is_kept_until_its_row_is_left() {
        const LENGTH: FieldId = FieldId::new("pin.len");
        let mut ui = SymbolPropsUi::default();
        ui.apply(&SymbolPropsMsg::IllustrationHovered(
            IllustrationMsg::FieldHovered(LENGTH),
        ));
        assert!(ui.hover.is_field_highlighted(&LENGTH));
        ui.apply(&SymbolPropsMsg::IllustrationHovered(
            IllustrationMsg::FieldLeft(LENGTH),
        ));
        assert_eq!(ui.hover.hovered(), None);
    }
}
