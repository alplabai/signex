//! The default drawing order of a package and its top / bottom pairs. See
//! `docs/GERBER_VIEWER.md`, "Package".

use crate::{FileFunction, JobFile, Side};

use super::PackageLayer;
use super::naming::copper_number;

/// `layers` in the default drawing order, index 0 on top: the board
/// profile, the drill files and the drawings above everything; then the
/// stack from the top: top legend, paste and mask, copper L1..Ln, bottom
/// mask, paste and legend. Layers at the same place keep the job file's
/// order, then the order given.
pub(super) fn in_default_order(
    layers: Vec<PackageLayer>,
    job: Option<&JobFile>,
) -> Vec<PackageLayer> {
    let mut keyed: Vec<((u8, u32, usize), PackageLayer)> = layers
        .into_iter()
        .map(|layer| {
            let (band, place) = place_in_stack(layer.function.as_ref());
            let listed = job
                .and_then(|job| job_position(job, &layer.image.file))
                .unwrap_or(usize::MAX);
            ((band, place, listed), layer)
        })
        .collect();
    // A stable sort: equal keys keep the order given.
    keyed.sort_by_key(|(key, _)| *key);
    keyed.into_iter().map(|(_, layer)| layer).collect()
}

/// Where a layer of `function` sits, top first: a band (overlays, top
/// side, copper, bottom side) and the place in it.
fn place_in_stack(function: Option<&FileFunction>) -> (u8, u32) {
    const OVERLAY: u8 = 0;
    const TOP: u8 = 1;
    const COPPER: u8 = 2;
    const BOTTOM: u8 = 3;
    let Some(function) = function else {
        return (OVERLAY, 3);
    };
    match function {
        FileFunction::Profile { .. } => (OVERLAY, 0),
        FileFunction::Drill { plated: true, .. } => (OVERLAY, 1),
        FileFunction::Drill { plated: false, .. } => (OVERLAY, 2),
        FileFunction::Other(_) => (OVERLAY, 3),
        FileFunction::Legend(Side::Top) => (TOP, 0),
        FileFunction::Paste(Side::Top) => (TOP, 1),
        FileFunction::Soldermask(Side::Top) => (TOP, 2),
        FileFunction::Copper { number, .. } => (COPPER, *number),
        FileFunction::Soldermask(Side::Bottom) => (BOTTOM, 0),
        FileFunction::Paste(Side::Bottom) => (BOTTOM, 1),
        FileFunction::Legend(Side::Bottom) => (BOTTOM, 2),
    }
}

/// The place of `file` in the job file's list.
fn job_position(job: &JobFile, file: &str) -> Option<usize> {
    let name = file.rsplit(['/', '\\']).next().unwrap_or(file);
    job.entries.iter().position(|entry| {
        let listed = entry.path.rsplit(['/', '\\']).next().unwrap_or(&entry.path);
        listed.eq_ignore_ascii_case(name)
    })
}

/// What a layer pairs by: the kind and the side, or the copper number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PairKey {
    Legend(bool),
    Soldermask(bool),
    Paste(bool),
    Copper(u32),
}

fn pair_key(function: &FileFunction) -> Option<PairKey> {
    let top = |side: &Side| matches!(side, Side::Top);
    match function {
        FileFunction::Legend(side) => Some(PairKey::Legend(top(side))),
        FileFunction::Soldermask(side) => Some(PairKey::Soldermask(top(side))),
        FileFunction::Paste(side) => Some(PairKey::Paste(top(side))),
        FileFunction::Copper { .. } => copper_number(function).map(PairKey::Copper),
        _ => None,
    }
}

/// The key of the far-side partner of a layer keyed `key`, with `copper`
/// copper layers: copper Lk pairs with L(n+1-k), never with itself.
fn partner_key(key: PairKey, copper: u32) -> Option<PairKey> {
    match key {
        PairKey::Legend(top) => Some(PairKey::Legend(!top)),
        PairKey::Soldermask(top) => Some(PairKey::Soldermask(!top)),
        PairKey::Paste(top) => Some(PairKey::Paste(!top)),
        PairKey::Copper(number) => {
            let other = copper.checked_add(1)?.checked_sub(number)?;
            (other != number && other >= 1).then_some(PairKey::Copper(other))
        }
    }
}

/// `layers` with each one paired to its far-side partner: the first
/// unpaired layer of the partner key, so pairs are always mutual. Copper
/// pairs by the job file's layer number, else by the highest copper number.
pub(super) fn with_pairs(
    mut layers: Vec<PackageLayer>,
    job: Option<&JobFile>,
) -> Vec<PackageLayer> {
    let keys: Vec<Option<PairKey>> = layers
        .iter()
        .map(|layer| layer.function.as_ref().and_then(pair_key))
        .collect();
    let highest = layers
        .iter()
        .filter_map(|layer| layer.function.as_ref().and_then(copper_number))
        .max()
        .unwrap_or(0);
    let copper = job
        .and_then(|job| job.copper_layers)
        .filter(|count| *count >= 1)
        .unwrap_or(highest);
    for index in 0..layers.len() {
        if layers[index].pair.is_some() {
            continue;
        }
        let Some(wanted) = keys[index].and_then(|key| partner_key(key, copper)) else {
            continue;
        };
        let partner = (0..layers.len()).find(|&other| {
            other != index && layers[other].pair.is_none() && keys[other] == Some(wanted)
        });
        if let Some(partner) = partner {
            layers[index].pair = Some(partner);
            layers[partner].pair = Some(index);
        }
    }
    layers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copper_pairs_by_position_and_the_middle_layer_has_none() {
        assert_eq!(partner_key(PairKey::Copper(1), 4), Some(PairKey::Copper(4)));
        assert_eq!(partner_key(PairKey::Copper(2), 4), Some(PairKey::Copper(3)));
        assert_eq!(partner_key(PairKey::Copper(2), 3), None);
        assert_eq!(partner_key(PairKey::Copper(5), 4), None);
        assert_eq!(
            partner_key(PairKey::Legend(true), 4),
            Some(PairKey::Legend(false))
        );
    }

    #[test]
    fn overlays_sit_above_the_stack_and_the_stack_reads_top_down() {
        let profile = FileFunction::Profile { plated: false };
        let silk_top = FileFunction::Legend(Side::Top);
        let silk_bottom = FileFunction::Legend(Side::Bottom);
        assert!(place_in_stack(Some(&profile)) < place_in_stack(Some(&silk_top)));
        assert!(place_in_stack(None) < place_in_stack(Some(&silk_top)));
        assert!(place_in_stack(Some(&silk_top)) < place_in_stack(Some(&silk_bottom)));
    }
}
