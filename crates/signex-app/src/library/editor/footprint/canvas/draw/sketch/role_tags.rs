//! Role tags on baked loops (mockup row 190): a closed loop with a role
//! names it in a small pill at its top-left corner, in the role's colour,
//! whether the loop is selected or not. The role's colour also tints the
//! loop's fill (`fills`); courtyard takes the board's Courtyard Top colour,
//! so it no longer reads as a keepout.

use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Size};
use signex_sketch::SketchData;
use signex_types::sketch::SketchRole;

use super::fills::ClosedLoop;
use super::sketch_ink::SketchInk;
use crate::library::editor::footprint::canvas::FootprintCanvasState;
use crate::library::editor::footprint::layers::{FpLayer, LayerPalette};
use crate::library::editor::footprint::sketch_dispatch::current_role_of;
use crate::library::editor::footprint::state::FootprintEditorState;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
/// Size of a tag's text.
const TAG_TEXT: f32 = TS.caption;
/// Advance of one character, the pill's height and its side padding, in
/// text heights.
const ADVANCE_EM: f32 = 0.6;
const HEIGHT_EM: f32 = 1.5;
const PAD_EM: f32 = 0.5;
/// How far the tag sits inside the loop's top-left corner, in text heights.
const INSET_EM: f32 = 0.4;
/// Border width of a tag, in px.
const BORDER_PX: f32 = 1.0;

/// The role a loop bakes as: the first of its edges, then its corners,
/// that has one.
pub(super) fn loop_role(sketch: &SketchData, closed: &ClosedLoop) -> SketchRole {
    closed
        .lines
        .iter()
        .chain(closed.points.iter())
        .filter_map(|id| sketch.entities.iter().find(|e| e.id == *id))
        .map(current_role_of)
        .find(|role| *role != SketchRole::Unassigned)
        .unwrap_or(SketchRole::Unassigned)
}

/// The colour a loop with `role` is tinted and tagged in: its layer's
/// colour in the board palette. `None` for a role with no footprint layer
/// (the loop keeps the neutral fill).
pub(super) fn role_colour(role: SketchRole) -> Option<Color> {
    use SketchRole as R;
    let layer = match role {
        R::Courtyard => return Some(LayerPalette::Editor.courtyard_colour()),
        R::Pad | R::PourTop => FpLayer::FCu,
        R::PourBottom => FpLayer::BCu,
        R::SilkTop => FpLayer::FSilks,
        R::SilkBottom => FpLayer::BSilks,
        R::MaskOpeningTop | R::MaskExcludeTop | R::PasteApertureTop => FpLayer::FFab,
        R::MaskOpeningBottom | R::MaskExcludeBottom | R::PasteApertureBottom => FpLayer::BFab,
        R::Keepout | R::Cutout | R::Slot => FpLayer::EdgeCuts,
        R::Unassigned
        | R::BoardOutline
        | R::MountingHole
        | R::VScore
        | R::BoardPour
        | R::MechanicalLayer
        | R::ComponentAnchor => return None,
    };
    Some(layer.color())
}

/// One loop's tag: its words, its top-left corner on screen and its colour.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LoopTag<'a> {
    pub text: &'a str,
    pub at: Point,
    pub colour: Color,
}

/// The tag of `closed`, named as the host's role picker names its role;
/// `None` for a loop without a role or before the words are rendered.
pub(super) fn loop_tag<'a>(
    sketch: &SketchData,
    closed: &ClosedLoop,
    state: &'a FootprintEditorState,
    cstate: &FootprintCanvasState,
) -> Option<LoopTag<'a>> {
    let role = loop_role(sketch, closed);
    let host = state.sketch_host();
    let text = state.canvas_text.role_tag(host, role.on_host(host))?;
    let colour = role_colour(role).unwrap_or(SketchInk::board().label);
    let (left, top) = closed
        .polygon
        .iter()
        .map(|p| cstate.world_to_screen((p[0], p[1])))
        .fold((f32::INFINITY, f32::INFINITY), |(x, y), p| {
            (x.min(p.x), y.min(p.y))
        });
    if !left.is_finite() || !top.is_finite() {
        return None;
    }
    let inset = TAG_TEXT * INSET_EM;
    Some(LoopTag {
        text,
        at: Point::new(left + inset, top + inset),
        colour,
    })
}

