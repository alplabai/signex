//! #1245 - Design > Parameter Manager... opens the Parameter Manager.

use super::model::{ItemState, MenuEntry, menu_bar_model};
use super::{MenuContext, MenuMessage};
use crate::app::command::CommandContext;

/// Design > Parameter Manager... exists, is live on a schematic, and
/// its click opens the Parameter Manager (`ParameterManagerMsg::Open`).
#[test]
fn design_menu_opens_the_parameter_manager() {
    let ctx = MenuContext {
        commands: CommandContext {
            has_schematic: true,
            ..CommandContext::default()
        },
        ..MenuContext::default()
    };
    let model = menu_bar_model(&ctx);
    let design = model
        .menus
        .iter()
        .find(|m| m.title == "Design")
        .expect("Design menu");
    let row = design
        .entries
        .iter()
        .find_map(|e| match e {
            MenuEntry::Item(i) if i.label == "Parameter Manager..." => Some(i),
            _ => None,
        })
        .expect("Design > Parameter Manager... row");
    let ItemState::Live { action, is_enabled } = &row.state else {
        panic!("the row must be live, not a stub");
    };
    assert!(is_enabled);
    assert!(matches!(action, MenuMessage::OpenParameterManager));

    let (mut app, _task) = crate::app::Signex::new();
    assert!(!app.ui_state.parameter_manager_open);
    let _ = app.update(crate::app::Message::Menu(action.clone()));
    assert!(
        app.ui_state.parameter_manager_open,
        "the row must reach ParameterManagerMsg::Open"
    );
}
