//! #86: the Messages panel's level chips, verbosity rule, filters and Clear.

use std::path::PathBuf;

use signex_types::schematic::{SelectedItem, SelectedKind};

use super::*;
use crate::layout_probe::{lay_out_texts, messages_on_click};

fn entry(id: u64, level: DiagnosticLevel, code: &str, message: &str) -> DiagnosticEntry {
    DiagnosticEntry {
        id,
        level,
        code: code.into(),
        message: message.into(),
        at: chrono::Utc::now(),
        link: None,
        document: None,
        object: None,
    }
}

/// An entry about `object` on the sheet `document`.
fn entry_at(
    id: u64,
    message: &str,
    document: &str,
    object: Option<SelectedItem>,
) -> DiagnosticEntry {
    DiagnosticEntry {
        document: Some(PathBuf::from(document)),
        object,
        ..entry(id, DiagnosticLevel::Info, "SIGNEX-FIND_SIMILAR", message)
    }
}

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

/// Mockup row 86: the Document filter keeps the rows that name its sheet;
/// a row that names none is kept only by All documents.
#[test]
fn the_document_filter_keeps_the_rows_that_name_its_sheet() {
    let mut entries = log();
    entries.push(entry_at(7, "R3 on power", "/w/p/power.snxsch", None));
    entries.push(entry_at(8, "R4 on mcu", "/w/p/mcu.snxsch", None));
    let i18n = I18n::en_us();
    let mut view = MessagesView::default();
    let all = shown(&entries, &view, false, &i18n).len();
    assert_eq!(all, 6, "All documents keeps every row");
    view.apply(
        MessagesPanelMsg::DocumentPicked(Some(PathBuf::from("/w/p/power.snxsch"))),
        None,
    );
    let ids: Vec<u64> = shown(&entries, &view, false, &i18n)
        .iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids, [7]);
    let visible = visible(&entries, &MessagesView::default(), false);
    assert_eq!(
        super::named_documents(&visible),
        [
            PathBuf::from("/w/p/mcu.snxsch"),
            PathBuf::from("/w/p/power.snxsch")
        ],
        "the dropdown lists each named sheet once, by name"
    );
}

/// Mockup row 86: a row that names a sheet shows it in the Document cell
/// and offers Show, which sends the sheet and the object; a plain click on
/// the row's text sends nothing.
#[test]
fn a_row_that_names_a_sheet_shows_it_and_offers_show() {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_view.panel_ctx.clone();
    let item = SelectedItem::new(uuid::Uuid::now_v7(), SelectedKind::Symbol);
    ctx.diagnostics = vec![
        entry(1, DiagnosticLevel::Info, "SIGNEX-PROJECT", "no sheet here"),
        entry_at(2, "Symbol R3 elsewhere", "/w/p/power.snxsch", Some(item)),
    ];
    ctx.diagnostics_level = "info".into();
    let window = iced::Size::new(1400.0, 600.0);
    let shown: Vec<String> = lay_out_texts(view_messages(&ctx), window)
        .into_iter()
        .map(|t| plain(&t.text))
        .collect();
    assert!(shown.iter().any(|t| t == "power.snxsch"), "{shown:?}");
    assert_eq!(
        shown.iter().filter(|t| *t == "Show").count(),
        1,
        "only the row that names a sheet offers Show: {shown:?}"
    );
    let pressed = messages_on_click(view_messages(&ctx), window, "Show");
    assert!(
        matches!(
            pressed.as_slice(),
            [PanelMsg::MessagesView(MessagesPanelMsg::ShowPressed { document, object })]
                if document == &PathBuf::from("/w/p/power.snxsch") && *object == Some(item)
        ),
        "{pressed:?}"
    );
    let clicked = messages_on_click(view_messages(&ctx), window, "Symbol R3 elsewhere");
    assert!(clicked.is_empty(), "{clicked:?}");
}

fn log() -> Vec<DiagnosticEntry> {
    vec![
        entry(
            1,
            DiagnosticLevel::Info,
            "SIGNEX-PROJECT",
            "Update PCB: 21 changes",
        ),
        entry(
            2,
            DiagnosticLevel::Warning,
            "SIGNEX-LIBRARY",
            "library not opened",
        ),
        entry(
            3,
            DiagnosticLevel::Error,
            "SIGNEX-PROJECT",
            "report not written",
        ),
        entry(4, DiagnosticLevel::Debug, "SIGNEX-GFX", "frame took 3 ms"),
        entry(5, DiagnosticLevel::Trace, "SIGNEX-GFX", "glyph cache hit"),
        entry(6, DiagnosticLevel::Info, "SIGNEX-ERC", "ERC: 3 errors"),
    ]
}

