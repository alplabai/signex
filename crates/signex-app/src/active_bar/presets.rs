//! User-defined selection-filter presets: a named set of filter kinds the
//! user saves, applies with one click and keeps across restarts. One
//! mechanism for every editor that has a selection filter: the schematic
//! keys it on [`SelectionFilter`], the board on [`pcb_geom::PickKind`]
//! (#2383). Each list is persisted under its own `prefs.json` key. A click
//! on a preset applies it and selects it for editing: while it is selected,
//! each chip click changes it in place (#2873).

use std::collections::HashSet;
use std::hash::Hash;

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::SelectionFilter;

/// Maximum number of presets one editor keeps: shortcut buttons in its
/// filter dropdown, tabs in its Properties panel.
pub const CUSTOM_FILTER_PRESET_LIMIT: usize = 4;

/// A kind a selection filter turns on or off.
pub trait PresetKind: Copy + Eq + Hash + Serialize + DeserializeOwned + 'static {
    /// Every kind, in the order the filter shows them; a saved preset keeps
    /// this order so two saves of one set are byte-identical.
    fn all() -> &'static [Self];
}

impl PresetKind for SelectionFilter {
    fn all() -> &'static [Self] {
        SelectionFilter::ALL
    }
}

impl PresetKind for pcb_geom::PickKind {
    fn all() -> &'static [Self] {
        &pcb_geom::PickKind::ALL
    }
}

/// A named selection-filter preset. Stored as a `Vec` (not a `HashSet`) so
/// the saved list has a stable order and a plain JSON shape.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CustomFilterPreset<K = SelectionFilter> {
    pub name: String,
    pub filters: Vec<K>,
}

impl<K: PresetKind> CustomFilterPreset<K> {
    /// A preset of the kinds in `on`, in [`PresetKind::all`] order.
    pub fn of_kinds(name: String, on: impl Fn(K) -> bool) -> Self {
        let filters = K::all().iter().copied().filter(|k| on(*k)).collect();
        Self { name, filters }
    }

    /// Snapshot the active filter set into a preset named `name`.
    pub fn capture(name: String, filters: &HashSet<K>) -> Self {
        Self::of_kinds(name, |k| filters.contains(&k))
    }

    /// The preset's kinds as a set, to assign back to the active filter.
    pub fn as_set(&self) -> HashSet<K> {
        self.filters.iter().copied().collect()
    }
}

/// Add a preset holding the kinds `on` turns on, named after its place
/// ("Filter 2"). Its index, or `None` at the limit.
pub fn add<K: PresetKind>(
    presets: &mut Vec<CustomFilterPreset<K>>,
    on: impl Fn(K) -> bool,
) -> Option<usize> {
    if presets.len() >= CUSTOM_FILTER_PRESET_LIMIT {
        return None;
    }
    let name = format!("Filter {}", presets.len() + 1);
    presets.push(CustomFilterPreset::of_kinds(name, on));
    Some(presets.len() - 1)
}

/// Remove preset `idx`. `false` for an index past the end.
pub fn remove<K>(presets: &mut Vec<CustomFilterPreset<K>>, idx: usize) -> bool {
    if idx >= presets.len() {
        return false;
    }
    presets.remove(idx);
    true
}

/// What a click on a preset did (#2873).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetClick {
    /// The preset is now the one edited: its kinds become the filter.
    Selected,
    /// It was the one edited: editing ends and the filter stays.
    Released,
}

/// A click on preset `idx` of `len`: it becomes the edited preset, or, if
/// it already was, editing ends. `None` for an index past the end.
pub fn click(editing: &mut Option<usize>, idx: usize, len: usize) -> Option<PresetClick> {
    if idx >= len {
        return None;
    }
    if *editing == Some(idx) {
        *editing = None;
        return Some(PresetClick::Released);
    }
    *editing = Some(idx);
    Some(PresetClick::Selected)
}

/// The edited preset once preset `removed` is gone: none if it was that
/// one, one place down if it came after it.
pub fn edited_after_removal(editing: Option<usize>, removed: usize) -> Option<usize> {
    match editing {
        Some(idx) if idx == removed => None,
        Some(idx) if idx > removed => Some(idx - 1),
        other => other,
    }
}

/// Rename preset `idx`. `false` for an index past the end.
pub fn rename<K>(presets: &mut [CustomFilterPreset<K>], idx: usize, name: String) -> bool {
    let Some(preset) = presets.get_mut(idx) else {
        return false;
    };
    preset.name = name;
    true
}

/// Turn `kind` in or out of preset `idx`, keeping [`PresetKind::all`]
/// order. `false` for an index past the end.
pub fn toggle_member<K: PresetKind>(
    presets: &mut [CustomFilterPreset<K>],
    idx: usize,
    kind: K,
) -> bool {
    let Some(preset) = presets.get_mut(idx) else {
        return false;
    };
    let had = preset.filters.contains(&kind);
    let next = CustomFilterPreset::of_kinds(String::new(), |k| {
        if k == kind {
            !had
        } else {
            preset.filters.contains(&k)
        }
    });
    preset.filters = next.filters;
    true
}

/// Replace preset `idx`'s kinds with those `on` turns on, keeping its name.
/// `false` for an index past the end.
pub fn capture<K: PresetKind>(
    presets: &mut [CustomFilterPreset<K>],
    idx: usize,
    on: impl Fn(K) -> bool,
) -> bool {
    let Some(preset) = presets.get_mut(idx) else {
        return false;
    };
    preset.filters = CustomFilterPreset::of_kinds(String::new(), on).filters;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcb_geom::PickKind;

    #[test]
    fn the_board_kinds_use_the_same_preset_list() {
        let mut presets: Vec<CustomFilterPreset<PickKind>> = Vec::new();
        for n in 0..CUSTOM_FILTER_PRESET_LIMIT {
            assert_eq!(add(&mut presets, |k| k == PickKind::Vias), Some(n));
        }
        assert_eq!(add(&mut presets, |_| true), None, "the limit holds");
        assert!(toggle_member(&mut presets, 0, PickKind::Components));
        assert_eq!(
            presets[0].filters,
            vec![PickKind::Components, PickKind::Vias]
        );
        assert!(remove(&mut presets, 3));
        assert!(!remove(&mut presets, 3), "past the end");
        assert_eq!(presets.len(), 3);
        let json = serde_json::to_string(&presets[0]).expect("serialises");
        assert_eq!(
            json,
            r#"{"name":"Filter 1","filters":["Components","Vias"]}"#
        );
    }

    /// #2873: a click selects a preset for editing, a second click on it
    /// ends editing, and a removal keeps the edited index on its preset.
    #[test]
    fn a_click_selects_a_preset_and_a_second_click_releases_it() {
        let mut editing = None;
        assert_eq!(click(&mut editing, 1, 3), Some(PresetClick::Selected));
        assert_eq!(editing, Some(1));
        assert_eq!(click(&mut editing, 2, 3), Some(PresetClick::Selected));
        assert_eq!(click(&mut editing, 2, 3), Some(PresetClick::Released));
        assert_eq!(editing, None);
        assert_eq!(click(&mut editing, 3, 3), None, "past the end");
        assert_eq!(editing, None);

        assert_eq!(edited_after_removal(Some(2), 2), None);
        assert_eq!(edited_after_removal(Some(2), 0), Some(1));
        assert_eq!(edited_after_removal(Some(1), 2), Some(1));
        assert_eq!(edited_after_removal(None, 0), None);
    }
}
