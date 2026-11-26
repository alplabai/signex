//! The pieces of the multi-channel slice that need no project on disk: the
//! naming presets, the Repeat switch's default and the out-of-date rule.

use std::path::PathBuf;
use std::sync::Arc;

use signex_types::schematic::{
    ChildSheet, FillType, Point, SchematicSheet, SelectedItem, SelectedKind, check_format,
};
use uuid::Uuid;

use super::state::CompiledResult;
use super::{DESIGNATOR_PRESETS, NET_PRESETS};
use crate::app::{Signex, TabInfo, TabKind};
use crate::panels::compiled_design::RepeatField;

const SYMBOL: Uuid = Uuid::from_u128(0xC151);
const OTHER: Uuid = Uuid::from_u128(0xC152);

fn sheet_symbol(uuid: Uuid, designator: &str, file: &str) -> ChildSheet {
    ChildSheet {
        uuid,
        name: designator.to_string(),
        filename: file.to_string(),
        position: Point::new(20.0, 20.0),
        size: (40.64, 43.18),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

/// An app editing one sheet that places `SYMBOL` as `designator`, and `OTHER`
/// as `second` when given, with `SYMBOL` selected.
fn app_editing(designator: &str, second: Option<&str>) -> Signex {
    let path = PathBuf::from("repeat_edit.snxsch");
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    sheet
        .child_sheets
        .push(sheet_symbol(SYMBOL, designator, "child.snxsch"));
    if let Some(second) = second {
        sheet
            .child_sheets
            .push(sheet_symbol(OTHER, second, "other.snxsch"));
    }
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _task) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "repeat_edit".to_string(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(SYMBOL, SelectedKind::ChildSheet)];
    app
}

fn designator(app: &Signex) -> String {
    app.edited_sheet_symbol(SYMBOL)
        .expect("the sheet symbol")
        .name
        .clone()
}

fn type_designator(app: &mut Signex, text: &str) {
    app.document_state.panel_ctx.child_sheet_name_buf = Some((SYMBOL, text.to_string()));
    app.commit_child_sheet_name(SYMBOL);
}

fn undo(app: &mut Signex) {
    let engine = app.document_state.edit_engine_mut().expect("the engine");
    engine.undo().expect("undo");
}

/// Mockup row 151: selecting a sheet symbol brings its Repeat rows, and
/// leaving it takes them away, with no other refresh in between.
#[test]
fn a_selection_change_refreshes_the_repeat_rows() {
    let mut app = app_editing("Repeat(CH, 1, 4)", None);

    app.update_selection_info();
    let rows = app
        .document_state
        .panel_ctx
        .compiled_design
        .sheet_symbol
        .as_ref();
    assert_eq!(rows.map(|r| r.sheet_symbol), Some(SYMBOL));

    app.interaction_state.active_canvas_mut().selected.clear();
    app.update_selection_info();
    assert!(
        app.document_state
            .panel_ctx
            .compiled_design
            .sheet_symbol
            .is_none()
    );
}

/// Mockup row 51: the footer's breakdown follows the selection.
#[test]
fn a_selection_change_refreshes_the_kinds_the_footer_counts() {
    let mut app = app_editing("plain", Some("other"));
    app.interaction_state.active_canvas_mut().selected = vec![
        SelectedItem::new(SYMBOL, SelectedKind::ChildSheet),
        SelectedItem::new(OTHER, SelectedKind::ChildSheet),
    ];

    app.update_selection_info();

    let kinds: Vec<_> = app
        .document_state
        .panel_ctx
        .selection_form
        .kinds
        .iter()
        .collect();
    assert_eq!(kinds, [(SelectedKind::ChildSheet, 2)]);
}

#[test]
fn off_restores_the_designator_a_typed_repeat_replaced() {
    let mut app = app_editing("buck_channel", None);
    type_designator(&mut app, "Repeat(CH, 1, 6)");

    app.toggle_repeat(SYMBOL, false);

    assert_eq!(designator(&app), "buck_channel");
}

#[test]
fn a_second_off_after_undo_restores_the_plain_designator() {
    let mut app = app_editing("buck_channel", None);
    app.toggle_repeat(SYMBOL, true);
    app.toggle_repeat(SYMBOL, false);
    undo(&mut app);
    assert_eq!(designator(&app), "Repeat(CH, 1, 4)");

    app.toggle_repeat(SYMBOL, false);

    assert_eq!(designator(&app), "buck_channel");
}

#[test]
fn off_on_a_sheet_opened_repeated_takes_the_file_stem() {
    let mut app = app_editing("Repeat(CH, 1, 4)", None);

    app.toggle_repeat(SYMBOL, false);

    assert_eq!(designator(&app), "child");
}

/// #2790: the file stem another sheet symbol holds would be refused, so
/// Repeat off would leave the repeat on; it takes the next free name.
#[test]
fn off_takes_a_free_name_when_the_file_stem_is_held() {
    let mut app = app_editing("Repeat(CH, 1, 4)", Some("child"));

    app.toggle_repeat(SYMBOL, false);

    assert_eq!(designator(&app), "child_2");
}

#[test]
fn a_clashing_to_is_refused_with_the_text_kept_and_marked() {
    let mut app = app_editing("Repeat(CH, 1, 4)", Some("Repeat(CH, 1, 6)"));
    app.type_repeat_field(SYMBOL, RepeatField::To, "6".to_string());

    app.submit_repeat_fields(SYMBOL);

    assert_eq!(designator(&app), "Repeat(CH, 1, 4)");
    assert!(app.document_state.error_notice.is_some());
    let rows = app.compiled_design_view().sheet_symbol.expect("the rows");
    assert_eq!(rows.to_text, "6");
    assert!(rows.is_to_refused);
    assert!(!rows.is_from_refused, "From was not typed");
}

#[test]
fn typing_again_clears_the_refused_mark() {
    let mut app = app_editing("Repeat(CH, 1, 4)", None);
    app.type_repeat_field(SYMBOL, RepeatField::From, "9".to_string());
    app.submit_repeat_fields(SYMBOL);
    let rows = app.compiled_design_view().sheet_symbol.expect("the rows");
    assert!(rows.is_from_refused);

    app.type_repeat_field(SYMBOL, RepeatField::From, "2".to_string());

    let rows = app.compiled_design_view().sheet_symbol.expect("the rows");
    assert!(!rows.is_from_refused);
}

#[test]
fn a_bad_channel_name_does_not_mark_the_range() {
    let mut app = app_editing("Repeat(CH, 1, 4)", None);
    app.type_repeat_field(SYMBOL, RepeatField::Channel, String::new());
    app.type_repeat_field(SYMBOL, RepeatField::To, "6".to_string());

    app.submit_repeat_fields(SYMBOL);

    let rows = app.compiled_design_view().sheet_symbol.expect("the rows");
    assert_eq!(designator(&app), "Repeat(CH, 1, 4)");
    assert!(!rows.is_to_refused);
}

#[test]
fn a_written_range_clears_the_typed_text() {
    let mut app = app_editing("Repeat(CH, 1, 4)", None);
    app.type_repeat_field(SYMBOL, RepeatField::To, "6".to_string());

    app.submit_repeat_fields(SYMBOL);

    assert_eq!(designator(&app), "Repeat(CH, 1, 6)");
    assert!(app.ui_state.compiled_design.repeat_draft.is_none());
}

#[test]
fn every_preset_is_a_valid_custom_format() {
    for preset in DESIGNATOR_PRESETS.iter().chain(NET_PRESETS.iter()) {
        assert_eq!(check_format(preset), Ok(()), "{preset}");
    }
}

#[test]
fn the_menu_presets_hold_the_default_naming() {
    let naming = signex_types::project::ChannelNaming::default();
    assert!(DESIGNATOR_PRESETS.contains(&naming.designator_format.as_str()));
    assert!(NET_PRESETS.contains(&naming.net_format.as_str()));
}

#[test]
fn the_switch_repeats_a_sheet_four_times_on_ch() {
    assert_eq!(
        super::repeat_edit::default_repeat().to_designator(),
        "Repeat(CH, 1, 4)"
    );
}

fn app_with_a_compile(path: &std::path::Path) -> Signex {
    let (mut app, _task) = Signex::new();
    let project = app.document_state.mint_project_id();
    let key = signex_net::SheetKey::new("child.snxsch");
    app.document_state.compiled_design.results.insert(
        project,
        CompiledResult {
            design: Arc::new(signex_net::CompiledDesign::default()),
            paths: std::collections::HashMap::from([(key, path.to_path_buf())]),
            compiled_at: chrono::DateTime::<chrono::Utc>::UNIX_EPOCH,
            miniatures: Default::default(),
            stale: Default::default(),
            has_unread_sheets: false,
            unread_paths: Vec::new(),
            bare_links_missing: Vec::new(),
        },
    );
    app
}

/// #2742: the compile tells a sheet it could not read from one that is gone.
#[test]
fn a_compile_tells_an_unreadable_sheet_from_an_absent_one() {
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("bad.snxsch"), "not a sheet").expect("seed");

    assert!(compile_of(dir.path(), &["bad.snxsch"], &[]).has_unread_sheets);
    // A sheet symbol still places the missing file (Caner, 2026-11-23).
    assert!(compile_of(dir.path(), &["gone.snxsch"], &[]).has_unread_sheets);
}

