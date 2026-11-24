//! Inks of the file-kind glyphs in the Add New flyouts (mockup row 20): the
//! Projects tree's kind-tag tones, so a Schematic row reads like a SCH row.

use iced::Color;
use signex_types::theme::ThemeTokens;
use signex_widgets::active_bar_dropdown::DropdownEntry;

use crate::app::Message;
use crate::styles::{PillTone, pill_colors, violet_pair};

/// Schematic info, board ok, library violet: the inks of the tree's SCH,
/// PCB and LIB tags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app::view::context_menu) struct KindInks {
    pub schematic: Color,
    pub pcb: Color,
    pub library: Color,
}

impl KindInks {
    pub(in crate::app::view::context_menu) fn of(tokens: &ThemeTokens) -> Self {
        Self {
            schematic: pill_colors(tokens, PillTone::Info).1,
            pcb: pill_colors(tokens, PillTone::Ok).1,
            library: violet_pair(tokens).1,
        }
    }
}

/// `entry` with its glyph drawn in `ink`; separators pass through.
pub(in crate::app::view::context_menu) fn kind_tinted(
    entry: DropdownEntry<Message>,
    ink: Color,
) -> DropdownEntry<Message> {
    match entry {
        DropdownEntry::Item(item) => DropdownEntry::Item(item.icon_tint(ink)),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::view::context_menu::project_tree::library::library_add_new_entries;
    use crate::app::view::context_menu::submenu::add_new_entries;
    use signex_types::theme::{ThemeId, theme_tokens};
    use signex_widgets::tree_view::BadgeTone;

    /// The glyph ink of the row labelled `label`.
    fn tint_of(rows: &[DropdownEntry<Message>], label: &str) -> Option<Color> {
        rows.iter()
            .find_map(|row| match row {
                DropdownEntry::Item(item) if item.label == label => Some(item.icon_tint),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing row {label:?}"))
    }

    /// The ink of the tree's kind tag in `tone`.
    fn tag_ink(tokens: &ThemeTokens, tone: BadgeTone) -> Option<Color> {
        let style = crate::panels::graphite::tree_style(tokens);
        style.tone_colors(tone).map(|(_, ink)| ink)
    }

    /// Mockup row 20: Add New to Project draws each live row's glyph in the
    /// ink of the tree's tag for that kind; the planned rows stay muted.
    #[test]
    fn add_new_to_project_glyphs_take_the_tree_kind_tag_inks() {
        for tid in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(tid);
            let i18n = signex_i18n::I18n::en_us_ref();
            let rows = add_new_entries(i18n, tid, KindInks::of(&tokens), vec!["p".into()]);
            let info = tag_ink(&tokens, BadgeTone::Info);
            let violet = tag_ink(&tokens, BadgeTone::Violet);
            assert_eq!(tint_of(&rows, "Schematic"), info, "{tid:?}");
            assert_eq!(
                tint_of(&rows, "PCB"),
                tag_ink(&tokens, BadgeTone::Ok),
                "{tid:?}"
            );
            assert_eq!(tint_of(&rows, "Component Library"), violet, "{tid:?}");
            assert_eq!(tint_of(&rows, "Symbol Library"), violet, "{tid:?}");
            for planned in [
                "Design Notebook",
                "Export Job",
                "Constraint File",
                "VHDL File",
            ] {
                assert_eq!(tint_of(&rows, planned), None, "{tid:?} {planned}");
            }
        }
    }

    /// Mockup row 20: the library's Add New flyout draws its glyphs in the
    /// library ink, the same way as the project's flyout.
    #[test]
    fn the_library_add_new_glyphs_take_the_library_ink() {
        for tid in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(tid);
            let i18n = signex_i18n::I18n::en_us_ref();
            let rows = library_add_new_entries(i18n, tid, KindInks::of(&tokens));
            let violet = tag_ink(&tokens, BadgeTone::Violet);
            assert!(violet.is_some());
            for row in &rows {
                if let DropdownEntry::Item(item) = row {
                    assert_eq!(item.icon_tint, violet, "{tid:?} {}", item.label);
                }
            }
        }
    }

    /// The three kinds read apart: no two share an ink.
    #[test]
    fn the_kind_inks_differ() {
        for tid in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let inks = KindInks::of(&theme_tokens(tid));
            assert_ne!(inks.schematic, inks.pcb, "{tid:?}");
            assert_ne!(inks.schematic, inks.library, "{tid:?}");
            assert_ne!(inks.pcb, inks.library, "{tid:?}");
        }
    }
}
