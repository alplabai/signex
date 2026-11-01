//! #1830 — a file that only differs from the canonical layout (CRLF line
//! endings, a UTF-8 BOM, hand-aligned TSV columns) opens clean, through
//! the real `FileMsg::Opened` read + parse + apply path.

use std::path::Path;

use futures::StreamExt;
use iced::Task;
use signex_app::app::{FileMsg, Message, Signex};
use tempfile::TempDir;

fn run(task: Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

fn canonical_sheet_text() -> String {
    let mut sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    sheet.wires.push(signex_types::schematic::Wire {
        uuid: uuid::Uuid::now_v7(),
        start: signex_types::schematic::Point::new(10.0, 10.0),
        end: signex_types::schematic::Point::new(20.0, 10.0),
        ..Default::default()
    });
    signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise")
}

/// Open `text` through `FileMsg::Opened` and report whether the tab is dirty.
fn open_is_dirty(text: &[u8]) -> (bool, bool) {
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Sheet.snxsch");
    std::fs::write(&path, text).expect("write");
    let (mut app, _boot) = Signex::new();
    let mut pending = run(app.update(Message::File(FileMsg::Opened(Some(path.clone())))));
    // Follow the follow-ups (recovery check, scans) the way the runtime would.
    for _ in 0..20 {
        if pending.is_empty() {
            break;
        }
        let mut next = Vec::new();
        for message in pending {
            next.extend(run(app.update(message)));
        }
        pending = next;
    }
    let opened = app.document_state.tabs.iter().any(|t| t.path == path);
    (opened, dirty(&app, &path))
}

fn dirty(app: &Signex, path: &Path) -> bool {
    app.document_state.is_document_dirty(path) || app.document_state.has_dirty_documents()
}

#[test]
fn a_canonical_file_opens_clean() {
    let (opened, is_dirty) = open_is_dirty(canonical_sheet_text().as_bytes());
    assert!(opened, "the file must open");
    assert!(!is_dirty);
}

#[test]
fn a_crlf_file_opens_clean() {
    let text = canonical_sheet_text().replace('\n', "\r\n");
    let (opened, is_dirty) = open_is_dirty(text.as_bytes());
    assert!(opened, "a CRLF file must open");
    assert!(!is_dirty, "line endings alone are not an edit");
}

#[test]
fn a_bom_file_opens_clean() {
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend_from_slice(canonical_sheet_text().as_bytes());
    let (opened, is_dirty) = open_is_dirty(&bytes);
    assert!(opened, "a BOM file must open");
    assert!(!is_dirty, "a BOM alone is not an edit");
}

#[test]
fn a_bom_and_crlf_file_opens_clean() {
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend_from_slice(canonical_sheet_text().replace('\n', "\r\n").as_bytes());
    let (opened, is_dirty) = open_is_dirty(&bytes);
    assert!(opened);
    assert!(!is_dirty);
}

#[test]
fn a_hand_aligned_file_opens_clean() {
    let text = canonical_sheet_text().replace("  ", "      ");
    let (opened, is_dirty) = open_is_dirty(text.as_bytes());
    assert!(opened);
    assert!(!is_dirty);
}

fn text_with_wires(second_end_x: f64) -> String {
    let mut sheet = signex_types::schematic::SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    let wire = |end_x: f64| signex_types::schematic::Wire {
        uuid: uuid::Uuid::from_u128(7),
        start: signex_types::schematic::Point::new(10.0, 10.0),
        end: signex_types::schematic::Point::new(end_x, 10.0),
        ..Default::default()
    };
    sheet.wires = vec![wire(20.0), wire(second_end_x)];
    signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise")
}

#[test]
fn a_file_repaired_on_open_is_dirty_and_says_so_in_messages() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("RepairedSheet1830.snxsch");
    std::fs::write(&path, text_with_wires(30.0)).expect("write");
    let (mut app, _boot) = Signex::new();
    for message in run(app.update(Message::File(FileMsg::Opened(Some(path.clone()))))) {
        let _ = app.update(message);
    }
    assert!(
        app.document_state.is_document_dirty(&path),
        "the repair is unsaved"
    );
    let told = signex_app::diagnostics::recent_entries()
        .iter()
        .any(|entry| {
            entry
                .message
                .contains("RepairedSheet1830.snxsch repaired on open: wire (row 2)")
        });
    assert!(told, "Messages must name the repair");
    let engine = app.document_state.engines.get(&path).expect("engine");
    assert_eq!(engine.document().wires.len(), 2);
}