/// `top.snxsch`, open, with a sheet symbol per `children`, and `pages` as the
/// project's declared pages.
fn compile_of(dir: &std::path::Path, children: &[&str], pages: &[&str]) -> super::job::Compiled {
    use crate::app::handlers::menu::export::tests::sheet_with_net;
    compile_sheet(dir, sheet_with_net("R1", "N", children), pages)
}

/// `sheet`, open as `top.snxsch`, compiled with `pages` declared.
fn compile_sheet(
    dir: &std::path::Path,
    sheet: SchematicSheet,
    pages: &[&str],
) -> super::job::Compiled {
    let root = dir.join("top.snxsch");
    let (mut app, _task) = Signex::new();
    let job = super::job::CompileJob {
        project: app.document_state.mint_project_id(),
        generation: 1,
        open: [(root.clone(), sheet)].into_iter().collect(),
        pages: pages.iter().map(|page| dir.join(page)).collect(),
        root,
        fold: Default::default(),
        project_dir: dir.to_path_buf(),
        naming: Default::default(),
    };
    job.run().result.expect("the root compiles")
}

/// #2742: a missing file no sheet symbol places is really gone, so its row
/// may leave the table.
#[test]
fn a_missing_page_no_sheet_symbol_places_counts_as_removed() {
    let dir = tempfile::tempdir().expect("dir");

    assert!(!compile_of(dir.path(), &[], &["gone.snxsch"]).has_unread_sheets);
}

