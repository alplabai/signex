//! What each main-menu row DOES (#1922).
//!
//! Every test builds the real menu model from a real `Signex`, presses one row
//! the way a click does (the row's message through `update`) and asserts on
//! the resulting state: the tool, the dialogs, the document, the clipboard.
//! None of them names the message a row sends, so they keep holding when a
//! row is rewired to another route to the same command.
//!
//! [`Probe`] reads the state a menu row can move; a test states the expected
//! change with struct-update syntax, so every other field is pinned as
//! unchanged.

mod annotation_tile_rows;
mod edit_rows;
mod file_rows;
mod help_tools_rows;
mod language_switch;
mod library_window_rows;
mod place_design_rows;
mod view_rows;
mod walk;
mod window_native_rows;

use crate::app::{Message, Signex, TabInfo, TabKind};
use crate::menu_bar::MenuContext;
use crate::menu_bar::model::{ItemState, Menu, MenuEntry, MenuItem, menu_bar_model};
use iced::Task;
use signex_types::schematic::{Point, SchematicSheet, SelectedItem, SelectedKind, Symbol};
use uuid::Uuid;

pub(super) fn symbol(n: u128, reference: &str, x: f64, y: f64) -> Symbol {
    Symbol {
        uuid: Uuid::from_u128(n),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        position: Point::new(x, y),
        ..Default::default()
    }
}

pub(super) fn sheet_of(symbols: Vec<Symbol>) -> SchematicSheet {
    SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols,
        ..Default::default()
    }
}

/// A named way to build an app in one state.
pub(super) type Fixture = (&'static str, fn() -> Signex);

/// A fresh app on the Altium keymap with the first-run tour closed.
pub(super) fn bare_app() -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.grid_size_mm = 1.0;
    app.ui_state.visible_grid_mm = 1.0;
    app.ui_state
        .keymap_profiles
        .set_active_profile("altium")
        .expect("altium profile");
    app.ui_state.active_keymap = app.ui_state.keymap_profiles.compile_active().into();
    app
}

/// An app showing `sheet` as its only tab, saved at `path`.
pub(super) fn app_with_at(sheet: SchematicSheet, path: std::path::PathBuf) -> Signex {
    let sheet_copy = sheet.clone();
    let mut app = bare_app();
    app.document_state.engines.insert(
        path.clone(),
        signex_engine::Engine::new(sheet).expect("engine"),
    );
    app.document_state.tabs.push(TabInfo {
        title: "menu-rows".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    // What a drawn frame leaves behind: the canvas bounds and the snapshot
    // the selection and fit commands read.
    let canvas = app.interaction_state.active_canvas_mut();
    canvas.set_viewport(iced::Rectangle::new(
        iced::Point::ORIGIN,
        iced::Size::new(800.0, 600.0),
    ));
    canvas.set_render_cache(Some(
        crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet_copy),
    ));
    app
}

pub(super) fn app_with(sheet: SchematicSheet) -> Signex {
    app_with_at(sheet, std::path::PathBuf::from("/w/menu-rows.snxsch"))
}

/// One symbol on the sheet, selected.
pub(super) fn one_symbol_selected() -> Signex {
    let mut app = app_with(sheet_of(vec![symbol(1, "R1", 10.0, 10.0)]));
    select_all_symbols(&mut app);
    app
}

pub(super) fn select_all_symbols(app: &mut Signex) {
    let items = doc(app)
        .symbols
        .iter()
        .map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol))
        .collect();
    app.interaction_state.active_canvas_mut().selected = items;
}

pub(super) fn doc(app: &Signex) -> &SchematicSheet {
    app.document_state
        .active_engine()
        .expect("engine")
        .document()
}

pub(super) fn symbol_editor_app() -> Signex {
    let mut app = bare_app();
    let path = std::path::PathBuf::from("/w/menu-rows.snxsym");
    let file = signex_library::SymbolFile::from_symbol(signex_library::Symbol::empty("s"));
    app.document_state.symbol_editors.insert(
        path.clone(),
        crate::app::SymbolDocument::new(path.clone(), file),
    );
    push_tab(&mut app, &path, TabKind::SymbolEditor(path.clone()));
    app
}

