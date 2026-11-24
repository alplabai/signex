//! The Bake panel of a board sketch (mockup row 179): while the bake preview
//! is open, Properties shows what Bake writes to the board, value by value,
//! or why the sketch does not bake, with the solve state, Close gap and the
//! failed outline checks. A line that names a place selects it on a click;
//! a click on the canvas only highlights. The mode bar's Bake preview
//! button closes the panel. See `docs/PCB_SKETCH.md`, "Sketch mode".
//! A footprint sketch's open preview has its Bake panel here too (mockup
//! row 190), so the preview card covers no canvas.

use iced::widget::{Column, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, RenderTranslatable as _, ids, t};
use signex_types::sketch::baked_kinds::BakedKind;
use signex_widgets::text;

use super::super::{
    BakedHole, BakedKeepout, BakedOutline, BoardBakePreview, BoardBakeRefusal, BoardBakeRows,
    BoardSketchProps, PanelMsg, SketchFixMsg,
};
use super::body::{Ctx, caption, ghost, is_open, kv_row, mini_head, object_header, section};
use super::sections::solve_pill;
use crate::library::editor::footprint::{CheckStatus, GapOffer, format_check_mm};
use crate::library::editor::standalone::sketch_exit::{
    FootprintBakeCard, preview_card, status_line, target_label_id,
};
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The footprint Bake panel's preview section.
const FOOTPRINT_PREVIEW_KEY: &str = "fp_bake_preview";

/// The Bake panel of a footprint sketch (mockup row 190): the Roles
/// section, the preview card (what Bake writes or why it does not, Close
/// gap, the sketch warnings), then Back to sketch and Bake. The canvas
/// draws the result meanwhile.
pub(super) fn view_footprint_bake_panel<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    card: &FootprintBakeCard,
) -> Column<Element<'a, PanelMsg>> {
    let (tokens, i18n) = (cx.tokens, cx.i18n);
    col = col.push(object_header(
        t!(i18n, ids::EDITORS_SKETCH_BAKE),
        None,
        tokens,
    ));
    col = super::roles::view_roles(col, cx);
    let title = t!(i18n, ids::EDITORS_SKETCH_BAKE_PREVIEW);
    col = col.push(section(
        FOOTPRINT_PREVIEW_KEY,
        &title,
        None,
        cx.collapsed,
        tokens,
        i18n,
    ));
    if is_open(FOOTPRINT_PREVIEW_KEY, cx.collapsed) {
        let issues = card.issues(i18n, |keep, merge| {
            PanelMsg::SketchFix(SketchFixMsg::CloseGapClicked { keep, merge })
        });
        col = col.push(
            container(preview_card(&card.preview, issues, tokens, i18n))
                .padding([SP.xs, SP.m])
                .width(Length::Fill)
                .boxed(),
        );
    }
    col.push(super::roles::bake_buttons(cx))
}

/// The panel's head, the solve state, what Bake writes or why it does not,
/// and the failed outline checks.
pub(super) fn view_bake_panel<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    board: &'a BoardSketchProps,
    preview: &'a BoardBakePreview,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = cx.i18n;
    let status = t!(i18n, ids::BOARD_SKETCH_BAKE_STATUS, "board" => board.board_name.as_str());
    col = col
        .push(object_header(
            t!(i18n, ids::BOARD_SKETCH_BAKE_TITLE),
            Some(status.into()),
            cx.tokens,
        ))
        .push(solve_row(cx));
    let mut selectable = false;
    col = match preview {
        BoardBakePreview::Writes(rows) => push_writes(col, cx, rows),
        BoardBakePreview::Refused(refusal) => {
            selectable |= refusal.place.is_some();
            push_refusal(col, cx, refusal)
        }
    };
    let failed: Vec<_> = board
        .outline_checks
        .iter()
        .filter(|line| line.status == CheckStatus::Failed)
        .collect();
    if !failed.is_empty() {
        col = col.push(mini_head(
            &t!(i18n, ids::SKETCH_BAKE_CHECKS),
            cx.tokens,
            i18n,
        ));
        for line in failed {
            selectable |= line.place.is_some();
            let select = line
                .place
                .map(|id| PanelMsg::FpEditorSelectSketchEntity { id });
            col = col.push(status_line(line.status, line.text.localized(i18n), select));
        }
        col = col.push(caption(t!(i18n, ids::SKETCH_BAKE_CHECKS_WARN), cx.tokens));
    }
    if selectable {
        col = col.push(caption(t!(i18n, ids::SKETCH_BAKE_SELECT_HINT), cx.tokens));
    }
    col
}