/// #2742: a sheet symbol whose file escapes its folder still references a
/// sheet the walk refused, so that sheet's row is not known to be gone.
#[test]
fn a_refused_child_reference_counts_as_unread() {
    let parent = tempfile::tempdir().expect("dir");
    let dir = parent.path().join("project");
    std::fs::create_dir(&dir).expect("folder");

    assert!(compile_of(&dir, &["../power.snxsch"], &[]).has_unread_sheets);
}

/// #2742: with a sheet unread, the job carries which bare contents links have
/// no file beside their table; a complete compile does not look.
#[test]
fn the_compile_job_checks_bare_contents_links_on_disk_only_with_a_sheet_unread() {
    use crate::app::handlers::menu::export::tests::sheet_with_net;
    use signex_types::schematic::{SchDrawing, SchTable, TableSource};
    let dir = tempfile::tempdir().expect("dir");
    std::fs::write(dir.path().join("bad.snxsch"), "not a sheet").expect("seed");
    std::fs::write(dir.path().join("beside.snxsch"), "x").expect("seed");
    let with_table = |children: &[&str]| {
        let mut table =
            SchTable::new_typed(Uuid::from_u128(1), signex_types::schematic::Point::ZERO);
        table.source = TableSource::ProjectSheets { per_channel: true };
        table.links = vec!["beside.snxsch".to_string(), "moved.snxsch".to_string()];
        let mut sheet = sheet_with_net("R1", "N", children);
        sheet.drawings.push(SchDrawing::Table(table));
        sheet
    };

    let unread = compile_sheet(dir.path(), with_table(&["bad.snxsch"]), &[]);
    let complete = compile_sheet(dir.path(), with_table(&[]), &[]);

    assert_eq!(unread.bare_links_missing, [dir.path().join("moved.snxsch")]);
    assert!(complete.bare_links_missing.is_empty());
}

