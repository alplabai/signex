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

/// Mockup row 142: the preview's corner control is the expand-arrows glyph,
/// and the shrink glyph once expanded.
#[test]
fn the_preview_control_draws_expand_arrows() {
    assert_eq!(super::expand_glyph(false), crate::preferences::GLYPH_EXPAND);
    assert_eq!(super::expand_glyph(true), crate::preferences::GLYPH_SHRINK);
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
    assert!(matches!(cancel.2, GridPropertiesMsg::CancelPressed));
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

/// Enter on a label holding text that is not a length is refused: the
/// label stays open, the draft keeps its step, and the preview's note
/// says why instead of the density note.
#[test]
fn a_label_holding_text_that_is_not_a_length_stays_open_with_a_note() {
    let mut st = draft("0.001", 1, GridDisplay::Lines, GridDisplay::Hidden);
    let send = |st: &mut GridPropertiesState, m| st.label_edited(m, en());
    send(&mut st, DimensionLabelMessage::LabelPressed(0));
    send(&mut st, DimensionLabelMessage::DraftEdited("abc".into()));
    assert!(st.open_label_refused(en()));
    send(&mut st, DimensionLabelMessage::DraftSubmitted);
    assert_eq!(st.preview_labels.editing_index(), Some(0));
    assert_eq!(st.preview_labels.draft(), Some("abc"));
    assert_eq!(st.step_mm, "0.001");
    let refused = en().message_plain(ids::DIALOG_GRID_LABEL_REFUSED, &[]);
    let texts: Vec<String> = primitives(&st)
        .into_iter()
        .filter_map(|p| match p {
            Primitive::Label(label) => Some(label.text.as_str().to_owned()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, [refused]);

    send(&mut st, DimensionLabelMessage::DraftEdited("0.2".into()));
    assert!(!st.open_label_refused(en()));
    send(&mut st, DimensionLabelMessage::DraftSubmitted);
    assert!(st.preview_labels.editing_index().is_none());
    assert_eq!(st.step_mm.parse::<f64>().ok(), Some(0.2));
}

/// Mockup row 142 (#3012): the scale pad is filled in top copper from the
/// board palette, with no outline, whatever the chrome theme.
#[test]
fn the_scale_pad_is_filled_top_copper() {
    use crate::styles::board_palette::{BoardPalette, BoardSwatch};
    use signex_widgets::property_illustration::{Paint, Tint};
    let st = draft("1", 5, GridDisplay::Lines, GridDisplay::Lines);
    let illustration = preview::illustration(&st, size(), en());
    let pads: Vec<_> = illustration
        .visible_parts()
        .filter(|p| matches!(p.primitive, Primitive::RoundedRect { .. }))
        .collect();
    assert_eq!(pads.len(), 1, "one pad for scale");
    let style = pads[0].style;
    assert!(style.stroke.is_none(), "filled, not outlined");
    let fill = style.fill.expect("the pad is filled");
    assert_eq!(fill.tint, Tint::Solid);
    let Paint::Document(colour) = fill.paint else {
        panic!("the pad takes a board colour, not a chrome ink: {fill:?}");
    };
    let copper = BoardPalette::new().swatch(BoardSwatch::TopCopper);
    assert_eq!(
        (colour.r, colour.g, colour.b),
        (copper.r, copper.g, copper.b)
    );
    assert!(colour.a >= 0.75, "a solid fill, not a tint: {}", colour.a);
    assert_eq!(colour, preview::pad_fill());
}

/// The preview's ground and inks are the board's: the board canvas colour
/// and the Graphite Dark tokens, under either chrome theme.
#[test]
fn the_preview_takes_the_board_ground_and_inks() {
    use crate::styles::board_palette::BoardPalette;
    use signex_types::theme::{ThemeId, theme_tokens};
    assert_eq!(preview::ground(), BoardPalette::new().canvas);
    let [r, g, b, _] = preview::ground().into_rgba8();
    assert_eq!((r, g, b), (0x11, 0x12, 0x14));
    assert_eq!(preview::board_tokens(), theme_tokens(ThemeId::GraphiteDark));
}

/// An app with a footprint editor in view, under the chrome theme `id`,
/// with Grid Properties opened through `Signex::update`.
fn app_with_grid_properties(id: signex_types::theme::ThemeId) -> crate::app::Signex {
    use crate::app::{FootprintDocument, Message, Signex, TabInfo, TabKind};
    let (mut app, _boot) = Signex::new();
    app.document_view.panel_ctx.tokens = signex_types::theme::theme_tokens(id);
    app.ui_state.window_size = (1400.0, 900.0);
    let path = std::path::PathBuf::from("/w/part.snxfpt");
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "part".to_string(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = 0;
    let _ = app.update(Message::GridProperties(GridPropertiesMsg::Open));
    assert!(app.ui_state.grid_properties.is_some(), "the dialog opened");
    app
}

fn rgb8(c: iced::Color) -> [u8; 3] {
    let [r, g, b, _] = c.into_rgba8();
    [r, g, b]
}

/// How many pixels of the drawn dialog are `want`, within `tolerance`.
fn pixels_of(app: &crate::app::Signex, want: [u8; 3], tolerance: u8) -> usize {
    let window = iced::Size::new(480.0, 600.0);
    let pixels = crate::layout_probe::draw_pixels(app.view_grid_properties_dialog_body(), window);
    pixels
        .chunks_exact(4)
        .filter(|p| (0..3).all(|i| p[i].abs_diff(want[i]) <= tolerance))
        .count()
}

/// Mockup row 142 (#3012): the drawn dialog shows the preview on the board
/// ground in the light chrome theme as in the dark one.
#[test]
fn the_drawn_preview_sits_on_the_board_ground_in_both_themes() {
    use signex_types::theme::ThemeId;
    let ground = rgb8(preview::ground());
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let app = app_with_grid_properties(id);
        let count = pixels_of(&app, ground, 1);
        // The preview is the dialog width by about 180 px; grid lines and
        // labels cover part of it.
        assert!(count >= 20_000, "{id:?}: {count} board-ground pixels");
    }
}

/// Mockup row 142 (#3012): the drawn scale pad is filled copper over the
/// board ground in both chrome themes.
#[test]
fn the_drawn_scale_pad_is_filled_copper_in_both_themes() {
    use signex_types::theme::ThemeId;
    let copper = rgb8(crate::styles::tokens::composite_over(
        preview::pad_fill(),
        preview::ground(),
    ));
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let app = app_with_grid_properties(id);
        let count = pixels_of(&app, copper, 3);
        assert!(
            count >= 150,
            "{id:?}: {count} copper pixels, want {copper:?}"
        );
    }
}
