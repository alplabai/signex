//! Function audit, wave B: the global chrome, mockup rows m1 to m17 (Start
//! screen, project hint, row menu, title and menu bar, tabs, status bar, panel
//! list, dock drag zones, command palette, hover tooltip, error notice, About,
//! Keyboard Shortcuts, first-run tour, autosave recovery, exit prompt, Passive
//! Calculator). Each row's controls are driven through `Signex::update`.

#[path = "b_chrome/bars.rs"]
mod bars;
#[path = "b_chrome/calculator.rs"]
mod calculator;
#[path = "b_chrome/dialogs.rs"]
mod dialogs;
#[path = "b_chrome/docking.rs"]
mod docking;
#[path = "b_chrome/palette.rs"]
mod palette;
#[path = "b_chrome/quit.rs"]
mod quit;
#[path = "b_chrome/recovery.rs"]
mod recovery;
#[path = "b_chrome/start.rs"]
mod start;
#[path = "b_chrome/support.rs"]
mod support;