pub(super) fn footprint_editor_app() -> Signex {
    let mut app = bare_app();
    let path = std::path::PathBuf::from("/w/menu-rows.snxfpt");
    let file = signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("f"));
    app.document_state.footprint_editors.insert(
        path.clone(),
        crate::app::FootprintDocument::new(path.clone(), file),
    );
    push_tab(&mut app, &path, TabKind::FootprintEditor(path.clone()));
    app
}

fn push_tab(app: &mut Signex, path: &std::path::Path, kind: TabKind) {
    app.document_state.tabs.push(TabInfo {
        title: "doc".into(),
        path: path.to_path_buf(),
        cached_document: None,
        project_id: None,
        kind,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
}

/// The menu context the bar of `window` (`None`: the main window) is built
/// from.
pub(super) fn menu_context_in(app: &Signex, window: Option<iced::window::Id>) -> MenuContext<'_> {
    MenuContext {
        commands: app.command_context(window),
        has_project: app.document_state.active_project.is_some(),
        active_keymap: Some(std::sync::Arc::clone(&app.ui_state.active_keymap)),
        recent_projects: app.ui_state.recent_projects.entries(),
        open_panels: app.open_panels(),
        i18n: std::sync::Arc::clone(&app.ui_state.i18n),
        cross_select: app.interaction_state.cross_select.enabled,
        pcb_sync: app.pcb_sync_label(window),
    }
}

/// The menu context the main window's bar is built from.
pub(super) fn menu_context(app: &Signex) -> MenuContext<'_> {
    menu_context_in(app, None)
}

fn find_in<'a>(menu: &'a Menu, rest: &[&str]) -> Option<&'a MenuItem> {
    let (head, tail) = rest.split_first()?;
    menu.entries.iter().find_map(|entry| match entry {
        MenuEntry::Item(item) if tail.is_empty() && item.label == *head => Some(item),
        MenuEntry::Submenu(sub) if !tail.is_empty() && sub.title == *head => find_in(sub, tail),
        _ => None,
    })
}

/// The row at `path` ("File/Save", "File/Export/PDF…") of the bar of `window`
/// the app would draw now.
pub(super) fn with_row_in<R>(
    app: &Signex,
    window: Option<iced::window::Id>,
    path: &str,
    read: impl FnOnce(&MenuItem) -> R,
) -> R {
    let ctx = menu_context_in(app, window);
    let model = menu_bar_model(&ctx);
    let parts: Vec<&str> = path.split('/').collect();
    let (root, rest) = parts.split_first().expect("a path");
    let menu = model
        .menus
        .iter()
        .find(|m| m.title == *root)
        .unwrap_or_else(|| panic!("no menu {root}"));
    let item = find_in(menu, rest).unwrap_or_else(|| panic!("no row {path}"));
    read(item)
}

pub(super) fn with_row<R>(app: &Signex, path: &str, read: impl FnOnce(&MenuItem) -> R) -> R {
    with_row_in(app, None, path, read)
}

pub(super) fn row_is_live(app: &Signex, path: &str) -> bool {
    with_row(app, path, |item| item.is_enabled())
}

/// Click the live row at `path`, as the main window's bar does.
pub(super) fn click(app: &mut Signex, path: &str) -> Task<Message> {
    let message = with_row(app, path, |item| {
        item.dispatches()
            .unwrap_or_else(|| panic!("the row {path} is not live"))
            .clone()
    });
    app.update(Message::Menu(message))
}

