//! Which commands the agent channel accepts (#1924), derived from the command
//! catalog: the #2172 queries, and a command that names its document with the
//! `document` operand and no file path (`apply_edits` and `revert` are two of
//! them). Everything else is refused with the reason. See
//! `docs/AGENT_SOCKET.md`, "Allowlist".

use crate::app::command::queries::is_query;
use crate::keymap::{AppCommandId, OperandKind, metadata_for};

/// Commands that name a document and are still the person's: their undo
/// stack is theirs, and an agent takes back its own step with `revert`.
/// `consistency` in the tests keeps this list to ids the catalog has.
pub(super) const PERSON_ONLY: &[&str] = &["undo", "redo"];

/// Why `command` is not on the agent channel, or `None` when it is.
pub(super) fn refusal(command: &AppCommandId) -> Option<String> {
    let id = command.as_str();
    let Some(meta) = metadata_for(command) else {
        return Some(format!("unknown command `{id}`"));
    };
    if PERSON_ONLY.contains(&id) {
        return Some(format!(
            "`{id}` is the person's own; an agent takes back its step with `revert`"
        ));
    }
    if is_query(command) {
        return None;
    }
    if let Some(reason) = meta.headless_refusal() {
        return Some(reason);
    }
    if meta.operands.iter().any(|op| op.kind == OperandKind::Path) {
        return Some(format!(
            "`{id}` takes a file path; the agent channel does not read or write a path it names"
        ));
    }
    // A required `document`: a placement's optional one does not open it, so a
    // placement stays reachable only through `apply_edits` (#2199).
    let names_document = meta
        .operands
        .iter()
        .any(|op| op.name == "document" && op.kind == OperandKind::Document && op.required);
    if !names_document {
        return Some(format!(
            "`{id}` does not name a document, so it is not on the agent channel"
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> AppCommandId {
        AppCommandId::new(name).expect("a valid id")
    }

    fn allowed() -> Vec<&'static str> {
        let mut ids: Vec<&str> = crate::keymap::all_command_ids()
            .filter(|name| refusal(&id(name)).is_none())
            .collect();
        ids.sort_unstable();
        ids
    }

    /// The accepted set, pinned: a catalog row that joins it (or leaves it)
    /// is a decision for the maintainer, so it shows up here.
    #[test]
    fn the_allowlist_is_the_queries_and_the_document_commands() {
        assert_eq!(
            allowed(),
            [
                "apply_edits",
                "assign_net_class",
                "check_erc",
                "clear_all_net_colors",
                "close_active_document",
                "color_net",
                "document_info",
                "find_items",
                "list_documents",
                "query_pins",
                "read_document",
                "resolve",
                "revert",
                "save_document",
            ]
        );
    }

    #[test]
    fn the_person_only_list_names_catalog_commands_that_would_otherwise_pass() {
        for name in PERSON_ONLY {
            let meta = metadata_for(&id(name)).expect("in the catalog");
            assert!(
                meta.operands
                    .iter()
                    .any(|op| op.kind == OperandKind::Document),
                "`{name}` names no document, so it needs no entry here"
            );
            assert!(meta.headless_refusal().is_none(), "`{name}` is window-only");
        }
    }

    /// A placement takes an optional `document` (#2199); that does not put it
    /// on the agent channel, where it stays reachable through `apply_edits`.
    #[test]
    fn the_placements_stay_off_the_channel_though_they_take_a_document() {
        for name in [
            "place_net_label",
            "place_local_net_label",
            "place_global_label",
            "place_hierarchical_label",
            "place_power_symbol",
            "place_no_connect",
            "place_junction",
            "place_no_erc",
            "place_wire",
            "draw_wire",
            "place_bus",
            "draw_bus",
            "place_text",
            "place_text_string",
            "draw_line",
            "draw_graphic_line",
            "draw_rectangle",
        ] {
            let meta = metadata_for(&id(name)).expect("in the catalog");
            assert!(
                meta.operands
                    .iter()
                    .any(|op| op.name == "document" && !op.required),
                "`{name}` takes an optional document"
            );
            let why = refusal(&id(name)).unwrap_or_else(|| panic!("`{name}` was accepted"));
            assert!(why.contains("does not name a document"), "{name}: {why}");
        }
    }

    #[test]
    fn undo_tools_views_and_window_commands_are_refused_with_a_reason() {
        for (name, wanted) in [
            ("undo", "the person's own"),
            ("redo", "the person's own"),
            ("save_document_as", "takes a file path"),
            ("open_document", "takes a file path"),
            ("new_document", "takes a file path"),
            ("open_erc_dialog", "only works in a window"),
            ("place_wire", "does not name a document"),
            ("next_grid", "only works in a window"),
            ("select_all", "does not name a document"),
            ("rotate_clockwise", "does not name a document"),
        ] {
            let why = refusal(&id(name)).unwrap_or_else(|| panic!("`{name}` was accepted"));
            assert!(why.contains(wanted), "{name}: {why}");
        }
        assert!(
            refusal(&id("not_a_command"))
                .unwrap()
                .contains("unknown command")
        );
    }
}
