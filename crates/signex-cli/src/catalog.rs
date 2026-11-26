//! `signex-cli commands --json` (#1923): the command catalog with what each
//! command takes and whether a headless run can use it.

use serde_json::{Value, json};
use signex_app::keymap::{CatalogTextExt, CommandMetadata, all_command_ids, metadata_for_str};

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
        "label": command.label_en_us(),
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

    /// The `library` commands a headless run may run: each was reviewed to
    /// read only the open libraries and the cache, never a library server
    /// or a distributor (#2938, #1297). A new runnable `library` command
    /// fails [`no_network_command_is_runnable_headless`] until it is
    /// reviewed and listed here.
    const RUNNABLE_LIBRARY_COMMANDS: &[&str] = &["query_library", "search_parts"];

    /// No network and no launches: the `library` commands are the ones that
    /// could reach a library server, a distributor or a browser, and none is
    /// runnable unless reviewed as reading only what is open
    /// ([`RUNNABLE_LIBRARY_COMMANDS`]); a command of any category that may
    /// reach the network says so (`needs_network`) and is not runnable
    /// either.
    #[test]
    fn no_network_command_is_runnable_headless() {
        for id in all_command_ids() {
            let meta = metadata_for_str(id).unwrap();
            assert!(
                meta.category != "library"
                    || meta.flags.gui_only
                    || RUNNABLE_LIBRARY_COMMANDS.contains(&id),
                "{id} is a runnable library command: give it a capability check for the network"
            );
            assert!(
                !meta.needs_network || meta.flags.gui_only,
                "{id} may reach the network and is runnable: give it a capability check for the network"
            );
        }
        for id in RUNNABLE_LIBRARY_COMMANDS {
            let meta = metadata_for_str(id).unwrap();
            assert_eq!(meta.category, "library", "{id}");
            assert!(!meta.needs_network, "{id} reads only what is open");
        }
        for id in ["open_library_browser", "open_datasheet"] {
            assert!(
                metadata_for_str(id).unwrap().needs_network,
                "{id} reaches the network"
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
