//! Maintainer decision 2026-11-18: Document Options ▸ Parameters is the one
//! place a sheet's title block is edited. A Title typed there shows its value
//! in the row, is one undo step, and is written to the sheet on Save.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use iced::Task;
use signex_types::schematic::SchematicSheet;

use crate::app::{EditMsg, FileMsg, Message, Signex, TabInfo, TabKind};
use crate::dock::DockMessage;
use crate::panels::{BuiltinParameter, PanelMsg, TitleBlockField, shown_title_block_value};
use crate::persist::{DocumentSaves, DocumentSink};

#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl DocumentSink for Recorder {
    fn write(&self, _path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.0
            .lock()
            .unwrap()
            .push(String::from_utf8_lossy(bytes).into_owned());
        Ok(())
    }
}

fn run(task: Task<Message>) -> Vec<Message> {
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

/// Update with `message` and every message its tasks publish.
fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    for next in run(task) {
        send(app, next);
    }
}

fn panel(app: &mut Signex, msg: PanelMsg) {
    send(app, Message::Dock(DockMessage::Panel(msg)));
}

/// An app showing one clean sheet, `power.snxsch`, with Save recorded.
fn app_on_sheet() -> (Signex, PathBuf, Arc<Recorder>) {
    let path = PathBuf::from("power.snxsch");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(SchematicSheet::default()).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "power".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    let recorder = Arc::new(Recorder::default());
    app.document_state.document_saves = DocumentSaves::new(recorder.clone());
    (app, path, recorder)
}

fn sheet_title(app: &Signex, path: &Path) -> Option<String> {
    let engine = &app.document_state.engines[path];
    engine.document().title_block.get("title").cloned()
}

/// Save, then read back the bytes Save wrote.
fn save_and_reopen(app: &mut Signex, recorder: &Recorder) -> SchematicSheet {
    send(app, Message::File(FileMsg::Save));
    let written = recorder.0.lock().unwrap().last().cloned();
    let written = written.expect("Save wrote the sheet");
    signex_types::format::SnxSchematic::parse(&written)
        .expect("reopens")
        .sheet
}

/// Type `text` into `field`'s row and press Enter.
fn apply(app: &mut Signex, field: TitleBlockField, text: &str) {
    panel(app, PanelMsg::TitleBlockTyping(field, text.to_string()));
    panel(app, PanelMsg::CommitTitleBlockField(field));
}

fn shown_title(app: &mut Signex) -> String {
    app.refresh_panel_ctx();
    shown_title_block_value(&app.document_state.panel_ctx, TitleBlockField::Title)
}

#[test]
fn a_title_typed_in_parameters_shows_and_is_saved_to_the_sheet() {
    let (mut app, path, recorder) = app_on_sheet();
    panel(&mut app, PanelMsg::PropertiesTab(1));
    assert_eq!(shown_title(&mut app), "", "an empty field shows empty");

    panel(
        &mut app,
        PanelMsg::TitleBlockTyping(TitleBlockField::Title, "dcdc".into()),
    );
    assert_eq!(shown_title(&mut app), "dcdc", "the typed text shows");
    assert_eq!(sheet_title(&app, &path), None, "a draft is not applied");

    panel(
        &mut app,
        PanelMsg::CommitTitleBlockField(TitleBlockField::Title),
    );
    assert_eq!(shown_title(&mut app), "dcdc", "the applied value shows");
    assert_eq!(sheet_title(&app, &path).as_deref(), Some("dcdc"));
    let in_panel = &app.document_state.panel_ctx.title_block;
    assert_eq!(in_panel.get("title").map(String::as_str), Some("dcdc"));

    send(&mut app, Message::Edit(EditMsg::Undo));
    let undone = sheet_title(&app, &path);
    assert_ne!(undone.as_deref(), Some("dcdc"), "one undo step");
    assert_eq!(shown_title(&mut app), "");
    send(&mut app, Message::Edit(EditMsg::Redo));
    assert_eq!(sheet_title(&app, &path).as_deref(), Some("dcdc"));

    let saved = save_and_reopen(&mut app, &recorder);
    assert_eq!(
        saved.title_block.get("title").map(String::as_str),
        Some("dcdc")
    );
}

/// The read-only rows hold what the special strings resolve to on the sheet.
#[test]
fn the_built_in_rows_hold_the_sheets_resolved_values() {
    let (mut app, _path, _recorder) = app_on_sheet();
    app.refresh_panel_ctx();
    let rows = &app.document_state.panel_ctx.builtin_parameters;
    let value = |wanted: BuiltinParameter| {
        rows.iter()
            .find(|(parameter, _)| *parameter == wanted)
            .map(|(_, value)| value.as_str())
    };
    assert_eq!(value(BuiltinParameter::DocumentName), Some("power.snxsch"));
    assert_eq!(value(BuiltinParameter::SheetName), Some("power"));
    assert_eq!(value(BuiltinParameter::ProjectName), Some(""));
}

