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
            let mut json = serde_json::json!({
                "panels": kinds,
                "collapsed": region.collapsed,
            });
            if let Some(width) = region.width {
                json["width"] = serde_json::json!(width);
            }
            json
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
    read_dock_layout_from(load_prefs_json().as_ref())
}

pub fn read_dock_layout_from(json: Option<&serde_json::Value>) -> Option<crate::dock::DockArea> {
    let json = json?;
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
        // A layout saved before #2395 has no width; the dock keeps the
        // fresh-layout width.
        if let Some(width) = region.get("width").and_then(|v| v.as_f64()) {
            dock.set_preferred_width(pos, width as f32);
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
        Drc => "drc",
        BomStudio => "bom_studio",
        Favorites => "favorites",
        Snippets => "snippets",
        Variants => "variants",
        OutputJobs => "output_jobs",
        Todo => "todo",
        Wiki => "wiki",
        LayerStack => "layer_stack",
        Layers => "layers",
        NetClasses => "net_classes",
        Library => "library",
        SchLibrary => "sch_library",
        FootprintLibrary => "footprint_library",
        SymbolPins => "symbol_pins",
        PcbNets => "pcb_nets",
        History => "history",
        NetInspector => "net_inspector",
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
        "drc" => Drc,
        "bom_studio" => BomStudio,
        "favorites" => Favorites,
        "snippets" => Snippets,
        "variants" => Variants,
        "output_jobs" => OutputJobs,
        "todo" => Todo,
        "wiki" => Wiki,
        "layer_stack" => LayerStack,
        "layers" => Layers,
        "net_classes" => NetClasses,
        "library" => Library,
        "sch_library" => SchLibrary,
        "footprint_library" => FootprintLibrary,
        // The footprint editor's own Layers panel, merged into Layers (#2359).
        "footprint_layers" => Layers,
        "symbol_pins" => SymbolPins,
        "pcb_nets" => PcbNets,
        "history" => History,
        "net_inspector" => NetInspector,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dock::{DockLayoutSnapshot, DockLayoutStore, PanelPosition, RegionLayout};
    use crate::panels::PanelKind;

    fn region(panels: &[PanelKind], collapsed: bool, width: Option<u32>) -> RegionLayout {
        RegionLayout {
            panels: panels.to_vec(),
            collapsed,
            width,
        }
    }

    /// Saves a layout through the real store and returns the saved
    /// `prefs.json` text.
    fn saved(dir: &tempfile::TempDir) -> String {
        let path = dir.path().join("prefs.json");
        let layout = DockLayoutSnapshot {
            left: region(&[PanelKind::Projects], false, Some(310)),
            right: region(
                &[PanelKind::Properties, PanelKind::History],
                false,
                Some(420),
            ),
            bottom: region(&[PanelKind::Messages], true, None),
        };
        PrefsDockLayoutStore::at(path.clone())
            .write(&layout)
            .expect("save the dock layout");
        std::fs::read_to_string(path).expect("read prefs.json")
    }

    #[test]
    fn a_saved_layout_round_trips_under_its_layout_keys() {
        let dir = tempfile::tempdir().expect("temp dir");
        let text = saved(&dir);
        let json: serde_json::Value = serde_json::from_str(&text).expect("prefs.json parses");

        assert_eq!(
            json["dock"]["right"]["panels"],
            serde_json::json!(["properties", "history"])
        );

        let dock = read_dock_layout_from(Some(&json)).expect("a saved layout loads");
        assert_eq!(
            dock.panel_kinds(PanelPosition::Right),
            [PanelKind::Properties, PanelKind::History]
        );
    }

    /// #2466: Community has no Signex AI panel. A `prefs.json` saved while
    /// it had one still loads; only the `signex_ai` entry is dropped.
    #[test]
    fn a_saved_signex_ai_entry_is_dropped_and_the_rest_loads() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("prefs.json");
        std::fs::write(
            &path,
            r#"{"theme":"graphite_dark","dock":{
                "left":{"panels":["projects","signex_ai"],"collapsed":false,"width":310},
                "right":{"panels":["properties","signex_ai","history"],"collapsed":false},
                "bottom":{"panels":["signex_ai","messages"],"collapsed":true}}}"#,
        )
        .expect("write an old prefs.json");

        let json = read_prefs_json(&path).expect("the old prefs.json parses");
        let dock = read_dock_layout_from(Some(&json)).expect("the layout still loads");

        assert_eq!(dock.panel_kinds(PanelPosition::Left), [PanelKind::Projects]);
        assert_eq!(
            dock.panel_kinds(PanelPosition::Right),
            [PanelKind::Properties, PanelKind::History]
        );
        assert_eq!(
            dock.panel_kinds(PanelPosition::Bottom),
            [PanelKind::Messages]
        );
        assert!(dock.is_collapsed(PanelPosition::Bottom));
        assert_eq!(dock.preferred_width(PanelPosition::Left), 310.0);
        assert!(
            crate::panels::ALL_PANELS
                .iter()
                .all(|k| panel_kind_key(*k) != "signex_ai")
        );
    }

    #[test]
    fn an_unknown_panel_key_drops_only_that_panel() {
        let dir = tempfile::tempdir().expect("temp dir");
        let text = saved(&dir).replace("history", "signal");
        let json: serde_json::Value = serde_json::from_str(&text).expect("prefs.json parses");

        let dock = read_dock_layout_from(Some(&json)).expect("the layout itself still loads");

        assert_eq!(dock.panel_kinds(PanelPosition::Left), [PanelKind::Projects]);
        assert_eq!(
            dock.panel_kinds(PanelPosition::Right),
            [PanelKind::Properties]
        );
        assert_eq!(
            dock.panel_kinds(PanelPosition::Bottom),
            [PanelKind::Messages]
        );
        assert!(dock.is_collapsed(PanelPosition::Bottom));
        assert!(!dock.is_collapsed(PanelPosition::Left));
    }

    /// #2395: the side docks' preferred widths are saved and load back.
    #[test]
    fn the_side_dock_widths_round_trip() {
        let dir = tempfile::tempdir().expect("temp dir");
        let json: serde_json::Value =
            serde_json::from_str(&saved(&dir)).expect("prefs.json parses");
        assert_eq!(json["dock"]["left"]["width"], serde_json::json!(310));
        assert_eq!(json["dock"]["right"]["width"], serde_json::json!(420));
        assert!(json["dock"]["bottom"].get("width").is_none());

        let dock = read_dock_layout_from(Some(&json)).expect("a saved layout loads");
        assert_eq!(dock.preferred_width(PanelPosition::Left), 310.0);
        assert_eq!(dock.preferred_width(PanelPosition::Right), 420.0);
    }

    /// A layout saved before the widths were stored still loads, with the
    /// fresh-layout widths.
    #[test]
    fn a_layout_without_widths_loads_with_the_default_widths() {
        let json = serde_json::json!({
            "dock": {
                "left": { "panels": ["projects"], "collapsed": false },
                "right": { "panels": ["properties"], "collapsed": false },
            }
        });
        let dock = read_dock_layout_from(Some(&json)).expect("an older layout loads");
        assert_eq!(dock.panel_kinds(PanelPosition::Left), [PanelKind::Projects]);
        assert_eq!(
            dock.preferred_width(PanelPosition::Left),
            crate::dock::DEFAULT_LEFT_DOCK_WIDTH
        );
        assert_eq!(
            dock.preferred_width(PanelPosition::Right),
            crate::dock::DEFAULT_RIGHT_DOCK_WIDTH
        );
    }
}
