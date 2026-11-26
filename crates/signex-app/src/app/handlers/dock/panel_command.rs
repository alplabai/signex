//! `set_dock_panel` (#2470): a dock panel opened, closed, collapsed or
//! expanded by its layout key, for an agent or a script; a person uses the
//! panel list, the tab's close button and the region's collapse arrow. The
//! open goes through [`Signex::show_panel`], the one "open panel X" entry
//! point, and the answer says where the panel is now.

use iced::Task;
use serde_json::{Value, json};
use signex_i18n::{Translatable, ids};

use crate::app::command::{CommandArgs, CommandOutcome, OperandValue};
use crate::app::{Message, Signex};
use crate::dock::{DockMessage, PanelPosition, PanelSite};
use crate::fonts::{panel_kind_key, parse_panel_kind};
use crate::panels::{ALL_PANELS, PanelKind};

type Ran = (CommandOutcome, Task<Message>);

fn refused(reason: Translatable) -> Ran {
    (CommandOutcome::Disabled { reason }, Task::none())
}

fn text<'a>(args: &'a CommandArgs, name: &str) -> Option<&'a str> {
    match args.operand(name) {
        Some(OperandValue::Text(text)) => Some(text.trim()),
        _ => None,
    }
}

/// `set_dock_panel panel=<key> state=open|close|collapse|expand`: `key` is
/// the panel's layout key (`messages`, `properties`, `projects`, ...).
/// `collapse` and `expand` act on the dock region the panel is docked in.
pub(crate) fn set_dock_panel(app: &mut Signex, args: &CommandArgs) -> Ran {
    // The Library aliases share the `library` key: the panel itself.
    let dockable = || {
        ALL_PANELS
            .iter()
            .copied()
            .filter(|kind| kind.library_alias().is_none())
    };
    let Some(kind) = text(args, "panel")
        .and_then(parse_panel_kind)
        .filter(|kind| dockable().any(|known| known == *kind))
    else {
        let keys: Vec<&str> = dockable().map(panel_kind_key).collect();
        return refused(
            Translatable::new(ids::PANELS_DOCK_COMMAND_UNKNOWN_PANEL)
                .arg("panels", keys.join(", ")),
        );
    };
    let key = panel_kind_key(kind);
    let task = match text(args, "state") {
        Some("open") => app.show_panel(kind),
        Some("close") => {
            if !app.document_state.dock.close_panel(kind) && app.is_panel_open(kind) {
                return refused(
                    Translatable::new(ids::PANELS_DOCK_COMMAND_IN_WINDOW).arg("panel", key),
                );
            }
            app.persist_dock_layout_soon()
        }
        Some(state @ ("collapse" | "expand")) => {
            let Some(PanelSite::Docked(position, _)) = app.document_state.dock.locate(kind) else {
                return refused(
                    Translatable::new(ids::PANELS_DOCK_COMMAND_NOT_DOCKED)
                        .arg("panel", key)
                        .arg("state", state),
                );
            };
            let collapse = state == "collapse";
            if app.document_state.dock.is_collapsed(position) != collapse {
                app.document_state
                    .dock
                    .update(DockMessage::ToggleCollapse(position));
            }
            app.persist_dock_layout_soon()
        }
        _ => return refused(Translatable::new(ids::PANELS_DOCK_COMMAND_UNKNOWN_STATE)),
    };
    (CommandOutcome::Answered(panel_json(app, kind)), task)
}

/// Where `kind` is: `open`, its `place` (`left`, `right`, `bottom`,
/// `floating`, `window` or null) and whether its region is `collapsed`.
fn panel_json(app: &Signex, kind: PanelKind) -> Value {
    let site = app.document_state.dock.locate(kind);
    let place = match site {
        Some(PanelSite::Docked(position, _)) => Some(match position {
            PanelPosition::Left => "left",
            PanelPosition::Right => "right",
            PanelPosition::Bottom => "bottom",
        }),
        Some(PanelSite::Floating(_)) => Some("floating"),
        None if app.is_panel_open(kind) => Some("window"),
        None => None,
    };
    let collapsed = matches!(site, Some(PanelSite::Docked(position, _))
        if app.document_state.dock.is_collapsed(position));
    json!({
        "panel": panel_kind_key(kind),
        "open": app.is_panel_open(kind),
        "place": place,
        "collapsed": collapsed,
    })
}
