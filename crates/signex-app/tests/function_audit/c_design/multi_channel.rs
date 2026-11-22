//! Row 151 and its selected state: a repeated sheet symbol. The Repeat rows
//! write its designator, Channel naming is a project setting, and a channel
//! tab is a read-only view of the compile.

use std::path::{Path, PathBuf};

use signex_app::app::{
    CompiledDesignMsg, ContextMenuMsg, EditMsg, FileMsg, Message, ProjectId, ProjectTreeAction,
    Signex,
};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_app::panels::compiled_design::{
    ChannelStripView, CompiledStatus, NamingTarget, RepeatField, SheetSymbolRepeatView,
};
use signex_types::format::SnxSchematic;
use signex_types::project::{ChannelNaming, ProjectData, SheetEntry};
use signex_types::schematic::{
    ChildSheet, FillType, Point, SchematicSheet, SelectedItem, SelectedKind, SheetPin, Symbol,
};
use tempfile::TempDir;
use uuid::Uuid;

use crate::settle_support::settle;

const SHEET_SYMBOL: Uuid = Uuid::from_u128(0xC151);
const OTHER_SYMBOL: Uuid = Uuid::from_u128(0xC152);
const PART: Uuid = Uuid::from_u128(0xC153);

fn entry_pin(name: &str, y: f64) -> SheetPin {
    SheetPin {
        uuid: Uuid::now_v7(),
        name: name.to_string(),
        direction: "input".to_string(),
        position: Point::new(20.0, y),
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    }
}

fn placement(uuid: Uuid, designator: &str, file: &str, x: f64) -> ChildSheet {
    ChildSheet {
        uuid,
        name: designator.to_string(),
        filename: file.to_string(),
        position: Point::new(x, 20.0),
        size: (40.64, 43.18),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: vec![
            entry_pin("VIN_12V", 25.4),
            entry_pin("Repeat(VOUT_3V3)", 30.48),
        ],
        instances: Vec::new(),
    }
}

fn write_sheet(path: &Path, sheet: SchematicSheet) {
    let text = SnxSchematic::new(sheet)
        .write_string()
        .expect("encode sheet");
    std::fs::write(path, text).expect("write sheet");
}

fn sheet_entry(file: &str) -> SheetEntry {
    SheetEntry {
        name: file.to_string(),
        filename: file.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

fn project_data(dir: &Path, sheets: &[&str]) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: sheets.first().map(|name| (*name).to_string()),
        pcb_file: None,
        sheets: sheets.iter().map(|name| sheet_entry(name)).collect(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        rules: Default::default(),
        net_classes: Default::default(),
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// A project read from disk with `root.snxsch` in front, the sheet symbol
/// selected. `root` places `child.snxsch` (one part, R5) as `designator`, and
/// `other.snxsch` as `second` when given.
struct Bench {
    app: Signex,
    id: ProjectId,
    prj: PathBuf,
    root: PathBuf,
    _tmp: TempDir,
}

fn bench_with(designator: &str, second: Option<&str>) -> Bench {
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path();
    let mut root = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    root.child_sheets
        .push(placement(SHEET_SYMBOL, designator, "child.snxsch", 20.0));
    let mut names = vec!["root.snxsch", "child.snxsch"];
    if let Some(second) = second {
        root.child_sheets
            .push(placement(OTHER_SYMBOL, second, "other.snxsch", 90.0));
        names.push("other.snxsch");
        write_sheet(
            &dir.join("other.snxsch"),
            SchematicSheet {
                uuid: Uuid::now_v7(),
                paper_size: "A4".to_string(),
                ..Default::default()
            },
        );
    }
    let mut child = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    child.symbols.push(Symbol {
        uuid: PART,
        lib_id: "R".to_string(),
        reference: "R5".to_string(),
        value: "31.6k".to_string(),
        position: Point::new(50.8, 50.8),
        ..Default::default()
    });
    write_sheet(&dir.join("root.snxsch"), root);
    write_sheet(&dir.join("child.snxsch"), child);
    let written = dir.join("Proj.snxprj");
    signex_types::project::write_project(&written, &project_data(dir, &names))
        .expect("write project");

    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(written.clone()))),
    );
    settle(
        &mut app,
        Message::File(FileMsg::Opened(Some(written.with_file_name("root.snxsch")))),
    );
    let project = app.document_state.projects.first().expect("project open");
    let (id, prj) = (project.id, project.path.clone());
    let root_path = app
        .document_state
        .active_path
        .clone()
        .expect("the root sheet in front");
    let mut bench = Bench {
        app,
        id,
        prj,
        root: root_path,
        _tmp: tmp,
    };
    bench.select(&[SHEET_SYMBOL]);
    assert!(!bench.sheet_dirty(), "fixture: the sheet opens clean");
    bench
}

