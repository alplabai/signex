//! Mockup group `global` (rows m1-m18).
//!
//! Still to add: m2 Project hint (project loaded, no document), m3 Start row
//! context menu, m4 Title bar + menu bar, m5 Tab bar + tab context menu, m6
//! Status bar, m7 Panel list (View > Panels), m8 Dock drag zones + detached
//! panel window, m10 Hover tooltip, m11 Error notice card, m12 About Signex,
//! m13 Keyboard shortcuts reference, m14 First-run onboarding tour, m15
//! Autosave recovery prompt, m16 Exit Signex - Unsaved Edits, m17 Passive
//! Network Calculator, m18 Signex AI Agent pane / journals.

use super::checks;
use crate::fixture;
use crate::surface::{Check, Surface, WindowSel};
use signex_app::app::{CommandPaletteMsg, Message, Signex};

pub fn rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m1",
            title: "Start screen (New/Open/Sample/Shortcuts + recent projects)",
            // Libraries the app has loaded, as the start screen counts them.
            open: |app, _, fx| fixture::mount_stock(app, fx),
            window: WindowSel::Auto,
            checks: START,
        },
        Surface {
            id: "m9",
            title: "Command palette",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let _ = app.update(Message::CommandPalette(CommandPaletteMsg::Open));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PALETTE,
        },
    ]
}

const START: &[Check] = &[("no document tab is open", checks::no_document_tab)];

const PALETTE: &[Check] = &[("the palette is open", palette_open)];

fn palette_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.command_palette.open {
        Ok(())
    } else {
        Err("command_palette.open is false".into())
    }
}
