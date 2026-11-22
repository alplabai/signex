//! Function audit, wave A batch g7a: mockup rows m52 to m58 and m61 to m64.
//!
//! Each test drives a row's controls through `Signex::update` and checks what
//! they change. Filter presets and the Previews switch run `update` without
//! their returned task, so the user's preferences file is never written.

use std::collections::VecDeque;
use std::sync::Arc;

use iced::{Point, Rectangle, Size, Vector};
use pcb_geom::PickKind;
use signex_app::active_bar::{
    ActiveBarAction, ActiveBarMenu, ActiveBarMsg, CUSTOM_FILTER_PRESET_LIMIT, SelectionFilter,
};
use signex_app::app::{
    ContextAction, ContextMenuMsg, ContextSubmenu, EditMsg, FileMsg, GridPropertiesMsg, Message,
    OverlayMsg, Signex, Tool, UiMsg,
};
use signex_app::canvas::{BoxDirection, CanvasEvent};
use signex_app::dock::DockMessage;
use signex_app::keymap::AppCommandId;
use signex_app::library::LibraryMessage;
use signex_app::library::state::{
    ComponentsMountSource, ComponentsPanelRowKey, ComponentsRowAction, ComponentsRowStyle,
};
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::components_panel::details::preview_view::{
    PreviewGesture, PreviewPane, PreviewView,
};
use signex_app::panels::net_inspector::{NetInspectorTab, NetListStatus};
use signex_app::panels::{
    FilterEditor, KindChip, NavigatorMsg, NetInspectorMsg, ObjectKind, ObjectListMsg,
    ObjectListStatus, ObjectListView, ObjectScope, PanelKind, PanelMsg, PcbBoardMsg, SchFilterMsg,
};
use signex_app::schematic_runtime::SchematicRenderCache;
use signex_app::task_driver::TaskDriver;
use signex_i18n::I18n;
use signex_library::RowId;
use signex_types::corner::CornerMode;
use signex_types::schematic::{SelectedItem, SelectedKind};
use signex_types::theme::ThemeId;
use signex_widgets::tree_view::TreeMsg;
use uuid::Uuid;

use crate::fixture::{self, Fixture};

#[path = "g7a/active_bar_menu.rs"]
mod active_bar_menu;
#[path = "g7a/components.rs"]
mod components;
#[path = "g7a/filters.rs"]
mod filters;
#[path = "g7a/grid_zoom.rs"]
mod grid_zoom;
#[path = "g7a/lists.rs"]
mod lists;

/// Send `message` and run what it leads to. The dock layout save is dropped,
/// so an opened panel is never written to the user's config folder.
fn drive(app: &mut Signex, message: Message) {
    let mut driver = TaskDriver::new();
    let mut queue = VecDeque::from([app.update(message)]);
    let mut steps = 0;
    while let Some(task) = queue.pop_front() {
        for next in driver.outputs(task).expect("the task chain settled") {
            steps += 1;
            assert!(steps < 10_000, "the task chain settles");
            if !matches!(next, Message::DockLayout(_)) {
                queue.push_back(app.update(next));
            }
        }
    }
}

fn panel(msg: PanelMsg) -> Message {
    Message::Dock(DockMessage::Panel(msg))
}

fn library(msg: LibraryMessage) -> Message {
    Message::Library(msg)
}

fn command(id: &str) -> AppCommandId {
    AppCommandId::new(id).expect("a catalog command id")
}

fn open_panel(app: &mut Signex, kind: PanelKind) {
    drive(app, Message::Overlay(OverlayMsg::OpenPanel(kind)));
    fixture::select_panel(app, kind).expect("the panel has a dock tab");
}

/// The sample project with its sheet in view, hit-testable, with every filter
/// kind on and no saved presets, whatever the user's preferences hold.
fn sample() -> (Signex, Fixture) {
    let fx = fixture::fixture().expect("fixture");
    let mut app = fixture::fresh(ThemeId::DEFAULT);
    drive(
        &mut app,
        Message::File(FileMsg::Opened(Some(fx.sample.clone()))),
    );
    drive(
        &mut app,
        Message::File(FileMsg::Opened(Some(fx.sample.with_extension("snxsch")))),
    );
    let sheet = app
        .document_state
        .active_engine()
        .expect("the sample sheet is open")
        .document()
        .clone();
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(SchematicRenderCache::from_sheet(&sheet)));
    app.interaction_state.custom_filter_presets.clear();
    app.interaction_state.active_custom_filter_tab = 0;
    app.document_state.panel_ctx.custom_filter_presets.clear();
    app.document_state.panel_ctx.active_custom_filter_tab = 0;
    let _ = app.update(panel(PanelMsg::SchFilter(SchFilterMsg::AllPicked)));
    (app, fx)
}

/// How many symbols, wires and labels the sheet in view holds.
fn doc_counts(app: &Signex) -> (usize, usize, usize) {
    let doc = app
        .document_state
        .active_engine()
        .expect("a sheet in view")
        .document();
    (doc.symbols.len(), doc.wires.len(), doc.labels.len())
}

fn symbol_id(app: &Signex, reference: &str) -> Option<Uuid> {
    app.document_state
        .active_engine()?
        .document()
        .symbols
        .iter()
        .find(|s| s.reference == reference)
        .map(|s| s.uuid)
}

fn is_clean(app: &Signex) -> bool {
    app.document_state.dirty_document_count() == 0
}

fn is_on(app: &Signex, kind: SelectionFilter) -> bool {
    app.interaction_state.selection_filters.contains(&kind)
}

fn on_count(app: &Signex) -> usize {
    app.interaction_state.selection_filters.len()
}