#[test]
fn the_chips_count_each_level_and_debug_rows_are_hidden_by_default() {
    let entries = log();
    let view = MessagesView::default();
    let quiet = visible(&entries, &view, false);
    assert_eq!(level_counts(&quiet), [4, 1, 1, 2]);
    assert!(
        quiet
            .iter()
            .all(|e| !matches!(e.level, DiagnosticLevel::Debug | DiagnosticLevel::Trace))
    );
    assert_eq!(quiet[0].id, 6, "newest first");
    let verbose = visible(&entries, &view, true);
    assert_eq!(level_counts(&verbose), [6, 1, 1, 2]);
    assert!(is_verbose("debug") && is_verbose("trace"));
    assert!(!is_verbose("info") && !is_verbose("warn"));
}

#[test]
fn a_level_chip_source_and_text_filter_narrow_the_rows() {
    let entries = log();
    let i18n = I18n::en_us();
    let mut view = MessagesView::default();
    view.apply(MessagesPanelMsg::LevelPicked(MessageLevel::Info), None);
    let ids: Vec<u64> = shown(&entries, &view, false, &i18n)
        .iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids, [6, 1]);
    view.apply(
        MessagesPanelMsg::SourcePicked(Some(MessageSource::Erc)),
        None,
    );
    let ids: Vec<u64> = shown(&entries, &view, false, &i18n)
        .iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids, [6]);
    view = MessagesView::default();
    view.apply(MessagesPanelMsg::QueryEdited("REPORT".into()), None);
    let ids: Vec<u64> = shown(&entries, &view, false, &i18n)
        .iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids, [3], "the filter text matches any case");
}

#[test]
fn clear_hides_what_was_logged_so_far_and_keeps_new_lines() {
    let mut entries = log();
    let mut view = MessagesView::default();
    view.apply(MessagesPanelMsg::Cleared, Some(6));
    assert!(visible(&entries, &view, true).is_empty());
    entries.push(entry(
        7,
        DiagnosticLevel::Warning,
        "SIGNEX-DRC",
        "after the clear",
    ));
    let ids: Vec<u64> = visible(&entries, &view, false)
        .iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids, [7]);
}

#[test]
fn copy_writes_the_shown_rows_as_tab_separated_lines() {
    let entries = log();
    let i18n = I18n::en_us();
    let view = MessagesView {
        level: MessageLevel::Error,
        ..MessagesView::default()
    };
    let text = copy_text(&shown(&entries, &view, false, &i18n), &i18n)
        .replace(['\u{2068}', '\u{2069}'], "");
    let cells: Vec<&str> = text.split('\t').collect();
    assert_eq!(cells.len(), 4, "{text}");
    assert_eq!(cells[0], "ERROR");
    assert_eq!(cells[1], "Project");
    assert_eq!(cells[3], "report not written");
}

#[test]
fn the_panel_draws_the_chips_and_an_error_row_and_a_chip_picks_its_level() {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_view.panel_ctx.clone();
    ctx.diagnostics = log();
    ctx.diagnostics_level = "info".into();
    let window = iced::Size::new(1200.0, 2000.0);
    let shown: Vec<String> = lay_out_texts(view_messages(&ctx), window)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
    assert!(shown.iter().any(|t| t == "report not written"), "{shown:?}");
    assert!(!shown.iter().any(|t| t == "frame took 3 ms"), "{shown:?}");
    assert!(shown.iter().any(|t| t == "MESSAGE"), "{shown:?}");
    let pressed = messages_on_click(view_messages(&ctx), window, "WARN");
    assert!(
        matches!(
            pressed.as_slice(),
            [PanelMsg::MessagesView(MessagesPanelMsg::LevelPicked(
                MessageLevel::Warning
            ))]
        ),
        "{pressed:?}"
    );
}

/// A row with a link shows its command as a button; pressing it sends the
/// link with the sheet the row names.
#[test]
fn a_linked_row_shows_its_command_and_pressing_it_sends_the_link() {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_view.panel_ctx.clone();
    let named = std::path::PathBuf::from("/w/a.snxsch");
    let mut linked = entry(
        1,
        DiagnosticLevel::Warning,
        "SIGNEX-SCHEMATIC",
        "a.snxsch: 1 pin touches a wire without a junction",
    );
    linked.link = Some(crate::diagnostics::MessageLink::RepairPinJunctions(
        named.clone(),
    ));
    ctx.diagnostics = vec![linked];
    ctx.diagnostics_level = "info".into();
    let window = iced::Size::new(1200.0, 600.0);
    let shown: Vec<String> = lay_out_texts(view_messages(&ctx), window)
        .into_iter()
        .map(|t| t.text)
        .collect();
    assert!(
        shown.iter().any(|t| t == "Repair Pin Junctions"),
        "{shown:?}"
    );
    let pressed = messages_on_click(view_messages(&ctx), window, "Repair Pin Junctions");
    assert!(
        matches!(
            pressed.as_slice(),
            [PanelMsg::MessageLinkPressed(
                crate::diagnostics::MessageLink::RepairPinJunctions(path)
            )] if *path == named
        ),
        "{pressed:?}"
    );
}

