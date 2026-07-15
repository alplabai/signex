//! GPU render path for the schematic canvas (issue #169 PR 2).
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: iced/wgpu public docs, IPC-2612-1, IEEE 315, IEC 60617.
//!
//! Thin schematic-flavoured constructor over the generic
//! [`crate::scene_shader`] GPU renderer. Both the schematic and the PCB editor
//! consume the **same** primitive-agnostic `signex_gfx` pipelines; this module
//! only adapts the schematic's [`ScreenTransform`] into the raw
//! `offset_px` / `scale_px_per_mm` the shared renderer expects.
//!
//! Gated behind [`crate::feature_flags::SCHEMATIC_GPU_RENDER`] and default-off.
//! Pointer interaction and presentation overlays (selection, ghost preview,
//! ERC marks) still live on the CPU `canvas` program; wiring those onto the
//! GPU surface is a follow-up. See [`crate::schematic_runtime::ScreenTransform`]
//! for the world→screen mapping the shared camera mirrors.

use std::sync::Arc;

use signex_gfx::scene::Scene;

use crate::scene_shader::SceneShaderProgram;
use crate::schematic_runtime::ScreenTransform;

/// Build the shared GPU [`SceneShaderProgram`] for the schematic from an
/// already-tessellated `Scene` and the current [`ScreenTransform`].
pub fn schematic_shader_program(
    scene: Scene,
    transform: &ScreenTransform,
) -> SceneShaderProgram {
    SceneShaderProgram::new(
        Arc::new(scene),
        // Uncached source: the schematic rebuilds its scene each frame, so it
        // has no stable generation and must upload every frame.
        None,
        [transform.offset_x, transform.offset_y],
        transform.scale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::shader::Program;
    use signex_gfx::primitive::line::LineSegment;

    fn transform(offset_x: f32, offset_y: f32, scale: f32) -> ScreenTransform {
        ScreenTransform {
            offset_x,
            offset_y,
            scale,
        }
    }

    #[test]
    fn schematic_program_carries_scene_and_transform() {
        let mut scene = Scene::default();
        scene.lines.push(LineSegment {
            p0: [0.0, 0.0],
            p1: [1.0, 1.0],
            width: 0.1,
            color: [1.0, 1.0, 1.0, 1.0],
            style: 0,
            _pad: 0,
        });

        let program = schematic_shader_program(scene, &transform(5.0, 6.0, 3.0));
        let primitive = program.draw(
            &(),
            iced::mouse::Cursor::Unavailable,
            iced::Rectangle::default(),
        );

        assert_eq!(primitive.scene.lines.len(), 1);
        assert_eq!(primitive.offset_px, [5.0, 6.0]);
        assert_eq!(primitive.scale_px_per_mm, 3.0);
    }
}
