//! Pattern links (mockup row 191): every instance of a pattern as a placement of its
//! innermost source entity, so a pattern of a pattern repeats each instance
//! of its source, and a live mirror reflects each instance of its pattern.
//!
//! Every pattern bakes through this one walk, plain or linked, and the
//! canvas's instance rings (`slots.rs`) read it too; an array's own layout
//! is laid out in `layout.rs`.

use std::collections::BTreeMap;

use signex_expr::ExprNode;
use signex_library::primitive::footprint::Pad as LibPad;
use signex_sketch::SketchError;
use signex_sketch::array::{Array, ArrayId, ArrayKind, NumberingScheme};
use signex_sketch::id::SketchEntityId;
use signex_sketch::sketch::SketchData;
use signex_sketch::solver::FullSolveOutput;
use signex_sketch::solver::state::point_xy;

use super::overrides::SourcePad;
use super::place::Place;

/// How a pattern numbers its instances: its scheme, and the row width when
/// it numbers row-major like a grid.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Basis<'a> {
    pub numbering: &'a NumberingScheme,
    pub nx: Option<usize>,
    /// The rows of a plain grid, for its sweep; `None` for a pattern of a
    /// pattern, which numbers row by row.
    pub ny: Option<usize>,
}

/// One instance of a pattern.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Instance {
    /// The key a per-instance override is stored under: `(array, i, j)` of
    /// a plain array, `(array, outer slot, inner instance)` of a linked one,
    /// `(mirror, mirrored instance, 0)` of a mirror (both indices numbered
    /// as [`Self::flat`]).
    pub key: (ArrayId, u32, u32),
    /// The instance's index in its pattern's numbering.
    pub flat: u64,
    pub place: Place,
    /// The `(i, j)` the pad expressions see: the innermost array's.
    pub index: (usize, usize),
    /// Whether this pattern bakes the instance itself. The outer slot 0 of
    /// a linked array is its source's instances, which the source bakes.
    pub own: bool,
}

/// Every instance of one pattern.
pub(crate) struct Expansion<'a> {
    /// The instance slots of the numbering, suppressed ones included.
    pub slots: u64,
    pub basis: Basis<'a>,
    pub instances: Vec<Instance>,
}

/// What an expansion reads, and where it reports.
pub(crate) struct Walk<'a, 'w> {
    pub sketch: &'a SketchData,
    pub solve: &'a FullSolveOutput,
    pub params_ast: &'a BTreeMap<String, ExprNode>,
    pub warnings: &'w mut Vec<String>,
    /// The patterns being expanded, outermost first: a pattern met again is
    /// a loop (only a damaged file has one).
    stack: Vec<ArrayId>,
    /// Keep the slots the suppression list holds back, marked `listed`
    /// ([`super::slots`]); the bake drops them.
    pub(super) keep_listed: bool,
    /// A pattern was skipped for going over [`super::MAX_ARRAY_INSTANCES`].
    over_cap: bool,
}

impl<'a, 'w> Walk<'a, 'w> {
    pub(crate) fn new(
        sketch: &'a SketchData,
        solve: &'a FullSolveOutput,
        params_ast: &'a BTreeMap<String, ExprNode>,
        warnings: &'w mut Vec<String>,
    ) -> Self {
        Self {
            sketch,
            solve,
            params_ast,
            warnings,
            stack: Vec::new(),
            keep_listed: false,
            over_cap: false,
        }
    }

    /// This walk, keeping the slots the suppression list holds back.
    pub(crate) fn keeping_listed(mut self) -> Self {
        self.keep_listed = true;
        self
    }

    /// Whether a pattern this walk met was skipped for going over the
    /// instance cap, so a board bake can refuse it as the cap.
    pub(crate) fn went_over_cap(&self) -> bool {
        self.over_cap
    }

