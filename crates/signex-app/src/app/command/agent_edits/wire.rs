//! The edit list of `apply_edits` as text (#2178), and which commands may be
//! in it.

use serde_json::{Map, Value, json};

use crate::app::command::operand_handlers::operand_handler;
use crate::keymap::{AppCommandId, metadata_for};

/// The most edits one call may carry: the app runs them all before the
/// person's next keystroke is handled.
pub const MAX_EDITS: usize = 256;

/// One edit of the list: a command id and its operands as the text
/// `CommandArgs::parse_operands` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditSpec {
    pub id: String,
    pub operands: Vec<(String, String)>,
}

/// The text of an operand given as JSON: a string, a number, a boolean,
/// `[x, y]` for a point, or a list of objects (the `edits` of `apply_edits`)
/// as compact JSON.
pub fn operand_text(name: &str, value: &Value) -> Result<String, String> {
    if name.contains('=') {
        return Err(format!("operand name `{name}` must not contain `=`"));
    }
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Bool(flag) => Ok(flag.to_string()),
        Value::Number(number) => Ok(number.to_string()),
        Value::Array(items) if name == "edits" && items.iter().all(Value::is_object) => {
            Ok(value.to_string())
        }
        Value::Array(items) if name == "documents" && items.iter().all(Value::is_string) => {
            Ok(value.to_string())
        }
        Value::Array(point) => match point.as_slice() {
            [Value::Number(x), Value::Number(y)] => Ok(format!("{x},{y}")),
            _ => Err(format!("operand `{name}` must be [x, y] when it is a list")),
        },
        Value::Object(map) if name == "expect_revisions" && map.values().all(Value::is_u64) => {
            Ok(value.to_string())
        }
        Value::Null | Value::Object(_) => Err(format!(
            "operand `{name}` must be a string, number, boolean or [x, y]"
        )),
    }
}

/// `{"<document>": revision, ..}`, the revision each other sheet of a
/// cross-sheet `apply_edits` call must be at, in document order of the text.
pub fn parse_expect_revisions(text: &str) -> Result<Vec<(String, u64)>, String> {
    const SHAPE: &str = "`expect_revisions` must be a JSON object of document to revision number";
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) else {
        return Err(SHAPE.to_string());
    };
    map.iter()
        .map(|(document, revision)| Ok((document.clone(), revision.as_u64().ok_or(SHAPE)?)))
        .collect()
}

/// `["<document>", ..]`, the sheets of a cross-sheet group a `revert` names.
pub fn parse_documents(text: &str) -> Result<Vec<String>, String> {
    const SHAPE: &str = "`documents` must be a JSON list of document ids or paths";
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text) else {
        return Err(SHAPE.to_string());
    };
    items
        .iter()
        .map(|item| item.as_str().map(str::to_string).ok_or(SHAPE.to_string()))
        .collect()
}

/// `[{"id": "..", "args": {..}}, ..]` as edits. Nothing else is read: an
/// unknown key, a missing id or a list over [`MAX_EDITS`] is refused with the
/// position of the edit.
pub fn parse_edit_list(text: &str) -> Result<Vec<EditSpec>, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|error| format!("`edits` is not JSON: {error}"))?;
    let Value::Array(items) = value else {
        return Err("`edits` must be a JSON list of {\"id\", \"args\"} objects".to_string());
    };
    if items.len() > MAX_EDITS {
        return Err(format!(
            "`edits` has {} entries; one call takes at most {MAX_EDITS}",
            items.len()
        ));
    }
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            parse_edit(item).map_err(|reason| format!("edit {}: {reason}", index + 1))
        })
        .collect()
}

fn parse_edit(item: &Value) -> Result<EditSpec, String> {
    let Value::Object(object) = item else {
        return Err("must be an object {\"id\", \"args\"}".to_string());
    };
    if let Some(key) = object
        .keys()
        .find(|key| !matches!(key.as_str(), "id" | "args"))
    {
        return Err(format!("has an unknown key `{key}`"));
    }
    let Some(Value::String(id)) = object.get("id") else {
        return Err("needs a string `id`".to_string());
    };
    let args = match object.get("args") {
        None => &Map::new(),
        Some(Value::Object(args)) => args,
        Some(_) => return Err("`args` must be an object".to_string()),
    };
    let operands = args
        .iter()
        .map(|(name, value)| Ok((name.clone(), operand_text(name, value)?)))
        .collect::<Result<_, String>>()?;
    Ok(EditSpec {
        id: id.clone(),
        operands,
    })
}

