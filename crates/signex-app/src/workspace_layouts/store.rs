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

/// Why one stored or imported layout was skipped. The handler words it with
/// the catalog (`problems::reason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyProblem {
    NotAnObject,
    NoName,
    PanelsUnreadable,
    SplitUnknown,
    NothingStored,
}

/// An entry of the stored list or of an import that was skipped, for a
/// Messages line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutProblem {
    /// The stored list is not an object; only the built-ins are available.
    ListUnreadable,
    /// An edit of a built-in this build does not know.
    UnknownBuiltinEdit {
        key: String,
    },
    BuiltinEditSkipped {
        key: String,
        why: BodyProblem,
    },
    /// A user layout stored under a built-in's id.
    BuiltinIdTaken {
        key: String,
    },
    StoredLayoutSkipped {
        key: String,
        why: BodyProblem,
    },
    /// Entry `entry` (from 1) of an imported file.
    ImportEntrySkipped {
        entry: usize,
        why: BodyProblem,
    },
}

/// Why an import file was refused as a whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRefusal {
    NotALayoutsFile,
    /// Written by a newer build, in export format `format`.
    Newer {
        format: u64,
    },
    NoLayoutsList,
    /// The file could not be read; the system's reason.
    ReadFailed(String),
    /// The file is not JSON; the parser's reason.
    NotJson(String),
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
fn body_from_json(id: LayoutId, body: &Value) -> Result<WorkspaceLayout, BodyProblem> {
    let object = body.as_object().ok_or(BodyProblem::NotAnObject)?;
    let name = match id {
        LayoutId::Builtin(_) => String::new(),
        LayoutId::User(_) => object
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .ok_or(BodyProblem::NoName)?
            .to_string(),
    };
    let dock = match object.get("dock") {
        None | Some(Value::Null) => None,
        Some(dock) => {
            Some(crate::fonts::snapshot_from_json(dock).ok_or(BodyProblem::PanelsUnreadable)?)
        }
    };
    let split = match object.get("split") {
        None | Some(Value::Null) => None,
        Some(split) => Some(
            split
                .as_str()
                .and_then(DocumentSplit::from_key)
                .ok_or(BodyProblem::SplitUnknown)?,
        ),
    };
    if dock.is_none() && split.is_none() {
        return Err(BodyProblem::NothingStored);
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

/// The list in `prefs` (its whole JSON), with a problem per entry that was
/// skipped. No key reads as the built-ins alone.
pub fn from_prefs(prefs: Option<&Value>) -> (Layouts, Vec<LayoutProblem>) {
    match prefs.and_then(|p| p.get(PREFS_KEY)) {
        None => (Layouts::default(), Vec::new()),
        Some(value) => from_json(value),
    }
}

/// The stored list. Tolerant: a malformed entry is skipped and named in the
/// returned problems, an unknown panel key drops that panel only, and a
/// user layout missing from `order` is kept at the end.
pub fn from_json(value: &Value) -> (Layouts, Vec<LayoutProblem>) {
    let mut problems = Vec::new();
    let Some(object) = value.as_object() else {
        problems.push(LayoutProblem::ListUnreadable);
        return (Layouts::default(), problems);
    };
    let mut overrides = BTreeMap::new();
    if let Some(stored) = object.get("overrides").and_then(Value::as_object) {
        for (key, body) in stored {
            let Some(b) = BuiltinLayout::from_key(key) else {
                problems.push(LayoutProblem::UnknownBuiltinEdit { key: key.clone() });
                continue;
            };
            match body_from_json(LayoutId::Builtin(b), body) {
                Ok(layout) => {
                    overrides.insert(b, layout);
                }
                Err(why) => problems.push(LayoutProblem::BuiltinEditSkipped {
                    key: key.clone(),
                    why,
                }),
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
            problems.push(LayoutProblem::BuiltinIdTaken { key });
            continue;
        }
        match body_from_json(LayoutId::User(key.clone()), body) {
            Ok(layout) => user.push(layout),
            Err(why) => problems.push(LayoutProblem::StoredLayoutSkipped { key, why }),
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

/// What an import read: the layouts, and a problem per skipped entry.
pub type ImportRead = (Vec<WorkspaceLayout>, Vec<LayoutProblem>);

/// The layouts of an export file, with a problem per entry that was
/// skipped. `Err` when the file is not a Signex layouts export at all.
pub fn import_json(value: &Value) -> Result<ImportRead, ImportRefusal> {
    let format = value
        .get(EXPORT_FORMAT_KEY)
        .and_then(Value::as_u64)
        .ok_or(ImportRefusal::NotALayoutsFile)?;
    if format > EXPORT_FORMAT {
        return Err(ImportRefusal::Newer { format });
    }
    let list = value
        .get("layouts")
        .and_then(Value::as_array)
        .ok_or(ImportRefusal::NoLayoutsList)?;
    let mut layouts = Vec::new();
    let mut problems = Vec::new();
    for (index, body) in list.iter().enumerate() {
        // The id is replaced on import; this one only names the entry.
        match body_from_json(LayoutId::User(format!("import-{index}")), body) {
            Ok(layout) => layouts.push(layout),
            Err(why) => problems.push(LayoutProblem::ImportEntrySkipped {
                entry: index + 1,
                why,
            }),
        }
    }
    Ok((layouts, problems))
}
