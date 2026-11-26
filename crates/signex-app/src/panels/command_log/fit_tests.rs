//! The detail pane and the empty card at the default bottom dock height
//! (#3005, mockup row 161), from a row the GUI logged through
//! `Signex::update`.

use iced::Size;
use signex_i18n::{MessageId, ids, t};

use super::{CM, PanelContext, detail, layout, view};
use crate::app::{Message, Signex};
use crate::layout_probe::{LaidOutText, find, lay_out_texts};
use crate::menu_bar::MenuMessage;

/// The panel's content at the first-run bottom dock of the audit capture:
/// the dock's height less its title bar (the `SHORT_DOCK` of `tests`).
const DEFAULT_DOCK: Size = Size::new(890.0, crate::dock::GRAPHITE_BOTTOM_HEIGHT - CM.dock_title);
/// The panel's content in the mock (470 px less its title bar).
const MOCK_DOCK: Size = Size::new(890.0, 470.0 - CM.dock_title);

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

fn laid_out(ctx: &PanelContext, window: Size) -> Vec<LaidOutText> {
    lay_out_texts(view(ctx), window)
        .into_iter()
        .map(|t| LaidOutText {
            text: plain(&t.text),
            ..t
        })
        .collect()
}

fn say(ctx: &PanelContext, id: MessageId) -> String {
    plain(&t!(ctx.i18n, id).into_string())
}

/// The texts of the detail pane only (the table repeats some of them).
fn in_pane(texts: &[LaidOutText], window: Size) -> Vec<LaidOutText> {
    let left = window.width - layout::detail_width(window.width);
    texts
        .iter()
        .filter(|t| t.bounds.x >= left)
        .cloned()
        .collect()
}

/// A fresh app whose GUI ran Place ▸ Wire with no schematic open: one
/// failed row, through `Signex::update`.
fn app_with_a_failed_row() -> Signex {
    let (mut app, _boot) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::command("place_wire")));
    let log = &app.document_state.panel_ctx.command_log.log;
    let row = log.rows().next_back().expect("the GUI logged a row");
    assert!(row.outcome.is_failed(), "{row:?}");
    app
}

#[test]
fn only_a_pane_tall_enough_pins_the_actions() {
    assert!(!detail::pins_actions(0.0));
    assert!(!detail::pins_actions(DEFAULT_DOCK.height));
    assert!(!detail::pins_actions(detail::PIN_MIN_HEIGHT - 1.0));
    assert!(detail::pins_actions(detail::PIN_MIN_HEIGHT));
    assert!(detail::pins_actions(MOCK_DOCK.height));
}

/// #3005: at the default dock height the pane scrolls whole: its heading,
/// then Replay and its hint, then the error with Copy error, the arguments
/// and Copy as CLI under them, none of it behind a pinned block.
#[test]
fn the_default_dock_scrolls_the_whole_detail_pane_actions_first() {
    let app = app_with_a_failed_row();
    let ctx = &app.document_state.panel_ctx;
    let row = ctx.command_log.log.rows().next_back().expect("a row");
    let error = plain(row.outcome.text().expect("a failed row says why"));
    let texts = laid_out(ctx, DEFAULT_DOCK);
    let footer = find(&texts, &say(ctx, ids::COMMAND_LOG_FOOTER_SESSION)).bounds;
    let pane = in_pane(&texts, DEFAULT_DOCK);
    let title = find(&pane, &row.label).bounds;
    let replay = find(&pane, &say(ctx, ids::COMMAND_LOG_REPLAY)).bounds;
    let hint = find(&pane, &say(ctx, ids::COMMAND_LOG_REPLAY_FAILED_HINT)).bounds;
    let banner = find(&pane, &error).bounds;
    let copy_error = find(&pane, &say(ctx, ids::COMMAND_LOG_COPY_ERROR)).bounds;
    let cli_words = t!(ctx.i18n, ids::COMMAND_LOG_COPY_AS_CLI).into_string();
    let cli = find(&pane, &plain(&ctx.i18n.upper_for_display(&cli_words))).bounds;
    assert!(
        replay.y >= title.y + title.height,
        "{replay:?} vs {title:?}"
    );
    assert!(
        replay.y + replay.height <= footer.y,
        "{replay:?} vs {footer:?}"
    );
    assert!(hint.y >= replay.y + replay.height, "{hint:?}");
    assert!(banner.y >= hint.y + hint.height, "{banner:?} vs {hint:?}");
    assert!(copy_error.y >= banner.y + banner.height, "{copy_error:?}");
    assert!(cli.y >= copy_error.y + copy_error.height, "{cli:?}");
}

/// A dock of the mock's height keeps the pinned layout: the error scrolls
/// in the body above Replay and its hint, both inside the panel.
#[test]
fn a_tall_dock_pins_replay_under_the_body() {
    let app = app_with_a_failed_row();
    let ctx = &app.document_state.panel_ctx;
    let row = ctx.command_log.log.rows().next_back().expect("a row");
    let error = plain(row.outcome.text().expect("a failed row says why"));
    let texts = laid_out(ctx, MOCK_DOCK);
    let footer = find(&texts, &say(ctx, ids::COMMAND_LOG_FOOTER_SESSION)).bounds;
    let pane = in_pane(&texts, MOCK_DOCK);
    let banner = find(&pane, &error).bounds;
    let replay = find(&pane, &say(ctx, ids::COMMAND_LOG_REPLAY)).bounds;
    let hint = find(&pane, &say(ctx, ids::COMMAND_LOG_REPLAY_FAILED_HINT)).bounds;
    assert!(
        replay.y >= banner.y + banner.height,
        "{replay:?} vs {banner:?}"
    );
    assert!(hint.y >= replay.y + replay.height, "{hint:?}");
    assert!(hint.y + hint.height <= footer.y, "{hint:?} vs {footer:?}");
}

/// #3005: the empty card keeps its third line, the `signex-cli commands`
/// key and its caption, under the body, at the default dock height and in
/// a dock of the mock's height, where the whole card sits above the footer.
#[test]
fn the_empty_card_keeps_its_key_line() {
    let (app, _boot) = Signex::new();
    let ctx = &app.document_state.panel_ctx;
    assert!(ctx.command_log.log.is_empty());
    for window in [DEFAULT_DOCK, MOCK_DOCK] {
        let texts = laid_out(ctx, window);
        let source = find(&texts, &say(ctx, ids::COMMAND_LOG_SOURCE)).bounds;
        let title = find(&texts, &say(ctx, ids::COMMAND_LOG_EMPTY_TITLE)).bounds;
        let body = find(&texts, &say(ctx, ids::COMMAND_LOG_EMPTY_BODY)).bounds;
        let key = find(&texts, &say(ctx, ids::COMMAND_LOG_EMPTY_KEY)).bounds;
        let caption = find(&texts, &say(ctx, ids::COMMAND_LOG_EMPTY_KEY_CAPTION)).bounds;
        assert!(
            title.y >= source.y + source.height,
            "{title:?} at {window:?}"
        );
        assert!(body.y >= title.y + title.height, "{body:?} at {window:?}");
        assert!(key.y >= body.y + body.height, "{key:?} at {window:?}");
        assert!(caption.x >= key.x + key.width, "{caption:?} at {window:?}");
    }
    let texts = laid_out(ctx, MOCK_DOCK);
    let footer = find(&texts, &say(ctx, ids::COMMAND_LOG_FOOTER_SESSION)).bounds;
    let caption = find(&texts, &say(ctx, ids::COMMAND_LOG_EMPTY_KEY_CAPTION)).bounds;
    assert!(caption.y + caption.height <= footer.y, "{caption:?}");
}