fn bench(designator: &str) -> Bench {
    bench_with(designator, None)
}

impl Bench {
    fn send(&mut self, msg: CompiledDesignMsg) {
        settle(&mut self.app, Message::from(msg));
    }

    fn panel(&mut self, msg: PanelMsg) {
        settle(&mut self.app, Message::Dock(DockMessage::Panel(msg)));
    }

    fn select(&mut self, ids: &[Uuid]) {
        self.app.interaction_state.active_canvas_mut().selected = ids
            .iter()
            .map(|id| SelectedItem::new(*id, SelectedKind::ChildSheet))
            .collect();
    }

    fn undo(&mut self) {
        settle(&mut self.app, Message::Edit(EditMsg::Undo));
    }

    fn redo(&mut self) {
        settle(&mut self.app, Message::Edit(EditMsg::Redo));
    }

    fn switch(&mut self, on: bool) {
        self.send(CompiledDesignMsg::RepeatToggled {
            sheet_symbol: SHEET_SYMBOL,
            on,
        });
    }

    fn type_field(&mut self, field: RepeatField, text: &str) {
        self.send(CompiledDesignMsg::RepeatFieldTyped {
            sheet_symbol: SHEET_SYMBOL,
            field,
            text: text.to_string(),
        });
    }

    fn press_enter(&mut self) {
        self.send(CompiledDesignMsg::RepeatFieldSubmitted {
            sheet_symbol: SHEET_SYMBOL,
        });
    }

    fn type_designator(&mut self, text: &str) {
        self.panel(PanelMsg::ChildSheetNameTyping(
            SHEET_SYMBOL,
            text.to_string(),
        ));
        self.panel(PanelMsg::CommitChildSheetName(SHEET_SYMBOL));
    }

    fn designator(&self) -> String {
        self.app.document_state.engines[&self.root]
            .document()
            .child_sheets
            .iter()
            .find(|sheet| sheet.uuid == SHEET_SYMBOL)
            .expect("the sheet symbol")
            .name
            .clone()
    }

    fn sheet_dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.root)
    }

    fn project_dirty(&self) -> bool {
        self.app.document_state.is_document_dirty(&self.prj)
    }

    fn naming(&self) -> ChannelNaming {
        self.app.document_state.projects[0]
            .data
            .schematic_design
            .channel_naming
            .clone()
    }

    /// Refresh the panels' view of the multi-channel rows.
    fn refresh(&mut self) {
        self.send(CompiledDesignMsg::FindChanged(String::new()));
    }

    /// The Properties rows of the selected sheet symbol.
    fn rows(&mut self) -> SheetSymbolRepeatView {
        self.refresh();
        self.app
            .document_state
            .panel_ctx
            .compiled_design
            .sheet_symbol
            .clone()
            .expect("the sheet symbol's rows")
    }

    fn strip(&mut self) -> ChannelStripView {
        self.refresh();
        self.app
            .document_state
            .panel_ctx
            .compiled_design
            .strip
            .clone()
            .expect("the channel strip")
    }

    /// Bring `child.snxsch` to the front.
    fn open_child(&mut self) -> PathBuf {
        let child = self.root.with_file_name("child.snxsch");
        settle(&mut self.app, Message::File(FileMsg::Opened(Some(child))));
        self.app
            .document_state
            .active_path
            .clone()
            .expect("the child in front")
    }

    fn open_project_tab(&mut self) {
        let key = vec![self.id.to_string()];
        settle(
            &mut self.app,
            Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
                ProjectTreeAction::OpenProjectOptions(key),
            )),
        );
    }
}

// ---- Selected state ----

#[test]
fn a_selected_sheet_symbol_shows_its_file_designator_and_entries() {
    let mut bench = bench("buck_channel");

    let rows = bench.rows();

    assert_eq!(rows.file, "child.snxsch");
    assert_eq!(rows.designator, "buck_channel");
    assert_eq!(rows.repeat, None, "a plain sheet");
    assert_eq!(rows.size_mm, (40.64, 43.18));
    assert_eq!(
        rows.entries,
        [
            ("VIN_12V".to_string(), false),
            ("VOUT_3V3".to_string(), true),
        ],
        "plain entries are shared, Repeat(...) is per channel"
    );
}

