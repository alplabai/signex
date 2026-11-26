//! The panes kept per project (mockup row 207): the `document_panes` key of
//! the user's `prefs.json`, one entry per project file, so a project's split
//! comes back when it is reopened. A document is stored relative to its
//! project folder. An entry this build cannot read is kept as it is, and a
//! key it does not know survives a rewrite of its entry. Format:
//! docs/MENU_BAR.md, "Kept per project".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::PaneTab;
use super::model::{Pane, PaneSplit};
use crate::app::TileLayout;

/// The `prefs.json` key.
pub const PREFS_KEY: &str = "document_panes";

/// Where the kept panes are written. The production store is
/// [`PrefsDocumentPanesStore`]; tests substitute a recorder. Blocking: the
/// app calls it from a `Task`, never inside `update`.
pub trait DocumentPanesStore: Send + Sync {
    fn write(&self, value: &Value) -> Result<(), String>;
}

/// Writes the kept panes into `prefs.json`, keeping every other setting.
pub struct PrefsDocumentPanesStore {
    path: PathBuf,
}

impl PrefsDocumentPanesStore {
    /// The store for the user's real `prefs.json`.
    pub fn user_prefs() -> Self {
        Self {
            path: crate::fonts::prefs_file_path(),
        }
    }
}

impl DocumentPanesStore for PrefsDocumentPanesStore {
    fn write(&self, value: &Value) -> Result<(), String> {
        crate::fonts::write_prefs_key(&self.path, PREFS_KEY, value.clone())
    }
}

/// The stored entries by project file, as read: each stays raw JSON until
/// its project asks for it.
pub fn read_kept(prefs: Option<&Value>) -> BTreeMap<String, Value> {
    prefs
        .and_then(|prefs| prefs.get(PREFS_KEY))
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .map(|(project, entry)| (project.clone(), entry.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// The whole key's value, written in one piece.
pub fn kept_to_json(kept: &BTreeMap<String, Value>) -> Value {
    Value::Object(kept.clone().into_iter().collect())
}

fn direction_key(direction: TileLayout) -> &'static str {
    match direction {
        TileLayout::SideBySide => "side_by_side",
        TileLayout::Stacked => "stacked",
    }
}

fn direction_from_key(key: &str) -> Option<TileLayout> {
    match key {
        "side_by_side" => Some(TileLayout::SideBySide),
        "stacked" => Some(TileLayout::Stacked),
        _ => None,
    }
}

fn tab_to_json(tab: &PaneTab, folder: &Path) -> Value {
    let path = tab.path.strip_prefix(folder).unwrap_or(tab.path.as_path());
    json!({ "path": path.to_string_lossy(), "page": tab.page })
}

fn tab_from_json(json: &Value, folder: &Path) -> Option<PaneTab> {
    let path = json.get("path")?.as_str()?;
    let page = json.get("page")?.as_str()?;
    if path.is_empty() || page.is_empty() {
        return None;
    }
    Some(PaneTab {
        path: folder.join(path),
        page: page.to_string(),
    })
}

/// `split` as stored for the project in `folder`. The keys of `old` (the
/// entry it replaces) this build does not write are kept.
pub fn split_to_json(split: &PaneSplit<PaneTab>, folder: &Path, old: Option<&Value>) -> Value {
    let panes: Vec<Value> = split
        .panes
        .iter()
        .map(|pane| {
            json!({
                "tabs": pane.tabs.iter().map(|tab| tab_to_json(tab, folder)).collect::<Vec<_>>(),
                "shown": pane.shown.as_ref().map(|tab| tab_to_json(tab, folder)),
            })
        })
        .collect();
    let mut entry: Map<String, Value> = old.and_then(Value::as_object).cloned().unwrap_or_default();
    entry.insert("direction".into(), json!(direction_key(split.direction)));
    entry.insert("panes".into(), Value::Array(panes));
    entry.insert("rest".into(), json!(split.rest));
    entry.insert("focused".into(), json!(split.focused));
    Value::Object(entry)
}

/// The split stored for the project in `folder`, or `None` when the entry
/// is not one this build can read: an unknown direction, fewer than two
/// panes, a pane or tab without its fields, an index past the panes.
pub fn split_from_json(json: &Value, folder: &Path) -> Option<PaneSplit<PaneTab>> {
    let direction = direction_from_key(json.get("direction")?.as_str()?)?;
    let panes = json
        .get("panes")?
        .as_array()?
        .iter()
        .map(|pane| {
            let tabs = pane
                .get("tabs")?
                .as_array()?
                .iter()
                .map(|tab| tab_from_json(tab, folder))
                .collect::<Option<Vec<_>>>()?;
            let shown = match pane.get("shown") {
                None | Some(Value::Null) => None,
                Some(tab) => Some(tab_from_json(tab, folder)?),
            };
            Some(Pane { tabs, shown })
        })
        .collect::<Option<Vec<_>>>()?;
    let index = |key: &str| {
        json.get(key)?
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n < panes.len())
    };
    let (rest, focused) = (index("rest")?, index("focused")?);
    (panes.len() >= 2).then_some(PaneSplit {
        direction,
        panes,
        rest,
        focused,
    })
}
