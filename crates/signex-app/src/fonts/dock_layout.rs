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
        write_prefs_key(&self.path, "dock", snapshot_to_json(layout))
    }
}

/// Replace the top-level `key` of the `prefs.json` at `path` with `value`,
/// keeping every other setting; refused (and nothing written) when the file
/// cannot be read as preferences. Blocking: call it off the UI thread.
pub(crate) fn write_prefs_key(
    path: &std::path::Path,
    key: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    try_update_prefs_json(path, key, |prefs| {
        prefs.insert(key.to_string(), value);
    })
}

// The dock codec: the `dock` key of `prefs.json` and the workspace layouts
// (mockup row 166) read and write regions through these, so the two forms cannot drift.

/// One dock region as `prefs.json` stores it.
pub(crate) fn region_to_json(region: &crate::dock::RegionLayout) -> serde_json::Value {
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

/// A stored region. An unknown panel key drops that panel only; a missing
/// or malformed field reads as its fresh-layout value (no panels, open, the
/// default width). A width is kept only when it is a positive number, in
/// whole px (Signex writes whole px).
pub(crate) fn region_from_json(region: &serde_json::Value) -> crate::dock::RegionLayout {
    let panels = region
        .get("panels")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|val| val.as_str().and_then(parse_panel_kind))
                // Layer Stack is a document tab now, never docked.
                .filter(|kind| *kind != crate::panels::PanelKind::LayerStack)
                .collect()
        })
        .unwrap_or_default();
    // A layout saved before #2395 has no width; the dock keeps the
    // fresh-layout width.
    let width = region
        .get("width")
        .and_then(|v| v.as_f64())
        .filter(|w| w.is_finite() && *w > 0.0)
        .map(|w| w.round() as u32);
    crate::dock::RegionLayout {
        panels,
        collapsed: region
            .get("collapsed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        width,
    }
}

/// The three regions as `{"left", "right", "bottom"}`.
pub(crate) fn snapshot_to_json(layout: &crate::dock::DockLayoutSnapshot) -> serde_json::Value {
    serde_json::json!({
        "left":   region_to_json(&layout.left),
        "right":  region_to_json(&layout.right),
        "bottom": region_to_json(&layout.bottom),
    })
}

/// The three regions of a stored dock; `None` when `json` is not an object.
/// A missing region is empty.
pub(crate) fn snapshot_from_json(
    json: &serde_json::Value,
) -> Option<crate::dock::DockLayoutSnapshot> {
    let dock_v = json.as_object()?;
    let region = |key: &str| {
        dock_v.get(key).map_or_else(
            || crate::dock::RegionLayout {
                panels: Vec::new(),
                collapsed: false,
                width: None,
            },
            region_from_json,
        )
    };
    Some(crate::dock::DockLayoutSnapshot {
        left: region("left"),
        right: region("right"),
        bottom: region("bottom"),
    })
}

/// Build a dock from a snapshot through the dock's own API (`add_panel`,
/// the collapse toggle, the width setter): the one path a saved layout and
/// an applied workspace layout both take.
pub(crate) fn dock_from_snapshot(
    layout: &crate::dock::DockLayoutSnapshot,
) -> crate::dock::DockArea {
    let mut dock = crate::dock::DockArea::new();
    for (region, pos) in [
        (&layout.left, crate::dock::PanelPosition::Left),
        (&layout.right, crate::dock::PanelPosition::Right),
        (&layout.bottom, crate::dock::PanelPosition::Bottom),
    ] {
        for kind in &region.panels {
            dock.add_panel(pos, *kind);
        }
        // Collapsed flag: run ToggleRegion once via the dock API so
        // the mutation path stays authoritative.
        if region.collapsed {
            dock.update(crate::dock::DockMessage::ToggleCollapse(pos));
        }
        if let Some(width) = region.width {
            dock.set_preferred_width(pos, width as f32);
        }
    }
    dock
}

/// Rebuild a DockArea from persisted JSON. Returns None when no saved
/// layout exists so the caller can fall back to the default seed.
pub fn read_dock_layout() -> Option<crate::dock::DockArea> {
    read_dock_layout_from(load_prefs_json().as_ref())
}

pub fn read_dock_layout_from(json: Option<&serde_json::Value>) -> Option<crate::dock::DockArea> {
    let stored = json?.get("dock")?;
    let snapshot = snapshot_from_json(stored)?;
    let mut dock = dock_from_snapshot(&snapshot);
    // The saved dock keeps a side width as written, fractional px included;
    // the dock's setter ignores a width that is not a positive number.
    for (key, pos) in [
        ("left", crate::dock::PanelPosition::Left),
        ("right", crate::dock::PanelPosition::Right),
    ] {
        if let Some(width) = stored
            .get(key)
            .and_then(|region| region.get("width"))
            .and_then(serde_json::Value::as_f64)
        {
            dock.set_preferred_width(pos, width as f32);
        }
    }
    Some(dock)
}

