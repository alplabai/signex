//! Properties after Bake (mockup row 190): Baked with the pad count, what
//! was written, and the way back to the sketch, until the selection
//! changes (`docs/DECISIONS.md`, 2026-11-22).

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;

use super::super::{PanelMsg, SketchFixMsg};
use super::body::{banner, ghost, object_header};
use crate::library::editor::footprint::sketch_mode::BakeOutcome;
use crate::library::editor::standalone::sketch_exit::target_label_id;
use crate::styles::{PillTone, Spacing};

const SP: Spacing = Spacing::DEFAULT;

pub(super) fn view_bake_outcome<'a>(
    col: Column<Element<'a, PanelMsg>>,
    outcome: &BakeOutcome,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    col.push(object_header(
        t!(i18n, ids::SKETCH_BAKE_OUTCOME_TITLE, "pads" => outcome.pads),
        None,
        tokens,
    ))
    .push(banner(wrote_line(outcome, i18n), PillTone::Ok, tokens))
    .push(
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
