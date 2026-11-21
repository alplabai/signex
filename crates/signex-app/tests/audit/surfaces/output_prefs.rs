//! Mockup group `output-prefs` (rows m111-m117, m143-m150).
//!
//! Still to add: m111 Print Preview / Export PDF, m112 BOM export, m113 BOM
//! Studio, m114 Output Jobs, m115 Fabrication Outputs, m116 Board Print /
//! PDF, m117 Board Reports, m143 Preferences shell + nav, m144a Language,
//! m145 Keyboard Shortcuts, m146 Annotation, m147 Editing (+ fast grids),
//! m148 Rule Profiles, m149 Distributor APIs, m150 Default Component
//! Classes.

use crate::surface::{Check, Surface, WindowSel};
use signex_app::app::{Message, PreferencesMsg, Signex};
use signex_app::preferences::PrefNav;

pub fn rows() -> Vec<Surface> {
    vec![Surface {
        id: "m144",
        title: "Appearance",
        open: |app, _, _| {
            let _ = app.update(Message::Preferences(PreferencesMsg::Open));
            let _ = app.update(Message::Preferences(PreferencesMsg::Nav(
                PrefNav::Appearance,
            )));
            Ok(())
        },
        window: WindowSel::Auto,
        checks: PREFERENCES,
    }]
}

const PREFERENCES: &[Check] = &[("Preferences is open", preferences_open)];

fn preferences_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.preferences_open {
        Ok(())
    } else {
        Err("preferences_open is false".into())
    }
}
