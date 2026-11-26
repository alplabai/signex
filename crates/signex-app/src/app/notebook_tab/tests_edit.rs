//! The notebook tab's editing pieces: the state's line mapping and views,
//! typing `@`, the insert toolbar, the picker, the next problem and a
//! compile landing (mockup row 195).

use std::sync::Arc;

use iced::widget::text_editor::{Action, Edit, Motion};
use signex_notebook::{
    AnnotationKind, DesignIndex, DesignPart, Insertion, NotebookDocument, Reference,
};

use super::compile::{CompileOutcome, land};
use super::edit::{Acted, act, insert_at_cursor, typed};
use super::picker::Picker;
use super::tests::{NOTEBOOK, app_with_notebook, body, send};
use super::{Load, NotebookMsg, NotebookTab, NotebookText, SidePanel, ViewMode};

/// An open notebook document with `body` below `header_lines` header
/// lines, and its tab.
fn open_tab(body: &str, header_lines: u32) -> (NotebookDocument, NotebookTab) {
    let mut doc = NotebookDocument::reading(std::path::PathBuf::from("/p/notebook/design.snxnbk"));
    doc.body = Some(body.to_string());
    doc.header_lines = header_lines;
    doc.load = Load::Open;
    let mut tab = NotebookTab {
        text: Some(NotebookText::new(body)),
        ..NotebookTab::default()
    };
    tab.text.as_mut().unwrap().refresh(&doc);
    (doc, tab)
}

/// Put `tab`'s cursor at the end of its text.
fn to_end(tab: &mut NotebookTab) {
    let text = tab.text.as_mut().unwrap();
    text.content.perform(Action::Move(Motion::DocumentEnd));
}

fn id_of(inserted: &signex_notebook::Inserted) -> Option<String> {
    inserted.id.as_ref().map(ToString::to_string)
}

/// A design of the parts `names`, on one sheet, with no nets.
fn design_of(names: &[&str]) -> Arc<DesignIndex> {
    let parts = names
        .iter()
        .zip(1u128..)
        .map(|(name, n)| DesignPart {
            designator: (*name).into(),
            id: uuid::Uuid::from_u128(n),
            value: String::new(),
            description: String::new(),
            footprint: String::new(),
            sheet: "power.snxsch".into(),
            fitted_in: Vec::new(),
            variant_count: 0,
            pins: Vec::new(),
        })
        .collect();
    Arc::new(DesignIndex::new(parts, Vec::new()))
}

#[test]
fn the_views_cycle_and_every_token_reads_back() {
    assert_eq!(ViewMode::default(), ViewMode::Split, "Split, as drawn");
    assert_eq!(ViewMode::Split.next(), ViewMode::Preview);
    assert_eq!(ViewMode::Preview.next(), ViewMode::Source);
    assert_eq!(ViewMode::Source.next(), ViewMode::Split);
    for view in ViewMode::ALL {
        assert_eq!(ViewMode::parse(view.token()), Some(view));
    }
    for panel in SidePanel::ALL {
        assert_eq!(SidePanel::parse(panel.token()), Some(panel));
    }
    assert!(ViewMode::Split.shows_source() && ViewMode::Split.shows_preview());
    assert!(!ViewMode::Source.shows_preview() && !ViewMode::Preview.shows_source());
}

#[test]
fn body_lines_are_numbered_from_one_past_the_header() {
    let (doc, _tab) = open_tab("a\nb\n", 3);
    assert_eq!(doc.file_line(0), 4);
    assert_eq!(doc.body_line(4), Some(0));
    assert_eq!(doc.body_line(3), None, "a header line");
    let sources = doc.compile_sources();
    let main = String::from_utf8(sources.get("design.snxnbk").unwrap().to_vec()).unwrap();
    assert_eq!(main, "\n\n\na\nb\n", "header lines blank, numbers kept");
}

#[test]
fn typing_at_opens_the_picker_and_types_nothing() {
    let (doc, mut tab) = open_tab("x", 0);
    assert_eq!(act(&doc, &mut tab, typed('@')), Acted::OpensPicker);
    assert_eq!(tab.text.as_ref().unwrap().body(), "x");
    assert_eq!(act(&doc, &mut tab, typed('y')), Acted::Edited);
    let right = Action::Move(Motion::Right);
    assert_eq!(act(&doc, &mut tab, right), Acted::Moved);
}