#[test]
fn a_row_without_a_link_shows_no_command() {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_view.panel_ctx.clone();
    ctx.diagnostics = log();
    ctx.diagnostics_level = "info".into();
    let shown: Vec<String> = lay_out_texts(view_messages(&ctx), iced::Size::new(1200.0, 2000.0))
        .into_iter()
        .map(|t| t.text)
        .collect();
    assert!(
        !shown.iter().any(|t| t == "Repair Pin Junctions"),
        "{shown:?}"
    );
}

#[test]
fn an_empty_log_shows_the_centred_empty_state() {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_view.panel_ctx.clone();
    ctx.diagnostics = Vec::new();
    let shown: Vec<String> = lay_out_texts(view_messages(&ctx), iced::Size::new(560.0, 150.0))
        .into_iter()
        .map(|t| t.text)
        .collect();
    assert!(
        shown.iter().any(|t| t == "No runtime messages yet"),
        "{shown:?}"
    );
    assert!(
        shown
            .iter()
            .any(|t| t == "RUST_LOG=debug  ·  SIGNEX_LOG=debug"),
        "{shown:?}"
    );
}

/// Mockup row 86: ERROR, WARN and INFO are tags in their level's tone (the
/// row pill's colours), not the neutral chip ink.
#[test]
fn each_level_chip_tag_takes_its_level_tone() {
    let (app, _boot) = crate::app::Signex::new();
    let tokens = &app.document_view.panel_ctx.tokens;
    for (level, tone) in [
        (DiagnosticLevel::Error, PillTone::Err),
        (DiagnosticLevel::Warning, PillTone::Warn),
        (DiagnosticLevel::Info, PillTone::Info),
    ] {
        let (fill, ink) = crate::styles::pill_colors(tokens, tone);
        assert_eq!(level_tag_tone(tokens, level), ChipTone { fill, ink });
        assert_ne!(ink, chip_style(tokens).ink, "{level:?}");
    }
    let ink = |level| level_tag_tone(tokens, level).ink;
    assert_ne!(ink(DiagnosticLevel::Error), ink(DiagnosticLevel::Warning));
    assert_ne!(ink(DiagnosticLevel::Warning), ink(DiagnosticLevel::Info));
}

/// Mockup row 86: a level chip is its tag and its count, the tag drawn at
/// the smaller badge size, on one line with the count.
#[test]
fn the_level_chips_draw_a_tag_and_a_count_each() {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_view.panel_ctx.clone();
    ctx.diagnostics = log();
    ctx.diagnostics_level = "info".into();
    let entries = ctx.diagnostics.clone();
    let shown = visible(&entries, &ctx.validation.messages, false);
    let texts = lay_out_texts(level_chips(&ctx, &shown), iced::Size::new(1200.0, 200.0));
    let words: Vec<String> = texts.iter().map(|t| plain(&t.text)).collect();
    assert_eq!(
        words,
        ["All", "4", "ERROR", "1", "WARN", "1", "INFO", "2"],
        "{texts:#?}"
    );
    let (all, error, count) = (&texts[0].bounds, &texts[2].bounds, &texts[3].bounds);
    assert!(error.height < all.height, "{error:?} vs {all:?}");
    assert!(
        count.x >= error.x + error.width,
        "{count:?} after {error:?}"
    );
    let centre = |b: &iced::Rectangle| b.y + b.height / 2.0;
    assert!((centre(error) - centre(count)).abs() < 2.0, "{texts:#?}");
}

/// Mockup row 86: the filter field leads with the search glyph, so its
/// input starts after the field padding and one glyph.
#[test]
fn the_filter_field_leads_with_the_search_glyph() {
    let (app, _boot) = crate::app::Signex::new();
    let ctx = app.document_view.panel_ctx.clone();
    let inputs =
        crate::layout_probe::lay_out_inputs(filter_field(&ctx), iced::Size::new(600.0, 100.0));
    let input = inputs
        .iter()
        .find(|i| plain(&i.text) == "Filter messages")
        .unwrap_or_else(|| panic!("{inputs:#?}"));
    assert!(input.bounds.x >= SP.m + TS.body, "{input:#?}");
}
