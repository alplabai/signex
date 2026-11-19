//! Where the layouts live: the `workspace_layouts` key of the user's
//! `prefs.json` (per user, never in a project), and the export file. The
//! dock regions use the dock's own codec (`fonts::region_to_json`), so a
//! layout and the saved dock spell panels the same way. Format:
//! docs/WORKSPACE_LAYOUTS.md, "Storage".

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::model::{BuiltinLayout, DocumentSplit, LayoutId, Layouts, WorkspaceLayout};

/// The `prefs.json` key.
pub const PREFS_KEY: &str = "workspace_layouts";

/// The top-level key of an export file, holding its format version.
pub const EXPORT_FORMAT_KEY: &str = "signex_workspace_layouts";

/// The export format this build writes and reads.
pub const EXPORT_FORMAT: u64 = 1;

/// Where the list is written. The production store is
/// [`PrefsWorkspaceLayoutsStore`]; tests substitute a recorder. Blocking:
/// the app calls it from a `Task`, never inside `update`.
pub trait WorkspaceLayoutsStore: Send + Sync {
    fn write(&self, value: &Value) -> Result<(), String>;
}

/// Writes the list into `prefs.json`, keeping every other setting.
pub struct PrefsWorkspaceLayoutsStore {
    path: std::path::PathBuf,
}

impl PrefsWorkspaceLayoutsStore {
    /// The store for the user's real `prefs.json`.
    pub fn user_prefs() -> Self {
        Self::at(crate::fonts::prefs_file_path())
    }

    /// The store for an explicit prefs file (tests).
    pub fn at(path: std::path::PathBuf) -> Self {
        Self { path }
    }
}

impl WorkspaceLayoutsStore for PrefsWorkspaceLayoutsStore {
    fn write(&self, value: &Value) -> Result<(), String> {
        crate::fonts::write_prefs_key(&self.path, PREFS_KEY, value.clone())
    }
}

/// The body of one layout: everything but its id (and, for a built-in, its
/// name).
fn body_to_json(layout: &WorkspaceLayout) -> Value {
    let mut body = Map::new();
    if layout.id.builtin().is_none() {
        body.insert("name".into(), json!(layout.name));
    }
    if let Some(dock) = &layout.dock {
        body.insert("dock".into(), crate::fonts::snapshot_to_json(dock));
    }
    if let Some(split) = layout.split {
        body.insert("split".into(), json!(split.key()));
    }
    if let Some(saved_at) = &layout.saved_at {
        body.insert("saved_at".into(), json!(saved_at));
    }
    body.insert("from_current".into(), json!(layout.from_current));
    Value::Object(body)
}

