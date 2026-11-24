//! The Live checks card beside the bake preview, under the parameters table
//! (mockup row 189): parameters in error, and pads that overlap, re-read on
//! every solve.

use iced::widget::{column, scrollable};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_widgets::text;

use super::view::Look;
use crate::library::editor::footprint::state::{CheckStatus, EditorPad};
use crate::library::editor::standalone::sketch_exit::status_line;
use crate::library::messages::LibraryMessage;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Tallest the list grows before it scrolls, in body text heights.
const CHECKS_MAX_EM: f32 = 7.0;
/// Most overlapping pairs listed.
const MAX_OVERLAPS: usize = 8;
/// Pads closer than this overlap by less than the model resolves, mm.
const OVERLAP_EPS_MM: f64 = 1e-6;

/// Whether two pads can short: they share a copper layer, or one is on all.
fn share_copper(a: &EditorPad, b: &EditorPad) -> bool {
    let on_all = |pad: &EditorPad| pad.layers.iter().any(|l| l.as_str().starts_with("*."));
    on_all(a) || on_all(b) || a.layers.iter().any(|l| b.layers.contains(l))
}

/// A pad and its outline box.
type PadBox<'a> = (&'a EditorPad, (f64, f64, f64, f64));

/// The designators of the pad pairs whose outlines overlap, at most
/// [`MAX_OVERLAPS`]. Sorted along X, so a large footprint stays cheap.
pub(super) fn overlapping_pads(pads: &[EditorPad]) -> Vec<(String, String)> {
    let mut boxes: Vec<PadBox<'_>> = pads.iter().map(|pad| (pad, pad.bbox_mm())).collect();
    boxes.sort_by(|a, b| a.1.0.total_cmp(&b.1.0));
    let mut out = Vec::new();
    for (k, (pad, (_, y0, x1, y1))) in boxes.iter().enumerate() {
        for (other, (ox0, oy0, _, oy1)) in &boxes[k + 1..] {
            if *ox0 >= x1 - OVERLAP_EPS_MM {
                break;
            }
            let meets_in_y = *oy0 < y1 - OVERLAP_EPS_MM && *y0 < oy1 - OVERLAP_EPS_MM;
            if meets_in_y && share_copper(pad, other) {
                out.push((pad.number.clone(), other.number.clone()));
                if out.len() == MAX_OVERLAPS {
                    return out;
                }
            }
        }
    }
    out
}

/// The failed checks in the order Live checks lists them: `problems`
/// (parameters in error) first, then the overlapping pads.
pub(super) fn failed_lines(
    problems: Vec<Localized>,
    pads: &[EditorPad],
    i18n: &I18n,
) -> Vec<Localized> {
    let overlaps = overlapping_pads(pads).into_iter().map(|(first, second)| {
        t!(
            i18n,
            ids::SKETCH_PARAMETERS_CHECK_OVERLAP,
            "first" => first,
            "second" => second
        )
    });
    problems.into_iter().chain(overlaps).collect()
}

/// The Live checks heading and its lines ([`failed_lines`]); one passed
/// line when there is nothing.
pub(super) fn section<'a>(
    problems: Vec<Localized>,
    pads: &[EditorPad],
    look: Look,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let failed = failed_lines(problems, pads, i18n);
    let mut lines = column![].spacing(SP.xs);
    if failed.is_empty() {
        lines = lines.push(status_line::<LibraryMessage>(
            CheckStatus::Passed,
            t!(i18n, ids::SKETCH_PARAMETERS_CHECKS_PASSED),
            None,
        ));
    }
    for line in failed {
        lines = lines.push(status_line::<LibraryMessage>(
            CheckStatus::Failed,
            line,
            None,
        ));
    }
    column![
        text(t!(i18n, ids::SKETCH_PARAMETERS_CHECKS_TITLE))
            .size(TS.caption)
            .color(look.muted),
        scrollable(lines).height(Length::Fit.max(TS.body * CHECKS_MAX_EM)),
    ]
    .spacing(SP.xs)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(number: &str, x: f64, layer: &str) -> EditorPad {
        let mut pad = EditorPad::new_default(number.to_string(), (x, 0.0));
        pad.size_mm = (1.0, 1.0);
        pad.layers = vec![signex_library::primitive::footprint::LayerId::new(layer)];
        pad
    }

    /// Mockup row 189: pads that overlap are named; pads apart, or on
    /// opposite sides of the board, are not.
    #[test]
    fn only_pads_that_overlap_on_one_layer_are_listed() {
        let pads = [
            pad("1", 0.0, "F.Cu"),
            pad("2", 0.5, "F.Cu"),
            pad("3", 5.0, "F.Cu"),
            pad("4", 5.5, "B.Cu"),
        ];
        assert_eq!(
            overlapping_pads(&pads),
            vec![("1".to_string(), "2".to_string())]
        );
    }

    #[test]
    fn a_through_pad_overlaps_either_side() {
        let pads = [pad("1", 0.0, "*.Cu"), pad("2", 0.5, "B.Cu")];
        assert_eq!(overlapping_pads(&pads).len(), 1);
    }

    #[test]
    fn pads_that_only_touch_do_not_overlap() {
        let pads = [pad("1", 0.0, "F.Cu"), pad("2", 1.0, "F.Cu")];
        assert!(overlapping_pads(&pads).is_empty());
    }
}
