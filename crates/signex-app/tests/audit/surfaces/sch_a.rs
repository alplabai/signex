//! Mockup group `sch-a` (rows m40-m57).
//!
//! Still to add: m40 Properties: Document Options (General / Parameters),
//! m41 Properties: pre-placement, m43 Properties: Wire / bus, m44
//! Properties: Net label / port, m45 Properties: Power port, m46 Properties:
//! Sheet symbol, m47 Properties: Text note, m48 Properties: Drawing shape,
//! m49 Properties: Directive / No-ERC, m50 Properties: Pin (placed), m51
//! Properties: Multi-selection, m53 Components row context menu + drag
//! label, m54 Navigator, m55 Filter (selection filter), m56 SCH Filter, m57
//! Object List.

use super::checks;
use crate::fixture;
use crate::render;
use crate::surface::{Check, Surface, WindowSel};
use signex_app::app::Message;
use signex_app::canvas::CanvasEvent;
use signex_app::panels::PanelKind;

pub fn rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m42",
            title: "Properties: Part",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                // Click U1 (88.9, 101.6 mm) so Properties shows the component.
                let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
                    world_x: 88.9,
                    world_y: 101.6,
                }));
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: SCHEMATIC,
        },
        Surface {
            id: "m52",
            title: "Components panel",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                fixture::mount_stock(app, fx)?;
                fixture::select_panel(app, PanelKind::Components)
            },
            window: WindowSel::Auto,
            checks: SCHEMATIC,
        },
    ]
}

const SCHEMATIC: &[Check] = &[("a schematic tab is active", checks::schematic_tab_active)];
