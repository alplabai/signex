//! One audited UI surface (a mockup row): how to open it, which window to
//! capture and the functional checks to drive through `Signex::update`.

use crate::fixture::Fixture;
use crate::render::{self, Crop};
use crate::surfaces::pointer::{PointerStep, draw_after};
use signex_app::app::Signex;

/// A functional check run after the capture. It may send messages through
/// `Signex::update`; an `Err` carries what went wrong.
pub type CheckFn = fn(&mut Signex) -> Result<(), String>;
/// A named check, so the report says which one failed.
pub type Check = (&'static str, CheckFn);

/// Brings a fresh app to the surface. An `Err` skips the row with a reason.
pub type OpenFn = fn(&mut Signex, &mut iced::Renderer, &Fixture) -> Result<(), String>;

/// Which window, or part of one, the PNG shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowSel {
    /// A dialog opened as its own window if there is one, else the main
    /// window.
    Auto,
    /// The main window, even when a detached dialog is open.
    Main,
    /// The detached dialog window; the row fails when none opened.
    Detached,
    /// The detached dialog window at this height, for a page that scrolls.
    DetachedTall(u32),
    /// A rectangle of the main window.
    MainCrop(Crop),
    /// The main window after these pointer steps, drawn from the widget
    /// state they left, so an open menu or pick list shows.
    MainPointer(&'static [PointerStep]),
    /// [`Self::MainPointer`] in a window this tall, for a menu that does not
    /// fit the usual height.
    MainPointerTall(&'static [PointerStep], u32),
    /// The newest dock panel opened as its own window.
    DetachedPanel,
}

/// The size a panel in its own window is drawn at.
const PANEL_SIZE: (u32, u32) = (720, 560);

pub struct Surface {
    /// The mockup row, `"m42"` for row 42; a letter suffix (`"m122b"`) as
    /// in the mockup ids.
    pub id: &'static str,
    pub title: &'static str,
    pub open: OpenFn,
    pub window: WindowSel,
    pub checks: &'static [Check],
}

/// Draw the window `sel` names and return its RGBA and size.
pub fn capture(
    app: &Signex,
    r: &mut iced::Renderer,
    sel: WindowSel,
) -> Result<(Vec<u8>, (u32, u32)), String> {
    // A first draw lets the canvases learn their size before the capture.
    let _ = render::draw_main(r, app);
    let main = (render::WIDTH, render::HEIGHT);
    match sel {
        WindowSel::Auto => Ok(match render::detached_modal(app) {
            Some(id) => (
                render::draw_window(r, app, id, render::DETACHED_SIZE),
                render::DETACHED_SIZE,
            ),
            None => (render::draw_main(r, app), main),
        }),
        WindowSel::Main => Ok((render::draw_main(r, app), main)),
        WindowSel::Detached => {
            let id = render::detached_modal(app).ok_or("no detached dialog window opened")?;
            Ok((
                render::draw_window(r, app, id, render::DETACHED_SIZE),
                render::DETACHED_SIZE,
            ))
        }
        WindowSel::DetachedTall(height) => {
            let id = render::detached_modal(app).ok_or("no detached dialog window opened")?;
            let size = (render::DETACHED_SIZE.0, height);
            Ok((render::draw_window(r, app, id, size), size))
        }
        WindowSel::MainCrop(rect) => Ok(render::crop(&render::draw_main(r, app), main, rect)),
        WindowSel::MainPointer(steps) => Ok((draw_after(r, app, steps, render::HEIGHT)?, main)),
        WindowSel::MainPointerTall(steps, height) => {
            Ok((draw_after(r, app, steps, height)?, (render::WIDTH, height)))
        }
        WindowSel::DetachedPanel => {
            let id = app
                .ui_state
                .windows
                .iter()
                .filter(|(_, kind)| format!("{kind:?}").starts_with("DetachedPanel"))
                .map(|(id, _)| *id)
                .max()
                .ok_or("no detached panel window opened")?;
            Ok((render::draw_window(r, app, id, PANEL_SIZE), PANEL_SIZE))
        }
    }
}