    /// [`super::within_instance_cap`], remembering a refusal.
    pub(super) fn within_cap(
        &mut self,
        label: &str,
        source: SketchEntityId,
        n: Option<usize>,
    ) -> bool {
        let within = super::within_instance_cap(label, source, n, self.warnings);
        self.over_cap |= !within;
        within
    }

    pub(super) fn xy(&self, id: SketchEntityId) -> Option<(f64, f64)> {
        point_xy(
            id,
            &self.solve.result.state,
            &self.solve.result.index,
            self.sketch,
        )
    }

    /// Every instance of pattern `id` (an array or a mirror). `None`, with
    /// a warning, when it cannot expand: a loop, a missing line or
    /// pattern, a count that does not evaluate or is over the cap.
    pub(crate) fn expand(&mut self, id: ArrayId) -> Result<Option<Expansion<'a>>, SketchError> {
        if self.stack.contains(&id) {
            self.warnings.push(format!(
                "pattern {}: its source chain loops back to itself — pattern skipped",
                id.0
            ));
            return Ok(None);
        }
        self.stack.push(id);
        let expansion = if let Some(mirror) = self.sketch.pattern_mirror(id).copied() {
            self.expand_mirror(mirror.id, mirror.pattern, mirror.line)
        } else if let Some(array) = self.sketch.arrays.iter().find(|a| a.id == id) {
            self.expand_array(array)
        } else {
            self.warnings.push(format!(
                "pattern {}: not in the sketch — link skipped",
                id.0
            ));
            Ok(None)
        };
        self.stack.pop();
        expansion
    }

    fn expand_mirror(
        &mut self,
        id: ArrayId,
        pattern: ArrayId,
        line: SketchEntityId,
    ) -> Result<Option<Expansion<'a>>, SketchError> {
        let ends = self
            .sketch
            .entities
            .iter()
            .find(|e| e.id == line)
            .and_then(|e| match e.kind {
                signex_sketch::entity::EntityKind::Line { start, end } => {
                    Some((self.xy(start)?, self.xy(end)?))
                }
                _ => None,
            });
        let Some(reflect) = ends.and_then(|(a, b)| Place::reflection(a, b)) else {
            self.warnings.push(format!(
                "pattern mirror {}: its line is missing or has no length — mirror skipped",
                id.0
            ));
            return Ok(None);
        };
        let Some(inner) = self.expand(pattern)? else {
            return Ok(None);
        };
        let instances = inner
            .instances
            .iter()
            .filter_map(|x| {
                Some(Instance {
                    key: (id, u32::try_from(x.flat).ok()?, 0),
                    flat: inner.slots + x.flat,
                    place: reflect.after(&x.place),
                    index: x.index,
                    own: true,
                })
            })
            .collect();
        Ok(Some(Expansion {
            slots: inner.slots.saturating_mul(2),
            basis: inner.basis,
            instances,
        }))
    }

    fn expand_array(&mut self, array: &'a Array) -> Result<Option<Expansion<'a>>, SketchError> {
        let source = array.kind.source();
        let Some(mut walked) = self.walk_slots(array)? else {
            return Ok(None);
        };
        walked.slots.retain(|s| !s.listed);
        let linked = self
            .sketch
            .pattern_source_of(array.id)
            .filter(|s| self.sketch.is_pattern(*s));
        let Some(inner_id) = linked else {
            let instances = walked
                .slots
                .iter()
                .map(|s| Instance {
                    key: (array.id, s.i as u32, s.j as u32),
                    flat: s.flat,
                    place: s.place,
                    index: (s.i, s.j),
                    own: true,
                })
                .collect();
            return Ok(Some(Expansion {
                slots: walked.count,
                basis: Basis {
                    numbering: &array.numbering,
                    nx: walked.nx,
                    ny: walked
                        .nx
                        .and_then(|nx| usize::try_from(walked.count).ok()?.checked_div(nx)),
                },
                instances,
            }));
        };
        let Some(inner) = self.expand(inner_id)? else {
            return Ok(None);
        };
        let total = walked.slots.len().checked_mul(inner.instances.len());
        if !self.within_cap("linked", source, total) {
            return Ok(None);
        }
        let mut instances = Vec::with_capacity(total.unwrap_or(0));
        for s in &walked.slots {
            for x in &inner.instances {
                let (Ok(i), Ok(j)) = (u32::try_from(s.flat), u32::try_from(x.flat)) else {
                    continue;
                };
                instances.push(Instance {
                    key: (array.id, i, j),
                    flat: s.flat * inner.slots + x.flat,
                    place: s.place.after(&x.place),
                    index: x.index,
                    own: s.flat != 0,
                });
            }
        }
        // A grid of a pattern numbers row by row, each row now `nx` times
        // as wide as the inner pattern has slots.
        let nx = walked
            .nx
            .and_then(|nx| usize::try_from(inner.slots).ok()?.checked_mul(nx));
        Ok(Some(Expansion {
            slots: walked.count.saturating_mul(inner.slots),
            basis: Basis {
                numbering: &array.numbering,
                nx,
                ny: None,
            },
            instances,
        }))
    }
}