#[test]
fn a_block_goes_on_a_line_of_its_own_and_a_reference_runs_in_the_text() {
    let (mut doc, mut tab) = open_tab("= Intro\nText", 0);
    to_end(&mut tab);
    let note = Insertion::Block(AnnotationKind::Note);
    let inserted = insert_at_cursor(&mut doc, &mut tab, &note).unwrap();
    assert_eq!(id_of(&inserted).as_deref(), Some("NOTE-001"));
    let typed_text = tab.text.as_ref().unwrap().body();
    let own_line = typed_text.contains("Text\n#note(id: \"NOTE-001\")");
    assert!(own_line, "{typed_text}");
    let (mut doc, mut tab) = open_tab("See ", 0);
    to_end(&mut tab);
    let reference = Insertion::Reference(Reference::Component("R7".into()));
    assert!(insert_at_cursor(&mut doc, &mut tab, &reference).is_some());
    assert_eq!(tab.text.as_ref().unwrap().body(), "See #component(\"R7\")");
}

#[test]
fn a_calculation_takes_the_section_of_the_cursor_in_the_file() {
    let (mut doc, mut tab) = open_tab("= A\n= B\n", 3);
    let text = tab.text.as_mut().unwrap();
    super::edit::move_to_line(text, 2);
    let calculation = Insertion::Block(AnnotationKind::Calculation);
    let inserted = insert_at_cursor(&mut doc, &mut tab, &calculation).unwrap();
    assert_eq!(id_of(&inserted).as_deref(), Some("CALC-2.1"));
}

#[test]
fn the_picker_filters_steps_and_chooses() {
    let mut picker = Picker::new(design_of(&["R10", "R2", "C1"]));
    picker.set_query("r".into());
    let names: Vec<String> = picker
        .entries
        .iter()
        .map(|entry| entry.reference.name())
        .collect();
    assert_eq!(names, ["R2", "R10"]);
    assert_eq!(picker.matches, 2);
    picker.step(true);
    picker.step(true);
    assert_eq!(picker.row, 1, "stays on the list");
    let r10 = Reference::Component("R10".into());
    assert_eq!(picker.chosen(None), Some(r10));
    picker.step(false);
    let r2 = Reference::Component("R2".into());
    assert_eq!(picker.chosen(Some(0)), Some(r2));
    assert_eq!(picker.chosen(Some(9)), None);
}

#[test]
fn a_failed_compile_keeps_the_last_good_one_on_screen() {
    let dir = tempfile::tempdir().unwrap();
    let (app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let tab = &app.ui_state.notebook_tabs[&id];
    let good = tab.compile.last.clone().expect("a compile");
    assert!(good.status.compiled);
    let (_doc, mut broken) = open_tab("#(1 + ", 0);
    broken.compile.last_good = Some(good.clone());
    broken.compile.shown = tab.compile.shown.clone();
    let failed = CompileOutcome {
        generation: 1,
        status: signex_notebook::NotebookStatus {
            compiled: false,
            pages: Vec::new(),
            ..good.status.clone()
        },
        notebook: None,
        elapsed_ms: 1,
        finished_at: "14:02:19".into(),
        page: None,
        thumbnails: Vec::new(),
        lane: super::Lane::default(),
    };
    land(&mut broken, Arc::new(failed));
    assert!(broken.compile.failed());
    assert!(broken.compile.shown.is_some(), "the last good page stays");
    let kept = broken.compile.last_good.as_ref().map(|g| &g.finished_at);
    assert_eq!(kept, Some(&good.finished_at));
    assert!(broken.compile.running.is_none());
}

#[test]
fn the_at_key_in_the_tab_opens_the_picker_and_enter_inserts_its_row() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    send(
        &mut app,
        NotebookMsg::TextActed {
            project_id: id,
            action: typed('@'),
        },
    );
    let picker = app.ui_state.notebook_tabs[&id].picker.as_ref();
    assert!(picker.is_some(), "the picker opened");
    // No design is read here, so the picker lists nothing and Enter
    // inserts nothing; an entry is offered once a design is.
    app.ui_state.notebook_tabs.get_mut(&id).unwrap().picker = Some(Picker::new(design_of(&["R7"])));
    send(
        &mut app,
        NotebookMsg::PickerChosen {
            project_id: id,
            row: None,
        },
    );
    assert!(app.ui_state.notebook_tabs[&id].picker.is_none());
    let typed_text = body(&app, id);
    assert!(typed_text.starts_with("#component(\"R7\")"), "{typed_text}");
}