/// The title-block fields the Parameters list added.
const NEW_FIELDS: [TitleBlockField; 7] = [
    TitleBlockField::Author,
    TitleBlockField::Engineer,
    TitleBlockField::Organization,
    TitleBlockField::Address1,
    TitleBlockField::Address2,
    TitleBlockField::Address3,
    TitleBlockField::Address4,
];

/// Each new field edits through its row, is written by Save and read back,
/// and reads as `${Name}` in a text note.
#[test]
fn each_new_field_edits_saves_reopens_and_reads_in_a_note() {
    let (mut app, path, recorder) = app_on_sheet();
    let value = |field: TitleBlockField| format!("{} value", field.token());
    for field in NEW_FIELDS {
        apply(&mut app, field, &value(field));
    }
    app.refresh_panel_ctx();
    let special = app.special_strings_for_path(&path).expect("the open sheet");
    let saved = save_and_reopen(&mut app, &recorder);
    for field in NEW_FIELDS {
        let ctx = &app.document_state.panel_ctx;
        assert_eq!(shown_title_block_value(ctx, field), value(field));
        let stored = saved.title_block.get(field.key()).cloned();
        assert_eq!(stored, Some(value(field)), "{field:?}");
        let note = format!("${{{}}}", field.token());
        assert_eq!(special.resolve_text(&note), value(field), "{note}");
    }
}

/// The computed tokens resolve in a note from the clock and file times kept
/// in state, and their read-only rows show the same values.
#[test]
fn the_computed_tokens_resolve_in_a_note_and_their_rows() {
    use chrono::TimeZone;
    let (mut app, path, _recorder) = app_on_sheet();
    let modified = chrono::Local
        .with_ymd_and_hms(2026, 11, 17, 8, 0, 0)
        .unwrap();
    let modified = std::time::SystemTime::from(modified);
    app.document_state.times.set_modified(&path, modified);
    app.refresh_panel_ctx();
    let now = app.document_state.times.now.expect("clock read");
    let today = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H:%M").to_string();
    let special = app.special_strings_for_path(&path).expect("the open sheet");
    for (note, value) in [
        ("${CurrentDate}", today.as_str()),
        ("${CurrentTime}", time.as_str()),
        ("${Time}", time.as_str()),
        ("${ModifiedDate}", "2026-11-17"),
        ("${DocumentFullPathAndName}", "power.snxsch"),
    ] {
        assert_eq!(special.resolve_text(note), value, "{note}");
    }
    let rows = &app.document_state.panel_ctx.builtin_parameters;
    for (parameter, value) in [
        (BuiltinParameter::CurrentDate, today.as_str()),
        (BuiltinParameter::CurrentTime, time.as_str()),
        (BuiltinParameter::ModifiedDate, "2026-11-17"),
        (BuiltinParameter::DocumentFullPathAndName, "power.snxsch"),
    ] {
        let row = rows.iter().find(|(p, _)| *p == parameter);
        assert_eq!(row.map(|(_, v)| v.as_str()), Some(value), "{parameter:?}");
    }
}

/// An empty Date shows today and Save writes nothing for it; a typed Date
/// is stored, wins over today, and survives Save and reopen.
#[test]
fn an_empty_date_shows_today_and_a_typed_one_round_trips() {
    let (mut app, path, recorder) = app_on_sheet();
    // Another field, so the sheet has an edit to save.
    apply(&mut app, TitleBlockField::Title, "dcdc");
    app.refresh_panel_ctx();
    let now = app.document_state.times.now.expect("clock read");
    let today = now.format("%Y-%m-%d").to_string();
    let special = app.special_strings_for_path(&path).expect("the open sheet");
    assert_eq!(special.resolve_text("${Date}"), today);
    let ctx = &app.document_state.panel_ctx;
    assert_eq!(shown_title_block_value(ctx, TitleBlockField::Date), "");
    let saved = save_and_reopen(&mut app, &recorder);
    let date = saved.title_block.get("date");
    assert_eq!(date, None, "an empty Date stores nothing");

    // A fresh sheet: the recorder writes nothing to disk, so a second Save
    // of the same sheet would find its file missing and hold the write.
    let (mut app, path, recorder) = app_on_sheet();
    apply(&mut app, TitleBlockField::Date, "2026-01-02");
    app.refresh_panel_ctx();
    let special = app.special_strings_for_path(&path).expect("the open sheet");
    assert_eq!(special.resolve_text("${Date}"), "2026-01-02");
    let saved = save_and_reopen(&mut app, &recorder);
    let date = saved.title_block.get("date").map(String::as_str);
    assert_eq!(date, Some("2026-01-02"));
}

