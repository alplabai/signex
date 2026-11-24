//! Land imported geometry in a sketch as one undo step (mockup row 179c),
//! solved and baked like every sketch edit. A result the solver rejects is
//! reverted and leaves no step.

use signex_sketch::SketchData;
use signex_sketch::import::SketchImport;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::sketch::SketchRole;

use super::sketch_conflict::mutate_solve_or_revert;
use super::sketch_dispatch::{report_solve_failure, set_entity_role};
use crate::app::FootprintDocument;

/// Add `import`, built on `plane`, with each edge's role from `role_of` (its
/// piece's tag). Whether the sketch changed.
pub fn land_sketch_import(
    editor: &mut FootprintDocument,
    import: SketchImport,
    plane: PlaneId,
    role_of: impl Fn(usize) -> SketchRole,
) -> bool {
    let landed = editor.record_edit(None, |editor| {
        let before = editor.primitive().sketch.clone();
        let solved = editor.with_parts(|state, fp| {
            mutate_solve_or_revert(state, fp, |fp| {
                let sketch = fp.sketch.get_or_insert_with(SketchData::default);
                if !sketch.planes.iter().any(|p| p.id == plane) {
                    sketch.planes.push(Plane {
                        id: plane,
                        kind: PlaneKind::BoardTop,
                    });
                }
                sketch.entities.extend(import.entities);
                sketch.constraints.extend(import.constraints);
                for (id, tag) in import.edges {
                    set_entity_role(fp, id, role_of(tag));
                }
            })
        });
        if let Err(error) = &solved {
            editor.with_parts(|state, _| report_solve_failure(state, error));
        }
        let changed = editor.primitive().sketch != before;
        if changed {
            editor.mark_dirty();
        }
        changed
    });
    editor.canvas_cache.clear();
    landed
}
