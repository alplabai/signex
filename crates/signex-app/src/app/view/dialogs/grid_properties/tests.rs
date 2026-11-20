use signex_i18n::{I18n, ids};
use signex_widgets::dimension_label::DimensionLabelMessage;
use signex_widgets::property_illustration::Primitive;

use super::preview::{self, PreviewLabel, PreviewSize};
use super::{coarse_caption, footer_buttons};
use crate::app::{GridPropertiesMsg, GridPropertiesState};
use crate::library::editor::footprint::GridDisplay;
use crate::styles::DialogButtonKind;

fn draft(
    step: &str,
    multiplier: u32,
    fine: GridDisplay,
    coarse: GridDisplay,
) -> GridPropertiesState {
    GridPropertiesState {
        step_mm: step.to_string(),
        fine_display: fine,
        coarse_display: coarse,
        multiplier,
        ..GridPropertiesState::default()
    }
}

fn en() -> &'static I18n {
    I18n::en_us_ref()
}

fn size() -> PreviewSize {
    PreviewSize::new(480.0, false, 11.0)
}

fn primitives(st: &GridPropertiesState) -> Vec<Primitive> {
    preview::illustration(st, size(), en())
        .visible_parts()
        .map(|p| p.primitive.clone())
        .collect()
}

fn lines(prims: &[Primitive]) -> usize {
    prims
        .iter()
        .filter(|p| matches!(p, Primitive::Line { .. }))
        .count()
}

fn notes(prims: &[Primitive]) -> usize {
    prims
        .iter()
        .filter(|p| matches!(p, Primitive::Label(_)))
        .count()
}

/// Mockup row 142: the dialog is "Grid Properties" with the "Cartesian · mm" note.
#[test]
fn the_title_is_grid_properties_with_the_cartesian_note() {
    assert_eq!(
        en().message_plain(ids::DIALOG_TITLE_GRID_PROPERTIES, &[]),
        "Grid Properties"
    );
    assert_eq!(
        en().message_plain(ids::DIALOG_GRID_NOTE, &[]),
        "Cartesian · mm"
    );
    assert_eq!(
        en().message_plain(ids::DIALOG_GRID_FOOTER, &[]),
        "One grid per footprint editor"
    );
}

/// Mockup row 142: OK sends the shipped Apply; Cancel closes.
#[test]
fn ok_sends_apply_and_cancel_closes() {
    let [cancel, ok] = footer_buttons();
    assert_eq!(en().message_plain(cancel.0, &[]), "Cancel");
    assert_eq!(cancel.1, DialogButtonKind::Cancel);
    assert!(matches!(cancel.2, GridPropertiesMsg::Close));
    assert_eq!(en().message_plain(ok.0, &[]), "OK");
    assert_eq!(ok.1, DialogButtonKind::Primary);
    assert!(matches!(ok.2, GridPropertiesMsg::Apply));
}

/// The multiplier caption reads the coarse spacing, step × multiplier.
#[test]
fn the_multiplier_caption_reads_the_coarse_spacing() {
    let st = draft("0.127", 5, GridDisplay::Dots, GridDisplay::Lines);
    assert_eq!(
        // Fluent isolates the placeable; the words are what is checked.
        coarse_caption(&st, en()).map(|c| c.into_string().replace(['\u{2068}', '\u{2069}'], "")),
        Some("coarse = 0.635".to_string())
    );
    let bad = draft("0", 5, GridDisplay::Dots, GridDisplay::Lines);
    assert!(coarse_caption(&bad, en()).is_none());
}

/// A Hidden grid leaves the drawing; the origin cross stays.
#[test]
fn a_hidden_grid_leaves_the_preview() {
    let both = draft("0.127", 5, GridDisplay::Lines, GridDisplay::Lines);
    let none = draft("0.127", 5, GridDisplay::Hidden, GridDisplay::Hidden);
    let coarse_only = draft("0.127", 5, GridDisplay::Hidden, GridDisplay::Lines);
    let (all, bare, coarse) = (
        lines(&primitives(&both)),
        lines(&primitives(&none)),
        lines(&primitives(&coarse_only)),
    );
    assert_eq!(bare, 2, "only the origin cross");
    assert!(bare < coarse && coarse < all, "{bare} {coarse} {all}");
    // Dots draw circles, not lines.
    let dots = draft("0.127", 5, GridDisplay::Dots, GridDisplay::Hidden);
    assert_eq!(lines(&primitives(&dots)), 2);
    assert!(
        primitives(&dots)
            .iter()
            .any(|p| matches!(p, Primitive::Circle { .. }))
    );
}

/// A fine grid too dense to draw is left out, and a note says so.
#[test]
fn a_too_dense_fine_grid_says_so() {
    let dense = draft("0.001", 1, GridDisplay::Lines, GridDisplay::Hidden);
    let prims = primitives(&dense);
    assert_eq!(lines(&prims), 2);
    assert_eq!(notes(&prims), 1);
    let fine = draft("0.127", 5, GridDisplay::Lines, GridDisplay::Hidden);
    assert_eq!(notes(&primitives(&fine)), 0);
}

/// The coarse label goes with the coarse grid; neither shows without a
/// valid step.
#[test]
fn the_labels_follow_the_grids() {
    let kinds = |st: &GridPropertiesState| -> Vec<PreviewLabel> {
        preview::labels(st, size(), en())
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    };
    let st = draft("0.127", 5, GridDisplay::Dots, GridDisplay::Lines);
    assert_eq!(kinds(&st), [PreviewLabel::Step, PreviewLabel::Coarse]);
    let st = draft("0.127", 5, GridDisplay::Dots, GridDisplay::Hidden);
    assert_eq!(kinds(&st), [PreviewLabel::Step]);
    let st = draft("abc", 5, GridDisplay::Dots, GridDisplay::Lines);
    assert!(kinds(&st).is_empty());
}

#[test]
fn the_nearest_multiplier_keeps_the_smaller_on_a_tie() {
    use crate::app::contracts::grid_properties::nearest_multiplier;
    assert_eq!(nearest_multiplier(0.2), 1);
    assert_eq!(nearest_multiplier(4.0), 5);
    assert_eq!(nearest_multiplier(3.5), 2);
    assert_eq!(nearest_multiplier(7.5), 5);
    assert_eq!(nearest_multiplier(100.0), 10);
}

/// Enter on the step label writes the step field; on the coarse label it
/// picks the multiplier closest to the typed spacing.
#[test]
fn a_label_commit_writes_the_draft() {
    let mut st = draft("0.127", 5, GridDisplay::Dots, GridDisplay::Lines);
    let send = |st: &mut GridPropertiesState, m| st.label_edited(m, en());
    assert!(send(&mut st, DimensionLabelMessage::LabelPressed(0)));
    send(&mut st, DimensionLabelMessage::DraftEdited("0.2".into()));
    send(&mut st, DimensionLabelMessage::DraftSubmitted);
    let step: f64 = st.step_mm.parse().expect("a number");
    assert!((step - 0.2).abs() < 1e-9, "{}", st.step_mm);
    assert_eq!(st.multiplier, 5);

    assert!(send(&mut st, DimensionLabelMessage::LabelPressed(1)));
    send(&mut st, DimensionLabelMessage::DraftEdited("2".into()));
    send(&mut st, DimensionLabelMessage::DraftSubmitted);
    assert_eq!(st.multiplier, 10);
    assert!(st.preview_labels.editing_index().is_none());
}
