//! User-defined selection-filter presets: a named set of filter kinds the
//! user saves, applies with one click and keeps across restarts. One
//! mechanism for every editor that has a selection filter: the schematic
//! keys it on [`SelectionFilter`], the board on [`pcb_geom::PickKind`]
//! (#2383). Each list is persisted under its own `prefs.json` key.

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
/// ("Filter 2"), and make it the active tab. `false` at the limit.
pub fn add<K: PresetKind>(
    presets: &mut Vec<CustomFilterPreset<K>>,
    tab: &mut usize,
    on: impl Fn(K) -> bool,
) -> bool {
    if presets.len() >= CUSTOM_FILTER_PRESET_LIMIT {
        return false;
    }
    let name = format!("Filter {}", presets.len() + 1);
    presets.push(CustomFilterPreset::of_kinds(name, on));
    *tab = presets.len() - 1;
    true
}

/// Remove preset `idx`, keeping the active tab in range. `false` for an
/// index past the end.
pub fn remove<K>(presets: &mut Vec<CustomFilterPreset<K>>, tab: &mut usize, idx: usize) -> bool {
    if idx >= presets.len() {
        return false;
    }
    presets.remove(idx);
    *tab = (*tab).min(presets.len().saturating_sub(1));
    true
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
        let mut tab = 0;
        for _ in 0..CUSTOM_FILTER_PRESET_LIMIT {
            assert!(add(&mut presets, &mut tab, |k| k == PickKind::Vias));
        }
        assert!(!add(&mut presets, &mut tab, |_| true), "the limit holds");
        assert_eq!(tab, CUSTOM_FILTER_PRESET_LIMIT - 1);
        assert!(toggle_member(&mut presets, 0, PickKind::Components));
        assert_eq!(
            presets[0].filters,
            vec![PickKind::Components, PickKind::Vias]
        );
        assert!(remove(&mut presets, &mut tab, 3));
        assert_eq!((presets.len(), tab), (3, 2));
        let json = serde_json::to_string(&presets[0]).expect("serialises");
        assert_eq!(
            json,
            r#"{"name":"Filter 1","filters":["Components","Vias"]}"#
        );
    }
}