#[test]
fn esc_closes_the_picker_then_the_block_menu() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    send(&mut app, NotebookMsg::BlockMenuToggled { project_id: id });
    send(&mut app, NotebookMsg::PickerOpened { project_id: id });
    let tab = &app.ui_state.notebook_tabs[&id];
    assert!(tab.picker.is_some());
    assert!(!tab.block_menu, "one at a time");
    send(&mut app, NotebookMsg::BlockMenuToggled { project_id: id });
    send(&mut app, NotebookMsg::EscapePressed { project_id: id });
    assert!(app.ui_state.notebook_tabs[&id].picker.is_none());
    assert!(!app.ui_state.notebook_tabs[&id].block_menu);
}

#[test]
fn f8_moves_the_cursor_to_the_next_problem_and_wraps() {
    let dir = tempfile::tempdir().unwrap();
    let broken = NOTEBOOK.replace("= Power stage", "= Power stage\n#(1 + ");
    let (mut app, id) = app_with_notebook(dir.path(), &broken);
    let at = app.notebook_next_problem(id).expect("a problem");
    assert_eq!(at.0, 6, "the line in the file");
    let (doc, tab) = app.notebook_parts(id).unwrap();
    let (line, _) = tab.text.as_ref().unwrap().cursor();
    assert_eq!(doc.file_line(line), 6);
    // However many problems follow, F8 comes back round to the first.
    let lines: Vec<u32> = (0..8)
        .filter_map(|_| app.notebook_next_problem(id).map(|(line, _)| line))
        .collect();
    assert!(lines.iter().all(|line| *line >= 6), "{lines:?}");
    assert!(lines.contains(&6), "it wraps: {lines:?}");
}

#[test]
fn a_view_pick_and_ctrl_backslash_change_only_the_view() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    send(
        &mut app,
        NotebookMsg::ViewPicked {
            project_id: id,
            view: ViewMode::Source,
        },
    );
    assert_eq!(app.ui_state.notebook_tabs[&id].view, ViewMode::Source);
    send(&mut app, NotebookMsg::ViewCycled { project_id: id });
    assert_eq!(app.ui_state.notebook_tabs[&id].view, ViewMode::Split);
    assert!(!super::tests::is_dirty(&app, id));
}

#[test]
fn an_edit_with_paste_is_one_edit() {
    let (mut doc, mut tab) = open_tab("", 0);
    let paste = Action::Edit(Edit::Paste(Arc::new("= A\n".into())));
    assert_eq!(act(&doc, &mut tab, paste), Acted::Edited);
    super::edit::sync_body(&mut doc, &mut tab);
    assert_eq!(tab.text.as_ref().unwrap().outline.sections.len(), 1);
}

/// A compile that is not the one running (a stale one, landing after the
/// tab was reopened or a later compile started) is dropped.
#[test]
fn a_compile_that_is_not_the_running_one_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, id) = app_with_notebook(dir.path(), NOTEBOOK);
    let kept = app.ui_state.notebook_tabs[&id].compile.last.clone();
    let good = kept.clone().expect("a compile");
    let stale = CompileOutcome {
        generation: 999,
        finished_at: "stale".into(),
        notebook: None,
        page: None,
        thumbnails: Vec::new(),
        lane: super::Lane::default(),
        elapsed_ms: 1,
        status: good.status.clone(),
    };
    send(
        &mut app,
        NotebookMsg::Compiled {
            project_id: id,
            outcome: Arc::new(stale),
        },
    );
    let last = app.ui_state.notebook_tabs[&id].compile.last.clone();
    assert_eq!(
        last.map(|l| l.finished_at.clone()),
        kept.map(|k| k.finished_at.clone())
    );
}
