//! Driving a paste in a headless test: the request goes through
//! `EditMsg::Paste`, and the system clipboard's answer — which only a running
//! window can give — is delivered by hand.

use signex_app::app::{EditMsg, Message, PasteIntent, PasteRequest, Signex};
use signex_app::paste_prefs::PasteDesignators;

/// The request a paste made now would carry.
pub(crate) fn request(app: &Signex, intent: PasteIntent) -> PasteRequest {
    PasteRequest {
        intent,
        epoch: app.interaction_state.placement_epoch,
        path: app.document_state.active_path.clone(),
    }
}

/// Run the numbering read a request started (when it started one) and hand
/// its answer back, as the runtime would. A task that waits for the system
/// clipboard is never run: only Next free and Paste Array read first.
pub(crate) fn settle(app: &mut Signex, task: iced::Task<Message>, reads_numbers: bool) {
    if !reads_numbers {
        return;
    }
    for message in crate::library_place_symbol::run(task) {
        if matches!(message, Message::Edit(EditMsg::PasteCountersSeeded { .. })) {
            let _ = app.update(message);
        }
    }
}

fn reads_numbers(app: &Signex, intent: PasteIntent) -> bool {
    intent == PasteIntent::Array || app.ui_state.paste_designators == PasteDesignators::NextFree
}

/// Send `message` and settle the numbering read it starts, for Duplicate and
/// rubber stamp, which have no clipboard read.
pub(crate) fn send_and_settle(app: &mut Signex, message: EditMsg, intent: PasteIntent) {
    let task = app.update(Message::Edit(message));
    let reads = reads_numbers(app, intent);
    settle(app, task, reads);
}

/// `text` as the system clipboard's answer to a paste request.
pub(crate) fn paste_with_clipboard(app: &mut Signex, text: Option<&str>) {
    let task = app.update(Message::Edit(EditMsg::Paste));
    let reads = reads_numbers(app, PasteIntent::Paste);
    settle(app, task, reads);
    answer(app, PasteIntent::Paste, text);
}

/// A paste with nothing usable on the system clipboard: the copy made in the
/// window is pasted.
pub(crate) fn paste(app: &mut Signex) {
    paste_with_clipboard(app, None);
}

/// Smart Paste with nothing usable on the system clipboard.
pub(crate) fn smart_paste(app: &mut Signex) {
    let task = app.update(Message::Edit(EditMsg::SmartPaste));
    let reads = reads_numbers(app, PasteIntent::SmartPaste);
    settle(app, task, reads);
    answer(app, PasteIntent::SmartPaste, None);
}

/// The clipboard's answer to a request made now.
pub(crate) fn answer(app: &mut Signex, intent: PasteIntent, text: Option<&str>) {
    let request = request(app, intent);
    answer_request(app, request, text);
}

pub(crate) fn answer_request(app: &mut Signex, request: PasteRequest, text: Option<&str>) {
    let _ = app.update(Message::Edit(EditMsg::PasteTextRead {
        request,
        text: text.map(str::to_owned),
    }));
}

/// What a `Task` asked the runtime to do, without a runtime to do it.
pub(crate) fn actions(task: iced::Task<Message>) -> Vec<iced_runtime::Action<Message>> {
    use futures::StreamExt;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            out.push(action);
        }
        out
    })
}

/// The text a task writes to the system clipboard, if it writes any.
pub(crate) fn written_text(task: iced::Task<Message>) -> Option<String> {
    actions(task).into_iter().find_map(|action| match action {
        iced_runtime::Action::Clipboard(iced_runtime::clipboard::Action::Write {
            contents,
            ..
        }) => Some(contents),
        _ => None,
    })
}

/// Whether a task asks the system clipboard for its text. Only the first
/// action is looked at: a read then waits for an answer that needs a window.
pub(crate) fn reads_clipboard(task: iced::Task<Message>) -> bool {
    use futures::StreamExt;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return false;
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let first = runtime.block_on(stream.next());
    matches!(
        first,
        Some(iced_runtime::Action::Clipboard(
            iced_runtime::clipboard::Action::Read { .. }
        ))
    )
}

pub(crate) mod sheet {
    //! A sheet with parts and a label to copy.

