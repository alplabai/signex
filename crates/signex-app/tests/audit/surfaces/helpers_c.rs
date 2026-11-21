//! Set-up steps the `sch_a` and `sch_b` rows share: driving a message with
//! its tasks, showing a dock panel, picking on the canvas and adding objects
//! the sample sheet does not have.

use crate::fixture::{self, Fixture};
use crate::render;
use signex_app::app::{FileMsg, Message, OverlayMsg, Signex};
use signex_app::canvas::{BoxDirection, CanvasEvent};
use signex_app::panels::PanelKind;
use signex_app::schematic_runtime::SchematicRenderCache;
use signex_app::task_driver::TaskDriver;
use signex_types::schematic::{SchematicSheet, SelectedKind};
use std::collections::VecDeque;

/// Messages one drive may feed back before it gives up.
const MAX_STEPS: usize = 10_000;

/// Send `message` and run what it leads to through `driver`. The dock
/// layout save is dropped, so an opened panel is never written to the
/// user's config folder.
pub fn drive_in(driver: &mut TaskDriver, app: &mut Signex, message: Message) -> Result<(), String> {
    let mut queue = VecDeque::from([app.update(message)]);
    let mut steps = 0;
    while let Some(task) = queue.pop_front() {
        for next in driver.outputs(task).map_err(|e| e.to_string())? {
            steps += 1;
            if steps > MAX_STEPS {
                return Err("the task chain did not settle".into());
            }
            if !matches!(next, Message::DockLayout(_)) {
                queue.push_back(app.update(next));
            }
        }
    }
    Ok(())
}

/// [`drive_in`] with a fresh driver.
pub fn drive(app: &mut Signex, message: Message) -> Result<(), String> {
    drive_in(&mut TaskDriver::new(), app, message)
}

/// The message a main-menu row sends for command `id`.
pub fn command(id: &str) -> Result<Message, String> {
    let id = signex_app::keymap::AppCommandId::new(id)?;
    Ok(Message::Menu(signex_app::menu_bar::MenuMessage::Command(
        id,
    )))
}

/// Open the dock panel `kind` as View > Panels does and bring its tab to
/// the front.
pub fn show_panel(app: &mut Signex, kind: PanelKind) -> Result<(), String> {
    drive(app, Message::Overlay(OverlayMsg::OpenPanel(kind)))?;
    fixture::select_panel(app, kind)
}

/// The kinds of the objects selected on the active schematic canvas.
pub fn selected_kinds(app: &Signex) -> Vec<SelectedKind> {
    app.interaction_state
        .active_canvas()
        .selected
        .iter()
        .map(|item| item.kind)
        .collect()
}

/// `Ok` when the selection holds a `want`, else what it holds.
pub fn expect_selected(app: &Signex, want: SelectedKind) -> Result<(), String> {
    let kinds = selected_kinds(app);
    if kinds.contains(&want) {
        Ok(())
    } else {
        Err(format!("expected a {want:?} selected, got {kinds:?}"))
    }
}

/// Click the sheet at `at` with the select tool and expect a `want` picked.
pub fn click_pick(app: &mut Signex, at: (f64, f64), want: SelectedKind) -> Result<(), String> {
    drive(
        app,
        Message::CanvasEvent(CanvasEvent::Clicked {
            world_x: at.0,
            world_y: at.1,
        }),
    )?;
    expect_selected(app, want)
}

/// Box-select from `from` to `to` (world mm). Left to right takes what is
/// wholly inside, right to left what the box touches.
pub fn box_select(
    app: &mut Signex,
    from: (f64, f64),
    to: (f64, f64),
    direction: BoxDirection,
) -> Result<(), String> {
    drive(
        app,
        Message::CanvasEvent(CanvasEvent::BoxSelect {
            x1: from.0,
            y1: from.1,
            x2: to.0,
            y2: to.1,
            direction,
        }),
    )
}

/// Add objects to the active sheet in memory (never saved) and redraw its
/// hit-test cache, so they can be picked like the sample's own.
pub fn add_to_sheet(app: &mut Signex, edit: fn(&mut SchematicSheet)) -> Result<(), String> {
    let engine = app
        .document_state
        .active_engine_mut()
        .ok_or("no active schematic engine")?;
    let mut sheet = engine.document().clone();
    edit(&mut sheet);
    engine.set_document(sheet.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(SchematicRenderCache::from_sheet(&sheet)));
    Ok(())
}

/// Open the first stock footprint in its editor.
pub fn open_footprint(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    drive(
        app,
        Message::File(FileMsg::Opened(Some(fx.footprint.clone()))),
    )?;
    let _ = render::draw_main(r, app);
    Ok(())
}

/// The library, table and id of the first component row the stock library
/// lists.
pub fn first_stock_row(
    app: &Signex,
    fx: &Fixture,
) -> Result<signex_app::library::state::ComponentsPanelRowKey, String> {
    let (_, row) = app
        .library
        .all_components()
        .into_iter()
        .next()
        .ok_or("stock lists no component")?;
    let table = app
        .library
        .table_of_row(&fx.stock, row.row_id)
        .ok_or("no table for the row")?;
    Ok(signex_app::library::state::ComponentsPanelRowKey {
        library_path: fx.stock.clone(),
        table,
        row_id: signex_library::RowId(row.row_id),
    })
}
