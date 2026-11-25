//! The board views kept across launches (#2480): what each board shows of
//! its layers and kinds of object, per board path, in `prefs.json` under
//! `pcb_board_views`. The board files never carry them; a layer colour is
//! board data, saved with the board. The highlight and Show only unrouted
//! end with the board and are not kept (`PcbLayerView::kept`).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Map, Value, json};
use signex_renderer::pcb::{BoardObject, BuiltinLayerSet, LayerSetId, PcbLayerView, ShownLayerSet};
use signex_types::format::{parse_pcb_layer, pcb_layer_token};
use signex_types::pcb_layer::PcbLayer;

/// The `prefs.json` key of the kept board views.
pub const BOARD_VIEWS_PREF_KEY: &str = "pcb_board_views";

/// How long the views must stay still before they are written: a slider
/// drag coalesces into one write.
pub const BOARD_VIEWS_SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

/// The kept views of the boards shown this session, by board path. A write
/// merges them into what is on disk, so another board's view survives.
pub type BoardViews = BTreeMap<PathBuf, PcbLayerView>;

/// Change detection and debounce of the board view writes.
pub type BoardViewSync = crate::persist::DebouncedSync<BoardViews>;

/// One board view write to run off the UI thread.
pub type BoardViewWrite = crate::persist::PendingWrite<BoardViews>;

/// Messages of the board view save loop, under `PcbMsg::ViewSave`.
#[derive(Debug, Clone, PartialEq)]
pub enum BoardViewSaveMsg {
    /// The quiet period for generation `.0` ran out; a stale generation
    /// (the views changed again meanwhile) is ignored.
    SaveDebounceElapsed(u64),
    /// A background write finished; the error is what Messages shows.
    Saved(Result<(), String>),
}

/// The layer tokens of `layers`, sorted so a file diff stays quiet.
fn layer_tokens(layers: impl Iterator<Item = PcbLayer>) -> Vec<String> {
    let mut tokens: Vec<String> = layers.map(pcb_layer_token).collect();
    tokens.sort();
    tokens
}

/// The layers a token list names; a token this build does not know is
/// left out.
fn layers_of(json: &Value) -> Vec<PcbLayer> {
    json.as_array()
        .into_iter()
        .flatten()
        .filter_map(|token| parse_pcb_layer(token.as_str()?))
        .collect()
}

fn set_to_json(set: &ShownLayerSet) -> Value {
    let layers: Vec<String> = set.layers.iter().map(|&l| pcb_layer_token(l)).collect();
    match &set.id {
        LayerSetId::Builtin(builtin) => json!({ "builtin": builtin.token(), "layers": layers }),
        LayerSetId::Saved(name) => json!({ "saved": name, "layers": layers }),
    }
}

fn set_from_json(json: &Value) -> Option<ShownLayerSet> {
    let id = match json["builtin"].as_str() {
        Some(token) => LayerSetId::Builtin(BuiltinLayerSet::from_token(token)?),
        None => LayerSetId::Saved(json["saved"].as_str()?.to_string()),
    };
    Some(ShownLayerSet {
        id,
        layers: layers_of(&json["layers"]),
    })
}

/// The `prefs.json` value of one board view.
pub fn view_to_json(view: &PcbLayerView) -> Value {
    let hidden_objects: Vec<&str> = BoardObject::ALL
        .into_iter()
        .filter(|kind| !view.shows_object(*kind))
        .map(BoardObject::token)
        .collect();
    let transparency: Map<String, Value> = BoardObject::ALL
        .into_iter()
        .filter_map(|kind| {
            let percent = view.paint().transparency(kind);
            (percent > 0).then(|| (kind.token().to_string(), json!(percent)))
        })
        .collect();
    let mut value = json!({
        "hidden": layer_tokens(view.hidden_layers()),
        "active": pcb_layer_token(view.active()),
        "ratsnest": view.ratsnest_visible(),
        "multi_layer": view.multi_layer_visible(),
        "origin_marker": view.origin_marker_visible(),
        "single_layer": view.single_layer(),
        "flipped": view.flipped(),
        "used_only": view.used_only(),
        "hidden_objects": hidden_objects,
        "transparency": transparency,
    });
    if let Some(set) = view.layer_set() {
        value["layer_set"] = set_to_json(set);
    }
    value
}

