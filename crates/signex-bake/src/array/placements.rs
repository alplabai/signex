//! Where the patterns of one source entity place it, for a bake that writes
//! something other than pads (a board's mounting holes, mockup row 179).
//! Rules: `docs/PCB_SKETCH.md`, "Patterned mounting holes".

use std::collections::{BTreeMap, HashSet};

use signex_expr::ExprNode;
use signex_sketch::array::ArrayId;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use uuid::{Builder, Uuid};

use super::instance_at::SAME_PLACE_MM;
use super::links::Walk;

const FNV_OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
const FNV_PRIME: u128 = (1 << 88) + 0x13b;

/// One place a pattern puts its source.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SourcePlace {
    pub(crate) at: (f64, f64),
    key: (ArrayId, u32, u32),
    is_source: bool,
}

impl SourcePlace {
    /// The item's uuid: the source's own at the source's place, else derived
    /// from the source and the instance, so an unchanged sketch re-bakes
    /// equal.
    pub(crate) fn uuid(&self, source: Uuid) -> Uuid {
        if self.is_source {
            return source;
        }
        let (pattern, i, j) = self.key;
        let hash = source
            .as_bytes()
            .iter()
            .chain(pattern.0.as_bytes())
            .chain(&i.to_le_bytes())
            .chain(&j.to_le_bytes())
            .fold(FNV_OFFSET, |hash, &byte| {
                (hash ^ u128::from(byte)).wrapping_mul(FNV_PRIME)
            });
        Builder::from_custom_bytes(hash.to_be_bytes()).into_uuid()
    }
}

/// Every place the patterns repeating any of `sources` put the point at
/// `at`, one per distinct position. `None` when no pattern repeats them.
/// `Err` says why a pattern does not expand: a caller that must not drop an
/// instance refuses on it.
pub(crate) fn source_places(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_ast: &BTreeMap<String, ExprNode>,
    sources: &[SketchEntityId],
    at: (f64, f64),
) -> Result<Option<Vec<SourcePlace>>, String> {
    let patterns: Vec<ArrayId> = sketch
        .arrays
        .iter()
        .map(|a| a.id)
        .chain(sketch.pattern_mirrors.iter().map(|m| m.id))
        .filter(|&id| repeats(sketch, id, sources))
        .collect();
    if patterns.is_empty() {
        return Ok(None);
    }
    let mut seen = HashSet::new();
    let mut places = Vec::new();
    for id in patterns {
        let mut warnings = Vec::new();
        let expansion = Walk::new(sketch, solve, params_ast, &mut warnings)
            .expand(id)
            .map_err(|error| error.to_string())?;
        let Some(expansion) = expansion else {
            return Err(warnings
                .pop()
                .unwrap_or_else(|| format!("pattern {} does not expand", id.0)));
        };
        for instance in expansion.instances.iter().filter(|x| x.own) {
            let place = instance.place.apply(at);
            if seen.insert(place_key(place)) {
                places.push(SourcePlace {
                    at: place,
                    key: instance.key,
                    is_source: place_key(place) == place_key(at),
                });
            }
        }
    }
    Ok(Some(places))
}

/// Whether pattern `id`, or a pattern it is built from, repeats one of
/// `sources`. Reads a looped chain too, so a damaged file is refused rather
/// than its pattern passed over.
fn repeats(sketch: &SketchData, id: ArrayId, sources: &[SketchEntityId]) -> bool {
    let mut seen = HashSet::new();
    let mut at = Some(id);
    while let Some(pattern) = at {
        if !seen.insert(pattern) {
            return false;
        }
        let array = sketch.arrays.iter().find(|a| a.id == pattern);
        if array.is_some_and(|a| sources.contains(&a.kind.source())) {
            return true;
        }
        at = sketch.pattern_input(pattern);
    }
    false
}

fn place_key(p: (f64, f64)) -> (i64, i64) {
    (
        (p.0 / SAME_PLACE_MM).round() as i64,
        (p.1 / SAME_PLACE_MM).round() as i64,
    )
}
