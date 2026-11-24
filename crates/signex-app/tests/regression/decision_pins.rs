//! Behaviour the maintainers decided to keep on 2026-11-24 (docs/DECISIONS.md,
//! the rows naming mockup rows 2, 58, 64, 65, 67, 75, 80 and 103). Each test
//! pins what trunk already does, so a later change has to name a new decision.
//! Mockup row 27 (the default child sheet path beside the parent) is pinned in
//! the crate, by `child_sheet_create_tests::the_dialog_shows_the_default_path_beside_the_parent`.

use std::path::{Path, PathBuf};

use iced::Size;
use signex_app::app::{
    AnnotateMsg, AssignFootprintsMsg, EditMsg, FileMsg, Message, Signex, TabInfo, TabKind,
    TextEditMsg, WindowMsg,
};
use signex_app::canvas::CanvasEvent;
use signex_app::dock::DockMessage;
use signex_app::find_replace::{FindReplaceMsg, FindScope};
use signex_app::panels::{
    LayersPanelUi, LayersTab, NetInspectorMsg, PanelKind, PanelMsg, PcbLayersPanel,
};
use signex_app::start_screen::Surface;
use signex_app::task_driver::TaskDriver;
use signex_types::schematic::{Label, LabelType, Point, SchematicSheet, Wire};
use tempfile::TempDir;
use uuid::Uuid;

use crate::settle_support::settle;

const WINDOW: Size = Size::new(1400.0, 900.0);

/// Lay a view out with a headless renderer and read back its text widgets,
/// the way the crate's own `layout_probe` does for in-crate view tests.
mod probe {
    use iced::advanced::widget::operation::Text;
    use iced::advanced::widget::{Id, Operation, Tree};
    use iced::advanced::{Layout, Widget as _, layout, renderer};
    use iced::{Element, Font, Pixels, Rectangle, Size, Theme};

    /// One text widget: its content and its bounds in window coordinates.
    #[derive(Debug, Clone)]
    pub(super) struct LaidOut {
        pub text: String,
        pub bounds: Rectangle,
    }

    /// Every text widget of `element` laid out in a window of `window`.
    pub(super) fn texts<M>(
        mut element: Element<'_, M, Theme, iced::Renderer>,
        window: Size,
    ) -> Vec<LaidOut> {
        let renderer =
            iced::Renderer::Secondary(iced_vello_cpu::Renderer::new(renderer::Settings {
                font: Font::DEFAULT,
                text_size: Pixels(16.0),
                ..renderer::Settings::default()
            }));
        let mut tree = Tree::new(&element);
        // The fork's widgets build their child trees in `diff`.
        tree.diff(&mut element);
        element.layout(
            &mut tree,
            &renderer,
            &layout::Limits::new(Size::ZERO, window),
        );
        let size = tree.size;
        let mut collect = Collect(Vec::new());
        element.operate(
            &mut tree,
            Layout::new(size),
            &Rectangle::with_size(size),
            &renderer,
            &mut collect,
        );
        collect.0
    }

    struct Collect(Vec<LaidOut>);

    impl<T> Operation<T> for Collect {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<T>)) {
            operate(self);
        }

        fn text(&mut self, _id: Option<&Id>, bounds: Rectangle, text: &mut dyn Text) {
            self.0.push(LaidOut {
                text: text.text().into_owned(),
                bounds,
            });
        }
    }
}

fn shows(texts: &[probe::LaidOut], label: &str) -> bool {
    texts.iter().any(|t| t.text == label)
}

// ── mockup row 2: a project opens to the hint, no document ─────────────

/// Create project `stem` under `dir` through File ▸ New Project and return
/// its `.snxprj` and the schematic it was written with.
fn written_project(dir: &Path, stem: &str) -> (PathBuf, PathBuf) {
    let (mut app, _boot) = Signex::new();
    let picked = dir.join(format!("{stem}.snxprj"));
    settle(&mut app, Message::File(FileMsg::NewProject(Some(picked))));
    let project = app.document_state.projects[0].path.clone();
    let sheet = app
        .document_state
        .tabs
        .iter()
        .find(|t| matches!(t.kind, TabKind::Schematic))
        .expect("New Project opens its schematic")
        .path
        .clone();
    (project, sheet)
}

