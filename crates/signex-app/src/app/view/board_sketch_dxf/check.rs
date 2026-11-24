//! The profile check and the result of the Import DXF card (mockup row
//! 179c): what the drawing holds, Heal and merge or Import as is, then what
//! landed and Done.

use iced::widget::{Column, column, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_sketch::heal::HEAL_TOLERANCE_MM;
use signex_types::pcb::Point;
use signex_types::sketch::SketchHost;
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input, theme_ext};

use super::{LABEL_EM, LAYERS_MAX_EM, SP, TS, heading, send};
use crate::app::Message;
use crate::app::state::{DxfImportMode, SketchDxfDialog, SketchDxfMsg, SketchDxfResult};
use crate::library::editor::footprint::format_check_mm as mm;
use crate::library::editor::footprint::parameters_table::view::field_style;
use crate::library::editor::footprint::sketch_mode::roles::role_label;
use crate::styles::{DialogButtonKind, dialog_button};

/// The width of the Heal under field, in body text heights.
const HEAL_FIELD_EM: f32 = 5.0;

/// The lines of the check, each with whether it is a problem.
pub(super) fn check_lines(dialog: &SketchDxfDialog, i18n: &I18n) -> Vec<(Localized, bool)> {
    let report = &dialog.report;
    let profile = |tag| role_label(dialog.role_of(tag), SketchHost::Board, i18n);
    let user = |(x, y): (f64, f64)| {
        let p = dialog.origin.to_user(Point::new(x, y));
        (mm(p.x, 2), mm(p.y, 2))
    };
    let mut lines = Vec::new();
    for gap in &report.gaps {
        let (x, y) = user(gap.at);
        let line = t!(i18n, ids::BOARD_SKETCH_DXF_GAP,
            "profile" => profile(gap.tag),
            "gap" => mm(gap.gap_mm, 2),
            "x" => x,
            "y" => y);
        lines.push((line, true));
    }
    for &(at, tag) in &report.open_ends {
        let (x, y) = user(at);
        let line = t!(i18n, ids::BOARD_SKETCH_DXF_OPEN_END,
            "profile" => profile(tag),
            "x" => x,
            "y" => y,
            "tolerance" => mm(HEAL_TOLERANCE_MM, 2));
        lines.push((line, true));
    }
    for overlap in &report.overlaps {
        let (x, y) = user(overlap.at);
        let line = t!(i18n, ids::BOARD_SKETCH_DXF_OVERLAP,
            "count" => overlap.segments,
            "x" => x,
            "y" => y);
        lines.push((line, true));
    }
    if report.circles > 0 {
        let line = t!(i18n, ids::BOARD_SKETCH_DXF_CIRCLES_CLOSED, "count" => report.circles);
        lines.push((line, false));
    }
    lines
}

/// The check: a heading with the problem count, a line per problem, the
/// heal tolerance and the two ways to import.
pub(super) fn check_card<'a>(
    dialog: &'a SketchDxfDialog,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let report = &dialog.report;
    let profiles = report.profiles.len();
    let title = if report.problems() > 0 {
        heading(
            t!(i18n, ids::BOARD_SKETCH_DXF_PROBLEMS,
                "problems" => report.problems(),
                "profiles" => profiles),
            theme_ext::error_color(tokens),
        )
    } else {
        heading(
            t!(i18n, ids::BOARD_SKETCH_DXF_NO_PROBLEMS, "profiles" => profiles),
            theme_ext::success_color(tokens),
        )
    };
    let mut list: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
    for (line, problem) in check_lines(dialog, i18n) {
        let color = if problem {
            theme_ext::error_color(tokens)
        } else {
            theme_ext::success_color(tokens)
        };
        list = list.push(text(line).size(TS.caption).color(color).boxed());
    }
    let list = scrollable(list)
        .height(Length::Fit.max(TS.body * LAYERS_MAX_EM))
        .boxed();
    column![
        title,
        list,
        heal_row(tokens, i18n),
        row![
            dialog_button(
                t!(i18n, ids::BOARD_SKETCH_DXF_HEAL),
                DialogButtonKind::Primary,
                Some(send(SketchDxfMsg::Confirmed(DxfImportMode::HealAndMerge))),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::BOARD_SKETCH_DXF_AS_IS),
                DialogButtonKind::Secondary,
                Some(send(SketchDxfMsg::Confirmed(DxfImportMode::AsIs))),
                tokens,
            ),
        ]
        .spacing(SP.s)
        .boxed(),
    ]
    .spacing(SP.s)
    .boxed()
}

/// Heal under: the one heal tolerance, shared with Close gap, read only.
fn heal_row<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, Message> {
    // No on_input: the field shows the tolerance and takes no edit.
    let value: text_input::TextInput<'a, Message> = text_input(
        t!(i18n, ids::BOARD_SKETCH_DXF_HEAL_UNDER),
        mm(HEAL_TOLERANCE_MM, 2),
    )
    .size(TS.dense)
    .padding(SP.xs)
    .width(Length::Fixed(TS.body * HEAL_FIELD_EM))
    .style(field_style(tokens, false));
    row![
        text(t!(i18n, ids::BOARD_SKETCH_DXF_HEAL_UNDER))
            .size(TS.caption)
            .color(theme_ext::text_secondary(tokens))
            .width(Length::Fixed(TS.body * LABEL_EM)),
        value,
        text(t!(i18n, ids::BOARD_SKETCH_DXF_UNITS_MM))
            .size(TS.caption)
            .color(theme_ext::text_secondary(tokens)),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// What landed: the profiles, what was healed, the constraints and DOF,
/// that the file is not linked, and Done.
pub(super) fn result_card<'a>(
    result: SketchDxfResult,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, Message> {
    let check = result.check;
    let body = |line: Localized| {
        text(line)
            .size(TS.caption)
            .color(theme_ext::text_primary(tokens))
            .boxed()
    };
    let mut card: Column<Element<'a, Message>> = Column::new().spacing(SP.s).push(heading(
        t!(i18n, ids::BOARD_SKETCH_DXF_RESULT_TITLE,
            "outlines" => result.outlines,
            "holes" => result.holes),
        theme_ext::success_color(tokens),
    ));
    if result.others > 0 {
        card = card.push(body(t!(
            i18n,
            ids::BOARD_SKETCH_DXF_RESULT_OTHERS,
            "others" => result.others
        )));
    }
    let healed = match result.mode {
        DxfImportMode::HealAndMerge => t!(i18n, ids::BOARD_SKETCH_DXF_RESULT_HEALED,
            "healed" => check.healed,
            "merged" => check.merged),
        DxfImportMode::AsIs => t!(i18n, ids::BOARD_SKETCH_DXF_RESULT_AS_IS),
    };
    card = card
        .push(body(healed))
        .push(body(t!(i18n, ids::BOARD_SKETCH_DXF_RESULT_CONSTRAINTS,
            "constraints" => check.constraints,
            "dof" => check.dof)))
        .push(body(t!(i18n, ids::BOARD_SKETCH_DXF_RESULT_UNLINKED)))
        .push(dialog_button(
            t!(i18n, ids::BOARD_SKETCH_DXF_DONE),
            DialogButtonKind::Primary,
            Some(send(SketchDxfMsg::Dismissed)),
            tokens,
        ));
    card.boxed()
}