#[test]
fn two_selected_sheet_symbols_show_no_repeat_rows() {
    let mut bench = bench_with("buck_channel", Some("other"));
    bench.select(&[SHEET_SYMBOL, OTHER_SYMBOL]);

    bench.refresh();

    assert!(
        bench
            .app
            .document_state
            .panel_ctx
            .compiled_design
            .sheet_symbol
            .is_none()
    );
}

// ---- The Repeat switch ----

#[test]
fn the_repeat_switch_writes_the_designator_and_marks_the_sheet_modified() {
    let mut bench = bench("buck_channel");

    bench.switch(true);

    assert_eq!(bench.designator(), "Repeat(CH, 1, 4)");
    assert!(bench.sheet_dirty());
    let rows = bench.rows();
    assert_eq!(rows.repeat.map(|repeat| repeat.count()), Some(4));
    assert_eq!(rows.channel_text, "CH");
    assert_eq!((rows.from_text.as_str(), rows.to_text.as_str()), ("1", "4"));
}

#[test]
fn switching_repeat_off_restores_the_plain_designator() {
    let mut bench = bench("buck_channel");
    bench.switch(true);

    bench.switch(false);

    assert_eq!(bench.designator(), "buck_channel");
    assert_eq!(bench.rows().repeat, None);
}

#[test]
fn undo_takes_the_repeat_switch_back_to_the_save_point_and_redo_repeats_it() {
    let mut bench = bench("buck_channel");
    bench.switch(true);

    bench.undo();

    assert_eq!(bench.designator(), "buck_channel");
    assert!(!bench.sheet_dirty(), "back at the save point");

    bench.redo();

    assert_eq!(bench.designator(), "Repeat(CH, 1, 4)");
    assert!(bench.sheet_dirty());
}

#[test]
fn switching_off_a_repeat_typed_as_the_designator_restores_the_designator_it_had() {
    let mut bench = bench("buck_channel");
    bench.type_designator("Repeat(CH, 1, 6)");
    assert_eq!(bench.designator(), "Repeat(CH, 1, 6)");

    bench.switch(false);

    assert_eq!(
        bench.designator(),
        "buck_channel",
        "typing and clicking give one result"
    );
}

#[test]
fn switching_repeat_off_again_after_undoing_the_first_off_restores_the_plain_designator() {
    let mut bench = bench("buck_channel");
    bench.switch(true);
    bench.switch(false);
    bench.undo();
    assert_eq!(bench.designator(), "Repeat(CH, 1, 4)");

    bench.switch(false);

    assert_eq!(bench.designator(), "buck_channel");
}

// ---- Channel name, From and To ----

#[test]
fn a_typed_repeat_field_writes_nothing_until_enter() {
    let mut bench = bench("Repeat(CH, 1, 4)");

    bench.type_field(RepeatField::To, "6");

    assert_eq!(bench.designator(), "Repeat(CH, 1, 4)");
    assert_eq!(bench.rows().to_text, "6", "the box shows the typed text");
    assert!(!bench.sheet_dirty());
}

#[test]
fn enter_in_the_repeat_fields_writes_the_same_designator_as_typing_it() {
    let mut bench = bench("Repeat(CH, 1, 4)");

    bench.type_field(RepeatField::Channel, "BUCK");
    bench.type_field(RepeatField::From, "2");
    bench.type_field(RepeatField::To, "7");
    bench.press_enter();

    assert_eq!(bench.designator(), "Repeat(BUCK, 2, 7)");
    assert!(bench.sheet_dirty());
    let names: Vec<String> = bench
        .rows()
        .repeat
        .expect("repeated")
        .occurrences()
        .into_iter()
        .map(|channel| channel.name)
        .collect();
    assert_eq!(
        names,
        ["BUCK2", "BUCK3", "BUCK4", "BUCK5", "BUCK6", "BUCK7"]
    );
}

#[test]
fn one_enter_in_the_repeat_fields_is_one_undo_step() {
    let mut bench = bench("Repeat(CH, 1, 4)");
    bench.type_field(RepeatField::From, "2");
    bench.type_field(RepeatField::To, "7");
    bench.press_enter();

    bench.undo();

    assert_eq!(bench.designator(), "Repeat(CH, 1, 4)");
    assert!(!bench.sheet_dirty());
}

#[test]
fn from_after_to_is_refused_with_the_typed_text_kept() {
    let mut bench = bench("Repeat(CH, 1, 4)");

    bench.type_field(RepeatField::From, "9");
    bench.press_enter();

    assert_eq!(bench.designator(), "Repeat(CH, 1, 4)");
    assert!(!bench.sheet_dirty());
    assert_eq!(bench.rows().from_text, "9");
}

