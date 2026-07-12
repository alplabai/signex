//! Projection texture pass (footprint/UV alignment + emission).

use super::*;

// ---------------------------------------------------------------------------
// Projection texture pass
// ---------------------------------------------------------------------------

/// Footprint-space or UV-space rectangular bounds used by the projection pass.
///
/// For `ProjectionPassConfig.footprint_bounds`: coordinates are in millimetres.
/// For `ProjectionPassConfig.uv_bounds`: coordinates are normalized [0.0, 1.0].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionBounds {
    pub min_mm: [f32; 2],
    pub max_mm: [f32; 2],
}

impl Default for ProjectionBounds {
    fn default() -> Self {
        Self {
            min_mm: [0.0; 2],
            max_mm: [1.0; 2],
        }
    }
}

impl ProjectionBounds {
    fn width(&self) -> f32 {
        self.max_mm[0] - self.min_mm[0]
    }

    fn height(&self) -> f32 {
        self.max_mm[1] - self.min_mm[1]
    }
}

/// Configuration for the projection texture pass.
///
/// `footprint_bounds` defines the board-layer footprint extent in mm.
/// `uv_bounds` defines normalized UV coverage within that footprint; both
/// components must be in `[0.0, 1.0]` with `min < max`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectionPassConfig {
    pub footprint_bounds: ProjectionBounds,
    pub uv_bounds: ProjectionBounds,
    pub tile_columns: usize,
    pub fill_alpha: f32,
    pub stroke_width_mm: f32,
}

impl Default for ProjectionPassConfig {
    fn default() -> Self {
        Self {
            footprint_bounds: ProjectionBounds::default(),
            uv_bounds: ProjectionBounds::default(),
            tile_columns: 8,
            fill_alpha: 0.72,
            stroke_width_mm: 0.05,
        }
    }
}

/// Errors produced by [`check_projection_alignment`] and [`emit_projection_pass`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionAlignmentError {
    ZeroAreaFootprint {
        model_id: String,
    },
    UvBoundsOutOfRange {
        model_id: String,
        axis: &'static str,
        which: &'static str,
    },
    UvBoundsInverted {
        model_id: String,
        axis: &'static str,
    },
}

impl fmt::Display for ProjectionAlignmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroAreaFootprint { model_id } => {
                write!(f, "model {model_id}: footprint bounds have zero area")
            }
            Self::UvBoundsOutOfRange {
                model_id,
                axis,
                which,
            } => {
                write!(
                    f,
                    "model {model_id}: UV bounds {axis}.{which} is outside [0.0, 1.0]"
                )
            }
            Self::UvBoundsInverted { model_id, axis } => {
                write!(f, "model {model_id}: UV bounds {axis}.min >= {axis}.max")
            }
        }
    }
}

impl std::error::Error for ProjectionAlignmentError {}

/// Validate that `config` is geometrically consistent for projection.
///
/// Returns `Ok(())` when:
/// - The footprint bounds have positive area on both axes.
/// - All UV bound values are in `[0.0, 1.0]`.
/// - UV min is strictly less than UV max on both axes.
pub fn check_projection_alignment(
    model_id: &str,
    config: &ProjectionPassConfig,
) -> Result<(), ProjectionAlignmentError> {
    let fb = &config.footprint_bounds;
    if fb.width() <= 0.0 || fb.height() <= 0.0 {
        return Err(ProjectionAlignmentError::ZeroAreaFootprint {
            model_id: model_id.to_string(),
        });
    }

    let uv = &config.uv_bounds;
    let components = [
        ("x", uv.min_mm[0], uv.max_mm[0]),
        ("y", uv.min_mm[1], uv.max_mm[1]),
    ];

    for (axis, uv_min, uv_max) in components {
        if !(0.0..=1.0).contains(&uv_min) {
            return Err(ProjectionAlignmentError::UvBoundsOutOfRange {
                model_id: model_id.to_string(),
                axis,
                which: "min",
            });
        }
        if !(0.0..=1.0).contains(&uv_max) {
            return Err(ProjectionAlignmentError::UvBoundsOutOfRange {
                model_id: model_id.to_string(),
                axis,
                which: "max",
            });
        }
        if uv_min >= uv_max {
            return Err(ProjectionAlignmentError::UvBoundsInverted {
                model_id: model_id.to_string(),
                axis,
            });
        }
    }

    Ok(())
}

/// Emit one overlay polygon per staged opaque primitive into `scene.overlay_polygons`.
///
/// Ordering boundary: this function writes to `scene.overlay_polygons` only.
/// Callers must call [`emit_opaque_pass_preview`] first (which writes to
/// `scene.polygons`) to maintain the expected render order: opaque → projection.
///
/// Returns `Err` if alignment validation fails; `scene.overlay_polygons` is not
/// modified in that case.
pub fn emit_projection_pass(
    model: &RuntimeGlbModel,
    theme: &ResolvedTheme,
    scene: &mut Scene,
    config: ProjectionPassConfig,
) -> Result<(), ProjectionAlignmentError> {
    check_projection_alignment(&model.model_id, &config)?;

    let fb = &config.footprint_bounds;
    let uv = &config.uv_bounds;

    let proj_min_x = fb.min_mm[0] + uv.min_mm[0] * fb.width();
    let proj_min_y = fb.min_mm[1] + uv.min_mm[1] * fb.height();
    let proj_max_x = fb.min_mm[0] + uv.max_mm[0] * fb.width();
    let proj_max_y = fb.min_mm[1] + uv.max_mm[1] * fb.height();

    let proj_w = proj_max_x - proj_min_x;
    let proj_h = proj_max_y - proj_min_y;

    let columns = config.tile_columns.max(1);
    let count = model.mesh_staging.opaque_primitives.len();
    let rows = (count + columns - 1).max(1);
    let tile_w = (proj_w / columns as f32).max(0.01);
    let tile_h = (proj_h / rows as f32).max(0.01);

    let fill_base = with_alpha_mul(theme.color(ColorSlot::LassoFill), config.fill_alpha);
    let stroke_color = theme.color(ColorSlot::LassoStroke);
    let stroke_width = config.stroke_width_mm.max(0.01);

    scene.overlay_polygons.clear();
    scene.overlay_polygons.reserve(count);

    for (index, _primitive) in model.mesh_staging.opaque_primitives.iter().enumerate() {
        let col = (index % columns) as f32;
        let row = (index / columns) as f32;
        let x = proj_min_x + col * tile_w;
        let y = proj_min_y + row * tile_h;

        scene.overlay_polygons.push(GpuPolygon {
            vertices: rect_vertices([x, y], tile_w, tile_h),
            fill_color: fill_base,
            stroke_color: Some(stroke_color),
            stroke_width,
        });
    }

    Ok(())
}