pub(crate) fn panel_kind_key(k: crate::panels::PanelKind) -> &'static str {
    use crate::panels::PanelKind::*;
    match k {
        Projects => "projects",
        Components => "components",
        Navigator => "navigator",
        Properties => "properties",
        Filter => "filter",
        Erc => "erc",
        ObjectList => "object_list",
        Messages => "messages",
        CommandLog => "command_log",
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
        // The alias rows are never docked; should one be, it saves as the
        // Library panel it stands for (mockup row 119).
        Library | SchLibrary | FootprintLibrary => "library",
        PcbNets => "pcb_nets",
        LengthMatching => "length_matching",
        History => "history",
        NetInspector => "net_inspector",
        Unions => "unions",
        CompiledDesign => "compiled_design",
        Annotations => "annotations",
    }
}

pub(crate) fn parse_panel_kind(s: &str) -> Option<crate::panels::PanelKind> {
    use crate::panels::PanelKind::*;
    Some(match s {
        "projects" => Projects,
        "components" => Components,
        "navigator" => Navigator,
        "properties" => Properties,
        "filter" => Filter,
        "erc" => Erc,
        // SCH Filter is retired into Filter (mockup rows 40 and 55).
        "sch_filter" => Filter,
        "object_list" => ObjectList,
        "messages" => Messages,
        "command_log" => CommandLog,
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
        // The footprint editor's own Layers panel, merged into Layers (#2359).
        "footprint_layers" => Layers,
        "pcb_nets" => PcbNets,
        "length_matching" => LengthMatching,
        "history" => History,
        "net_inspector" => NetInspector,
        "unions" => Unions,
        "compiled_design" => CompiledDesign,
        "annotations" => Annotations,
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

    /// #1259: the Object List saves as `object_list` and loads back; a layout
    /// saved under the old `sch_list` key drops that panel (no alias).
    #[test]
    fn the_object_list_round_trips_and_the_old_key_is_dropped() {
        assert_eq!(panel_kind_key(PanelKind::ObjectList), "object_list");
        assert_eq!(parse_panel_kind("object_list"), Some(PanelKind::ObjectList));
        assert_eq!(parse_panel_kind("sch_list"), None);

        let dir = tempfile::tempdir().expect("temp dir");
        let text = saved(&dir).replace("history", "object_list");
        let json: serde_json::Value = serde_json::from_str(&text).expect("prefs.json parses");
        let dock = read_dock_layout_from(Some(&json)).expect("the layout loads");
        assert_eq!(
            dock.panel_kinds(PanelPosition::Right),
            [PanelKind::Properties, PanelKind::ObjectList]
        );

        let old = saved(&dir).replace("history", "sch_list");
        let json: serde_json::Value = serde_json::from_str(&old).expect("prefs.json parses");
        let dock = read_dock_layout_from(Some(&json)).expect("the layout still loads");
        assert_eq!(
            dock.panel_kinds(PanelPosition::Right),
            [PanelKind::Properties]
        );
    }

    /// Mockup rows 119-122: the old SCH Library, Footprint Library and Pins
    /// keys of a saved layout are dropped with no alias (no-legacy rule), the
    /// rest of the layout loads, and no panel saves under those keys.
    #[test]
    fn a_saved_layout_drops_the_merged_library_panel_keys() {
        let json = serde_json::json!({
            "dock": {
                "left": {
                    "panels": ["projects", "sch_library", "footprint_library"],
                    "collapsed": false
                },
                "right": { "panels": ["properties"], "collapsed": false },
                "bottom": { "panels": ["messages", "symbol_pins", "erc"], "collapsed": false },
            }
        });
        let dock = read_dock_layout_from(Some(&json)).expect("the layout loads");
        assert_eq!(dock.panel_kinds(PanelPosition::Left), [PanelKind::Projects]);
        assert_eq!(
            dock.panel_kinds(PanelPosition::Bottom),
            [PanelKind::Messages, PanelKind::Erc]
        );
        for kind in crate::panels::ALL_PANELS {
            let key = panel_kind_key(*kind);
            assert!(
                !matches!(key, "sch_library" | "footprint_library" | "symbol_pins"),
                "{kind:?} saves as {key}"
            );
        }
        assert_eq!(panel_kind_key(PanelKind::SchLibrary), "library");
        assert_eq!(panel_kind_key(PanelKind::FootprintLibrary), "library");
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

    /// #2751: raising the default leaves a layout saved at the old 220 px
    /// at 220 px.
    #[test]
    fn a_layout_saved_at_the_old_default_keeps_its_width() {
        let json = serde_json::json!({
            "dock": {
                "right": { "panels": ["properties"], "collapsed": false, "width": 220 },
            }
        });
        let dock = read_dock_layout_from(Some(&json)).expect("a saved layout loads");
        assert_eq!(dock.preferred_width(PanelPosition::Right), 220.0);
    }

    /// #2458: the Command Log is saved as `command_log` and loads back.
    #[test]
    fn the_command_log_round_trips_under_its_key() {
        assert_eq!(panel_kind_key(PanelKind::CommandLog), "command_log");
        assert_eq!(parse_panel_kind("command_log"), Some(PanelKind::CommandLog));
        let json = serde_json::json!({
            "dock": {
                "bottom": { "panels": ["messages", "command_log"], "collapsed": false },
            }
        });
        let dock = read_dock_layout_from(Some(&json)).expect("the layout loads");
        assert_eq!(
            dock.panel_kinds(PanelPosition::Bottom),
            [PanelKind::Messages, PanelKind::CommandLog]
        );
    }

    /// Mockup row 105: Layer Stack is a document tab, so a layout saved while
    /// it was a dock panel loads without it.
    #[test]
    fn a_saved_layout_drops_the_layer_stack_panel() {
        let json = serde_json::json!({
            "dock": {
                "left": { "panels": ["projects"], "collapsed": false },
                "right": { "panels": ["properties", "layer_stack"], "collapsed": false },
                "bottom": { "panels": ["messages"], "collapsed": false },
            }
        });
        let dock = read_dock_layout_from(Some(&json)).expect("the layout loads");
        assert_eq!(
            dock.panel_kinds(PanelPosition::Right),
            [PanelKind::Properties]
        );
    }

    /// Mockup row 165: the Annotations panel is saved under the key `annotations` and
    /// loads back from it; every panel kind's key reads back as that kind.
    #[test]
    fn the_annotations_panel_round_trips_under_its_key() {
        assert_eq!(panel_kind_key(PanelKind::Annotations), "annotations");
        for kind in crate::panels::ALL_PANELS {
            // The SCH Library and Footprint Library rows are aliases of the
            // Library panel and save as it (mockup row 119).
            let expected = match kind {
                PanelKind::SchLibrary | PanelKind::FootprintLibrary => PanelKind::Library,
                other => *other,
            };
            assert_eq!(
                parse_panel_kind(panel_kind_key(*kind)),
                Some(expected),
                "{kind:?}"
            );
        }
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("prefs.json");
        let layout = DockLayoutSnapshot {
            left: region(&[PanelKind::Projects], false, None),
            right: region(
                &[PanelKind::Properties, PanelKind::Annotations],
                false,
                None,
            ),
            bottom: region(&[], false, None),
        };
        PrefsDockLayoutStore::at(path.clone())
            .write(&layout)
            .expect("save the dock layout");
        let text = std::fs::read_to_string(path).expect("read prefs.json");
        let json: serde_json::Value = serde_json::from_str(&text).expect("prefs.json parses");
        assert_eq!(
            json["dock"]["right"]["panels"],
            serde_json::json!(["properties", "annotations"])
        );
        let dock = read_dock_layout_from(Some(&json)).expect("the layout loads");
        assert_eq!(
            dock.panel_kinds(PanelPosition::Right),
            [PanelKind::Properties, PanelKind::Annotations]
        );
    }

    /// Mockup rows 40 and 55: SCH Filter is retired into Filter. A layout
    /// saved with `sch_filter` loads it as Filter, once even when the layout
    /// also names `filter`, and no panel saves under `sch_filter` any more.
    #[test]
    fn a_saved_sch_filter_loads_as_the_filter_panel() {
        assert_eq!(parse_panel_kind("sch_filter"), Some(PanelKind::Filter));
        let json = serde_json::json!({
            "dock": {
                "left": { "panels": ["projects"], "collapsed": false },
                "right": { "panels": ["properties", "sch_filter"], "collapsed": false },
                "bottom": { "panels": ["messages"], "collapsed": false },
            }
        });
        let dock = read_dock_layout_from(Some(&json)).expect("the layout loads");
        assert_eq!(
            dock.panel_kinds(PanelPosition::Right),
            [PanelKind::Properties, PanelKind::Filter]
        );

        let both = serde_json::json!({
            "dock": {
                "right": { "panels": ["filter", "properties", "sch_filter"], "collapsed": false },
            }
        });
        let dock = read_dock_layout_from(Some(&both)).expect("the layout loads");
        assert_eq!(
            dock.panel_kinds(PanelPosition::Right),
            [PanelKind::Filter, PanelKind::Properties]
        );
        for kind in crate::panels::ALL_PANELS {
            assert_ne!(panel_kind_key(*kind), "sch_filter", "{kind:?}");
        }
        assert_eq!(panel_kind_key(PanelKind::Filter), "filter");
    }
}
