//! Dock-layout preference IO. Split from `fonts.rs`.

use super::*;

/// Writes the dock layout into `prefs.json`. The production
/// [`crate::dock::DockLayoutStore`]; it runs on a background thread via
/// `Task`, never inside `update` (#692).
pub struct PrefsDockLayoutStore {
    path: std::path::PathBuf,
}

impl PrefsDockLayoutStore {
    /// The store for the user's real `prefs.json`.
    pub fn user_prefs() -> Self {
        Self::at(prefs_path())
    }

    /// The store for an explicit prefs file (tests).
    pub fn at(path: std::path::PathBuf) -> Self {
        Self { path }
    }
}

impl crate::dock::DockLayoutStore for PrefsDockLayoutStore {
    fn write(&self, layout: &crate::dock::DockLayoutSnapshot) -> Result<(), String> {
        fn region_to_json(region: &crate::dock::RegionLayout) -> serde_json::Value {
            let kinds: Vec<&str> = region.panels.iter().map(|k| panel_kind_key(*k)).collect();
            serde_json::json!({
                "panels": kinds,
                "collapsed": region.collapsed,
            })
        }

        try_update_prefs_json(&self.path, "dock", |prefs| {
            prefs.insert(
                "dock".to_string(),
                serde_json::json!({
                    "left":   region_to_json(&layout.left),
                    "right":  region_to_json(&layout.right),
                    "bottom": region_to_json(&layout.bottom),
                }),
            );
        })
    }
}

/// Rebuild a DockArea from persisted JSON. Returns None when no saved
/// layout exists so the caller can fall back to the default seed.
pub fn read_dock_layout() -> Option<crate::dock::DockArea> {
    let path = prefs_path();
    let bytes = std::fs::read(&path).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let dock_v = json.get("dock")?.as_object()?;
    let mut dock = crate::dock::DockArea::new();
    for (region_key, pos) in [
        ("left", crate::dock::PanelPosition::Left),
        ("right", crate::dock::PanelPosition::Right),
        ("bottom", crate::dock::PanelPosition::Bottom),
    ] {
        let Some(region) = dock_v.get(region_key) else {
            continue;
        };
        if let Some(arr) = region.get("panels").and_then(|v| v.as_array()) {
            for val in arr {
                if let Some(s) = val.as_str()
                    && let Some(kind) = parse_panel_kind(s)
                {
                    dock.add_panel(pos, kind);
                }
            }
        }
        // Collapsed flag: run ToggleRegion once via the dock API so
        // the mutation path stays authoritative.
        if let Some(c) = region.get("collapsed").and_then(|v| v.as_bool())
            && c
        {
            dock.update(crate::dock::DockMessage::ToggleCollapse(pos));
        }
    }
    Some(dock)
}

fn panel_kind_key(k: crate::panels::PanelKind) -> &'static str {
    use crate::panels::PanelKind::*;
    match k {
        Projects => "projects",
        Components => "components",
        Navigator => "navigator",
        Properties => "properties",
        Filter => "filter",
        Erc => "erc",
        SchFilter => "sch_filter",
        SchList => "sch_list",
        Messages => "messages",
        Signal => "signal",
        Drc => "drc",
        BomStudio => "bom_studio",
        Favorites => "favorites",
        Snippets => "snippets",
        Variants => "variants",
        OutputJobs => "output_jobs",
        Todo => "todo",
        Wiki => "wiki",
        LayerStack => "layer_stack",
        NetClasses => "net_classes",
        Library => "library",
        SchLibrary => "sch_library",
        FootprintLibrary => "footprint_library",
        History => "history",
    }
}

fn parse_panel_kind(s: &str) -> Option<crate::panels::PanelKind> {
    use crate::panels::PanelKind::*;
    Some(match s {
        "projects" => Projects,
        "components" => Components,
        "navigator" => Navigator,
        "properties" => Properties,
        "filter" => Filter,
        "erc" => Erc,
        "sch_filter" => SchFilter,
        "sch_list" => SchList,
        "messages" => Messages,
        "signal" => Signal,
        "drc" => Drc,
        "bom_studio" => BomStudio,
        "favorites" => Favorites,
        "snippets" => Snippets,
        "variants" => Variants,
        "output_jobs" => OutputJobs,
        "todo" => Todo,
        "wiki" => Wiki,
        "layer_stack" => LayerStack,
        "net_classes" => NetClasses,
        "library" => Library,
        "sch_library" => SchLibrary,
        "footprint_library" => FootprintLibrary,
        "history" => History,
        _ => return None,
    })
}