/// Everything a menu row can move, as plain comparable values.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Probe {
    pub tool: String,
    pub tabs: usize,
    pub active_tab: usize,
    pub selected: usize,
    pub symbols: usize,
    pub wires: usize,
    pub clipboard: bool,
    pub placement: bool,
    pub pending_port: bool,
    pub pending_power: bool,
    pub ghost_label: bool,
    pub grid_visible: bool,
    pub grid_size_mm: f32,
    pub visible_grid_mm: f32,
    pub auto_focus: bool,
    pub find_open: bool,
    pub replace_mode: bool,
    pub picker: bool,
    pub preferences: bool,
    pub shortcuts: bool,
    pub about: bool,
    pub annotate: bool,
    pub annotate_reset: bool,
    pub erc: bool,
    pub parameter_manager: bool,
    pub passive_calculator: bool,
    pub print_preview: bool,
    pub bom_preview: bool,
    pub quit_confirm: bool,
    pub projects: usize,
    pub recent: usize,
    pub can_undo: bool,
    pub can_redo: bool,
    pub dirty: bool,
    pub zoom_percent: f64,
    pub context_menu: bool,
    pub fit_queued: bool,
    pub window_fit_queued: bool,
    pub tour_open: bool,
    pub pcb_camera: (f32, f32, f32),
    pub pcb_fit_queued: bool,
    /// What each open board shows of its layers and ratsnest (#2223).
    pub pcb_views: String,
    pub error_notice: bool,
    pub refs: Vec<String>,
    /// Edit ▸ Move arms a pick on a sheet or a board (#2240).
    pub move_pick: bool,
    pub pcb_carry: bool,
    /// Design ▸ Update PCB Document opens the Update PCB dialog (#1312).
    pub update_pcb: bool,
    /// Design ▸ Board Sketch puts the board in sketch mode (#2234).
    pub board_sketch: bool,
}

pub(super) fn probe(app: &Signex) -> Probe {
    let canvas = app.interaction_state.active_canvas();
    let commands = app.command_context(None);
    Probe {
        tool: format!("{:?}", app.interaction_state.current_tool),
        tabs: app.document_state.tabs.len(),
        active_tab: app.document_state.active_tab,
        selected: canvas.selected.len(),
        symbols: app
            .document_state
            .active_engine()
            .map_or(0, |e| e.document().symbols.len()),
        wires: app
            .document_state
            .active_engine()
            .map_or(0, |e| e.document().wires.len()),
        clipboard: app.interaction_state.clipboard.is_some(),
        placement: app.interaction_state.placement.is_some(),
        pending_port: app.interaction_state.pending_port.is_some(),
        pending_power: app.interaction_state.pending_power.is_some(),
        ghost_label: canvas.ghost_label.is_some(),
        grid_visible: app.ui_state.grid_visible,
        grid_size_mm: app.ui_state.grid_size_mm,
        visible_grid_mm: app.ui_state.visible_grid_mm,
        auto_focus: app.ui_state.auto_focus,
        find_open: app.ui_state.find_replace.open,
        replace_mode: app.ui_state.find_replace.replace_mode,
        picker: app.library.picker.is_some(),
        preferences: app.ui_state.preferences_open,
        shortcuts: app.ui_state.keyboard_shortcuts_open,
        about: app.ui_state.about_open,
        annotate: app.ui_state.annotate_dialog_open,
        annotate_reset: app.ui_state.annotate_reset_confirm,
        erc: app.ui_state.erc_dialog_open,
        parameter_manager: app.ui_state.parameter_manager_open,
        passive_calculator: app.ui_state.passive_calculator_open,
        print_preview: app.document_state.preview.is_some(),
        bom_preview: app.document_state.bom_preview.is_some(),
        quit_confirm: app.ui_state.app_quit_confirm.is_some(),
        projects: app.document_state.projects.len(),
        recent: app.ui_state.recent_projects.entries().len(),
        can_undo: commands.can_undo,
        can_redo: commands.can_redo,
        dirty: commands.has_dirty_documents,
        zoom_percent: canvas.zoom_percent(),
        context_menu: app.interaction_state.context_menu.is_some(),
        fit_queued: canvas.pending_fit.get().is_some(),
        tour_open: app.ui_state.first_run_tour_open,
        pcb_camera: app.interaction_state.pcb_canvas.live_camera(),
        pcb_fit_queued: app.interaction_state.pcb_canvas.pending_fit.get().is_some(),
        pcb_views: format!("{:?}", app.ui_state.pcb_layer_views),
        window_fit_queued: app
            .interaction_state
            .canvases
            .values()
            .any(|c| c.pending_fit.get().is_some()),
        error_notice: app.document_state.error_notice.is_some(),
        refs: app
            .document_state
            .active_engine()
            .map(|e| {
                e.document()
                    .symbols
                    .iter()
                    .map(|s| s.reference.clone())
                    .collect()
            })
            .unwrap_or_default(),
        move_pick: app.ui_state.move_pick.is_some(),
        pcb_carry: app.interaction_state.pcb_carry.is_some(),
        update_pcb: app.ui_state.update_pcb.is_some(),
        board_sketch: app.interaction_state.board_sketch.is_some(),
    }
}

