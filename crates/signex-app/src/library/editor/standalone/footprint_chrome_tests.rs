//! #2882, mockup row 187: the footprint editor's DOF chips, Active Bar and
//! editor bar laid out together at a narrow and a wide canvas, in both
//! Graphite themes and in Pads and Sketch mode. The Active Bar is mounted by
//! the app view at the canvas top; its rectangle here is the one that mount
//! places.

use iced::{Rectangle, Size};
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::Label;
use signex_widgets::active_bar::{Metrics, slot_offsets_within};

use crate::app::FootprintDocument;
use crate::layout_probe::{LaidOutText, find, lay_out_texts};
use crate::library::editor::footprint::state::EditorMode;
use crate::library::editor::footprint::unified_active_bar as bar;

const THEMES: [ThemeId; 2] = [ThemeId::GraphiteLight, ThemeId::GraphiteDark];
const WIDTHS: [f32; 2] = [1000.0, 1600.0];
const MODES: [EditorMode; 2] = [EditorMode::Normal, EditorMode::Sketch];

fn editor(mode: EditorMode) -> FootprintDocument {
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("Untitled"));
    let mut editor = FootprintDocument::new(std::path::PathBuf::from("Untitled.snxfpt"), file);
    editor.state.mode = mode;
    editor
}

/// The Active Bar of `editor` in a canvas `width` wide at the window's left
/// edge, as `footprint_active_bar_overlay` places it.
fn active_bar_rect(editor: &FootprintDocument, theme: ThemeId, width: f32) -> Rectangle {
    let tokens = theme_tokens(theme);
    let i18n = signex_i18n::I18n::en_us();
    let items = bar::bar_items(editor, theme, &tokens, &i18n);
    let run = bar::scrolling_slots(editor);
    let span = bar::BarSpan {
        x: 0.0,
        width,
        window_width: width,
    };
    let left = bar::bar_left_in(&items, run.clone(), span);
    let metrics = Metrics::GRAPHITE;
    let (_, bar_width) = slot_offsets_within(&items, run, bar::bar_max_width(width), &metrics);
    Rectangle {
        x: left,
        y: crate::active_bar::bar_top_margin(),
        width: bar_width,
        height: crate::active_bar::bar_height_for(&metrics),
    }
}

/// The texts of the footprint editor laid out `width` x 800, without
/// Fluent's isolation marks.
fn editor_texts(editor: &FootprintDocument, theme: ThemeId, width: f32) -> Vec<LaidOutText> {
    let tokens = theme_tokens(theme);
    let i18n = signex_i18n::I18n::en_us();
    lay_out_texts(
        super::super::view_footprint(editor, &tokens, theme, &i18n, false, "Inter", None),
        Size::new(width, 800.0),
    )
    .into_iter()
    .map(|t| LaidOutText {
        text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
        ..t
    })
    .collect()
}

/// The solve-state pill's text, in en-US.
fn pill_label(editor: &FootprintDocument) -> String {
    let i18n = signex_i18n::I18n::en_us_ref();
    let (_, id) = super::dof_state(editor);
    Label::from(signex_i18n::t!(i18n, id))
        .as_str()
        .replace(['\u{2068}', '\u{2069}'], "")
}

fn starting<'t>(texts: &'t [LaidOutText], prefix: &str) -> &'t LaidOutText {
    texts
        .iter()
        .find(|t| t.text.starts_with(prefix))
        .unwrap_or_else(|| panic!("no text starting {prefix:?} in {texts:#?}"))
}

/// The pill and the name chip.
fn chips<'t>(texts: &'t [LaidOutText], pill: &str) -> [&'t LaidOutText; 2] {
    [find(texts, pill), starting(texts, "Untitled ·")]
}

/// The top-most text reading `label`: the sketch mode bar at the bottom
/// is titled "Sketch" too.
fn top_most<'t>(texts: &'t [LaidOutText], label: &str) -> &'t LaidOutText {
    texts
        .iter()
        .filter(|t| t.text == label)
        .min_by(|a, b| a.bounds.y.total_cmp(&b.bounds.y))
        .unwrap_or_else(|| panic!("no {label:?} in {texts:#?}"))
}

/// The editor bar's texts in `mode`.
fn editor_bar(texts: &[LaidOutText], mode: EditorMode) -> Vec<&LaidOutText> {
    let mut items = vec![
        top_most(texts, "Footprint"),
        top_most(texts, "Sketch"),
        top_most(texts, "3D"),
    ];
    if mode == EditorMode::Sketch {
        items.push(starting(texts, "Footprint sketch"));
        items.push(find(texts, "Parameters"));
    }
    items
}

fn overlaps(a: &Rectangle, b: &Rectangle) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

#[test]
fn the_active_bar_covers_neither_the_chips_nor_the_editor_bar() {
    for mode in MODES {
        let editor = editor(mode);
        let pill = pill_label(&editor);
        for theme in THEMES {
            for width in WIDTHS {
                let bar = active_bar_rect(&editor, theme, width);
                let texts = editor_texts(&editor, theme, width);
                let mut chrome = chips(&texts, &pill).to_vec();
                chrome.extend(editor_bar(&texts, mode));
                for t in chrome {
                    assert!(
                        !overlaps(&t.bounds, &bar),
                        "{mode:?} {theme:?} at {width}: {t:?} is under the Active Bar {bar:?}"
                    );
                    assert!(
                        t.bounds.y >= bar.y + bar.height,
                        "{mode:?} {theme:?} at {width}: {t:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_chips_and_the_editor_bar_never_overlap() {
    for mode in MODES {
        let editor = editor(mode);
        let pill = pill_label(&editor);
        for theme in THEMES {
            for width in WIDTHS {
                let texts = editor_texts(&editor, theme, width);
                for chip in chips(&texts, &pill) {
                    for item in editor_bar(&texts, mode) {
                        assert!(
                            !overlaps(&chip.bounds, &item.bounds),
                            "{mode:?} {theme:?} at {width}: {chip:?} under {item:?}"
                        );
                        assert!(
                            chip.bounds.x < item.bounds.x,
                            "{mode:?} {theme:?} at {width}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn the_pill_text_is_laid_out_whole() {
    for mode in MODES {
        let editor = editor(mode);
        let pill = pill_label(&editor);
        for theme in THEMES {
            let natural = find(&editor_texts(&editor, theme, 100_000.0), &pill).bounds;
            for width in WIDTHS {
                let laid = find(&editor_texts(&editor, theme, width), &pill).bounds;
                assert!(
                    laid.width >= natural.width - 0.5 && laid.height <= natural.height + 0.5,
                    "{mode:?} {theme:?} at {width}: {laid:?} against {natural:?}"
                );
            }
        }
    }
}