/// How a bake warning names a plain array: by its kind.
fn kind_label(kind: &ArrayKind) -> &'static str {
    match kind {
        ArrayKind::Linear { .. } => "linear array",
        ArrayKind::Grid { .. } => "grid array",
        ArrayKind::Polar { .. } => "polar array",
    }
}

/// Bake the instances pattern `id` places itself into `out`. A plain array
/// (one that repeats no other pattern) warns by its kind and, as a lone pad
/// does, refuses a source with no position; a linked pattern or a mirror
/// warns and is skipped.
pub(crate) fn bake_pattern(
    id: ArrayId,
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_ast: &BTreeMap<String, ExprNode>,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    let Some(source) = sketch.innermost_source(id) else {
        warnings.push(format!(
            "pattern {}: its source chain loops or ends at a missing pattern — pattern skipped",
            id.0
        ));
        return Ok(());
    };
    let plain = sketch
        .pattern_input(id)
        .is_none()
        .then(|| sketch.arrays.iter().find(|a| a.id == id))
        .flatten();
    let label = plain.map_or("pattern", |a| kind_label(&a.kind));
    let Some(entity) = sketch.entities.iter().find(|e| e.id == source) else {
        warnings.push(match plain {
            Some(_) => format!("{label} source {source}: entity not found — array skipped"),
            None => super::not_a_pad(label, sketch, solve, source),
        });
        return Ok(());
    };
    let Some(pad) = entity.pad.as_ref() else {
        warnings.push(super::not_a_pad(label, sketch, solve, source));
        return Ok(());
    };
    let Some(expansion) = Walk::new(sketch, solve, params_ast, warnings).expand(id)? else {
        return Ok(());
    };
    let Some(at) = point_xy(source, &solve.result.state, &solve.result.index, sketch) else {
        if plain.is_none() {
            warnings.push(format!(
                "pattern source {source}: position unknown — pattern skipped"
            ));
            return Ok(());
        }
        if expansion.instances.iter().any(|x| x.own) {
            return Err(SketchError::EntityNotFound(source));
        }
        return Ok(());
    };
    let source = SourcePad {
        id: source,
        pad,
        at,
    };
    super::overrides::bake_instances(
        &source, &expansion, sketch, solve, params_ast, out, warnings,
    )
}

/// Bake every live mirror's instances into `out`.
pub(crate) fn bake_mirrors(
    sketch: &SketchData,
    solve: &FullSolveOutput,
    params_ast: &BTreeMap<String, ExprNode>,
    out: &mut Vec<LibPad>,
    warnings: &mut Vec<String>,
) -> Result<(), SketchError> {
    for mirror in &sketch.pattern_mirrors {
        bake_pattern(mirror.id, sketch, solve, params_ast, out, warnings)?;
    }
    Ok(())
}
