//! [`CommandArgs`] - what a caller knows about an invocation beyond the
//! command id (#278, #1727).
//!
//! A command id behaves the same on every surface: the context menu, the
//! palette, a keystroke, a menu bar row and, later, a CLI or an agent run
//! the same id with the same effect. The arguments have two parts, and they
//! must stay apart so a CLI operand can never change what a GUI surface does:
//!
//! - [`InvocationContext`]: what a GUI surface knows about where it was
//!   invoked (the right-click point, the project a tree row stands on). Only
//!   GUI surfaces fill it.
//! - [`Operands`]: typed values a command acts on (a point to place at, a
//!   text, a library id). Only a CLI or an agent fills them; no GUI caller
//!   sets one.
//!
//! Both are plain `Clone + Debug` data a CLI could build from text: no UI
//! handles, no window ids.

/// Where a GUI surface invoked the command from. [`Self::default`] is what a
/// keystroke or the command palette sends.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct InvocationContext {
    /// Document point, in mm, of the right-click that opened the menu.
    /// `find_similar_objects` seeds from the object there.
    pub origin: Option<(f64, f64)>,
    /// Id of the project the invocation stands on, as `ProjectId` prints it.
    /// `open_project_options` opens that project's tab; without one it acts
    /// on the active document's project.
    pub project: Option<String>,
}

/// Values a command acts on, parsed and checked against the catalog's
/// operand descriptor ([`CommandArgs::parse_operands`]); only the `command`
/// module can read or set them. GUI modules cannot construct a
/// non-empty one, so a context-menu row that carries the click point in
/// [`InvocationContext::origin`] can never trigger an operand form of the
/// same command.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Operands {
    pub(super) values: std::collections::BTreeMap<String, super::operands::OperandValue>,
    /// Each operand as the text it was given, which the agent journal keeps
    /// so a replay sends exactly what was sent (#2253).
    pub(super) texts: std::collections::BTreeMap<String, String>,
}

#[cfg(test)]
impl Operands {
    /// No operand was given: what every GUI surface dispatches with.
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The context and operands of one command invocation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CommandArgs {
    pub(crate) context: InvocationContext,
    /// Private to the `command` module: a GUI surface cannot set it, nor
    /// write a `CommandArgs { .. }` literal.
    pub(super) operands: Operands,
}

impl CommandArgs {
    /// No context, no operands: a keystroke, the command palette or a
    /// headless run of a command that takes no operand.
    pub fn none() -> Self {
        Self::default()
    }

    /// A GUI surface's invocation: its context, and never an operand.
    pub(crate) fn in_context(context: InvocationContext) -> Self {
        Self {
            context,
            operands: Operands::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every constructor a GUI surface uses (keymap, palette, menu bar and
    /// context menu all go through these two) leaves the operands empty.
    #[test]
    fn gui_constructors_never_set_an_operand() {
        let context = InvocationContext {
            origin: Some((1.0, 2.0)),
            project: Some("p1".to_string()),
        };
        assert!(CommandArgs::none().operands.is_empty());
        assert!(CommandArgs::in_context(context).operands.is_empty());
    }

    /// Tripwire behind the type-level guard above (`operands` is private to
    /// `command`): no GUI dispatch site names the operands: the keymap (stroke, chord and
    /// the UI keymap message), the palette and the context menu build their
    /// args only with `CommandArgs::none()` or `CommandArgs::in_context(..)`,
    /// so an operand a CLI adds later cannot fire from a click or a key.
    #[test]
    fn gui_dispatch_sites_never_name_the_operands() {
        let sites = [
            ("keymap stroke", include_str!("../dispatch/keymap.rs")),
            ("chord", include_str!("../dispatch/chord.rs")),
            ("ui keymap message", include_str!("../dispatch/ui.rs")),
            (
                "command palette",
                include_str!("../dispatch/command_palette.rs"),
            ),
            ("context menu", include_str!("../dispatch/overlay.rs")),
        ];
        for (name, source) in sites {
            for forbidden in ["operands", "Operands", "CommandArgs {"] {
                assert!(
                    !source.contains(forbidden),
                    "{name} dispatch names `{forbidden}`: a GUI surface must not set an operand"
                );
            }
        }
    }
}
