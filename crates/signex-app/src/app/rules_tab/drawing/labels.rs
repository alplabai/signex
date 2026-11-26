//! The scene's values as editable dimension labels, in drawing pixels.

use iced::Point;
use signex_drc_dsl::table::LimitField;
use signex_i18n::{I18n, t};
use signex_types::coord::Unit;
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::LengthFormat;
use signex_widgets::dimension_label::{
    DimensionLabel, DimensionLabelStyle, DragHandle, ValueLimits,
};

use super::scene::{Scene, VIEW_H, VIEW_W, ValueLabel};
use crate::styles::{TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

/// One view unit, in captions: the approved drawings set an 11-unit
/// caption, so a view unit is about one pixel at the default scale.
const UNIT_EM: f32 = 1.0 / 11.0;
/// How much larger the expanded drawing is.
const EXPANDED_ZOOM: f32 = 1.5;
/// The rule text keeps millimetres, and so does the grid beside it.
pub(super) const UNIT: Unit = Unit::Mm;
/// Decimals a label shows, in millimetres.
const DECIMALS: usize = 3;
/// A dragged length snaps to this, in millimetres.
const DRAG_STEP_MM: f64 = 0.005;

/// The pixel size of a drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Layout {
    /// Drawing pixels per view unit.
    pub unit_px: f32,
}

impl Layout {
    pub fn new(is_expanded: bool) -> Self {
        let zoom = if is_expanded { EXPANDED_ZOOM } else { 1.0 };
        Self {
            unit_px: TS.caption * UNIT_EM * zoom,
        }
    }

    pub fn width(self) -> f32 {
        VIEW_W * self.unit_px
    }

    pub fn height(self) -> f32 {
        VIEW_H * self.unit_px
    }

    /// The drawing height in captions, for the illustration widget.
    pub fn height_em(self) -> f32 {
        self.height() / TS.caption
    }

    /// The caption size in view units: the drawing scales, its text does
    /// not.
    pub fn caption_units(self) -> f32 {
        TS.caption / self.unit_px
    }

    /// A view point in drawing pixels.
    pub fn px(self, p: Point) -> Point {
        Point::new(p.x * self.unit_px, p.y * self.unit_px)
    }
}

/// Expansions may go negative; every other limit is a positive length.
fn is_signed(field: Option<LimitField>) -> bool {
    matches!(
        field,
        Some(
            LimitField::MinMaskExpansion
                | LimitField::MaxMaskExpansion
                | LimitField::MinPasteExpansion
                | LimitField::MaxPasteExpansion
        )
    )
}

fn limits(label: &ValueLabel) -> ValueLimits {
    ValueLimits {
        min: (!is_signed(label.field)).then_some(DRAG_STEP_MM),
        max: None,
        step: DRAG_STEP_MM,
    }
}

/// The labels of `scene` for `layout`, in the scene's order: index `i` of
/// the result is `scene.labels[i]`.
pub(super) fn dimension_labels<'a>(
    scene: &Scene,
    layout: Layout,
    i18n: &'a I18n,
) -> Vec<DimensionLabel<'a>> {
    let format = LengthFormat::new(i18n, UNIT);
    scene
        .labels
        .iter()
        .map(|label| {
            let mut out = DimensionLabel::new(label.value_mm, format, layout.px(label.at))
                .caption(t!(i18n, label.caption))
                .anchor(label.anchor)
                .decimals(DECIMALS)
                .limits(limits(label))
                .edit_glyph();
            if let Some(handle) = label.handle {
                out = out.handle(DragHandle {
                    at: layout.px(handle.at),
                    axis: handle.axis,
                    value_per_px: handle.mm_per_unit / f64::from(layout.unit_px),
                });
            }
            out
        })
        .collect()
}

/// Label colours from the Graphite Dark ink, so they read on the board
/// ground under both chrome themes.
pub(super) fn style() -> DimensionLabelStyle {
    let dark = theme_tokens(ThemeId::GraphiteDark);
    let resolved = crate::styles::tokens::resolve(&dark);
    DimensionLabelStyle {
        ink: ti(dark.warning),
        fill: ti(dark.bg),
        field: resolved.field,
        selection: ti(dark.selection),
        handle: ti(dark.text),
        text_size: TS.caption,
    }
}