#[test]
fn more_than_64_channels_are_refused_with_the_typed_text_kept() {
    let mut bench = bench("Repeat(CH, 1, 4)");

    bench.type_field(RepeatField::To, "65");
    bench.press_enter();

    assert_eq!(bench.designator(), "Repeat(CH, 1, 4)");
    assert_eq!(bench.rows().to_text, "65");

    bench.type_field(RepeatField::To, "64");
    bench.press_enter();

    assert_eq!(bench.designator(), "Repeat(CH, 1, 64)");
}

#[test]
fn an_empty_channel_name_is_refused_with_the_field_left_empty() {
    let mut bench = bench("Repeat(CH, 1, 4)");

    bench.type_field(RepeatField::Channel, "");
    bench.press_enter();

    assert_eq!(bench.designator(), "Repeat(CH, 1, 4)");
    assert_eq!(bench.rows().channel_text, "");
}

#[test]
fn a_from_to_that_clashes_with_another_sheet_is_refused_with_the_typed_text_kept() {
    let mut bench = bench_with("Repeat(CH, 1, 4)", Some("Repeat(CH, 1, 6)"));

    bench.type_field(RepeatField::To, "6");
    bench.press_enter();

    assert_eq!(bench.designator(), "Repeat(CH, 1, 4)", "refused");
    assert!(bench.app.document_state.error_notice.is_some());
    assert_eq!(bench.rows().to_text, "6", "the typed text stays to fix");
}

// ---- The designator as text ----

#[test]
fn typing_a_repeat_as_the_designator_turns_the_rows_on() {
    let mut bench = bench("buck_channel");

    bench.type_designator("Repeat(CH, 1, 6)");

    let rows = bench.rows();
    assert_eq!(rows.repeat.map(|repeat| repeat.count()), Some(6));
    assert_eq!(rows.to_text, "6");
}

#[test]
fn the_repeat_keyword_is_case_sensitive() {
    let mut bench = bench("buck_channel");

    bench.type_designator("repeat(CH, 1, 4)");

    assert_eq!(bench.rows().repeat, None, "a plain name");
}

#[test]
fn a_repeat_designator_may_carry_spaces() {
    let mut bench = bench("buck_channel");

    bench.type_designator("Repeat( CH , 1 , 2 )");

    assert_eq!(bench.rows().repeat.map(|repeat| repeat.count()), Some(2));
}

// ---- Channel naming ----

#[test]
fn a_designator_format_from_the_menu_is_a_project_change_and_closes_the_menu() {
    let mut bench = bench("Repeat(CH, 1, 4)");
    bench.send(CompiledDesignMsg::NamingMenuToggled(
        NamingTarget::Designators,
    ));
    assert_eq!(
        bench.rows().menu.target,
        Some(NamingTarget::Designators),
        "the menu is open"
    );

    bench.send(CompiledDesignMsg::NamingPicked {
        target: NamingTarget::Designators,
        format: "${Designator}_${Index}".to_string(),
    });

    assert_eq!(bench.naming().designator_format, "${Designator}_${Index}");
    assert_eq!(
        bench.naming().net_format,
        ChannelNaming::default().net_format
    );
    assert!(bench.project_dirty());
    assert_eq!(bench.rows().menu.target, None);
}

#[test]
fn a_local_net_format_from_the_menu_changes_only_the_net_format() {
    let mut bench = bench("Repeat(CH, 1, 4)");

    bench.send(CompiledDesignMsg::NamingPicked {
        target: NamingTarget::Nets,
        format: "${Channel}_${Net}".to_string(),
    });

    assert_eq!(bench.naming().net_format, "${Channel}_${Net}");
    assert_eq!(
        bench.naming().designator_format,
        ChannelNaming::default().designator_format
    );
}

#[test]
fn closing_the_naming_menu_changes_nothing() {
    let mut bench = bench("Repeat(CH, 1, 4)");
    bench.send(CompiledDesignMsg::NamingMenuToggled(NamingTarget::Nets));

    bench.send(CompiledDesignMsg::NamingMenuClosed);

    assert_eq!(bench.rows().menu.target, None);
    assert_eq!(bench.naming(), ChannelNaming::default());
    assert!(!bench.project_dirty());
}

