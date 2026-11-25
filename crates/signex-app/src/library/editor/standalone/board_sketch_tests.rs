//! #2882, mockup row 179: the board sketch's chips, Active Bar and sketch
//! info bar laid out together at a narrow and a wide canvas, in both
//! Graphite themes. The Active Bar is mounted by the app view at the canvas
//! top; its rectangle here is the one that mount places.

use iced::{Rectangle, Size};
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::Label;
use signex_widgets::active_bar::{Metrics, slot_offsets_within};

use super::view_board_sketch;
use crate::app::BoardSketchSession;
use crate::layout_probe::{LaidOutText, find, lay_out_texts};
use crate::library::editor::footprint::pads_active_bar::EDITOR_BAR_CONTROL_HEIGHT;
use crate::library::editor::footprint::unified_active_bar as bar;

const THEMES: [ThemeId; 2] = [ThemeId::GraphiteLight, ThemeId::GraphiteDark];
const WIDTHS: [f32; 2] = [1000.0, 1600.0];

fn session() -> BoardSketchSession {
    let board: signex_types::pcb::PcbBoard =
        serde_json::from_value(serde_json::json!({ "uuid": uuid::Uuid::nil() })).expect("a board");
    BoardSketchSession::open(std::path::Path::new("Untitled.snxpcb"), &board)
}

/// The Active Bar of `session` in a canvas `width` wide at the window's
/// left edge, as `footprint_active_bar_overlay` places it.
fn active_bar_rect(session: &BoardSketchSession, theme: ThemeId, width: f32) -> Rectangle {
    let tokens = theme_tokens(theme);
    let i18n = signex_i18n::I18n::en_us();
    let items = bar::bar_items(&session.editor, theme, &tokens, &i18n);
    let run = bar::scrolling_slots(&session.editor);
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

/// The texts of the board sketch canvas laid out `width` x 800, without
/// Fluent's isolation marks.
fn canvas_texts(session: &BoardSketchSession, theme: ThemeId, width: f32) -> Vec<LaidOutText> {
    let tokens = theme_tokens(theme);
    let i18n = signex_i18n::I18n::en_us();
    lay_out_texts(
        view_board_sketch(&session.editor, &tokens, &i18n, None),
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
fn pill_label(session: &BoardSketchSession) -> String {
    let i18n = signex_i18n::I18n::en_us_ref();
    let (_, id) = super::super::dof_chip::dof_state(&session.editor);
    Label::from(signex_i18n::t!(i18n, id))
        .as_str()
        .replace(['\u{2068}', '\u{2069}'], "")
}

fn overlaps(a: &Rectangle, b: &Rectangle) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

fn caption(texts: &[LaidOutText]) -> &LaidOutText {
    texts
        .iter()
        .find(|t| t.text.starts_with("Board shape"))
        .unwrap_or_else(|| panic!("no info caption in {texts:#?}"))
}

#[test]
fn the_active_bar_covers_neither_chip_nor_the_info_bar() {
    let session = session();
    let pill = pill_label(&session);
    for theme in THEMES {
        for width in WIDTHS {
            let bar = active_bar_rect(&session, theme, width);
            let texts = canvas_texts(&session, theme, width);
            let chrome = [
                find(&texts, &pill),
                find(&texts, "Untitled"),
                caption(&texts),
                find(&texts, "Import DXF…"),
                find(&texts, "Parameters"),
            ];
            for t in chrome {
                assert!(
                    !overlaps(&t.bounds, &bar),
                    "{theme:?} at {width}: {t:?} is under the Active Bar {bar:?}"
                );
                assert!(
                    t.bounds.y >= bar.y + bar.height,
                    "{theme:?} at {width}: {t:?}"
                );
            }
        }
    }
}

#[test]
fn the_chips_and_the_info_bar_never_overlap() {
    let session = session();
    let pill = pill_label(&session);
    for theme in THEMES {
        for width in WIDTHS {
            let texts = canvas_texts(&session, theme, width);
            let chips = [find(&texts, &pill), find(&texts, "Untitled")];
            let info = [
                caption(&texts),
                find(&texts, "Import DXF…"),
                find(&texts, "Parameters"),
            ];
            for chip in chips {
                for item in info {
                    assert!(
                        !overlaps(&chip.bounds, &item.bounds),
                        "{theme:?} at {width}: {chip:?} under {item:?}"
                    );
                    assert!(chip.bounds.x < item.bounds.x, "{theme:?} at {width}");
                }
            }
        }
    }
}

#[test]
fn the_pill_text_is_laid_out_whole() {
    let session = session();
    let pill = pill_label(&session);
    for theme in THEMES {
        let natural = find(&canvas_texts(&session, theme, 100_000.0), &pill).bounds;
        for width in WIDTHS {
            let laid = find(&canvas_texts(&session, theme, width), &pill).bounds;
            assert!(
                laid.width >= natural.width - 0.5 && laid.height <= natural.height + 0.5,
                "{theme:?} at {width}: {laid:?} against {natural:?}"
            );
        }
    }
}

#[test]
fn the_info_bar_buttons_are_the_bar_control_height() {
    let session = session();
    for theme in THEMES {
        for width in WIDTHS {
            let texts = canvas_texts(&session, theme, width);
            for label in ["Import DXF…", "Parameters"] {
                let height = find(&texts, label).bounds.height;
                assert_eq!(
                    height, EDITOR_BAR_CONTROL_HEIGHT,
                    "{theme:?} at {width}: {label}"
                );
            }
        }
    }
}
