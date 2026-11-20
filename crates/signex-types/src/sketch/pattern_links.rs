//! Live pattern links (#191): a pattern that repeats another pattern, and
//! a mirror of a pattern kept as a pattern object of its own. Data and the
//! cycle guard only; `signex-bake` expands them (`array::links`). See
//! docs/FOOTPRINT_SKETCH.md, "Pattern links".

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::sketch::array::{ArrayId, ArrayKind};
use crate::sketch::data::SketchData;
use crate::sketch::id::SketchEntityId;

/// Pattern `array` repeats every instance of pattern `source` (an array or
/// a mirror) instead of only its own source entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternSource {
    pub array: ArrayId,
    pub source: ArrayId,
}

/// A live mirror of every instance of `pattern` about the Line `line`. It
/// is a pattern object: another pattern or mirror may take it as source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatternMirror {
    pub id: ArrayId,
    pub pattern: ArrayId,
    pub line: SketchEntityId,
}

/// A link refused because `pattern` would repeat itself through `through`,
/// directly or along a chain of patterns and mirrors (or `through` already
/// sits on a loop, as in a damaged file).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("pattern {} would repeat itself through pattern {}", .pattern.0, .through.0)]
pub struct PatternCycle {
    pub pattern: ArrayId,
    pub through: ArrayId,
}

impl SketchData {
    /// Whether `id` names an array or a pattern mirror of this sketch.
    pub fn is_pattern(&self, id: ArrayId) -> bool {
        self.arrays.iter().any(|a| a.id == id) || self.pattern_mirrors.iter().any(|m| m.id == id)
    }

    /// The pattern `array` repeats, when it repeats one.
    pub fn pattern_source_of(&self, array: ArrayId) -> Option<ArrayId> {
        self.pattern_sources
            .iter()
            .find(|l| l.array == array)
            .map(|l| l.source)
    }

    /// The mirror with id `id`.
    pub fn pattern_mirror(&self, id: ArrayId) -> Option<&PatternMirror> {
        self.pattern_mirrors.iter().find(|m| m.id == id)
    }

    /// The one pattern `id` is built from: an array's linked source, a
    /// mirror's pattern. `None` for a plain array.
    pub fn pattern_input(&self, id: ArrayId) -> Option<ArrayId> {
        self.pattern_mirror(id)
            .map(|m| m.pattern)
            .or_else(|| self.pattern_source_of(id))
    }

    /// Whether following inputs from `from` reaches `target` (`from`
    /// itself included). Stops at a loop, so a damaged file cannot hang it.
    pub fn pattern_reaches(&self, from: ArrayId, target: ArrayId) -> bool {
        let mut seen = HashSet::new();
        let mut at = Some(from);
        while let Some(id) = at {
            if id == target {
                return true;
            }
            if !seen.insert(id) {
                return false;
            }
            at = self.pattern_input(id);
        }
        false
    }

    /// Whether the chain of inputs from `from` runs into a loop.
    pub fn pattern_chain_loops(&self, from: ArrayId) -> bool {
        let mut seen = HashSet::new();
        let mut at = Some(from);
        while let Some(id) = at {
            if !seen.insert(id) {
                return true;
            }
            at = self.pattern_input(id);
        }
        false
    }

    /// The entity every instance of pattern `id` is ultimately repeated
    /// from: the source of the plain array at the end of its chain. `None`
    /// when the chain loops or ends at a pattern the sketch lacks.
    pub fn innermost_source(&self, id: ArrayId) -> Option<SketchEntityId> {
        if self.pattern_chain_loops(id) {
            return None;
        }
        let mut at = id;
        while let Some(next) = self.pattern_input(at) {
            at = next;
        }
        self.arrays
            .iter()
            .find(|a| a.id == at)
            .map(|a| a.kind.source())
    }

