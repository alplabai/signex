//! #86: the Messages panel's level chips, verbosity rule, filters and Clear.

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
    }
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
    let mut ctx = app.document_state.panel_ctx.clone();
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
    let mut ctx = app.document_state.panel_ctx.clone();
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
    let mut ctx = app.document_state.panel_ctx.clone();
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
    let mut ctx = app.document_state.panel_ctx.clone();
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