#[test]
fn undo_in_the_project_tab_takes_a_naming_pick_back() {
    let mut bench = bench("Repeat(CH, 1, 4)");
    bench.send(CompiledDesignMsg::NamingPicked {
        target: NamingTarget::Designators,
        format: "${Designator}_${Index}".to_string(),
    });
    bench.open_project_tab();

    bench.undo();

    assert_eq!(bench.naming(), ChannelNaming::default());
    assert!(!bench.project_dirty());
}

#[test]
fn a_valid_custom_format_is_written_and_closes_the_menu() {
    let mut bench = bench("Repeat(CH, 1, 4)");
    bench.send(CompiledDesignMsg::NamingMenuToggled(
        NamingTarget::Designators,
    ));

    bench.send(CompiledDesignMsg::NamingCustomTyped(
        "${ChannelAlpha}-${Designator}".to_string(),
    ));
    bench.send(CompiledDesignMsg::NamingCustomSubmitted);

    assert_eq!(
        bench.naming().designator_format,
        "${ChannelAlpha}-${Designator}"
    );
    assert_eq!(bench.rows().menu.target, None);
}

#[test]
fn a_bad_custom_format_is_refused_with_its_reason_and_the_text_kept() {
    let mut bench = bench("Repeat(CH, 1, 4)");
    bench.send(CompiledDesignMsg::NamingMenuToggled(
        NamingTarget::Designators,
    ));

    bench.send(CompiledDesignMsg::NamingCustomTyped(
        "${Designator}".to_string(),
    ));
    bench.send(CompiledDesignMsg::NamingCustomSubmitted);

    assert_eq!(bench.naming(), ChannelNaming::default());
    assert!(!bench.project_dirty());
    let menu = bench.rows().menu;
    assert_eq!(menu.target, Some(NamingTarget::Designators), "stays open");
    assert_eq!(menu.custom, "${Designator}");
    assert!(menu.error.is_some(), "the reason shows under the field");
}

// ---- Compiled channels ----

#[test]
fn compiling_a_repeated_sheet_lists_its_channels_with_their_designators() {
    let mut bench = bench("Repeat(CH, 1, 4)");

    bench.send(CompiledDesignMsg::CompileRequested);

    let panel = bench
        .app
        .document_state
        .panel_ctx
        .compiled_design
        .panel
        .clone();
    assert_eq!(panel.status, CompiledStatus::Ready);
    assert_eq!(panel.totals.channels, 4);
    assert_eq!(panel.totals.parts, 4, "one part in each of four channels");

    bench.open_child();
    let strip = bench.strip();
    let designators: Vec<&str> = strip
        .tabs
        .iter()
        .flat_map(|tab| tab.designators.iter().map(String::as_str))
        .collect();
    assert_eq!(designators, ["CH1_R5", "CH2_R5", "CH3_R5", "CH4_R5"]);
    assert_eq!(strip.active, None, "the child opens on Editor");
}

#[test]
fn a_new_designator_format_renames_the_parts_in_the_channel_tabs() {
    let mut bench = bench("Repeat(CH, 1, 2)");
    bench.open_child();

    bench.send(CompiledDesignMsg::NamingPicked {
        target: NamingTarget::Designators,
        format: "${Designator}_${Channel}".to_string(),
    });

    let strip = bench.strip();
    assert_eq!(strip.tabs[0].designators, ["R5_CH1"]);
    assert_eq!(strip.tabs[1].designators, ["R5_CH2"]);
}

#[test]
fn a_channel_tab_is_read_only_and_editor_edits_again() {
    let mut bench = bench("Repeat(CH, 1, 2)");
    let child = bench.open_child();
    let has_part = |bench: &Bench| {
        bench.app.document_state.engines[&child]
            .document()
            .symbols
            .iter()
            .any(|symbol| symbol.uuid == PART)
    };
    let select_part = |bench: &mut Bench| {
        bench.app.interaction_state.active_canvas_mut().selected =
            vec![SelectedItem::new(PART, SelectedKind::Symbol)];
    };
    bench.send(CompiledDesignMsg::ChannelPicked {
        sheet: child.clone(),
        channel: Some(1),
    });
    assert_eq!(bench.strip().active, Some(1));

    select_part(&mut bench);
    settle(&mut bench.app, Message::Edit(EditMsg::DeleteSelected));
    assert!(has_part(&bench), "a channel tab never edits the sheet");

    bench.send(CompiledDesignMsg::ChannelPicked {
        sheet: child.clone(),
        channel: None,
    });
    select_part(&mut bench);
    settle(&mut bench.app, Message::Edit(EditMsg::DeleteSelected));
    assert!(!has_part(&bench), "Editor is the editable sheet");
}
