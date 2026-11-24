//! Properties after Bake (mockup row 190): Baked with the pad count, what
//! was written, the clicked pad's sketch source and the way back to the
//! sketch. See `docs/FOOTPRINT_SKETCH.md`, "Leaving sketch mode".

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;

use super::super::{FootprintPadSummary, PanelMsg, PanelPalette, SketchFixMsg};
use super::body::{banner, ghost, object_header};
use super::pad::source_text;
use super::rows::value_row;
use crate::library::editor::footprint::PadSketchSource;
use crate::library::editor::footprint::sketch_mode::BakeOutcome;
use crate::library::editor::standalone::sketch_exit::target_label_id;
use crate::styles::{PillTone, Spacing};

const SP: Spacing = Spacing::DEFAULT;

pub(super) fn view_bake_outcome<'a>(
    col: Column<Element<'a, PanelMsg>>,
    outcome: &BakeOutcome,
    pad: Option<&FootprintPadSummary>,
    palette: PanelPalette,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Column<Element<'a, PanelMsg>> {
    let link = link_text(
        pad.map(|pad| (pad.number.as_str(), pad.sketch_source)),
        i18n,
    );
    col.push(object_header(
        t!(i18n, ids::SKETCH_BAKE_OUTCOME_TITLE, "pads" => outcome.pads),
        None,
        tokens,
    ))
    .push(banner(wrote_line(outcome, i18n), PillTone::Ok, tokens))
    .push(value_row(
        &t!(i18n, ids::SKETCH_BAKE_OUTCOME_LINK).into_string(),
        &link,
        palette.muted,
        palette.input_bg,
    ))
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
