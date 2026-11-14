//! The Component view's facts, copied from the board and the selection.

use std::path::Path;

use pcb_geom::ItemRef;
use signex_engine::pcb::component;
use signex_engine::pcb::designators::FootprintTextField;
use signex_types::pcb::{Footprint, PcbBoard, SymbolLink};

use crate::app::Signex;
use crate::panels::{
    ComponentSettingFacts, FootprintLibrary, ParameterFacts, PartIdentity, PcbComponentFacts,
    Shared,
};

/// The value every footprint gives, or mixed. Only called on a non-empty
/// list, where it always has an answer.
fn shared<T: PartialEq>(footprints: &[&Footprint], read: impl Fn(&Footprint) -> T) -> Shared<T> {
    Shared::of(footprints.iter().map(|fp| read(fp))).unwrap_or(Shared::Mixed)
}

/// Whether every footprint has the text, then whether it shows.
fn text_shown(footprints: &[&Footprint], field: FootprintTextField) -> Option<Shared<bool>> {
    let shown: Option<Vec<bool>> = footprints
        .iter()
        .map(|fp| component::text_shown(fp, field))
        .collect();
    Shared::of(shown?)
}

/// The sheet path and the part's unit uuids, `/` joined: where the link
/// points in the schematic (#1312).
fn hierarchical_path(link: &SymbolLink) -> String {
    let sheet = link.sheet.to_string();
    let base = sheet.trim_end_matches('/');
    let units: Vec<String> = link.units.iter().map(ToString::to_string).collect();
    format!("{base}/{}", units.join(","))
}

/// The component settings the footprints share (#2365).
fn settings(footprints: &[&Footprint]) -> ComponentSettingFacts {
    ComponentSettingFacts {
        component_type: shared(footprints, |fp| fp.component.component_type),
        height_mm: shared(footprints, |fp| fp.component.height_mm),
        body_opacity: shared(footprints, |fp| fp.component.body_opacity),
        lock_primitives: shared(footprints, |fp| fp.component.lock_primitives),
        lock_strings: shared(footprints, |fp| fp.component.lock_strings),
        pin_swapping: shared(footprints, |fp| fp.component.pin_swapping),
        part_swapping: shared(footprints, |fp| fp.component.part_swapping),
        channel_offset: shared(footprints, |fp| fp.component.channel_offset),
        description: shared(footprints, |fp| fp.component.description.clone()),
    }
}

/// The parameters every footprint has, in the first one's order.
fn parameters(footprints: &[&Footprint]) -> Vec<ParameterFacts> {
    let Some(first) = footprints.first() else {
        return Vec::new();
    };
    component::parameter_keys(first)
        .into_iter()
        .filter_map(|key| {
            let values: Option<Vec<String>> = footprints
                .iter()
                .map(|fp| component::parameter(fp, key).map(|p| p.value.clone()))
                .collect();
            Some(ParameterFacts {
                key: key.to_string(),
                value: Shared::of(values?)?,
            })
        })
        .collect()
}

impl Signex {
    /// The Component view's facts of the footprints selected on the board at
    /// `path`, when the selection is footprints only.
    pub(crate) fn pcb_component_facts(
        &self,
        path: &Path,
        board: &PcbBoard,
    ) -> Option<PcbComponentFacts> {
        let selection = &self.interaction_state.pcb_selection;
        if selection.document() != Some(path) {
            return None;
        }
        let mut ids = Vec::new();
        for item in selection.selection().items() {
            let ItemRef::Footprint(id) = item else {
                return None;
            };
            ids.push(id);
        }
        ids.sort_unstable();
        ids.dedup();
        let footprints: Vec<&Footprint> = ids
            .iter()
            .filter_map(|id| board.footprints.iter().find(|fp| fp.uuid == *id))
            .collect();
        if footprints.is_empty() || footprints.len() != ids.len() {
            return None;
        }
        let fps = footprints.as_slice();
        let state = &self.interaction_state.pcb_component;
        let drafts = state
            .draft
            .as_ref()
            .filter(|d| d.is_for(path, &ids))
            .map(|d| d.texts.clone())
            .unwrap_or_default();
        let identities: Vec<PartIdentity> = fps
            .iter()
            .map(|fp| self.part_identity(path, fp))
            .collect();
        let designator_mismatches = fps
            .iter()
            .zip(&identities)
            .filter(|(fp, identity)| {
                matches!(identity, PartIdentity::Read { designator, .. } if *designator != fp.reference.trim())
            })
            .count();
        Some(PcbComponentFacts {
            settings: settings(fps),
            identity: Shared::of(identities).unwrap_or(Shared::Mixed),
            designator_mismatches,
            board: path.to_path_buf(),
            x: shared(fps, |fp| fp.position.x),
            y: shared(fps, |fp| fp.position.y),
            rotation: shared(fps, |fp| fp.rotation),
            locked: shared(fps, |fp| fp.locked),
            side: shared(fps, component::side_of),
            designator: shared(fps, |fp| fp.reference.clone()),
            designator_shown: text_shown(fps, FootprintTextField::Designator),
            comment: shared(fps, |fp| fp.value.clone()),
            comment_shown: text_shown(fps, FootprintTextField::Value),
            area_mm2: shared(fps, component::body_area_mm2),
            footprint_name: shared(fps, |fp| fp.footprint_id.clone()),
            library: shared(fps, |fp| {
                fp.library_ref
                    .as_ref()
                    .map(|source| self.footprint_library(source.library))
            }),
            schematic_path: shared(fps, |fp| fp.schematic_link.as_ref().map(hierarchical_path)),
            parameters: parameters(fps),
            drafts,
            read_only: board.unknown.newer_minor_source.is_some(),
            search: state.search.clone(),
            footprints: ids,
        })
    }

    /// The library `id` as the Library row names it: an open library by its
    /// display name, else by its id.
    fn footprint_library(&self, id: uuid::Uuid) -> FootprintLibrary {
        self.library
            .open_libraries
            .iter()
            .find(|lib| lib.library_id == id)
            .map_or(FootprintLibrary::NotOpen(id), |lib| {
                FootprintLibrary::Open(lib.display_name.clone())
            })
    }
}

#[cfg(test)]
mod tests {
    use signex_types::schematic::InstancePath;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn the_hierarchical_path_joins_the_sheet_path_and_the_units() {
        let unit = Uuid::from_u128(7);
        let root = SymbolLink::new(InstancePath::root(), [unit]);
        assert_eq!(hierarchical_path(&root), format!("/{unit}"));
        let sheet = Uuid::from_u128(3);
        let nested = SymbolLink::new(InstancePath::from_chain(vec![sheet]), [unit]);
        assert_eq!(hierarchical_path(&nested), format!("/{sheet}/{unit}"));
    }
}