/// The list as the text [`parse_edit_list`] reads back, every operand in its
/// text form.
pub fn edit_list_text(edits: &[EditSpec]) -> String {
    let items: Vec<Value> = edits
        .iter()
        .map(|edit| {
            let args: Map<String, Value> = edit
                .operands
                .iter()
                .map(|(name, text)| (name.clone(), json!(text)))
                .collect();
            json!({"id": edit.id, "args": args})
        })
        .collect();
    Value::Array(items).to_string()
}

/// Commands that edit a sheet and still may not be in the list, and why.
/// Everything else is derived from the catalog (`edit_refusal`).
const PROJECT_WIDE: &[(&str, &str)] = &[
    (
        "assign_net_class",
        "it writes the project's net classes, which no sheet's undo step covers",
    ),
    (
        "update_pcb_from_schematic",
        "it edits the project's board, which no sheet's undo step covers; run it as its own command",
    ),
];

/// Why `command` may not be one of the edits, or `None` when it may: it must
/// be an undoable edit of a document that runs headless through an operand
/// handler, and touch nothing outside that one sheet's engine.
pub fn edit_refusal(command: &AppCommandId) -> Option<String> {
    let id = command.as_str();
    let Some(meta) = metadata_for(command) else {
        return Some(format!("`{id}` is not a command"));
    };
    if id == "apply_edits" {
        return Some("`apply_edits` cannot be one of its own edits".to_string());
    }
    if meta.flags.gui_only {
        return Some(format!("`{id}` is not runnable headless"));
    }
    if !meta.flags.mutates_doc {
        return Some(format!(
            "`{id}` does not edit a document in memory (queries, save, open and close run as their own commands)"
        ));
    }
    if !meta.flags.undoable {
        return Some(format!(
            "`{id}` steps the history; an agent takes back its own step with `revert`"
        ));
    }
    if operand_handler(command).is_none() {
        return Some(format!("`{id}` has no operand form yet"));
    }
    PROJECT_WIDE
        .iter()
        .find(|(held, _)| *held == id)
        .map(|(_, why)| format!("`{id}` is refused in edits: {why}"))
}

