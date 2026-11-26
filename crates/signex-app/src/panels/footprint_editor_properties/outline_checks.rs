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

use crate::library::editor::footprint::{CheckStatus, GapOffer, format_check_mm};
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
        col = push_check_lines(col, board, palette, i18n, true);
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

/// One line per check, a click on a failing line selecting its place, then
/// Add fillets when a corner can take one (mockup row 179d). The Bake panel
/// shows Close gap with its refusal, so it passes `close_gap` false.
pub(in crate::panels::footprint_editor_properties) fn push_check_lines<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    board: &'a BoardSketchProps,
    palette: PanelPalette,
    i18n: &I18n,
    close_gap: bool,
) -> Column<Element<'a, PanelMsg>> {
    for line in &board.outline_checks {
        let select = line
            .place
            .map(|id| PanelMsg::FpEditorSelectSketchEntity { id });
        col = col.push(status_line(line.status, line.text.localized(i18n), select));
        if let Some(offer) = line.close_gap.as_ref().filter(|_| close_gap) {
            col = col.push(close_gap_row(offer, palette, i18n));
        }
    }
    match board.fillet_radius {
        Some(radius) => col.push(add_fillets_row(radius, i18n)),
        None => col,
    }
}

/// Whether a check of `board` fails, so Properties lifts the section to the
/// top where the failure shows without scrolling (mockup row 179d).
pub(in crate::panels::footprint_editor_properties) fn has_failure(
    board: &BoardSketchProps,
) -> bool {
    board
        .outline_checks
        .iter()
        .any(|line| line.status == CheckStatus::Failed)
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
/// project's rules were seeded from, the router tool and the min slot, each
/// or that the project names none.
fn fab_profile_lines(board: &BoardSketchProps, i18n: &I18n) -> [Localized; 3] {
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
    let min_slot = match board.min_slot {
        Some(width) => {
            t!(i18n, ids::BOARD_SKETCH_FAB_MIN_SLOT, "width" => format_check_mm(width, 2))
        }
        None => t!(i18n, ids::BOARD_SKETCH_FAB_MIN_SLOT_NONE),
    };
    [profile, router, min_slot]
}

/// The Fab profile group (mockup row 179d): the project's profile, its
/// router tool and its min slot.
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
    use iced::Widget as _;
    use signex_bake::board::{CheckState, Finding, NotChecked, OutlineChecks};
    use signex_sketch::id::SketchEntityId;

    use super::*;
    use crate::app::Signex;
    use crate::layout_probe::lay_out_texts;
    use crate::library::editor::footprint::check_lines;

    const DOCK: iced::Size = iced::Size::new(300.0, 4000.0);

    fn texts(board: &BoardSketchProps) -> Vec<String> {
        let (app, _task) = Signex::new();
        let ctx = &app.document_view.panel_ctx;
        let i18n = I18n::en_us();
        let palette = PanelPalette::from_tokens(&ctx.tokens);
        let col = view_outline_checks(
            Column::new(),
            board,
            palette,
            &ctx.collapsed_sections,
            &i18n,
        );
        lay_out_texts(col.boxed(), DOCK)
            .into_iter()
            .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    fn checks(min_slot: CheckState) -> OutlineChecks {
        let no_outline = || CheckState::NotChecked(NotChecked::NoOutline);
        OutlineChecks {
            closed: no_outline(),
            no_crossing: no_outline(),
            openings_inside: no_outline(),
            inside_corners: no_outline(),
            min_slot,
        }
    }

    #[test]
    fn a_narrow_slot_is_listed_with_the_other_checks_and_the_fab_group_names_the_min_slot() {
        let slot = SketchEntityId::new();
        let found = Finding::NarrowSlot {
            entity: slot,
            width_mm: 0.6,
            min_slot_mm: 0.8,
        };
        let lines = check_lines(Some(&checks(CheckState::Failed(vec![found]))));
        assert_eq!(lines.len(), 5, "one line per check");
        assert_eq!(lines[4].place, Some(slot), "a click selects the slot");
        let board = BoardSketchProps {
            outline_checks: lines,
            router_tool_diameter: Some(2.0),
            min_slot: Some(0.8),
            ..BoardSketchProps::default()
        };
        let texts = texts(&board);
        let has = |label: &str| texts.iter().any(|t| t == label);
        assert!(has("Slot 0.60 mm wide < min slot 0.80 mm"), "{texts:#?}");
        assert!(has("Router tool Ø 2.00 · from profile"), "{texts:#?}");
        assert!(has("Min slot Ø 0.80 · from profile"), "{texts:#?}");
        let router = texts.iter().position(|t| t.starts_with("Router tool"));
        let min_slot = texts.iter().position(|t| t.starts_with("Min slot Ø"));
        assert!(
            router < min_slot,
            "Min slot follows Router tool: {texts:#?}"
        );
    }

    #[test]
    fn without_a_min_slot_the_check_and_the_fab_group_say_so() {
        let lines = check_lines(Some(&checks(CheckState::NotChecked(NotChecked::NoMinSlot))));
        let board = BoardSketchProps {
            outline_checks: lines,
            ..BoardSketchProps::default()
        };
        let texts = texts(&board);
        let has = |label: &str| texts.iter().any(|t| t == label);
        assert!(
            has("Min slot: not checked, the project's fab profile names no min slot"),
            "{texts:#?}"
        );
        assert!(
            has("Min slot: the project's fab profile names none"),
            "{texts:#?}"
        );
    }

    #[test]
    fn a_passing_min_slot_check_reads_ok() {
        let lines = check_lines(Some(&checks(CheckState::Passed)));
        let board = BoardSketchProps {
            outline_checks: lines,
            min_slot: Some(0.5),
            ..BoardSketchProps::default()
        };
        let texts = texts(&board);
        assert!(
            texts
                .iter()
                .any(|t| t == "Slots: none narrower than the min slot"),
            "{texts:#?}"
        );
    }

    /// Mockup row 179d (#2903): with a named fab profile the min slot and
    /// inside-corner lines say which profile they ran against; the other
    /// checks read as before.
    #[test]
    fn the_fab_checks_name_the_profile_they_ran_against() {
        use crate::library::editor::footprint::check_lines_against;
        use signex_i18n::RenderTranslatable;
        let english = |lines: Vec<crate::library::editor::footprint::CheckLine>| -> Vec<String> {
            lines
                .iter()
                .map(|line| {
                    line.text
                        .render_en_us()
                        .replace(['\u{2068}', '\u{2069}'], "")
                })
                .collect()
        };
        let passed = english(check_lines_against(
            Some(&checks(CheckState::Passed)),
            Some("Standard"),
        ));
        assert_eq!(passed[4], "Slots: none narrower than the Standard min slot");
        assert_eq!(
            passed[3],
            "Inside corners: not checked, the sketch draws no outline (the board keeps its own)"
        );
        let skipped = english(check_lines_against(
            Some(&checks(CheckState::NotChecked(NotChecked::NoMinSlot))),
            Some("Standard"),
        ));
        assert_eq!(
            skipped[4],
            "Min slot: not checked, Standard names no min slot"
        );
        let unnamed = english(check_lines(Some(&checks(CheckState::Passed))));
        assert_eq!(unnamed[4], "Slots: none narrower than the min slot");
    }

    fn lines(board: &BoardSketchProps) -> Vec<String> {
        fab_profile_lines(board, &I18n::en_us())
            .into_iter()
            .map(|line| line.into_string().replace(['\u{2068}', '\u{2069}'], ""))
            .collect()
    }

    /// Mockup row 179d and DECISIONS 2026-11-23: the Fab profile group names
    /// the project's profile above its router tool, and says when there is
    /// none set and where to set one (#2903).
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
        assert_eq!(
            none[0],
            "Profile: none set. Set one in the project's Rules (Presets) to check corners and slots against it."
        );
        assert_eq!(none[1], "Router tool: the project's fab profile names none");
    }
}
