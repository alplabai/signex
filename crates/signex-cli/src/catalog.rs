//! `signex-cli commands --json` (#1923): the command catalog with what each
//! command takes and whether a headless run can use it.

use serde_json::{Value, json};
use signex_app::keymap::{CommandMetadata, all_command_ids, metadata_for_str};

use crate::capability::capability_of;

fn entry(command: &CommandMetadata) -> Value {
    let operands: Vec<Value> = command
        .operands
        .iter()
        .map(|operand| {
            json!({
                "name": operand.name,
                "kind": operand.kind.name(),
                "required": operand.required,
                "unit": operand.kind.unit(),
            })
        })
        .collect();
    let mut entry = json!({
        "id": command.id,
        "category": command.category,
        "label": command.label,
        "capability": capability_of(command),
        "headless": if command.headless_refusal().is_some() { "gui_only" } else { "runnable" },
        "operands": operands,
    });
    if let Some(reason) = command.headless_refusal() {
        entry["reason"] = json!(reason);
    }
    entry
}

/// Every catalog command, in catalog order.
pub fn catalog_json() -> Value {
    let commands: Vec<Value> = all_command_ids()
        .filter_map(metadata_for_str)
        .map(entry)
        .collect();
    json!({ "commands": commands })
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_app::keymap::HeadlessUse;

    fn find<'a>(catalog: &'a Value, id: &str) -> &'a Value {
        catalog["commands"]
            .as_array()
            .unwrap()
            .iter()
            .find(|command| command["id"] == id)
            .unwrap_or_else(|| panic!("{id} is not listed"))
    }

    #[test]
    fn every_catalog_command_is_listed_once_with_its_capability() {
        let catalog = catalog_json();
        let listed = catalog["commands"].as_array().unwrap();
        assert_eq!(listed.len(), all_command_ids().count());
        for command in listed {
            assert!(command["capability"].is_string(), "{command}");
            assert!(command["operands"].is_array(), "{command}");
        }
    }

    #[test]
    fn a_runnable_command_lists_its_operands_and_a_gui_only_one_its_reason() {
        let catalog = catalog_json();
        let label = find(&catalog, "place_net_label");
        assert_eq!(label["headless"], "runnable");
        assert_eq!(label["capability"], "edit");
        assert_eq!(
            label["operands"][0],
            json!({"name": "at", "kind": "point", "required": true, "unit": "mm"})
        );
        assert_eq!(label["operands"][2]["required"], false);
        assert!(label.get("reason").is_none());

        let gui = find(&catalog, "toggle_properties_panel");
        assert_eq!(gui["headless"], "gui_only");
        assert_eq!(
            gui["reason"],
            "`toggle_properties_panel` only works in a window"
        );
        // A command the catalog newly classified as window only (#2170).
        assert_eq!(find(&catalog, "open_erc_dialog")["headless"], "gui_only");
        assert_eq!(find(&catalog, "run_erc")["headless"], "runnable");
        assert_eq!(find(&catalog, "save_document")["capability"], "save");
    }

    /// No network and no launches: the only commands that could reach a
    /// library server or a browser are the `library` ones, and none is runnable.
    #[test]
    fn no_library_command_is_runnable_headless() {
        for id in all_command_ids() {
            let meta = metadata_for_str(id).unwrap();
            assert!(
                meta.category != "library" || meta.flags.gui_only,
                "{id} is a runnable library command: give it a capability check for the network"
            );
        }
    }

    /// The catalog flag and the listing agree for every command.
    #[test]
    fn the_listing_follows_the_gui_only_flag() {
        let catalog = catalog_json();
        for command in catalog["commands"].as_array().unwrap() {
            let id = command["id"].as_str().unwrap();
            let meta = metadata_for_str(id).unwrap();
            let window_only = meta.headless_use() == Some(HeadlessUse::WindowOnly);
            assert_eq!(command["headless"] == "gui_only", window_only, "{id}");
            assert_eq!(
                command.get("reason").and_then(Value::as_str),
                meta.headless_refusal().as_deref(),
                "{id}"
            );
        }
    }
}
