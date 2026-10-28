//! The `muda` shell: the only place that touches AppKit menus.
//!
//! Every call here runs on the main thread, inside `update`: `muda::Menu`
//! panics anywhere else, and an `iced::Task` future is not on that thread.

use std::collections::HashMap;

use muda::accelerator::{Accelerator as KeyEquivalent, CMD_OR_CTRL, Code};
use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

use super::state::{MenuInputs, NativeMenuState, Refresh, plan_refresh};
use super::{Accelerator, NativeBuild, NativeMenu, NativeMenuSpec, NativeNode, Predefined};
use crate::diagnostics::{log_info, log_warning};
use crate::ignore::IgnoreResult;

/// The installed menu and the item handles enablement updates go through.
pub(super) struct Shell {
    /// Owns the NSMenu; dropping it releases the items.
    _menu: Menu,
    items: HashMap<String, MenuItem>,
}

impl Shell {
    fn install(spec: &NativeMenuSpec) -> Result<Self, muda::Error> {
        let menu = Menu::new();
        let mut items = HashMap::new();
        for root in &spec.menus {
            menu.append(&submenu(root, &mut items)?)?;
        }
        // Replaces winit's default menu, which exists by now.
        menu.init_for_nsapp();
        Ok(Self { _menu: menu, items })
    }
}

fn submenu(
    menu: &NativeMenu,
    items: &mut HashMap<String, MenuItem>,
) -> Result<Submenu, muda::Error> {
    let sub = Submenu::new(&menu.title, true);
    for node in &menu.nodes {
        match node {
            NativeNode::Item(item) => {
                let native = MenuItem::with_id(
                    MenuId::new(&item.id),
                    &item.label,
                    item.enabled,
                    item.accelerator.map(key_equivalent),
                );
                sub.append(&native)?;
                items.insert(item.id.clone(), native);
            }
            NativeNode::Predefined(kind) => sub.append(&predefined(*kind))?,
            NativeNode::Submenu(inner) => sub.append(&submenu(inner, items)?)?,
            NativeNode::Separator => sub.append(&PredefinedMenuItem::separator())?,
        }
    }
    Ok(sub)
}

fn key_equivalent(accelerator: Accelerator) -> KeyEquivalent {
    match accelerator {
        Accelerator::CommandComma => KeyEquivalent::new(CMD_OR_CTRL, Code::Comma),
        Accelerator::CommandQ => KeyEquivalent::new(CMD_OR_CTRL, Code::KeyQ),
    }
}

fn predefined(kind: Predefined) -> PredefinedMenuItem {
    match kind {
        Predefined::Hide => PredefinedMenuItem::hide(Some("Hide Signex")),
        Predefined::HideOthers => PredefinedMenuItem::hide_others(Some("Hide Others")),
        Predefined::ShowAll => PredefinedMenuItem::show_all(Some("Show All")),
    }
}

impl NativeMenuState {
    /// Bring the system menu bar in line with `build`. Installs the first
    /// time (once the main window is open), afterwards touches AppKit only
    /// when the spec changed, and flips enabled flags in place when that is
    /// all that changed.
    pub(crate) fn sync(&mut self, inputs: MenuInputs, build: NativeBuild) {
        if !self.armed || self.failed {
            return;
        }
        self.applied_inputs = Some(inputs);
        let refresh = plan_refresh(self.applied.as_ref(), &build.spec);
        let NativeBuild { spec, actions } = build;
        match (refresh, self.shell.as_ref()) {
            (Refresh::Unchanged, _) => {}
            (Refresh::Enablement(changes), Some(shell)) => {
                for (id, enabled) in changes {
                    if let Some(item) = shell.items.get(&id) {
                        item.set_enabled(enabled);
                    }
                }
            }
            (Refresh::Enablement(_) | Refresh::Rebuild, _) => {
                if !self.rebuild(&spec) {
                    return;
                }
            }
        }
        self.actions = actions;
        self.applied = Some(spec);
    }

    fn rebuild(&mut self, spec: &NativeMenuSpec) -> bool {
        match Shell::install(spec) {
            Ok(shell) => {
                if !self.installed {
                    log_info("Main menu: shown in the system menu bar");
                }
                self.shell = Some(shell);
                self.installed = true;
                true
            }
            Err(error) if self.shell.is_some() => {
                // The previous menu stays on screen but is now out of date.
                log_warning(format!(
                    "Main menu: could not rebuild the system menu bar ({error}); \
                     it may be out of date"
                ));
                self.failed = true;
                false
            }
            Err(error) => {
                log_warning(format!(
                    "Main menu: could not install the system menu bar ({error}); \
                     using the in-window menu"
                ));
                self.failed = true;
                false
            }
        }
    }
}

/// Stream of activated native item ids, from muda's global event channel.
/// The handler runs on the main thread and only forwards; `update` decides.
pub(crate) fn activations() -> impl iced::futures::Stream<Item = String> {
    iced::stream::channel(16, async |mut output| {
        use iced::futures::{SinkExt, StreamExt, channel::mpsc};
        let (tx, mut rx) = mpsc::unbounded::<String>();
        muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
            // The receiver only drops with the subscription.
            tx.unbounded_send(event.id().0.clone())
                .ignore("the receiver only drops when the subscription ends");
        }));
        while let Some(id) = rx.next().await {
            if output.send(id).await.is_err() {
                break;
            }
        }
        muda::MenuEvent::set_event_handler::<fn(muda::MenuEvent)>(None);
    })
}