/// Under the panel: what never bakes, and how the panel closes.
pub(super) fn view_bake_panel_tail<'a>(
    col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    col.push(caption(
        t!(cx.i18n, ids::EDITORS_SKETCH_BAKE_NEVER),
        cx.tokens,
    ))
    .push(caption(
        t!(cx.i18n, ids::BOARD_SKETCH_BAKE_CLOSE_HINT),
        cx.tokens,
    ))
}

/// The sketch's solve state, as the Sketch solve header shows it.
fn solve_row<'a>(cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    container(
        row![
            text(t!(cx.i18n, ids::BOARD_SKETCH_BAKE_SOLVE))
                .size(TS.caption)
                .color(cx.palette.muted)
                .width(Length::Fill),
            solve_pill(cx),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

fn push_writes<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    rows: &BoardBakeRows,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = cx.i18n;
    col = col.push(mini_head(
        &t!(i18n, ids::EDITORS_SKETCH_BAKE_RESULT),
        cx.tokens,
        i18n,
    ));
    let lines = write_lines(rows, i18n);
    if lines.is_empty() {
        return col.push(caption(t!(i18n, ids::BOARD_SKETCH_BAKE_NOTHING), cx.tokens));
    }
    for (key, value) in lines {
        col = col.push(kv_row(&key, value, cx.tokens, i18n));
    }
    col.push(caption(t!(i18n, ids::SKETCH_BAKE_DRAWN), cx.tokens))
}

/// One `(label, value)` per thing Bake writes: the outline's size, the
/// holes by drill and plating with their count, each keepout's layers and
/// kinds, each slot's size, each anchor's component, and a count for the
/// other targets. A slot is listed once, as a slot.
pub(super) fn write_lines(rows: &BoardBakeRows, i18n: &I18n) -> Vec<(Localized, String)> {
    let target = |kind| t!(i18n, target_label_id(kind));
    let mm = |value: f64| format_check_mm(value, 2);
    let mut lines = Vec::new();
    if let Some(outline) = rows.outline {
        let value = match outline {
            BakedOutline::Size(width, height) => t!(
                i18n, ids::BOARD_SKETCH_BAKE_SIZE,
                "width" => mm(width), "height" => mm(height)
            ),
            BakedOutline::Removed => t!(i18n, ids::BOARD_SKETCH_BAKE_OUTLINE_REMOVED),
        };
        lines.push((target(BakedKind::BoardOutline), value.into_string()));
    }
    for (hole, count) in hole_groups(&rows.holes) {
        let id = match (hole.plated, count) {
            (true, 1) => ids::BOARD_SKETCH_BAKE_HOLE_PLATED,
            (false, 1) => ids::BOARD_SKETCH_BAKE_HOLE_UNPLATED,
            (true, _) => ids::BOARD_SKETCH_BAKE_HOLES_PLATED,
            (false, _) => ids::BOARD_SKETCH_BAKE_HOLES_UNPLATED,
        };
        let value = t!(i18n, id, "drill" => mm(hole.drill_mm), "count" => count).into_string();
        lines.push((t!(i18n, ids::BOARD_SKETCH_HOLE), value));
    }
    for keepout in &rows.keepouts {
        lines.push((target(BakedKind::Keepouts), keepout_value(keepout, i18n)));
    }
    let count = |n: usize| t!(i18n, ids::EDITORS_SKETCH_BAKE_COUNT, "count" => n).into_string();
    if rows.cutouts > 0 {
        lines.push((target(BakedKind::Cutouts), count(rows.cutouts)));
    }
    for &(length, width) in &rows.slots {
        let value = t!(
            i18n, ids::BOARD_SKETCH_SLOT_SIZE,
            "length" => mm(length), "width" => mm(width)
        );
        lines.push((t!(i18n, ids::BOARD_SKETCH_SLOT), value.into_string()));
    }
    for (kind, n) in [
        (BakedKind::VScores, rows.v_scores),
        (BakedKind::BoardPours, rows.pours),
        (BakedKind::Mechanical, rows.mechanical),
    ] {
        if n > 0 {
            lines.push((target(kind), count(n)));
        }
    }
    for reference in &rows.anchors {
        let value = t!(i18n, ids::BOARD_SKETCH_BAKE_ANCHOR, "reference" => reference.as_str());
        lines.push((target(BakedKind::ComponentAnchors), value.into_string()));
    }
    lines
}