/// The pill of `tag`: the label ground, a border and text in the role's
/// colour.
pub(super) fn draw_loop_tag(frame: &mut canvas::Frame, tag: &LoopTag<'_>) {
    let ink = SketchInk::board();
    let height = TAG_TEXT * HEIGHT_EM;
    let pad = TAG_TEXT * PAD_EM;
    let width = tag.text.chars().count() as f32 * TAG_TEXT * ADVANCE_EM + 2.0 * pad;
    let pill = Path::rounded_rectangle(
        tag.at,
        Size::new(width, height),
        iced::border::Radius::from(height / 2.0),
    );
    frame.fill(&pill, ink.pill);
    frame.stroke(
        &pill,
        Stroke::default()
            .with_width(BORDER_PX)
            .with_color(tag.colour),
    );
    frame.fill_text(canvas::Text {
        content: tag.text.to_string(),
        position: Point::new(tag.at.x + pad, tag.at.y + height / 2.0),
        color: tag.colour,
        size: TAG_TEXT.into(),
        align_y: iced::alignment::Vertical::Center,
        ..canvas::Text::default()
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use signex_library::Footprint;
    use signex_sketch::entity::{Entity, EntityKind};
    use signex_sketch::id::SketchEntityId;
    use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
    use signex_types::sketch::SketchHost;

    use super::super::fills::find_closed_loops;
    use super::*;
    use crate::library::editor::footprint::sketch_dispatch::set_entity_role;
    use crate::library::editor::footprint::state::BoardBackdrop;

    /// A 4 x 2 mm rectangle of four lines: its corners and its first line.
    fn rectangle() -> (Footprint, [SketchEntityId; 4], SketchEntityId) {
        let plane = PlaneId::new();
        let mut sketch = SketchData::default();
        sketch.planes.push(Plane {
            id: plane,
            kind: PlaneKind::BoardTop,
        });
        let corners = [(1.0, 1.0), (5.0, 1.0), (5.0, 3.0), (1.0, 3.0)].map(|(x, y)| {
            let id = SketchEntityId::new();
            sketch
                .entities
                .push(Entity::new(id, plane, EntityKind::Point { x, y }));
            id
        });
        let lines: Vec<SketchEntityId> = (0..4)
            .map(|k| {
                let id = SketchEntityId::new();
                let kind = EntityKind::Line {
                    start: corners[k],
                    end: corners[(k + 1) % 4],
                };
                sketch.entities.push(Entity::new(id, plane, kind));
                id
            })
            .collect();
        let mut fp = Footprint::empty("tags");
        fp.sketch = Some(sketch);
        (fp, corners, lines[0])
    }

    /// The editor state on `fp` for `host`, its tag words each role's name.
    fn state_for(fp: &Footprint, host: SketchHost) -> FootprintEditorState {
        let mut state = FootprintEditorState::from_footprint(fp);
        if host == SketchHost::Board {
            state.board_backdrop = Some(Arc::new(BoardBackdrop::default()));
        }
        state.canvas_text.role_tags = [SketchHost::Footprint, SketchHost::Board]
            .into_iter()
            .flat_map(|h| h.roles().iter().map(move |&r| (h, r, format!("{r:?}"))))
            .filter(|(_, r, _)| *r != SketchRole::Unassigned)
            .collect();
        state
    }

    /// The tags of every loop of `fp`, as `state` draws them.
    fn tags(fp: &Footprint, state: &FootprintEditorState) -> Vec<(String, Point)> {
        let sketch = fp.sketch.as_ref().expect("a sketch");
        let cstate = FootprintCanvasState::default();
        find_closed_loops(sketch, state)
            .iter()
            .filter_map(|closed| loop_tag(sketch, closed, state, &cstate))
            .map(|tag| (tag.text.to_string(), tag.at))
            .collect()
    }

    /// Mockup row 190: every role a loop can take draws its tag, on each
    /// host; a loop without a role draws none.
    #[test]
    fn each_role_draws_its_tag() {
        for host in [SketchHost::Footprint, SketchHost::Board] {
            let (fp, _, _) = rectangle();
            assert!(tags(&fp, &state_for(&fp, host)).is_empty(), "no role");
            for &role in host.roles() {
                if role == SketchRole::Unassigned {
                    continue;
                }
                let (mut fp, corners, line) = rectangle();
                set_entity_role(&mut fp, line, role);
                set_entity_role(&mut fp, corners[0], role);
                let state = state_for(&fp, host);
                let sketch = fp.sketch.as_ref().expect("a sketch");
                let took = sketch
                    .entities
                    .iter()
                    .any(|e| current_role_of(e).on_host(host) == role);
                if !took {
                    // A mounting hole takes only a circle, never a loop edge.
                    assert_eq!(role, SketchRole::MountingHole, "{role:?} on {host:?}");
                    continue;
                }
                let drawn = tags(&fp, &state);
                assert_eq!(drawn.len(), 1, "{role:?} on {host:?}: {drawn:?}");
                assert_eq!(drawn[0].0, format!("{role:?}"), "on {host:?}");
            }
        }
    }

    /// A selected loop keeps its tag, in the same place.
    #[test]
    fn a_selected_loop_keeps_its_tag() {
        let (mut fp, corners, line) = rectangle();
        set_entity_role(&mut fp, line, SketchRole::Courtyard);
        let mut state = state_for(&fp, SketchHost::Footprint);
        let idle = tags(&fp, &state);
        assert_eq!(idle.len(), 1);

        state.selected_sketch = Some(line);
        state.selected_sketch_secondary = Some(corners[1]);
        assert_eq!(tags(&fp, &state), idle);
    }

    /// The tag sits inside the loop's top-left corner.
    #[test]
    fn the_tag_sits_at_the_top_left_corner() {
        let (mut fp, _, line) = rectangle();
        set_entity_role(&mut fp, line, SketchRole::Keepout);
        let state = state_for(&fp, SketchHost::Footprint);
        let cstate = FootprintCanvasState::default();
        let corner = cstate.world_to_screen((1.0, 1.0));
        let drawn = tags(&fp, &state);
        assert_eq!(drawn.len(), 1);
        assert!(
            drawn[0].1.x > corner.x && drawn[0].1.y > corner.y,
            "{drawn:?}"
        );
    }

    /// Mockup row 190: courtyard has its own colour, not the Keep-Out one.
    #[test]
    fn courtyard_and_keepout_colours_differ() {
        let courtyard = role_colour(SketchRole::Courtyard).expect("a colour");
        let keepout = role_colour(SketchRole::Keepout).expect("a colour");
        assert_ne!(courtyard, keepout);
        assert_eq!(courtyard, LayerPalette::Editor.courtyard_colour());
    }
}
