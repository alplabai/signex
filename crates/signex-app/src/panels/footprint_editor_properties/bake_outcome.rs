//! Properties after Bake (mockup row 190): Baked with the pad count and
//! what was written, the Pads ↔ Sketch section with the clicked pad's
//! sketch source, the Roles and Bake result sections closed below it, and
//! the way back to the sketch. See `docs/FOOTPRINT_SKETCH.md`, "Leaving
//! sketch mode".

use iced::widget::{Column, Row, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_widgets::text;

use super::super::{PanelMsg, SketchFixMsg};
use super::body::{
    Ctx, banner, caption, ghost, is_open, kv_row, link, object_header, section, section_shown,
    section_toggle,
};
use super::pad::source_text;
use super::rows::value_row;
use crate::library::editor::footprint::PadSketchSource;
use crate::library::editor::footprint::sketch_mode::BakeOutcome;
use crate::library::editor::footprint::sketch_mode::roles::role_label;
use crate::library::editor::standalone::sketch_exit::target_label_id;
use crate::styles::{PillTone, Spacing, TypeScale, ti};

const SP: Spacing = Spacing::DEFAULT;
const TS: TypeScale = TypeScale::DEFAULT;

/// The Pads ↔ Sketch section, open until it is closed.
pub(super) const LINK_KEY: &str = "fp_bake_outcome_link";
/// The Roles and Bake result sections, closed until they are opened: their
/// key is in the collapsed set while they are open.
pub(super) const ROLES_KEY: &str = "fp_bake_outcome_roles";
pub(super) const RESULT_KEY: &str = "fp_bake_outcome_result";

pub(super) fn view_bake_outcome<'a>(
    col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    outcome: &BakeOutcome,
) -> Column<Element<'a, PanelMsg>> {
    let (tokens, i18n) = (cx.tokens, cx.i18n);
    let col = col
        .push(object_header(
            t!(i18n, ids::SKETCH_BAKE_OUTCOME_TITLE, "pads" => outcome.pads),
            None,
            tokens,
        ))
        .push(banner(wrote_line(outcome, i18n), PillTone::Ok, tokens));
    let col = link_section(col, cx);
    let col = roles_section(col, cx);
    let col = result_section(col, cx, outcome);
    col.push(
        container(ghost(
            t!(i18n, ids::EDITORS_FP_PAD_EDIT_IN_SKETCH),
            Some(PanelMsg::SketchFix(SketchFixMsg::EditInSketchClicked)),
            tokens,
        ))
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed(),
    )
}

/// Pads ↔ Sketch: what the links are, then the clicked pad and what drives
/// it, or "No pad selected." until a pad is clicked.
fn link_section<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let (tokens, i18n) = (cx.tokens, cx.i18n);
    let title = t!(i18n, ids::SKETCH_BAKE_OUTCOME_LINK);
    col = col.push(section(LINK_KEY, &title, None, cx.collapsed, tokens, i18n));
    if !is_open(LINK_KEY, cx.collapsed) {
        return col;
    }
    col = col.push(caption(
        t!(i18n, ids::SKETCH_BAKE_OUTCOME_LINK_CAPTION),
        tokens,
    ));
    match cx.fp.selected_pad.as_ref() {
        Some(pad) => col.push(pad_link_row(pad.idx, &pad.number, pad.sketch_source, cx)),
        None => col.push(caption(link_text(None, i18n), tokens)),
    }
}

/// The clicked pad's row: "Pad 3 ← Profile of 4 edges", then Show in Sketch,
/// which opens that pad's source in Sketch mode (mockup row 192). A pad
/// without a source has nothing to show.
pub(super) fn pad_link_row<'a>(
    pad_idx: usize,
    number: &str,
    source: Option<PadSketchSource>,
    cx: Ctx<'a, '_>,
) -> Element<'a, PanelMsg> {
    let (tokens, i18n) = (cx.tokens, cx.i18n);
    let mut line = Row::new()
        .push(
            text(link_text(Some((number, source)), i18n))
                .size(TS.caption)
                .color(ti(tokens.text))
                .width(Length::Fill)
                .boxed(),
        )
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    if source.is_some() {
        line = line.push(link(
            t!(i18n, ids::SKETCH_BAKE_OUTCOME_SHOW_IN_SKETCH),
            PanelMsg::FpEditorEditPadInSketch { pad_idx },
            tokens,
        ));
    }
    container(line)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// Roles, closed: each closed profile and the role it baked as.
