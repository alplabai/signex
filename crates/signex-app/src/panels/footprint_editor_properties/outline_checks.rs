//! The Outline checks section of board sketch mode (mockup row 179d): under the Board
//! summary with nothing selected, one line per check of the current solve,
//! a failing check one line per place. A click on a failing line selects
//! its entity; an open outline says whether its gap heals and offers Close
//! gap when it does (mockup row 190). Every check warns only; Bake still applies the
//! sketch (decision 3, `docs/DECISIONS.md`). The Fab profile group under it
//! names the router tool the inside-corner check uses.

use iced::widget::{Column, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, RenderTranslatable as _, ids, t};
use signex_widgets::{button, text};

use crate::library::editor::footprint::{GapOffer, format_check_mm};
use crate::library::editor::standalone::sketch_exit::status_line;
use crate::panels::{BoardSketchProps, CollapsedSections, PanelMsg, PanelPalette, SketchFixMsg};
use crate::panels::{collapsible_section_header, is_section_collapsed};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The section's key in the collapsed-sections set.
const SECTION: &str = "fp_board_checks";

/// The Fab profile group's key in the collapsed-sections set.
const FAB_SECTION: &str = "fp_board_fab";

pub(super) fn view_outline_checks<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    board: &'a BoardSketchProps,
    palette: PanelPalette,
    collapsed_sections: &'a CollapsedSections,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    col = col.push(
        collapsible_section_header(
            SECTION,
            &t!(i18n, ids::BOARD_SKETCH_CHECKS_SECTION).into_string(),
            collapsed_sections,
            palette.primary,
            palette.border,
        )
        .boxed(),
    );
    if !is_section_collapsed(SECTION, collapsed_sections) {
        for line in &board.outline_checks {
            let select = line
                .place
                .map(|id| PanelMsg::FpEditorSelectSketchEntity { id });
            col = col.push(status_line(line.status, line.text.localized(i18n), select));
            if let Some(offer) = &line.close_gap {
                col = col.push(close_gap_row(offer, palette, i18n));
            }
        }
        if let Some(radius) = board.fillet_radius {
            col = col.push(add_fillets_row(radius, i18n));
        }
        col = col.push(
            container(
                text(t!(i18n, ids::BOARD_SKETCH_CHECKS_CAPTION))
                    .size(TS.caption)
                    .color(palette.muted),
            )
            .padding([SP.s, SP.m])
            .width(Length::Fill)
            .boxed(),
        );
    }
    view_fab_profile(col, board, palette, collapsed_sections, i18n)
}

/// Add fillets on the failing inside corners (mockup row 179d).
fn add_fillets_row<'a>(radius: f64, i18n: &I18n) -> Element<'a, PanelMsg> {
    let label = t!(i18n, ids::SKETCH_BAKE_ADD_FILLETS, "radius" => format_check_mm(radius, 2));
    container(
        button(text(label).size(TS.caption))
            .padding([SP.xs, SP.s])
            .on_press(PanelMsg::SketchFix(SketchFixMsg::AddFilletsClicked))
            .style(crate::panels::form_look::ghost_button),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// Whether the gap heals, and Close gap when it does.
fn close_gap_row<'a>(
    offer: &GapOffer,
    palette: PanelPalette,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let mut line = row![
        text(offer.text().localized(i18n))
            .size(TS.caption)
            .color(palette.muted)
            .width(Length::Fill)
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    if offer.closes() {
        line = line.push(
            button(text(t!(i18n, ids::SKETCH_BAKE_CLOSE_GAP)).size(TS.caption))
                .padding([SP.xs, SP.s])
                .on_press(PanelMsg::SketchFix(SketchFixMsg::CloseGapClicked {
                    keep: offer.keep,
                    merge: offer.merge,
                }))
                .style(crate::panels::form_look::ghost_button)
                .boxed(),
        );
    }
    container(line)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// The Fab profile group's lines (mockup row 179d): the profile the
/// project's rules were seeded from and the router tool, each or that the
/// project names none. Min slot waits for its rules field (DECISIONS
/// 2026-11-24, #2843).
fn fab_profile_lines(board: &BoardSketchProps, i18n: &I18n) -> [Localized; 2] {
    let profile = match board.fab_profile.as_ref() {
        Some(name) => t!(i18n, ids::BOARD_SKETCH_FAB_PROFILE, "name" => name.clone()),
        None => t!(i18n, ids::BOARD_SKETCH_FAB_PROFILE_NONE),
    };
    let router = match board.router_tool_diameter {
        Some(diameter) => {
            t!(i18n, ids::BOARD_SKETCH_FAB_ROUTER, "diameter" => format_check_mm(diameter, 2))
        }
        None => t!(i18n, ids::BOARD_SKETCH_FAB_ROUTER_NONE),
    };
    [profile, router]
}

/// The Fab profile group (mockup row 179d): the project's profile and its
/// router tool.
fn view_fab_profile<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    board: &'a BoardSketchProps,
    palette: PanelPalette,
    collapsed_sections: &'a CollapsedSections,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    col = col.push(
        collapsible_section_header(
            FAB_SECTION,
            &t!(i18n, ids::BOARD_SKETCH_FAB_SECTION).into_string(),
            collapsed_sections,
            palette.primary,
            palette.border,
        )
        .boxed(),
    );
    if is_section_collapsed(FAB_SECTION, collapsed_sections) {
        return col;
    }
    for line in fab_profile_lines(board, i18n) {
        col = col.push(
            container(text(line).size(TS.caption).color(palette.primary))
                .padding([SP.xs, SP.m])
                .width(Length::Fill)
                .boxed(),
        );
    }
    col
}

#[cfg(test)]
mod tests {
    use signex_i18n::I18n;

    use super::fab_profile_lines;
    use crate::panels::BoardSketchProps;

    fn lines(board: &BoardSketchProps) -> Vec<String> {
        fab_profile_lines(board, &I18n::en_us())
            .into_iter()
            .map(|line| line.into_string().replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    /// Mockup row 179d and DECISIONS 2026-11-23: the Fab profile group names
    /// the project's profile above its router tool, and says when there is
    /// none.
    #[test]
    fn the_fab_profile_group_names_the_profile_above_the_router_tool() {
        let board = BoardSketchProps {
            fab_profile: Some("Standard 4-layer".into()),
            router_tool_diameter: Some(2.0),
            ..BoardSketchProps::default()
        };
        let shown = lines(&board);
        assert_eq!(shown[0], "Profile Standard 4-layer");
        assert!(shown[1].starts_with("Router tool Ø 2.00"), "{shown:?}");
        let none = lines(&BoardSketchProps::default());
        assert_eq!(none[0], "Profile: the project's rules name none");
        assert_eq!(none[1], "Router tool: the project's fab profile names none");
    }
}
