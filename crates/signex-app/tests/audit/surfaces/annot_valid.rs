//! Mockup group `annot-valid` (rows m75-m92).
//!
//! Still to add: m76 Reset annotation confirm, m77 Re-prefix to standard
//! confirm, m78 Number Schematic Sheets, m79 Synchronize Sheet Entries and
//! Ports, m80 Assign Footprints, m81 Update PCB (ECO), m82 Back Annotate
//! Schematics, m83 Re-Annotate Board, m84 ERC settings / run dialog, m85 ERC
//! panel (+ waivers), m87 DRC panel, m88 DRC results, m89 Rules tab
//! (constraint editor), m90 Rules tab: clearance matrix, m91 Rules tab:
//! presets / text view, m92 Rule editor.

use super::checks;
use crate::surface::{Check, Surface, WindowSel};
use crate::{fixture, settle_support};
use signex_app::app::{AnnotateMsg, Message, PreferencesMsg, Signex};
use signex_app::preferences::PrefMsg;

pub fn rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m75",
            title: "Annotate Schematics",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                settle_support::settle(app, Message::Annotate(AnnotateMsg::OpenDialog));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: ANNOTATE,
        },
        Surface {
            id: "m86",
            title: "Messages panel",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                // Two real log entries, so the Messages tab shows rows and its
                // count.
                signex_app::diagnostics::log_warning("Unconnected input pin TS on U1 (BQ24072)");
                signex_app::diagnostics::log_warning(
                    "Unconnected output pin PGOOD on U1 (BQ24072)",
                );
                let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
                    PrefMsg::DraftTheme(app.ui_state.theme_id),
                )));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: MESSAGES,
        },
    ]
}

const ANNOTATE: &[Check] = &[("the Annotate dialog is open", annotate_open)];

const MESSAGES: &[Check] = &[("a schematic tab is active", checks::schematic_tab_active)];

fn annotate_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.annotate_dialog_open {
        Ok(())
    } else {
        Err("annotate_dialog_open is false".into())
    }
}
