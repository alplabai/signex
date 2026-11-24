//! Where the patterns of one source entity place it, for a bake that writes
//! something other than pads (the board's items, mockup row 179). Rules:
//! `docs/PCB_SKETCH.md`, "Patterned board items".

use std::cell::Cell;
use std::collections::{BTreeMap, HashSet};

use signex_expr::ExprNode;
use signex_sketch::array::ArrayId;
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_types::sketch::pattern_instance::pattern_instance_uuid;
use uuid::Uuid;

use super::MAX_ARRAY_INSTANCES;
use super::instance_at::SAME_PLACE_MM;
use super::links::{Place, Walk};

/// The pattern instances one whole bake may still write.
#[derive(Debug)]
pub(crate) struct InstanceBudget(Cell<usize>);

impl Default for InstanceBudget {
    fn default() -> Self {
        Self(Cell::new(MAX_ARRAY_INSTANCES))
    }
}

impl InstanceBudget {
    /// Spend `count` instances; `false`, spending nothing, when too few are
    /// left.
    fn take(&self, count: usize) -> bool {
        let left = self.0.get();
        if count > left {
            return false;
        }
        self.0.set(left - count);
        true
    }
}

/// Why a source's patterns gave no places.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PlacesRefused {
    /// A pattern does not expand, and this says why.
    DoesNotExpand(String),
    /// The bake's [`InstanceBudget`] ran out.
    OverCap,
}

/// One place a pattern puts its source.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SourcePlace {
    place: Place,
    key: (ArrayId, u32, u32),
    is_source: bool,
}

impl SourcePlace {
    /// Where this place puts the source point `p`.
    pub(crate) fn apply(&self, p: (f64, f64)) -> (f64, f64) {
        self.place.apply(p)
    }

    /// Whether this place reflects the source.
    pub(crate) fn mirrors(&self) -> bool {
        self.place.mirrors()
    }

    /// The item's uuid: the source's own at the source's place, else derived
    /// from the source and the instance, so an unchanged sketch re-bakes
    /// equal.
    pub(crate) fn uuid(&self, source: Uuid) -> Uuid {
        if self.is_source {
            return source;
        }
        let (pattern, i, j) = self.key;
        pattern_instance_uuid(source, pattern, i, j)
    }
}

/// Whether any pattern repeats one of `sources`.
pub(crate) fn is_patterned(sketch: &SketchData, sources: &[SketchEntityId]) -> bool {
    patterns_of(sketch, sources).next().is_some()
}

fn patterns_of<'a>(
    sketch: &'a SketchData,
    sources: &'a [SketchEntityId],
) -> impl Iterator<Item = ArrayId> + 'a {
    sketch
        .arrays
        .iter()
        .map(|a| a.id)
        .chain(sketch.pattern_mirrors.iter().map(|m| m.id))
        .filter(move |&id| repeats(sketch, id, sources))
}

/// Every place the patterns repeating any of `sources` put the item drawn
/// through the points `shape`, one per distinct image of the whole shape, so
/// a turn about one of its own points is not taken for the source. `None`
/// when no pattern repeats them. Every instance is charged to `budget`.
pub(crate) fn source_places(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_ast: &BTreeMap<String, ExprNode>,
    sources: &[SketchEntityId],
    shape: &[(f64, f64)],
    budget: &InstanceBudget,
) -> Result<Option<Vec<SourcePlace>>, PlacesRefused> {
    let patterns: Vec<ArrayId> = patterns_of(sketch, sources).collect();
    if patterns.is_empty() {
        return Ok(None);
    }
    let source_key = image_key(shape.iter().copied());
    let mut seen = HashSet::new();
    let mut places = Vec::new();
    for id in patterns {
        let mut warnings = Vec::new();
        let expansion = Walk::new(sketch, solve, params_ast, &mut warnings)
            .expand(id)
            .map_err(|error| PlacesRefused::DoesNotExpand(error.to_string()))?;
        let Some(expansion) = expansion else {
            return Err(PlacesRefused::DoesNotExpand(
                warnings
                    .pop()
                    .unwrap_or_else(|| format!("pattern {} does not expand", id.0)),
            ));
        };
        let own = expansion.instances.iter().filter(|x| x.own);
        if !budget.take(own.clone().count()) {
            return Err(PlacesRefused::OverCap);
        }
        for instance in own {
            let image = image_key(shape.iter().map(|&p| instance.place.apply(p)));
            let is_source = image == source_key;
            if seen.insert(image) {
                places.push(SourcePlace {
                    place: instance.place,
                    key: instance.key,
                    is_source,
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

/// The points of one image, in no order: a shape landing on itself is one
/// place however its vertices were carried round.
fn image_key(points: impl Iterator<Item = (f64, f64)>) -> Vec<(i64, i64)> {
    let mut key: Vec<(i64, i64)> = points.map(place_key).collect();
    key.sort_unstable();
    key
}

fn place_key(p: (f64, f64)) -> (i64, i64) {
    (
        (p.0 / SAME_PLACE_MM).round() as i64,
        (p.1 / SAME_PLACE_MM).round() as i64,
    )
}
