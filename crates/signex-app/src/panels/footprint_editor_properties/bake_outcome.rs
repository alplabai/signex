//! Properties after Bake (mockup row 190): Baked with the pad count and
//! what was written, the Pads ↔ Sketch section with the clicked pad's
//! sketch source, the Roles and Bake result sections closed below it, and
//! the way back to the sketch. See `docs/FOOTPRINT_SKETCH.md`, "Leaving
//! sketch mode".

use iced::widget::{Column, container, row};
use iced::{Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_widgets::{button, raw, text};

use super::super::{PanelMsg, SketchFixMsg};
use super::body::{Ctx, banner, caption, ghost, is_open, kv_row, object_header, section};
use super::pad::source_text;
use super::rows::value_row;
use crate::library::editor::footprint::PadSketchSource;
use crate::library::editor::footprint::sketch_mode::BakeOutcome;
use crate::library::editor::footprint::sketch_mode::roles::role_label;
use crate::library::editor::standalone::sketch_exit::target_label_id;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Spacing, TypeScale};

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
        Some(pad) => {
            let link = link_text(Some((pad.number.as_str(), pad.sketch_source)), i18n);
            col.push(value_row(
                &t!(i18n, ids::SKETCH_BAKE_OUTCOME_LINK).into_string(),
                &link,
                cx.palette.muted,
                cx.palette.input_bg,
            ))
        }
        None => col.push(caption(
            t!(i18n, ids::SKETCH_BAKE_OUTCOME_LINK_EMPTY),
            tokens,
        )),
    }
}

/// Roles, closed: each closed profile and the role it baked as.
fn roles_section<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let i18n = cx.i18n;
    let open = cx.collapsed.contains(ROLES_KEY);
    col = col.push(closed_section(
        ROLES_KEY,
        &t!(i18n, ids::SKETCH_BAKE_ROLES),
        open,
        cx,
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
    let open = cx.collapsed.contains(RESULT_KEY);
    let title = t!(i18n, ids::EDITORS_SKETCH_BAKE_RESULT);
    col = col.push(closed_section(RESULT_KEY, &title, open, cx));
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

/// The header of a section that starts closed: the chevron follows `open`,
/// and a click toggles `key` as every section header does.
fn closed_section<'a>(
    key: &str,
    title: &Localized,
    open: bool,
    cx: Ctx<'a, '_>,
) -> Element<'a, PanelMsg> {
    let ink = resolve(cx.tokens);
    let label = row![
        signex_widgets::chevron::chevron(open, TS.section, ink.ink4),
        // raw: the catalog title, upper-cased for the locale.
        text(raw(cx.i18n.upper_for_display(&title.to_string())))
            .size(TS.section)
            .color(ink.ink3)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.xs)
    .align_y(iced::Alignment::Center);
    let toggle = button(label)
        .padding([SP.s, SP.xs])
        .width(Length::Fill)
        .on_press(PanelMsg::ToggleSection(key.to_string()))
        .style(|_: &Theme, _| button::Style::default());
    container(toggle)
        .padding(iced::Padding {
            top: SP.xs,
            right: SP.m,
            bottom: 0.0,
            left: SP.xs,
        })
        .width(Length::Fill)
        .boxed()
}

/// The Pads ↔ Sketch row's value: the clicked pad's number and source.
fn link_text(pad: Option<(&str, Option<PadSketchSource>)>, i18n: &I18n) -> String {
    match pad {
        None => t!(i18n, ids::SKETCH_BAKE_OUTCOME_LINK_HINT).into_string(),
        Some((number, None)) => {
            t!(i18n, ids::SKETCH_BAKE_OUTCOME_LINK_NONE, "number" => number).into_string()
        }
        Some((number, Some(source))) => t!(
            i18n,
            ids::SKETCH_BAKE_OUTCOME_LINK_PAD,
            "number" => number,
            "source" => source_text(source, i18n)
        )
        .into_string(),
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

    fn plain(s: String) -> String {
        s.replace(['\u{2068}', '\u{2069}'], "")
    }

    /// DECISIONS 2026-11-23 (#2835): the row waits for a click, then names
    /// the clicked pad and what drives it.
    #[test]
    fn the_link_row_fills_from_the_clicked_pad() {
        let i18n = I18n::en_us();
        assert_eq!(plain(link_text(None, &i18n)), "Click a pad");
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
            "Pad 1 · Pad point"
        );
        let profile = PadSketchSource {
            edges: 4,
            instance: None,
        };
        assert_eq!(
            plain(link_text(Some(("A2", Some(profile))), &i18n)),
            "Pad A2 · Profile of 4 edges"
        );
    }
}
