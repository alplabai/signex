//! The Live checks card beside the bake preview, under the parameters table
//! (mockup row 189): parameters in error, pads that overlap, and the two
//! closest pads (a warning under the solder-mask web, else the passed line),
//! re-read on every solve.

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Alignment, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_sketch::unit::Quantity;
use signex_widgets::text;

use super::state::{DisplayUnit, display_value};
use super::view::{Look, unit_id};
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
/// A warning line's dot, in caption heights, as [`status_line`]'s.
const DOT_EM: f32 = 0.6;

/// Narrowest solder-mask web between two pads before Live checks warns, mm
/// (mockup row 189).
pub(super) const MASK_WEB_MM: f64 = 0.20;

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

/// The two closest pads that can short and do not overlap.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PadGap {
    pub(super) gap_mm: f64,
    pub(super) first: String,
    pub(super) second: String,
}

/// The smallest edge-to-edge gap between two pads that share copper and do
/// not overlap. Read from the outline boxes, as [`overlapping_pads`], so a
/// round pad's gap reads at most its true one. Touching pads are 0 apart.
pub(super) fn closest_pads(pads: &[EditorPad]) -> Option<PadGap> {
    let mut boxes: Vec<PadBox<'_>> = pads.iter().map(|pad| (pad, pad.bbox_mm())).collect();
    boxes.sort_by(|a, b| a.1.0.total_cmp(&b.1.0));
    let mut best: Option<(f64, &EditorPad, &EditorPad)> = None;
    for (k, (pad, (x0, y0, x1, y1))) in boxes.iter().enumerate() {
        for (other, (ox0, oy0, ox1, oy1)) in &boxes[k + 1..] {
            // Every later box starts further right: none can be closer.
            if best.is_some_and(|(gap, _, _)| ox0 - x1 >= gap) {
                break;
            }
            let x_overlap = x1.min(*ox1) - x0.max(*ox0);
            let y_overlap = y1.min(*oy1) - y0.max(*oy0);
            if (x_overlap > OVERLAP_EPS_MM && y_overlap > OVERLAP_EPS_MM)
                || !share_copper(pad, other)
            {
                continue;
            }
            let gap = (-x_overlap).max(0.0).hypot((-y_overlap).max(0.0));
            if gap.is_finite() && best.is_none_or(|(b, _, _)| gap < b) {
                best = Some((gap, *pad, *other));
            }
        }
    }
    best.map(|(gap_mm, a, b)| PadGap {
        gap_mm,
        first: a.number.clone(),
        second: b.number.clone(),
    })
}

/// How a Live checks line reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LineKind {
    Failed,
    Warned,
    Passed,
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

/// Every Live checks line in order: [`failed_lines`], then the closest pads
/// as a warning under [`MASK_WEB_MM`], or as the passed line when no pads
/// overlap. With no two pads to measure and nothing failed, one passed line.
pub(super) fn live_lines(
    problems: Vec<Localized>,
    pads: &[EditorPad],
    i18n: &I18n,
) -> Vec<(LineKind, Localized)> {
    let mut lines: Vec<(LineKind, Localized)> = failed_lines(problems, pads, i18n)
        .into_iter()
        .map(|line| (LineKind::Failed, line))
        .collect();
    let has_overlaps = !overlapping_pads(pads).is_empty();
    let mm = |value: f64| display_value(Quantity::length(value)).0;
    let unit = || t!(i18n, unit_id(DisplayUnit::Mm)).into_string();
    match closest_pads(pads) {
        Some(near) if near.gap_mm < MASK_WEB_MM => lines.push((
            LineKind::Warned,
            t!(
                i18n,
                ids::SKETCH_PARAMETERS_CHECK_MASK_WEB,
                "first" => near.first,
                "second" => near.second,
                "gap" => mm(near.gap_mm),
                "web" => mm(MASK_WEB_MM),
                "unit" => unit()
            ),
        )),
        Some(near) if !has_overlaps => lines.push((
            LineKind::Passed,
            t!(
                i18n,
                ids::SKETCH_PARAMETERS_CHECKS_GAP,
                "gap" => mm(near.gap_mm),
                "unit" => unit()
            ),
        )),
        _ if lines.is_empty() => lines.push((
            LineKind::Passed,
            t!(i18n, ids::SKETCH_PARAMETERS_CHECKS_PASSED),
        )),
        _ => {}
    }
    lines
}