/// Read one kept board view. Anything missing or unknown reads as the
/// default view's: a fresh view is drawn, never a refusal.
pub fn view_from_json(json: &Value) -> PcbLayerView {
    let mut view = PcbLayerView::default();
    view.set_layers_visible(&layers_of(&json["hidden"]), false);
    if let Some(active) = json["active"].as_str().and_then(parse_pcb_layer) {
        view.set_active(active);
    }
    let flag = |key: &str, default: bool| json[key].as_bool().unwrap_or(default);
    if !flag("ratsnest", true) {
        view.toggle_ratsnest();
    }
    if !flag("multi_layer", true) {
        view.toggle_multi_layer();
    }
    view.set_origin_marker_visible(flag("origin_marker", true));
    if flag("single_layer", false) {
        view.toggle_single_layer();
    }
    if flag("flipped", false) {
        view.toggle_flipped();
    }
    view.set_used_only(flag("used_only", false));
    let hidden_objects = json["hidden_objects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|token| BoardObject::from_token(token.as_str()?));
    for kind in hidden_objects {
        view.set_object_visible(kind, false);
    }
    for (token, percent) in json["transparency"].as_object().into_iter().flatten() {
        if let (Some(kind), Some(percent)) = (BoardObject::from_token(token), percent.as_u64()) {
            let percent = u8::try_from(percent.min(100)).unwrap_or(100);
            view.set_object_transparency(kind, percent);
        }
    }
    view.set_layer_set(set_from_json(&json["layer_set"]));
    view
}

/// The kept views in a parsed `prefs.json`, by board path; a missing or
/// broken key reads as none.
pub fn read_board_views_from(prefs: Option<&Value>) -> HashMap<PathBuf, PcbLayerView> {
    prefs
        .and_then(|prefs| prefs[BOARD_VIEWS_PREF_KEY].as_object())
        .into_iter()
        .flatten()
        .map(|(path, view)| (PathBuf::from(path), view_from_json(view)))
        .collect()
}

/// Merge `views` into the `prefs.json` object `prefs`: each board's entry
/// is replaced, every other board's is left as it is.
pub fn merge_board_views(prefs: &mut Map<String, Value>, views: &BoardViews) {
    if views.is_empty() {
        return;
    }
    let entry = prefs
        .entry(BOARD_VIEWS_PREF_KEY.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(Map::new());
    }
    if let Some(saved) = entry.as_object_mut() {
        for (path, view) in views {
            saved.insert(path.display().to_string(), view_to_json(view));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::pcb_layer::{CopperLayer, TechnicalLayer};

    const TOP: PcbLayer = PcbLayer::Copper(CopperLayer::Top);
    const BOTTOM: PcbLayer = PcbLayer::Copper(CopperLayer::Bottom);
    const OVERLAY: PcbLayer = PcbLayer::Technical(TechnicalLayer::SilkscreenTop);

    /// A view with every kept field away from the default.
    fn changed_view(set: LayerSetId) -> PcbLayerView {
        let mut view = PcbLayerView::default();
        view.hide(OVERLAY);
        view.hide(TOP);
        view.set_active(BOTTOM);
        view.toggle_ratsnest();
        view.toggle_multi_layer();
        view.set_origin_marker_visible(false);
        view.toggle_single_layer();
        view.toggle_flipped();
        view.set_used_only(true);
        view.set_object_visible(BoardObject::Vias, false);
        view.set_object_visible(BoardObject::Texts, false);
        view.set_object_transparency(BoardObject::Tracks, 40);
        view.set_object_transparency(BoardObject::Pads, 100);
        view.set_layer_set(Some(ShownLayerSet {
            id: set,
            layers: vec![BOTTOM, TOP],
        }));
        view
    }

    #[test]
    fn a_kept_view_round_trips_through_json() {
        for set in [
            LayerSetId::Builtin(BuiltinLayerSet::Signal),
            LayerSetId::Saved("Assembly".to_string()),
        ] {
            let view = changed_view(set);
            assert_ne!(view, PcbLayerView::default());
            assert_eq!(view_from_json(&view_to_json(&view)), view);
        }
        let plain = PcbLayerView::default();
        assert_eq!(view_from_json(&view_to_json(&plain)), plain);
    }

    #[test]
    fn the_highlight_and_show_only_unrouted_are_not_kept() {
        let mut view = PcbLayerView::default();
        view.toggle_highlight(TOP);
        view.set_only_unrouted(true);
        let kept = view_from_json(&view_to_json(&view.kept()));
        assert_eq!(kept, PcbLayerView::default());
    }

    #[test]
    fn missing_or_broken_values_read_as_the_default_view() {
        assert_eq!(view_from_json(&Value::Null), PcbLayerView::default());
        let broken = json!({
            "hidden": ["Nope", "TopCopper"],
            "active": 7,
            "ratsnest": "no",
            "hidden_objects": ["wires"],
            "transparency": { "tracks": 250, "wires": 10 },
            "layer_set": { "builtin": "tiny" }
        });
        let view = view_from_json(&broken);
        assert!(view.is_hidden(TOP));
        assert_eq!(view.active(), TOP);
        assert!(view.ratsnest_visible());
        assert!(!view.hides_objects());
        assert_eq!(view.paint().transparency(BoardObject::Tracks), 100);
        assert!(view.layer_set().is_none());
    }

    #[test]
    fn a_merge_replaces_each_board_and_keeps_the_others() {
        let mut prefs = Map::new();
        prefs.insert("theme".to_string(), json!("graphite_dark"));
        let a = PathBuf::from("/w/a.snxpcb");
        let b = PathBuf::from("/w/b.snxpcb");
        let first: BoardViews = [
            (a.clone(), PcbLayerView::default()),
            (b.clone(), PcbLayerView::default()),
        ]
        .into_iter()
        .collect();
        merge_board_views(&mut prefs, &first);
        let changed = changed_view(LayerSetId::Builtin(BuiltinLayerSet::Plane));
        let second: BoardViews = [(a.clone(), changed.clone())].into_iter().collect();
        merge_board_views(&mut prefs, &second);
        let json = Value::Object(prefs);
        assert_eq!(json["theme"], json!("graphite_dark"));
        let read = read_board_views_from(Some(&json));
        assert_eq!(read.get(&a), Some(&changed));
        assert_eq!(read.get(&b), Some(&PcbLayerView::default()));
    }
}
