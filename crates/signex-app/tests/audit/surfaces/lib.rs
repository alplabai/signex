//! Mockup group `lib` (rows m118-m142).
//!
//! Still to add: m119 Library panel, m120 SCH Library panel, m121 Footprint
//! Library panel, m122 Symbol Pins panel, m122b Pin table tab, m126
//! Footprint Properties: Pour / Keepout / Cutout / Pattern / Sketch pad,
//! m127 Library row Properties (component sheet), m129 Library picker, m130
//! Edit row, m131 Primitive picker, m132 Library Document Options, m133 New
//! Library (Create Options), m134 Copy stock primitive to library, m135
//! Library Updates, m136 Close library with unsaved drafts, m137 Delete
//! library row confirm, m138 Library recovery, m139 Prefix / unit pickers,
//! m140 Footprint active bar / context menu / Move By / Align, m141 Symbol
//! editor active bar / context menu, m142 Grid Properties (Cartesian grid
//! editor).

use crate::fixture::{self, Fixture};
use crate::surface::{Surface, WindowSel};
use crate::{render, settle_support};
use signex_app::app::{FileMsg, Message, Signex};
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::library::LibraryMessage;
use signex_app::library::messages::{FootprintEditorMsg, PrimitiveEdit};

pub fn rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m118",
            title: "Library Browser tab",
            open: |app, _, fx| {
                fixture::mount_stock(app, fx)?;
                settle_support::settle(
                    app,
                    Message::Library(LibraryMessage::OpenLibraryBrowser(fx.stock.clone())),
                );
                // Collapse the side docks, as the mock does for the browser, so
                // the grid has the width the design assumes.
                let _ = app.update(Message::Dock(DockMessage::ToggleCollapse(
                    PanelPosition::Left,
                )));
                let _ = app.update(Message::Dock(DockMessage::ToggleCollapse(
                    PanelPosition::Right,
                )));
                // Select the first row so the preview pane shows.
                let (table, row_id) = first_stock_row(app, fx)?;
                let _ = app.update(Message::Library(LibraryMessage::BrowserSelectRow {
                    library_path: fx.stock.clone(),
                    table,
                    row_id,
                }));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m123",
            title: "Symbol editor Properties (symbol / pin / graphic)",
            open: |app, r, fx| {
                fixture::mount_stock(app, fx)?;
                settle_support::settle(
                    app,
                    Message::File(FileMsg::Opened(Some(fx.symbol.clone()))),
                );
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m124",
            title: "Footprint editor Properties (board / selection / snap)",
            open: open_footprint,
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m125",
            title: "Footprint Properties: Pad / pad stack",
            open: |app, r, fx| {
                open_footprint(app, r, fx)?;
                let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
                    path: fx.footprint.clone(),
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SelectPad(Some(0))),
                }));
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m128",
            title: "Component editor window",
            open: |app, _, fx| {
                fixture::mount_stock(app, fx)?;
                let (table, row_id) = first_stock_row(app, fx)?;
                settle_support::settle(
                    app,
                    Message::Library(LibraryMessage::OpenComponentRow {
                        library_path: fx.stock.clone(),
                        table,
                        row_id,
                    }),
                );
                Ok(())
            },
            window: WindowSel::Auto,
            checks: &[],
        },
    ]
}

/// The table and id of the first component row the stock library lists.
fn first_stock_row(app: &Signex, fx: &Fixture) -> Result<(String, signex_library::RowId), String> {
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
    Ok((table, signex_library::RowId(row.row_id)))
}

/// Open the first stock footprint in its editor.
fn open_footprint(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::mount_stock(app, fx)?;
    settle_support::settle(
        app,
        Message::File(FileMsg::Opened(Some(fx.footprint.clone()))),
    );
    let _ = render::draw_main(r, app);
    Ok(())
}