#[test]
fn opening_a_saved_project_shows_the_hint_and_opens_no_document() {
    let dir = TempDir::new().expect("tempdir");
    let (project, sheet) = written_project(dir.path(), "Hint");
    assert!(sheet.exists(), "the project has a schematic to open");

    let (mut app, _boot) = Signex::new();
    settle(&mut app, Message::File(FileMsg::Opened(Some(project))));

    let state = &app.document_state;
    assert!(state.active_project.is_some(), "the project is open");
    let sheet_or_board = |t: &&TabInfo| matches!(t.kind, TabKind::Schematic | TabKind::Pcb);
    let opened: Vec<&TabInfo> = state.tabs.iter().filter(sheet_or_board).collect();
    assert!(
        opened.is_empty(),
        "no document opens on its own: {opened:?}"
    );
    assert!(state.engines.is_empty() && state.active_path.is_none());
    assert_eq!(
        Surface::select(state.active_project.is_some()),
        Surface::ProjectHint
    );
}

// ── mockup row 58: an expanded net lists 6 members, then "+ N more" ────

/// A sheet of `count` short wires, each with the global label `VCC` at its
/// start, so the one net `VCC` has `count` label members.
fn vcc_sheet(count: u32) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        generator_version: "0.9".into(),
        paper_size: "A4".into(),
        ..Default::default()
    };
    for i in 0..count {
        let y = f64::from(i) * 5.08;
        sheet.wires.push(Wire {
            uuid: Uuid::from_u128(5800 + u128::from(i)),
            start: Point::new(0.0, y),
            end: Point::new(5.08, y),
            ..Default::default()
        });
        sheet.labels.push(Label {
            uuid: Uuid::from_u128(5900 + u128::from(i)),
            ..Label::new("VCC", Point::new(0.0, y), LabelType::Global)
        });
    }
    sheet
}

/// [`settle`], except the dock layout save is dropped, so opening a panel
/// does not write the layout into the run's shared config folder (as
/// `net_inspector::settle` does).
fn settle_quietly(app: &mut Signex, message: Message) {
    let mut driver = TaskDriver::new();
    let mut queue = std::collections::VecDeque::from([app.update(message)]);
    let mut steps = 0;
    while let Some(task) = queue.pop_front() {
        for message in driver.outputs(task).expect("the task chain settled") {
            steps += 1;
            assert!(steps < 1_000, "the task chain settles");
            if !matches!(message, Message::DockLayout(_)) {
                queue.push_back(app.update(message));
            }
        }
    }
}

fn inspector(app: &mut Signex, msg: NetInspectorMsg) {
    settle_quietly(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::NetInspector(msg))),
    );
}

/// The members `VCC` lists and the count of its "+ N more" row.
fn vcc_listing(app: &Signex) -> (usize, usize) {
    let view = &app.document_state.panel_ctx.net_inspector;
    let vcc = view
        .nets
        .iter()
        .find(|n| n.name == "VCC")
        .expect("VCC is listed");
    (vcc.members.len(), vcc.more)
}

#[test]
fn an_expanded_net_lists_six_members_then_a_more_row_that_lists_the_rest() {
    let dir = TempDir::new().expect("tempdir");
    let (project, sheet) = written_project(dir.path(), "Members");
    let text = signex_types::format::SnxSchematic::new(vcc_sheet(8))
        .write_string()
        .expect("encode");
    std::fs::write(&sheet, text).expect("write the sheet");

    let (mut app, _boot) = Signex::new();
    settle(&mut app, Message::File(FileMsg::Opened(Some(project))));
    settle(&mut app, Message::File(FileMsg::Opened(Some(sheet))));
    settle_quietly(
        &mut app,
        crate::menu_support::menu_row("open_net_inspector"),
    );
    inspector(&mut app, NetInspectorMsg::NetToggled("VCC".into()));
    assert_eq!(vcc_listing(&app), (6, 2), "six members, then + 2 more");

    inspector(&mut app, NetInspectorMsg::MoreMembersShown("VCC".into()));
    assert_eq!(vcc_listing(&app), (8, 0), "the more row lists the rest");
}

