//! Function audit, wave C: the Preferences dialog (mockup rows 143 to 150,
//! with 144a) and multi-channel sheets (row 151 and its selected state).
//! Each row's controls are driven through `Signex::update` and checked by
//! what they change.

#[path = "c_design/appearance.rs"]
mod appearance;
#[path = "c_design/classes.rs"]
mod classes;
#[path = "c_design/distributors.rs"]
mod distributors;
#[path = "c_design/editing.rs"]
mod editing;
#[path = "c_design/keyboard.rs"]
mod keyboard;
#[path = "c_design/language.rs"]
mod language;
#[path = "c_design/multi_channel.rs"]
mod multi_channel;
#[path = "c_design/rule_profiles.rs"]
mod rule_profiles;
#[path = "c_design/shell.rs"]
mod shell;
#[path = "c_design/support.rs"]
mod support;