/// A stored body as a layout with `id`; `Err` says why it was skipped.
fn body_from_json(id: LayoutId, body: &Value) -> Result<WorkspaceLayout, String> {
    let object = body
        .as_object()
        .ok_or_else(|| String::from("it is not an object"))?;
    let name = match id {
        LayoutId::Builtin(_) => String::new(),
        LayoutId::User(_) => object
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .ok_or_else(|| String::from("it has no name"))?
            .to_string(),
    };
    let dock = match object.get("dock") {
        None | Some(Value::Null) => None,
        Some(dock) => Some(
            crate::fonts::snapshot_from_json(dock)
                .ok_or_else(|| String::from("its panels are not readable"))?,
        ),
    };
    let split = match object.get("split") {
        None | Some(Value::Null) => None,
        Some(split) => Some(
            split
                .as_str()
                .and_then(DocumentSplit::from_key)
                .ok_or_else(|| String::from("its document split is unknown"))?,
        ),
    };
    if dock.is_none() && split.is_none() {
        return Err(String::from("it stores neither panels nor a split"));
    }
    Ok(WorkspaceLayout {
        id,
        name,
        dock,
        split,
        saved_at: object
            .get("saved_at")
            .and_then(Value::as_str)
            .map(str::to_string),
        from_current: object
            .get("from_current")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// The list as `prefs.json` stores it.
pub fn to_json(layouts: &Layouts) -> Value {
    let mut user = Map::new();
    let mut order = Vec::new();
    for layout in layouts.user() {
        order.push(json!(layout.id.key()));
        user.insert(layout.id.key().to_string(), body_to_json(layout));
    }
    let overrides: Map<String, Value> = layouts
        .overrides()
        .iter()
        .map(|(b, layout)| (b.key().to_string(), body_to_json(layout)))
        .collect();
    json!({
        "active": layouts.active().map(|id| id.key().to_string()),
        "order": order,
        "user": user,
        "overrides": overrides,
    })
}

/// The list in `prefs` (its whole JSON), with a line per entry that was
/// skipped. No key reads as the built-ins alone.
pub fn from_prefs(prefs: Option<&Value>) -> (Layouts, Vec<String>) {
    match prefs.and_then(|p| p.get(PREFS_KEY)) {
        None => (Layouts::default(), Vec::new()),
        Some(value) => from_json(value),
    }
}

/// The stored list. Tolerant: a malformed entry is skipped and named in the
/// returned lines, an unknown panel key drops that panel only, and a user
/// layout missing from `order` is kept at the end.
pub fn from_json(value: &Value) -> (Layouts, Vec<String>) {
    let mut problems = Vec::new();
    let Some(object) = value.as_object() else {
        problems.push(String::from(
            "Workspace layouts: the stored list is not readable; only the built-in layouts are available",
        ));
        return (Layouts::default(), problems);
    };
    let mut overrides = BTreeMap::new();
    if let Some(stored) = object.get("overrides").and_then(Value::as_object) {
        for (key, body) in stored {
            let Some(b) = BuiltinLayout::from_key(key) else {
                problems.push(format!(
                    "Workspace layouts: skipped the edit of an unknown built-in layout \"{key}\""
                ));
                continue;
            };
            match body_from_json(LayoutId::Builtin(b), body) {
                Ok(layout) => {
                    overrides.insert(b, layout);
                }
                Err(why) => problems.push(format!(
                    "Workspace layouts: skipped your edit of the built-in layout \"{key}\": {why}"
                )),
            }
        }
    }
    let stored_user = object.get("user").and_then(Value::as_object);
    let mut keys: Vec<String> = object
        .get("order")
        .and_then(Value::as_array)
        .map(|order| {
            order
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if let Some(stored) = stored_user {
        for key in stored.keys() {
            if !keys.contains(key) {
                keys.push(key.clone());
            }
        }
    }
    let mut user = Vec::new();
    for key in keys {
        let Some(body) = stored_user.and_then(|s| s.get(&key)) else {
            continue;
        };
        if BuiltinLayout::from_key(&key).is_some() {
            problems.push(format!(
                "Workspace layouts: skipped a layout stored under the built-in id \"{key}\""
            ));
            continue;
        }
        match body_from_json(LayoutId::User(key.clone()), body) {
            Ok(layout) => user.push(layout),
            Err(why) => problems.push(format!(
                "Workspace layouts: skipped the stored layout \"{key}\": {why}"
            )),
        }
    }
    let active = object
        .get("active")
        .and_then(Value::as_str)
        .map(LayoutId::from_key);
    (Layouts::from_parts(overrides, user, active), problems)
}

/// An export file: the given layouts with their names, in order.
pub fn export_json(layouts: &[WorkspaceLayout]) -> Value {
    let list: Vec<Value> = layouts.iter().map(body_to_json).collect();
    json!({ EXPORT_FORMAT_KEY: EXPORT_FORMAT, "layouts": list })
}

/// The layouts of an export file, with a line per entry that was skipped.
/// `Err` when the file is not a Signex layouts export at all.
pub fn import_json(value: &Value) -> Result<(Vec<WorkspaceLayout>, Vec<String>), String> {
    let format = value
        .get(EXPORT_FORMAT_KEY)
        .and_then(Value::as_u64)
        .ok_or_else(|| String::from("it is not a Signex workspace layouts file"))?;
    if format > EXPORT_FORMAT {
        return Err(format!(
            "it was written by a newer Signex (format {format}, this one reads {EXPORT_FORMAT})"
        ));
    }
    let list = value
        .get("layouts")
        .and_then(Value::as_array)
        .ok_or_else(|| String::from("it has no layouts list"))?;
    let mut layouts = Vec::new();
    let mut problems = Vec::new();
    for (index, body) in list.iter().enumerate() {
        // The id is replaced on import; this one only names the entry.
        match body_from_json(LayoutId::User(format!("import-{index}")), body) {
            Ok(layout) => layouts.push(layout),
            Err(why) => problems.push(format!(
                "Workspace layouts: skipped entry {} of the imported file: {why}",
                index + 1
            )),
        }
    }
    Ok((layouts, problems))
}