// ── mockup row 64: an undocked sheet window has no zoom pill ───────────

/// The main window shows `pin-main.snxsch`; an undocked window shows
/// `pin-window.snxsch`. Returns the app and both window ids.
fn app_with_an_undocked_sheet() -> (Signex, iced::window::Id, iced::window::Id) {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let main = iced::window::Id::unique();
    app.ui_state.main_window_id = Some(main);
    app.ui_state.window_size = (WINDOW.width, WINDOW.height);
    let shown = PathBuf::from("pin-main.snxsch");
    let other = PathBuf::from("pin-window.snxsch");
    for path in [&shown, &other] {
        let engine =
            signex_engine::Engine::new_with_path(Default::default(), Some(path.to_path_buf()))
                .expect("engine");
        app.document_state
            .engines
            .insert(path.to_path_buf(), engine);
        app.document_state.tabs.push(TabInfo {
            title: path.display().to_string(),
            path: path.to_path_buf(),
            cached_document: None,
            project_id: None,
            kind: TabKind::Schematic,
        });
    }
    let _ = app.update(Message::Window(WindowMsg::UndockTab(1)));
    let window = *app.ui_state.windows.keys().next().expect("undocked window");
    let _ = app.update(Message::Window(WindowMsg::UndockedTabOpened {
        path: other,
        id: window,
    }));
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(shown);
    (app, main, window)
}

#[test]
fn an_undocked_sheet_window_has_no_zoom_pill_and_the_main_window_has_one() {
    let (app, main, window) = app_with_an_undocked_sheet();
    let main_texts = probe::texts(app.view(main), WINDOW);
    assert!(shows(&main_texts, "Fit"), "the main canvas has the pill");
    let window_texts = probe::texts(app.view(window), WINDOW);
    assert!(
        !shows(&window_texts, "Fit"),
        "an undocked window has no known canvas rectangle, so no pill: {window_texts:#?}"
    );
}

// ── mockup row 65: a click outside the in-place editor applies it ──────

#[test]
fn a_click_outside_the_in_place_editor_applies_the_edit_as_one_undo_step() {
    use crate::field_edit_support::{app_with, doc, empty_sheet, label};
    let mut sheet = empty_sheet();
    sheet.labels = vec![label("CLK", (20.0, 100.0))];
    let mut app = app_with(sheet);
    let _ = app.update(Message::CanvasEvent(CanvasEvent::DoubleClicked {
        world_x: 20.0,
        world_y: 100.0,
        screen_x: 0.0,
        screen_y: 0.0,
    }));
    assert!(app.interaction_state.editing_text.is_some(), "editor open");
    let _ = app.update(Message::TextEdit(TextEditMsg::Changed("CLK2".into())));

    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: 150.0,
        world_y: 40.0,
    }));

    assert!(
        app.interaction_state.editing_text.is_none(),
        "editor closed"
    );
    assert_eq!(doc(&app).labels[0].text, "CLK2", "the typed text applied");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(doc(&app).labels[0].text, "CLK", "one undo step");
}

// ── mockup row 67: Replace All over the project runs with no prompt ────

fn first_label(app: &Signex, path: &Path) -> String {
    app.document_state.engines[path].document().labels[0]
        .text
        .clone()
}

fn find(app: &mut Signex, msg: FindReplaceMsg) {
    settle(app, Message::FindReplaceMsg(msg));
}

#[test]
fn project_replace_all_changes_every_sheet_with_no_prompt_and_one_undo() {
    let tmp = TempDir::new().expect("tempdir");
    let (mut app, a, b) = crate::net_nav::flows::open_project(tmp.path());
    find(&mut app, FindReplaceMsg::ScopeChanged(FindScope::Project));
    find(&mut app, FindReplaceMsg::QueryChanged("VCC".into()));
    find(&mut app, FindReplaceMsg::ReplacementChanged("VDD".into()));
    assert_eq!(app.ui_state.find_replace.matches.len(), 2, "both sheets");

    find(&mut app, FindReplaceMsg::ReplaceAll);

    assert!(
        !app.ui_state.find_replace.is_busy(),
        "nothing waits on a confirm"
    );
    assert_eq!(first_label(&app, &a), "VDD");
    assert_eq!(first_label(&app, &b), "VDD");
    let _ = app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(first_label(&app, &a), "VCC", "one undo reverts it");
    assert_eq!(first_label(&app, &b), "VCC", "on every sheet");
}