#[test]
fn an_electrical_edit_marks_a_compiled_sheet_out_of_date() {
    let path = PathBuf::from("/p/child.snxsch");
    let mut app = app_with_a_compile(&path);
    app.document_state
        .mark_compiled_stale(&path, signex_engine::DocumentPatch::SYMBOLS);
    let result = app.document_state.compiled_design.results.values().next();
    assert!(result.is_some_and(|r| r.stale.contains(&path)));
}

#[test]
fn a_colour_only_edit_or_another_sheet_leaves_the_compile_fresh() {
    let path = PathBuf::from("/p/child.snxsch");
    let mut app = app_with_a_compile(&path);
    app.document_state
        .mark_compiled_stale(&path, signex_engine::DocumentPatch::NET_COLORS);
    app.document_state.mark_compiled_stale(
        &PathBuf::from("/p/other.snxsch"),
        signex_engine::DocumentPatch::WIRES,
    );
    let result = app.document_state.compiled_design.results.values().next();
    assert!(result.is_some_and(|r| r.stale.is_empty()));
}

/// #2733: the core keeps when the compile finished, not a worded time; the
/// panel words it in the user's locale.
#[test]
fn the_compile_time_is_worded_in_the_users_locale() {
    let mut app = app_with_a_compile(&PathBuf::from("/p/child.snxsch"));
    let store = &mut app.document_state.compiled_design;
    let (&project, result) = store.results.iter_mut().next().expect("the compile");
    let at = chrono::DateTime::from_timestamp(1_700_000_000, 0).expect("a time");
    result.compiled_at = at;
    app.document_state.active_project = Some(project);
    let panel = app.compiled_design_view().panel;
    assert_eq!(
        panel.status,
        crate::panels::compiled_design::CompiledStatus::Ready
    );
    assert_eq!(panel.compiled_at, app.ui_state.i18n.time(at));
}

/// #2733: a language switch after the compile rewords its time; nothing
/// worded at compile time is kept. The hour depends on the host's clock,
/// so the test reads the shape: en-US has a day period, tr-TR has none.
#[test]
fn a_language_switch_rewords_the_compile_time() {
    let mut app = app_with_a_compile(&PathBuf::from("/p/child.snxsch"));
    let store = &mut app.document_state.compiled_design;
    let (&project, result) = store.results.iter_mut().next().expect("the compile");
    result.compiled_at = chrono::DateTime::from_timestamp(1_700_000_000, 0).expect("a time");
    app.document_state.active_project = Some(project);
    let worded_in = |app: &mut Signex, tag: &str| {
        use signex_i18n::{I18n, LanguagePref};
        app.ui_state.i18n = Arc::new(I18n::new(&LanguagePref::Locale(tag.into())));
        app.compiled_design_view().panel.compiled_at
    };
    let en = worded_in(&mut app, "en-US");
    let tr = worded_in(&mut app, "tr-TR");
    assert!(en.contains("AM") || en.contains("PM"), "{en}");
    assert!(!tr.contains("AM") && !tr.contains("PM"), "{tr}");
    assert_ne!(en, tr);
}