/// The ids an edit list may carry, in catalog order.
#[cfg(test)]
pub(crate) fn editable_ids() -> Vec<&'static str> {
    crate::keymap::all_command_ids()
        .filter(|id| AppCommandId::new(*id).is_ok_and(|id| edit_refusal(&id).is_none()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> AppCommandId {
        AppCommandId::new(name).unwrap()
    }

    /// The revisions of a cross-sheet call are an object of whole numbers, and
    /// only the `expect_revisions` operand takes one.
    #[test]
    fn expect_revisions_is_an_object_of_whole_numbers_and_only_there() {
        let map = json!({"b.snxsch": 12});
        assert_eq!(
            operand_text("expect_revisions", &map).unwrap(),
            r#"{"b.snxsch":12}"#
        );
        assert!(operand_text("expect_revisions", &json!({"b": "x"})).is_err());
        assert!(operand_text("summary", &map).is_err());
        assert_eq!(
            parse_expect_revisions(r#"{"b.snxsch":12}"#).unwrap(),
            [("b.snxsch".to_string(), 12)]
        );
        assert!(parse_expect_revisions("[1]").is_err());
        assert!(parse_expect_revisions(r#"{"b":-1}"#).is_err());
    }

    /// The sheets a `revert` names are a list of strings, and only the
    /// `documents` operand takes one.
    #[test]
    fn documents_is_a_list_of_strings_and_only_there() {
        let list = json!(["a.snxsch", "b.snxsch"]);
        assert_eq!(
            operand_text("documents", &list).unwrap(),
            r#"["a.snxsch","b.snxsch"]"#
        );
        assert!(operand_text("documents", &json!([1, 2, 3])).is_err());
        assert!(operand_text("summary", &list).is_err());
        assert_eq!(
            parse_documents(r#"["a","b"]"#).unwrap(),
            ["a".to_string(), "b".to_string()]
        );
        assert!(parse_documents(r#"["a",1]"#).is_err());
        assert!(parse_documents("{}").is_err());
    }

    #[test]
    fn a_list_round_trips_with_every_operand_as_text() {
        let list = r##"[{"id":"place_junction","args":{"at":[5.08,0]}},{"id":"color_net","args":{"color":"#336699","at":"5,0"}},{"id":"clear_all_net_colors"}]"##;
        let edits = parse_edit_list(list).unwrap();
        assert_eq!(edits[0].operands, [("at".into(), "5.08,0".into())]);
        assert_eq!(edits[2].operands, []);
        assert_eq!(parse_edit_list(&edit_list_text(&edits)).unwrap(), edits);
    }

    /// The list-of-objects form belongs to `edits` alone: any other operand
    /// keeps the old point rule, `[]` included.
    #[test]
    fn only_the_edits_operand_takes_a_list_of_objects() {
        let list = json!([{"id": "place_junction"}]);
        assert!(operand_text("edits", &list).is_ok());
        assert_eq!(operand_text("edits", &json!([])).unwrap(), "[]");
        for value in [json!([]), list] {
            let error = operand_text("at", &value).unwrap_err();
            assert_eq!(error, "operand `at` must be [x, y] when it is a list");
        }
    }

    #[test]
    fn a_malformed_list_is_refused_with_the_position() {
        for (list, needle) in [
            ("{}", "must be a JSON list"),
            ("[1]", "edit 1: must be an object"),
            (
                r#"[{"id":"x","extra":1}]"#,
                "edit 1: has an unknown key `extra`",
            ),
            (r#"[{"args":{}}]"#, "edit 1: needs a string `id`"),
            (
                r#"[{"id":"a"},{"id":"b","args":[]}]"#,
                "edit 2: `args` must be an object",
            ),
            (
                r#"[{"id":"a","args":{"at":null}}]"#,
                "must be a string, number",
            ),
            ("nope", "is not JSON"),
        ] {
            let error = parse_edit_list(list).unwrap_err();
            assert!(error.contains(needle), "{list}: {error}");
        }
        let long = format!("[{}]", vec![r#"{"id":"a"}"#; MAX_EDITS + 1].join(","));
        assert!(parse_edit_list(&long).unwrap_err().contains("at most"));
    }

    /// The set is derived from the catalog; this pins it so a command that
    /// gains an operand handler is decided here, not by accident.
    #[test]
    fn the_editable_commands_are_the_sheet_placements_and_net_colours() {
        let expected = [
            "clear_all_net_colors",
            "color_net",
            "draw_bus",
            "draw_graphic_line",
            "draw_line",
            "draw_rectangle",
            "draw_wire",
            "place_bus",
            "place_component",
            "place_global_label",
            "place_hierarchical_label",
            "place_junction",
            "place_local_net_label",
            "place_net_label",
            "place_no_connect",
            "place_no_erc",
            "place_power_symbol",
            "place_sheet_entry",
            "place_sheet_symbol",
            "place_text",
            "place_text_string",
            "place_wire",
            "sync_sheet_pins",
        ];
        let mut found = editable_ids();
        found.sort_unstable();
        assert_eq!(found, expected);
    }

    #[test]
    fn queries_history_files_and_project_data_are_refused_with_a_reason() {
        for (command, needle) in [
            ("read_document", "does not edit a document"),
            ("undo", "steps the history"),
            ("redo", "steps the history"),
            ("revert", "steps the history"),
            ("save_document", "does not edit a document"),
            ("close_active_document", "does not edit a document"),
            ("open_document", "does not edit a document"),
            ("apply_edits", "its own edits"),
            ("assign_net_class", "net classes"),
            ("update_pcb_from_schematic", "edits the project's board"),
            ("toggle_properties_panel", "not runnable headless"),
        ] {
            let reason = edit_refusal(&id(command)).unwrap_or_else(|| panic!("{command}"));
            assert!(reason.contains(needle), "{command}: {reason}");
        }
    }
}
