//! The Outline checks section of board sketch mode (mockup 179d): under the Board
//! summary with nothing selected, one line per check of the current solve,
//! a failing check one line per place. A click on a failing line selects
//! its entity. A failing check warns only; Bake still applies the sketch
//! (`docs/DECISIONS.md`, pending the router decision).

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, RenderTranslatable as _, ids, t};
use signex_widgets::text;

use crate::library::editor::standalone::sketch_exit::status_line;
use crate::panels::{BoardSketchProps, CollapsedSections, PanelMsg, PanelPalette};
use crate::panels::{collapsible_section_header, is_section_collapsed};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The section's key in the collapsed-sections set.
const SECTION: &str = "fp_board_checks";

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
    if is_section_collapsed(SECTION, collapsed_sections) {
        return col;
    }
    for line in &board.outline_checks {
        let select = line
            .place
            .map(|id| PanelMsg::FpEditorSelectSketchEntity { id });
        col = col.push(status_line(line.status, line.text.localized(i18n), select));
    }
    col.push(
        container(
            text(t!(i18n, ids::BOARD_SKETCH_CHECKS_CAPTION))
                .size(TS.caption)
                .color(palette.muted),
        )
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed(),
    )
}