/// A warning line, shaped as [`status_line`]'s, in the warning colour.
fn warned_line<'a>(label: Localized, look: Look) -> Element<'a, LibraryMessage> {
    let warn = look.warn;
    let dot_size = TS.caption * DOT_EM;
    let dot = container(Space::new())
        .width(Length::Fixed(dot_size))
        .height(Length::Fixed(dot_size))
        .style(move |_: &Theme| container::Style {
            background: Some(warn.into()),
            border: Border {
                radius: crate::styles::Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(
        row![dot, text(label).size(TS.caption).color(warn)]
            .spacing(SP.s)
            .align_y(Alignment::Center),
    )
    .padding([SP.xs, SP.s])
    .width(Length::Fill)
    .boxed()
}

/// The Live checks heading and its lines ([`live_lines`]).
pub(super) fn section<'a>(
    problems: Vec<Localized>,
    pads: &[EditorPad],
    look: Look,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut lines = column![].spacing(SP.xs);
    for (kind, line) in live_lines(problems, pads, i18n) {
        lines = lines.push(match kind {
            LineKind::Failed => status_line::<LibraryMessage>(CheckStatus::Failed, line, None),
            LineKind::Passed => status_line::<LibraryMessage>(CheckStatus::Passed, line, None),
            LineKind::Warned => warned_line(line, look),
        });
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

    fn kinds(lines: &[(LineKind, Localized)]) -> Vec<LineKind> {
        lines.iter().map(|(kind, _)| *kind).collect()
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

    /// The closest pads are the pair with the smallest edge gap that share
    /// copper; a pad on the other side does not count.
    #[test]
    fn the_closest_pads_share_copper() {
        let pads = [
            pad("1", 0.0, "F.Cu"),
            pad("2", 1.1, "B.Cu"),
            pad("3", 3.0, "F.Cu"),
            pad("4", 4.5, "F.Cu"),
        ];
        let near = closest_pads(&pads).expect("two front pads");
        assert_eq!((near.first.as_str(), near.second.as_str()), ("3", "4"));
        assert!((near.gap_mm - 0.5).abs() < 1e-9, "{}", near.gap_mm);
    }

    #[test]
    fn a_diagonal_gap_is_measured_corner_to_corner() {
        let mut second = pad("2", 1.3, "F.Cu");
        second.position_mm.1 = 1.4;
        let near = closest_pads(&[pad("1", 0.0, "F.Cu"), second]).expect("a pair");
        assert!(
            (near.gap_mm - 0.5).abs() < 1e-9,
            "0.3 by 0.4: {}",
            near.gap_mm
        );
    }

    #[test]
    fn overlapping_pads_have_no_gap() {
        assert_eq!(
            closest_pads(&[pad("1", 0.0, "F.Cu"), pad("2", 0.5, "F.Cu")]),
            None
        );
        let touching = closest_pads(&[pad("1", 0.0, "F.Cu"), pad("2", 1.0, "F.Cu")]);
        assert_eq!(touching.map(|near| near.gap_mm), Some(0.0));
    }

    /// Mockup row 189: pads closer than the solder-mask web warn, naming
    /// the pads, the gap and the web.
    #[test]
    fn pads_under_the_mask_web_warn() {
        let i18n = I18n::en_us();
        let pads = [pad("1", 0.0, "F.Cu"), pad("2", 1.1, "F.Cu")];
        let lines = live_lines(Vec::new(), &pads, &i18n);
        assert_eq!(kinds(&lines), vec![LineKind::Warned]);
        let line = lines[0].1.clone().into_string();
        for part in ["1", "2", "0.1", "0.2", "solder-mask web"] {
            assert!(line.contains(part), "{part:?} in {line:?}");
        }
    }

    /// Mockup row 189: with nothing wrong, the passed line names the
    /// smallest pad gap.
    #[test]
    fn the_passed_line_names_the_smallest_gap() {
        let i18n = I18n::en_us();
        let pads = [pad("1", 0.0, "F.Cu"), pad("2", 1.5, "F.Cu")];
        let lines = live_lines(Vec::new(), &pads, &i18n);
        assert_eq!(kinds(&lines), vec![LineKind::Passed]);
        let line = lines[0].1.clone().into_string();
        assert!(
            line.contains("No overlaps") && line.contains("0.5"),
            "{line}"
        );
    }

    /// A parameter in error still lists the pad gap after it, as the
    /// mockup lists the drawing's checks after the errors.
    #[test]
    fn a_parameter_error_comes_before_the_pad_gap() {
        let i18n = I18n::en_us();
        let pads = [pad("1", 0.0, "F.Cu"), pad("2", 1.5, "F.Cu")];
        let problem = t!(i18n, ids::SKETCH_PARAMETERS_CHECKS_TITLE);
        let lines = live_lines(vec![problem], &pads, &i18n);
        assert_eq!(kinds(&lines), vec![LineKind::Failed, LineKind::Passed]);
    }

    #[test]
    fn overlaps_drop_the_passed_line() {
        let i18n = I18n::en_us();
        let pads = [pad("1", 0.0, "F.Cu"), pad("2", 0.5, "F.Cu")];
        assert_eq!(
            kinds(&live_lines(Vec::new(), &pads, &i18n)),
            vec![LineKind::Failed]
        );
    }

    #[test]
    fn with_no_pads_and_nothing_failed_one_passed_line() {
        let i18n = I18n::en_us();
        let lines = live_lines(Vec::new(), &[], &i18n);
        assert_eq!(kinds(&lines), vec![LineKind::Passed]);
        assert_eq!(
            lines[0].1.clone().into_string(),
            t!(i18n, ids::SKETCH_PARAMETERS_CHECKS_PASSED).into_string()
        );
        assert!(
            live_lines(
                vec![t!(i18n, ids::SKETCH_PARAMETERS_CHECKS_TITLE)],
                &[],
                &i18n
            )
            .iter()
            .all(|(kind, _)| *kind == LineKind::Failed)
        );
    }

    /// Layout probe: the card shows its title above the warning, each on
    /// one line.
    #[test]
    fn the_card_lays_out_its_title_above_the_warning() {
        use crate::layout_probe::lay_out_texts;
        use signex_types::theme::{ThemeId, theme_tokens};

        let i18n = I18n::en_us();
        let tokens = theme_tokens(ThemeId::GraphiteDark);
        let pads = [pad("1", 0.0, "F.Cu"), pad("2", 1.1, "F.Cu")];
        let texts = lay_out_texts(
            section(Vec::new(), &pads, Look::of(&tokens), &i18n),
            iced::Size::new(TS.body * 60.0, TS.body * 20.0),
        );
        let title = t!(i18n, ids::SKETCH_PARAMETERS_CHECKS_TITLE).into_string();
        let title = texts.iter().find(|t| t.text == title).expect("the title");
        let warning = texts
            .iter()
            .find(|t| t.text.contains("solder-mask web"))
            .expect("the warning");
        assert!(warning.bounds.y >= title.bounds.y + title.bounds.height);
        assert!(warning.bounds.height < TS.caption * 2.0, "one line");
    }
}