/// Whether `task` carries work (a dialog, a job); `Task::none()` does not.
pub(super) fn has_work(task: &Task<Message>) -> bool {
    task.units() > 0
}

pub(super) fn line(n: u128) -> signex_types::schematic::SchDrawing {
    signex_types::schematic::SchDrawing::Line {
        uuid: Uuid::from_u128(n),
        start: Point::new(n as f64, 0.0),
        end: Point::new(n as f64, 5.0),
        width: 0.25,
        stroke_color: None,
        locked: false,
    }
}

/// Run `task` the way the iced runtime does and feed every message back to
/// the app, answering each clipboard read with `text`.
pub(super) fn settle_answering_clipboard(app: &mut Signex, task: Task<Message>, text: &str) {
    use futures::StreamExt as _;
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    let mut pending = vec![task];
    while let Some(task) = pending.pop() {
        let Some(mut stream) = iced_runtime::task::into_stream(task) else {
            continue;
        };
        let mut messages = Vec::new();
        runtime.block_on(async {
            while let Some(action) = stream.next().await {
                match action {
                    iced_runtime::Action::Output(next) => messages.push(next),
                    iced_runtime::Action::Clipboard(iced_runtime::clipboard::Action::Read {
                        channel,
                        ..
                    }) => assert!(
                        channel
                            .send(Ok(iced::clipboard::Content::Text(text.to_owned())))
                            .is_ok()
                    ),
                    _ => {}
                }
            }
        });
        pending.extend(messages.into_iter().map(|m| app.update(m)));
    }
}

/// An app that copied its selected symbol through the Copy row; the text is
/// what the system clipboard then holds.
pub(super) fn app_that_copied() -> (Signex, String) {
    let mut app = one_symbol_selected();
    let _ = click(&mut app, "Edit/Copy");
    let text = app
        .interaction_state
        .clipboard_text
        .clone()
        .expect("the copy reached the clipboard text");
    (app, text)
}

/// Load a project named `file` into the app and make it the active one.
pub(super) fn open_project(app: &mut Signex, file: &str) -> usize {
    use crate::app::LoadedProject;
    use signex_types::project::ProjectData;
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: std::path::PathBuf::from(format!("/w/{file}")),
        data: ProjectData {
            document_id: None,
            description: None,
            name: file.to_string(),
            dir: "/w".to_string(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            project_defaults: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    let index = app.document_state.projects.len() - 1;
    app.document_state.active_project = Some(app.document_state.projects[index].id);
    index
}

/// An app whose active tab is a PCB loaded the way opening a file loads it:
/// a board with one via, so Fit All has something to fit to.
pub(super) fn loaded_board_app() -> Signex {
    let mut app = bare_app();
    let via = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::now_v7(),
        "position": { "x": 10.0, "y": 10.0 },
        "diameter": 0.6,
    }))
    .expect("via");
    let board = signex_types::pcb::PcbBoard {
        sketch: None,
        unknown: Default::default(),
        uuid: Uuid::now_v7(),
        version: 1,
        generator: "signex-test".into(),
        thickness: 1.6,
        outline: vec![],
        layers: vec![],
        setup: None,
        nets: vec![],
        footprints: vec![],
        segments: vec![],
        vias: vec![via],
        zones: vec![],
        graphics: vec![],
        texts: vec![],
    };
    let _ = app.update(Message::File(crate::app::FileMsg::PcbOpenFinished {
        disk: None,
        path: std::path::PathBuf::from("/w/menu-rows.snxpcb"),
        title: "board".to_string(),
        result: Ok(Box::new(board)),
    }));
    app.interaction_state
        .pcb_canvas
        .set_viewport(iced::Rectangle::new(
            iced::Point::ORIGIN,
            iced::Size::new(800.0, 600.0),
        ));
    // Opening the board queued its first fit; take it out of the picture.
    app.interaction_state.pcb_canvas.pending_fit.set(None);
    assert!(app.has_active_pcb(), "the board is the active tab");
    app
}