/// #2531: the chips count every row the filter text leaves: All is the
/// parameters plus the rules, Parameters the three groups, Rules none.
#[test]
fn the_filter_chips_count_the_rows_the_filter_leaves() {
    use super::super::document_options::ParametersFilter;
    use super::ParameterListing;
    let en = signex_i18n::I18n::en_us();
    let variables = ["VIN".to_string(), "VOUT".to_string()];
    let all = ParameterListing::filtered(&en, "", &variables, BuiltinParameter::ALL);
    let rows = TitleBlockField::ALL.len() + 2 + BuiltinParameter::ALL.len();
    assert_eq!(all.count(ParametersFilter::Parameters), rows);
    assert_eq!(all.count(ParametersFilter::All), rows);
    assert_eq!(all.count(ParametersFilter::Rules), 0);

    let dates = ParameterListing::filtered(&en, " DATE ", &variables, BuiltinParameter::ALL);
    assert_eq!(dates.title_block, [TitleBlockField::Date]);
    assert!(dates.variables.is_empty());
    assert_eq!(
        dates.builtins,
        [
            BuiltinParameter::CurrentDate,
            BuiltinParameter::ModifiedDate
        ]
    );
    assert_eq!(dates.count(ParametersFilter::All), 3);

    let vin = ParameterListing::filtered(&en, "vin", &variables, BuiltinParameter::ALL);
    assert_eq!(vin.variables, ["VIN".to_string()]);
    assert_eq!(vin.count(ParametersFilter::Parameters), 1);
}

/// What the value input on the row named `name` shows in the laid-out
/// Parameters tab: its value, or its placeholder while empty.
fn shown_in_row(app: &mut Signex, name: &str) -> String {
    use crate::layout_probe::{lay_out_inputs, lay_out_texts};
    use iced::{Size, Widget};
    app.refresh_panel_ctx();
    let ctx = &app.document_state.panel_ctx;
    let window = Size::new(320.0, 4000.0);
    let view = || super::view_properties_parameters(ctx, iced::Color::BLACK).boxed();
    let texts = lay_out_texts(view(), window);
    let label = texts
        .iter()
        .find(|t| t.text.contains(name))
        .unwrap_or_else(|| panic!("no row {name:?}: {texts:#?}"));
    let row = label.bounds;
    let inputs = lay_out_inputs(view(), window);
    let on_row = |y: f32| y >= row.y - 1.0 && y <= row.y + row.height + 1.0;
    inputs
        .iter()
        .find(|input| on_row(input.bounds.center().y))
        .unwrap_or_else(|| panic!("no input on the row {name:?}: {inputs:#?}"))
        .text
        .clone()
}

/// Mockup row 40: a sheet variable added with no value reads "Not set" in
/// its row, as an unset title-block value does; a typed value replaces it.
#[test]
fn an_empty_sheet_variable_reads_not_set() {
    use crate::panels::SheetVariableMsg;
    let (mut app, _path, _recorder) = app_on_sheet();
    panel(&mut app, PanelMsg::PropertiesTab(1));
    let variables = |app: &mut Signex, m: SheetVariableMsg| panel(app, PanelMsg::SheetVariables(m));
    variables(&mut app, SheetVariableMsg::NewNameTyped("VIN".into()));
    variables(&mut app, SheetVariableMsg::Added);
    app.refresh_panel_ctx();
    let stored = app.document_state.panel_ctx.sheet_variables.get("VIN");
    assert_eq!(stored.map(String::as_str), Some(""), "added with no value");
    let not_set = signex_i18n::I18n::en_us()
        .message_plain(signex_i18n::ids::PROPERTIES_PARAMETERS_NOT_SET, &[]);
    assert_eq!(shown_in_row(&mut app, "VIN"), not_set);
    assert_eq!(
        shown_in_row(&mut app, "Checked By"),
        not_set,
        "as a title-block value"
    );

    let typed = SheetVariableMsg::ValueTyped {
        name: "VIN".into(),
        text: "12V".into(),
    };
    variables(&mut app, typed);
    variables(&mut app, SheetVariableMsg::ValueSubmitted("VIN".into()));
    assert_eq!(shown_in_row(&mut app, "VIN"), "12V");
}

/// #2531: the Built-in group lists the sheet's names and numbers first.
#[test]
fn the_built_in_group_opens_with_the_sheets_names_and_numbers() {
    assert_eq!(
        BuiltinParameter::ALL[..5],
        [
            BuiltinParameter::DocumentName,
            BuiltinParameter::SheetName,
            BuiltinParameter::SheetNumber,
            BuiltinParameter::SheetCount,
            BuiltinParameter::ProjectName,
        ]
    );
}