fn roles_section<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = cx.i18n;
    let open = section_shown(ROLES_KEY, cx.collapsed, false);
    col = col.push(section_toggle(
        ROLES_KEY,
        &t!(i18n, ids::SKETCH_BAKE_ROLES),
        None,
        open,
        cx.tokens,
        i18n,
    ));
    if !open {
        return col;
    }
    if cx.fp.profile_roles.is_empty() {
        return col.push(caption(t!(i18n, ids::SKETCH_BAKE_ROLES_EMPTY), cx.tokens));
    }
    for (index, profile) in cx.fp.profile_roles.iter().enumerate() {
        let name = super::roles::profile_name(cx, index + 1, profile);
        let role = role_label(profile.role, cx.fp.sketch_host, i18n);
        col = col.push(value_row(
            &name,
            &role,
            cx.palette.muted,
            cx.palette.input_bg,
        ));
    }
    col
}

/// Bake result, closed: each target the bake wrote and its count.
fn result_section<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    outcome: &BakeOutcome,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = cx.i18n;
    let open = section_shown(RESULT_KEY, cx.collapsed, false);
    let title = t!(i18n, ids::EDITORS_SKETCH_BAKE_RESULT);
    col = col.push(section_toggle(
        RESULT_KEY, &title, None, open, cx.tokens, i18n,
    ));
    if !open {
        return col;
    }
    for &(kind, count) in &outcome.written {
        let value = t!(i18n, ids::EDITORS_SKETCH_BAKE_COUNT, "count" => count).into_string();
        col = col.push(kv_row(
            &t!(i18n, target_label_id(kind)),
            value,
            cx.tokens,
            i18n,
        ));
    }
    col
}

/// The Pads ↔ Sketch line: the clicked pad's number and source, or "No pad
/// selected." before a click.
fn link_text(pad: Option<(&str, Option<PadSketchSource>)>, i18n: &I18n) -> Localized {
    match pad {
        None => t!(i18n, ids::SKETCH_BAKE_OUTCOME_LINK_EMPTY),
        Some((number, None)) => t!(i18n, ids::SKETCH_BAKE_OUTCOME_LINK_NONE, "number" => number),
        Some((number, Some(source))) => t!(
            i18n,
            ids::SKETCH_BAKE_OUTCOME_LINK_FROM,
            "number" => number,
            "source" => source_text(source, i18n)
        ),
    }
}

/// "Baked Pads 8, Courtyard 1 into QFN-16."
fn wrote_line(outcome: &BakeOutcome, i18n: &I18n) -> Localized {
    if outcome.written.is_empty() {
        return t!(
            i18n,
            ids::SKETCH_BAKE_OUTCOME_NOTHING,
            "name" => outcome.name.as_str()
        );
    }
    let items: Vec<String> = outcome
        .written
        .iter()
        .map(|&(kind, count)| {
            t!(
                i18n,
                ids::SKETCH_BAKE_OUTCOME_ITEM,
                "target" => t!(i18n, target_label_id(kind)).into_string(),
                "count" => count
            )
            .into_string()
        })
        .collect();
    t!(
        i18n,
        ids::SKETCH_BAKE_OUTCOME_WROTE,
        "items" => items.join(", "),
        "name" => outcome.name.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(s: Localized) -> String {
        s.into_string().replace(['\u{2068}', '\u{2069}'], "")
    }

    /// DECISIONS 2026-11-23 (#2835): the row waits for a click, then names
    /// the clicked pad and what drives it, as mockup row 190 writes it.
    #[test]
    fn the_link_row_fills_from_the_clicked_pad() {
        let i18n = I18n::en_us();
        assert_eq!(plain(link_text(None, &i18n)), "No pad selected.");
        assert_eq!(
            plain(link_text(Some(("3", None)), &i18n)),
            "Pad 3 · no sketch source"
        );
        let point = PadSketchSource {
            edges: 0,
            instance: None,
        };
        assert_eq!(
            plain(link_text(Some(("1", Some(point))), &i18n)),
            "Pad 1 ← Pad point"
        );
        let profile = PadSketchSource {
            edges: 4,
            instance: None,
        };
        assert_eq!(
            plain(link_text(Some(("A2", Some(profile))), &i18n)),
            "Pad A2 ← Profile of 4 edges"
        );
    }
}