    use signex_app::app::{Message, Signex, TabInfo, TabKind};
    use signex_types::schematic::{
        Label, LabelType, LibSymbol, Point, SchematicSheet, SelectedItem, SelectedKind, Symbol,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    pub(crate) const SHEET: &str = "paste-1263.snxsch";

    pub(crate) fn part(reference: &str, lib: &str, unit: u32, at: (f64, f64)) -> Symbol {
        Symbol {
            uuid: Uuid::new_v4(),
            lib_id: lib.to_owned(),
            reference: reference.to_owned(),
            position: Point::new(at.0, at.1),
            unit,
            ..Default::default()
        }
    }

    pub(crate) fn net_label(text: &str, at: (f64, f64)) -> Label {
        Label {
            uuid: Uuid::new_v4(),
            text: text.to_owned(),
            position: Point::new(at.0, at.1),
            rotation: 0.0,
            label_type: LabelType::Net,
            shape: String::new(),
            font_size: 1.27,
            font_family: None,
            justify: Default::default(),
            justify_v: Default::default(),
            unknown_columns: Default::default(),
        }
    }

    /// An app whose active sheet is `sheet`, at the fixed test path.
    pub(crate) fn app_with_sheet(sheet: SchematicSheet) -> Signex {
        app_with_sheet_at(PathBuf::from(SHEET), sheet)
    }

    /// An app whose active sheet is `sheet`, at `path`.
    pub(crate) fn app_with_sheet_at(path: PathBuf, mut sheet: SchematicSheet) -> Signex {
        for lib in ["lib:r", "lib:dual"] {
            let body: LibSymbol = serde_json::from_str(&format!(r#"{{"id":"{lib}"}}"#)).unwrap();
            sheet.lib_symbols.insert(body.id.clone(), body);
        }
        let (mut app, _t) = Signex::new();
        app.ui_state.first_run_tour_open = false;
        app.document_state.engines.insert(
            path.clone(),
            signex_engine::Engine::new(sheet).expect("engine"),
        );
        app.document_state.tabs.push(TabInfo {
            title: "paste-1263".to_owned(),
            path: path.clone(),
            cached_document: None,
            dirty: false,
            project_id: None,
            kind: TabKind::Schematic,
        });
        app.document_state.active_tab = 0;
        app.document_state.active_path = Some(path);
        app
    }

    pub(crate) fn empty_sheet() -> SchematicSheet {
        SchematicSheet {
            uuid: Uuid::new_v4(),
            paper_size: "A4".to_owned(),
            ..Default::default()
        }
    }

    /// R1, R2, R3 and a two-unit U1 in a row, plus the label `D0`.
    pub(crate) fn populated_sheet() -> SchematicSheet {
        let mut sheet = empty_sheet();
        sheet.symbols = vec![
            part("R1", "lib:r", 1, (10.16, 10.16)),
            part("R2", "lib:r", 1, (20.32, 10.16)),
            part("R3", "lib:r", 1, (30.48, 10.16)),
            part("U1", "lib:dual", 1, (10.16, 30.48)),
            part("U1", "lib:dual", 2, (25.4, 30.48)),
        ];
        sheet.labels = vec![net_label("D0", (40.64, 10.16))];
        sheet
    }

    pub(crate) fn select(app: &mut Signex, symbols: &[(&str, u32)], labels: bool) {
        let doc = app.document_state.active_engine().unwrap().document();
        let mut items: Vec<SelectedItem> = symbols
            .iter()
            .map(|(reference, unit)| {
                let symbol = doc
                    .symbols
                    .iter()
                    .find(|s| s.reference == *reference && s.unit == *unit)
                    .expect("symbol on the sheet");
                SelectedItem::new(symbol.uuid, SelectedKind::Symbol)
            })
            .collect();
        if labels {
            items.extend(
                doc.labels
                    .iter()
                    .map(|l| SelectedItem::new(l.uuid, SelectedKind::Label)),
            );
        }
        app.interaction_state.active_canvas_mut().selected = items;
    }

    pub(crate) fn references(app: &Signex) -> Vec<String> {
        let doc = app.document_state.active_engine().unwrap().document();
        doc.symbols.iter().map(|s| s.reference.clone()).collect()
    }

    pub(crate) fn click(app: &mut Signex, at: (f64, f64)) {
        let _ = app.update(Message::CanvasEvent(
            signex_app::canvas::CanvasEvent::Clicked {
                world_x: at.0,
                world_y: at.1,
            },
        ));
    }
}
