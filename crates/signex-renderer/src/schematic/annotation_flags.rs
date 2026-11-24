//! Annotation flags (mockup row 165): a pole standing on a part's top-right
//! corner with a notched pennant at its top that holds how many annotations
//! of one kind name the part, in that kind's colour. Several kinds stand side
//! by side. Lowered to the overlay line and polygon buckets and one text per
//! flag, so the flags follow the camera and never reach a print or an export.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use signex_types::schematic::{HAlign, VAlign};

use super::{OverlayInputs, OverlayLineInput, OverlayPolygonInput, SchematicSnapshot, TextInput};

/// One flag: where its pole stands and what its pennant says.
#[derive(Clone, Debug, PartialEq)]
pub struct AnnotationFlagInput {
    /// The pole's foot, world mm (y grows downward).
    pub foot: [f32; 2],
    /// The number on the pennant.
    pub count: u32,
    /// The kind's colour: pole and pennant.
    pub color: [f32; 4],
    /// The number's colour on the pennant.
    pub text_color: [f32; 4],
}

/// Flag sizes in world mm; the caller converts its screen sizes at the
/// current zoom, so a flag keeps its size on screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnnotationFlagMetrics {
    /// From the foot to the pennant's top.
    pub pole_mm: f32,
    pub pennant_width_mm: f32,
    pub pennant_height_mm: f32,
    /// How deep the notch cuts into the pennant's free end.
    pub notch_mm: f32,
    pub stroke_mm: f32,
    pub text_mm: f32,
    /// From the pole to the number.
    pub text_inset_mm: f32,
    /// From one flag's pole to the next one's.
    pub step_mm: f32,
}

impl AnnotationFlagInput {
    /// The pennant, clockwise from the pole's top: top edge, the notch in
    /// the free end, bottom edge back to the pole.
    #[must_use]
    pub fn pennant(&self, m: &AnnotationFlagMetrics) -> Vec<[f32; 2]> {
        let (x, top) = (self.foot[0], self.foot[1] - m.pole_mm);
        let (right, bottom) = (x + m.pennant_width_mm, top + m.pennant_height_mm);
        let notch = m.notch_mm.clamp(0.0, m.pennant_width_mm);
        vec![
            [x, top],
            [right, top],
            [right - notch, top + m.pennant_height_mm * 0.5],
            [right, bottom],
            [x, bottom],
        ]
    }
}

/// The overlay snapshot that draws `flags`: a pole line and a pennant per
/// flag in the overlay buckets, the counts as labels. Draw it with the
/// overlay and text passes.
#[must_use]
pub fn annotation_flag_snapshot(
    flags: &[AnnotationFlagInput],
    m: &AnnotationFlagMetrics,
) -> SchematicSnapshot {
    let mut overlays = OverlayInputs::default();
    let mut labels = Vec::with_capacity(flags.len());
    for flag in flags {
        let top = flag.foot[1] - m.pole_mm;
        overlays.preview_lines.push(OverlayLineInput {
            p0: flag.foot,
            p1: [flag.foot[0], top],
            width_mm: m.stroke_mm,
            color: flag.color,
        });
        overlays.ghost_polygons.push(OverlayPolygonInput {
            vertices: flag.pennant(m),
            fill_color: flag.color,
            stroke_color: None,
            stroke_width_mm: 0.0,
        });
        labels.push(TextInput {
            content: flag.count.to_string(),
            position: [
                flag.foot[0] + m.text_inset_mm,
                top + m.pennant_height_mm * 0.5,
            ],
            size_mm: m.text_mm,
            color: flag.text_color,
            bold: true,
            italic: false,
            family: None,
            rotation_rad: 0.0,
            h_align: HAlign::Left,
            v_align: VAlign::Center,
        });
    }
    SchematicSnapshot {
        overlays,
        labels,
        ..SchematicSnapshot::default()
    }
}

/// The feet of `count` flags side by side from `first`, one step apart.
#[must_use]
pub fn flag_feet(first: [f32; 2], count: usize, m: &AnnotationFlagMetrics) -> Vec<[f32; 2]> {
    (0..count)
        .map(|i| [first[0] + m.step_mm * i as f32, first[1]])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schematic::{SchematicRenderer, ViewRenderer};
    use crate::theme::ResolvedTheme;
    use signex_gfx::scene::{DirtyFlags, Scene};

    const ISSUE: [f32; 4] = [0.9, 0.2, 0.2, 1.0];
    const INK: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

    fn metrics() -> AnnotationFlagMetrics {
        AnnotationFlagMetrics {
            pole_mm: 7.0,
            pennant_width_mm: 7.0,
            pennant_height_mm: 4.0,
            notch_mm: 1.5,
            stroke_mm: 0.5,
            text_mm: 2.5,
            text_inset_mm: 1.0,
            step_mm: 10.0,
        }
    }

    fn flag(foot: [f32; 2], count: u32) -> AnnotationFlagInput {
        AnnotationFlagInput {
            foot,
            count,
            color: ISSUE,
            text_color: INK,
        }
    }

    #[test]
    fn the_pennant_hangs_from_the_pole_top_with_a_notch_in_its_free_end() {
        let v = flag([10.0, 20.0], 2).pennant(&metrics());
        assert_eq!(
            v,
            vec![
                [10.0, 13.0],
                [17.0, 13.0],
                [15.5, 15.0],
                [17.0, 17.0],
                [10.0, 17.0]
            ]
        );
    }

    #[test]
    fn each_flag_is_a_pole_a_pennant_and_its_count() {
        let m = metrics();
        let snapshot =
            annotation_flag_snapshot(&[flag([10.0, 20.0], 2), flag([20.0, 20.0], 1)], &m);
        let poles = &snapshot.overlays.preview_lines;
        assert_eq!(poles.len(), 2);
        assert_eq!((poles[0].p0, poles[0].p1), ([10.0, 20.0], [10.0, 13.0]));
        assert_eq!(poles[0].color, ISSUE);
        assert_eq!(snapshot.overlays.ghost_polygons.len(), 2);
        assert_eq!(snapshot.overlays.ghost_polygons[1].fill_color, ISSUE);
        let counts: Vec<&str> = snapshot.labels.iter().map(|t| t.content.as_str()).collect();
        assert_eq!(counts, ["2", "1"]);
        assert_eq!(snapshot.labels[0].position, [11.0, 15.0]);
        assert_eq!(snapshot.labels[0].color, INK);
        assert!(snapshot.labels[0].bold);
    }

    #[test]
    fn the_flags_reach_the_overlay_and_text_buckets_of_the_scene() {
        let snapshot = annotation_flag_snapshot(&[flag([0.0, 0.0], 3)], &metrics());
        let mut scene = Scene::default();
        SchematicRenderer::build_scene(
            &snapshot,
            &ResolvedTheme::builtin_default(),
            DirtyFlags::OVERLAY | DirtyFlags::TEXT,
            &mut scene,
        );
        assert_eq!(scene.overlay_lines.len(), 1);
        assert_eq!(scene.overlay_polygons.len(), 1);
        assert_eq!(scene.texts.len(), 1);
    }

    #[test]
    fn the_feet_step_right_from_the_first() {
        assert_eq!(
            flag_feet([5.0, 2.0], 3, &metrics()),
            vec![[5.0, 2.0], [15.0, 2.0], [25.0, 2.0]]
        );
        assert!(flag_feet([5.0, 2.0], 0, &metrics()).is_empty());
    }
}