// ── mockup row 75: the Fixed options group opens collapsed ─────────────

fn annotate(app: &mut Signex, msg: AnnotateMsg) {
    let _ = app.update(Message::Annotate(msg));
}

fn fixed_options_open(app: &Signex) -> bool {
    app.ui_state.annotate_session.fixed_options_open
}

#[test]
fn the_annotate_fixed_options_group_opens_collapsed_every_session() {
    let (mut app, _boot) = Signex::new();
    annotate(&mut app, AnnotateMsg::OpenDialog);
    assert!(!fixed_options_open(&app), "collapsed when the dialog opens");
    annotate(&mut app, AnnotateMsg::FixedOptionsToggled);
    assert!(fixed_options_open(&app), "a click unfolds it");

    annotate(&mut app, AnnotateMsg::CloseDialog);
    annotate(&mut app, AnnotateMsg::OpenDialog);
    assert!(
        !fixed_options_open(&app),
        "the next session starts collapsed"
    );
}

// ── mockup row 80: Assign Footprints starts on the All chip ────────────

#[test]
fn assign_footprints_starts_on_all_and_lists_every_part() {
    let mut rig = crate::assign_footprints::rig();
    settle(
        &mut rig.app,
        crate::menu_support::menu_row(crate::assign_footprints::COMMAND),
    );
    crate::assign_footprints::assert_dialog_read(&rig.app);
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    let every: Vec<usize> = (0..state.parts().len()).collect();
    assert_eq!(state.visible_rows(), every, "All lists every part");

    let to = signex_app::app::AssignFilter::Changed;
    crate::assign_footprints::send(&mut rig.app, AssignFootprintsMsg::FilterPicked(to));
    let state = rig.app.ui_state.assign_footprints.as_ref().expect("open");
    assert!(
        state.visible_rows().len() < every.len(),
        "a chip narrows it"
    );
}

// ── mockup row 103: Draft is its own column in View Options ────────────

/// Top copper only, no objects.
fn bare_board() -> signex_types::pcb::PcbBoard {
    serde_json::from_str(
        r#"{
          "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "version": 1,
          "generator": "test", "thickness": 1.6, "outline": [],
          "layers": [ { "id": 0, "name": "TopCopper", "layer_type": "signal" } ],
          "setup": null, "nets": [], "footprints": [],
          "segments": [], "vias": [], "zones": [], "graphics": [], "texts": []
        }"#,
    )
    .expect("valid board")
}

#[test]
fn view_options_draws_draft_as_its_own_column_between_object_and_transparency() {
    let (mut app, _boot) = Signex::new();
    let ctx = &mut app.document_state.panel_ctx;
    let view = signex_renderer::pcb::PcbLayerView::default();
    ctx.pcb_layers = PcbLayersPanel::new(&bare_board(), &view, app.ui_state.i18n.clone());
    ctx.pcb_layers_ui = LayersPanelUi {
        tab: LayersTab::ViewOptions,
        ..LayersPanelUi::default()
    };
    let ctx = app.document_state.panel_ctx.clone();
    let texts = probe::texts(
        signex_app::panels::view_panel(PanelKind::Layers, &ctx),
        Size::new(320.0, 2600.0),
    );
    let draft = texts
        .iter()
        .find(|t| t.text == "Draft")
        .expect("a Draft header");
    let y = draft.bounds.center_y();
    let on_row = |label: &str| {
        texts
            .iter()
            .find(|t| t.text == label && (t.bounds.center_y() - y).abs() < 2.0)
            .unwrap_or_else(|| panic!("{label} beside Draft: {texts:#?}"))
    };
    let (object, fade) = (on_row("Object"), on_row("Transparency"));
    assert!(object.bounds.x < draft.bounds.x && draft.bounds.x < fade.bounds.x);
}
