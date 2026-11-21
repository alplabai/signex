//! Checks several groups share. A check only used by one group lives in
//! that group's file.

use signex_app::app::Signex;

/// The extension of the active document tab, if a tab is open.
fn active_extension(app: &Signex) -> Option<String> {
    let d = &app.document_state;
    let tab = d.tabs.get(d.active_tab)?;
    Some(tab.path.extension()?.to_string_lossy().into_owned())
}

fn active_tab_is(app: &Signex, want: &str) -> Result<(), String> {
    match active_extension(app) {
        Some(ext) if ext == want => Ok(()),
        Some(ext) => Err(format!("active tab is a .{ext}, not a .{want}")),
        None => Err(format!("no active tab; expected a .{want}")),
    }
}

pub fn schematic_tab_active(app: &mut Signex) -> Result<(), String> {
    active_tab_is(app, "snxsch")
}

pub fn board_tab_active(app: &mut Signex) -> Result<(), String> {
    active_tab_is(app, "snxpcb")
}

pub fn no_document_tab(app: &mut Signex) -> Result<(), String> {
    match app.document_state.tabs.len() {
        0 => Ok(()),
        n => Err(format!("{n} document tabs are open")),
    }
}
