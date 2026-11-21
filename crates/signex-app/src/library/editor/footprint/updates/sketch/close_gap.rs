//! Close gap (mockup row 190), on both hosts: join two open profile ends within the
//! heal tolerance, as one undo step, solved and baked like every sketch
//! edit. The joining is `signex_sketch::heal::close_gap`; a gap it refuses
//! (too wide, an end gone since the offer was drawn) changes nothing and is
//! said in Messages. Rules: `docs/FOOTPRINT_SKETCH.md` and
//! `docs/PCB_SKETCH.md`, "Close gap".

use signex_sketch::heal;
use signex_sketch::id::SketchEntityId;

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_conflict::mutate_solve_or_revert;
use crate::library::editor::footprint::sketch_dispatch::report_solve_failure;

pub(in crate::library::editor::footprint::updates) fn apply(
    editor: &mut FootprintDocument,
    keep: SketchEntityId,
    merge: SketchEntityId,
) {
    editor.record_edit(None, |editor| {
        let before = editor.primitive().sketch.clone();
        let joined = editor.with_parts(|state, fp| {
            let (Some(solve), Some(sketch)) = (state.last_solve.as_ref(), fp.sketch.as_ref())
            else {
                return Err(None);
            };
            let mut next = sketch.clone();
            heal::close_gap(&mut next, solve, keep, merge)
                .map(|_| next)
                .map_err(Some)
        });
        let next = match joined {
            Ok(next) => next,
            Err(refusal) => {
                if let Some(refusal) = refusal {
                    crate::diagnostics::log_warning(format!("Close gap: {refusal}"));
                }
                return false;
            }
        };
        let solved = editor
            .with_parts(|state, fp| mutate_solve_or_revert(state, fp, |fp| fp.sketch = Some(next)));
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
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use signex_library::{Footprint, FootprintFile};
    use signex_sketch::Solver;
    use signex_sketch::attr::CourtyardAttr;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::parameter::resolve;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_sketch::sketch::SketchData;

    use super::*;
    use crate::library::editor::footprint::updates::apply_footprint_primitive_edit;
    use crate::library::messages::FootprintEditorMsg;

    /// A footprint whose courtyard square stops `gap` short of its first
    /// corner, solved once; returns the editor and the two open ends.
    fn open_courtyard(gap: f64) -> (FootprintDocument, SketchEntityId, SketchEntityId) {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let mut add = |kind| {
            let id = SketchEntityId::new();
            sketch.entities.push(Entity::new(id, plane, kind));
            id
        };
        let corners = [(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 5.0), (0.0, gap)];
        let p: Vec<SketchEntityId> = corners
            .iter()
            .map(|&(x, y)| add(EntityKind::Point { x, y }))
            .collect();
        for pair in p.windows(2) {
            add(EntityKind::Line {
                start: pair[0],
                end: pair[1],
            });
        }
        for entity in &mut sketch.entities {
            if matches!(entity.kind, EntityKind::Line { .. }) {
                entity.courtyard = Some(CourtyardAttr {});
            }
        }
        let params = resolve(&sketch.parameters).unwrap();
        let solve = Solver::default().solve(&sketch, &params).unwrap();
        let mut fp = Footprint::empty("t");
        fp.sketch = Some(sketch);
        let mut editor =
            FootprintDocument::new(PathBuf::from("t.snxfpt"), FootprintFile::from_footprint(fp));
        editor.state.last_solve = Some(solve);
        (editor, p[0], p[4])
    }

    fn close(editor: &mut FootprintDocument, keep: SketchEntityId, merge: SketchEntityId) {
        apply_footprint_primitive_edit(
            editor,
            FootprintEditorMsg::SketchCloseGapRequested { keep, merge },
        );
    }

    #[test]
    fn a_footprint_gap_within_the_tolerance_is_offered_and_closes_in_one_undo_step() {
        let (mut editor, a, b) = open_courtyard(0.03);
        let offer = editor
            .state
            .gap_offer(editor.primitive().sketch.as_ref())
            .expect("an open courtyard offers Close gap");
        assert!(offer.closes());
        assert!(
            (offer.keep == a && offer.merge == b) || (offer.keep == b && offer.merge == a),
            "{offer:?}"
        );
        let before = editor.primitive().sketch.clone();

        close(&mut editor, offer.keep, offer.merge);

        let sketch = editor.primitive().sketch.as_ref().unwrap();
        assert!(sketch.entities.iter().all(|e| e.id != offer.merge));
        assert_eq!(editor.state.gap_offer(Some(sketch)), None, "closed");
        assert!(editor.undo(), "one undo step");
        assert_eq!(editor.primitive().sketch, before);
        assert!(!editor.history.can_undo(), "and only one");
    }

    #[test]
    fn a_gap_wider_than_the_tolerance_is_offered_no_close_and_refused() {
        let (mut editor, a, b) = open_courtyard(0.2);
        let offer = editor
            .state
            .gap_offer(editor.primitive().sketch.as_ref())
            .expect("the gap is still named");
        assert!(!offer.closes());
        let text = signex_i18n::RenderTranslatable::render_en_us(&offer.text())
            .replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(text, "Gap 0.20 mm is wider than the 0.05 mm heal tolerance");
        let before = editor.primitive().sketch.clone();

        close(&mut editor, a, b);

        assert_eq!(editor.primitive().sketch, before, "nothing joined");
        assert!(!editor.history.can_undo(), "no undo step");
    }

    #[test]
    fn the_offer_names_the_gap_and_the_tolerance() {
        let (editor, _, _) = open_courtyard(0.03);
        let offer = editor
            .state
            .gap_offer(editor.primitive().sketch.as_ref())
            .unwrap();
        let text = signex_i18n::RenderTranslatable::render_en_us(&offer.text())
            .replace(['\u{2068}', '\u{2069}'], "");
        assert_eq!(text, "Gap 0.03 mm, closes under the 0.05 mm heal tolerance");
    }
}