/// The holes by drill and plating, each with its count, in the order each
/// first appears: "4 × Ø 3.20 mm · unplated" (mockup row 179).
fn hole_groups(holes: &[BakedHole]) -> Vec<(BakedHole, usize)> {
    let shown = |hole: &BakedHole| (hole.plated, format_check_mm(hole.drill_mm, 2));
    let mut groups: Vec<(BakedHole, usize)> = Vec::new();
    for hole in holes {
        match groups
            .iter_mut()
            .find(|(seen, _)| shown(seen) == shown(hole))
        {
            Some((_, count)) => *count += 1,
            None => groups.push((*hole, 1)),
        }
    }
    groups
}

/// "On Top Layer · No copper, No routing": a keepout's layers and what it
/// keeps out (mockup row 179).
fn keepout_value(keepout: &BakedKeepout, i18n: &I18n) -> String {
    let layers: Vec<String> = keepout
        .layers
        .iter()
        .map(|&layer| crate::pcb_canvas::layer_style::layer_name(i18n, layer).into_string())
        .collect();
    let layers = layers.join(", ");
    let forbids = keepout.forbids;
    let kinds: Vec<String> = [
        (forbids.no_copper, ids::EDITORS_ROLE_KEEPOUT_NO_COPPER),
        (forbids.no_routing, ids::EDITORS_ROLE_KEEPOUT_NO_ROUTING),
        (forbids.no_vias, ids::EDITORS_ROLE_KEEPOUT_NO_VIAS),
        (forbids.no_pours, ids::EDITORS_ROLE_KEEPOUT_NO_POURS),
        (
            forbids.no_components,
            ids::EDITORS_ROLE_KEEPOUT_NO_COMPONENTS,
        ),
        (forbids.no_drilling, ids::EDITORS_ROLE_KEEPOUT_NO_DRILLING),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, id)| t!(i18n, id).into_string())
    .collect();
    if kinds.is_empty() {
        return t!(i18n, ids::BOARD_SKETCH_BAKE_KEEPOUT_LAYERS, "layers" => layers).into_string();
    }
    t!(
        i18n, ids::BOARD_SKETCH_BAKE_KEEPOUT_KINDS,
        "layers" => layers, "kinds" => kinds.join(", ")
    )
    .into_string()
}

/// Why the sketch does not bake, selecting the entity it names, then the
/// open profile's gap with Close gap when it heals.
fn push_refusal<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    refusal: &BoardBakeRefusal,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = cx.i18n;
    col = col.push(mini_head(
        &t!(i18n, ids::SKETCH_BAKE_ERRORS),
        cx.tokens,
        i18n,
    ));
    let line = match &refusal.reason {
        Some(reason) => t!(i18n, ids::EDITORS_SKETCH_BAKE_REFUSED, "reason" => reason.as_str()),
        None => t!(i18n, ids::BOARD_SKETCH_BAKE_UNSOLVED),
    };
    let select = refusal
        .place
        .map(|id| PanelMsg::FpEditorSelectSketchEntity { id });
    col = col.push(status_line(CheckStatus::Failed, line, select));
    match &refusal.gap {
        Some(offer) => col.push(gap_row(offer, cx)),
        None => col,
    }
}

/// Whether the gap heals, and Close gap when it does (mockup row 190).
fn gap_row<'a>(offer: &GapOffer, cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    let mut line = row![
        text(offer.text().localized(cx.i18n))
            .size(TS.caption)
            .color(cx.palette.muted)
            .width(Length::Fill)
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center);
    if offer.closes() {
        let close = PanelMsg::SketchFix(SketchFixMsg::CloseGapClicked {
            keep: offer.keep,
            merge: offer.merge,
        });
        line = line.push(ghost(
            t!(cx.i18n, ids::SKETCH_BAKE_CLOSE_GAP),
            Some(close),
            cx.tokens,
        ));
    }
    container(line)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

#[cfg(test)]
#[path = "bake_panel_tests.rs"]
mod tests;