    /// Make `array` repeat every instance of `source`, and point its own
    /// source entity at the innermost source so entity references and the
    /// delete cascade keep working. Refuses a self-reference and any link
    /// that would close a loop through sources and mirrors.
    pub fn set_pattern_source(
        &mut self,
        array: ArrayId,
        source: ArrayId,
    ) -> Result<(), PatternCycle> {
        self.refuse_loop(array, source)?;
        self.pattern_sources.retain(|l| l.array != array);
        self.pattern_sources.push(PatternSource { array, source });
        if let Some(entity) = self.innermost_source(source)
            && let Some(a) = self.arrays.iter_mut().find(|a| a.id == array)
        {
            set_kind_source(&mut a.kind, entity);
        }
        Ok(())
    }

    /// Drop `array`'s link; it repeats its source entity again. Returns
    /// whether it had one.
    pub fn clear_pattern_source(&mut self, array: ArrayId) -> bool {
        let before = self.pattern_sources.len();
        self.pattern_sources.retain(|l| l.array != array);
        self.pattern_sources.len() != before
    }

    /// Add (or replace, by id) a live mirror. Refuses a mirror of itself
    /// and one that would close a loop.
    pub fn add_pattern_mirror(&mut self, mirror: PatternMirror) -> Result<(), PatternCycle> {
        self.refuse_loop(mirror.id, mirror.pattern)?;
        self.pattern_mirrors.retain(|m| m.id != mirror.id);
        self.pattern_mirrors.push(mirror);
        Ok(())
    }

    /// Remove the mirror `id`. Returns whether there was one.
    pub fn remove_pattern_mirror(&mut self, id: ArrayId) -> bool {
        let before = self.pattern_mirrors.len();
        self.pattern_mirrors.retain(|m| m.id != id);
        self.pattern_mirrors.len() != before
    }

    fn refuse_loop(&self, pattern: ArrayId, through: ArrayId) -> Result<(), PatternCycle> {
        if self.pattern_reaches(through, pattern) || self.pattern_chain_loops(through) {
            return Err(PatternCycle { pattern, through });
        }
        Ok(())
    }

    /// Drop every link whose ends are gone: mirrors whose pattern or line
    /// is gone (and mirrors of those, in turn), links whose array or source
    /// is gone (the dependent array stays), overrides of a gone pattern and
    /// driven ids of a gone constraint. Returns the mirrors removed.
    pub fn prune_pattern_refs(&mut self) -> Vec<ArrayId> {
        let entities: HashSet<SketchEntityId> = self.entities.iter().map(|e| e.id).collect();
        let mut removed = Vec::new();
        loop {
            let patterns: HashSet<ArrayId> = self
                .arrays
                .iter()
                .map(|a| a.id)
                .chain(self.pattern_mirrors.iter().map(|m| m.id))
                .collect();
            let before = removed.len();
            self.pattern_mirrors.retain(|m| {
                let kept = patterns.contains(&m.pattern) && entities.contains(&m.line);
                if !kept {
                    removed.push(m.id);
                }
                kept
            });
            if removed.len() == before {
                break;
            }
        }
        let arrays: HashSet<ArrayId> = self.arrays.iter().map(|a| a.id).collect();
        let patterns: HashSet<ArrayId> = arrays
            .iter()
            .copied()
            .chain(self.pattern_mirrors.iter().map(|m| m.id))
            .collect();
        self.pattern_sources
            .retain(|l| arrays.contains(&l.array) && patterns.contains(&l.source));
        self.pad_overrides.retain(|o| patterns.contains(&o.array));
        let constraints: HashSet<_> = self.constraints.iter().map(|c| c.id).collect();
        self.driven_dimensions.retain(|id| constraints.contains(id));
        removed
    }
}

/// Point `kind`'s source entity at `entity`.
fn set_kind_source(kind: &mut ArrayKind, entity: SketchEntityId) {
    match kind {
        ArrayKind::Linear { source, .. }
        | ArrayKind::Grid { source, .. }
        | ArrayKind::Polar { source, .. } => *source = entity,
    }
}
