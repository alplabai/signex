//! What the sketch canvas has open over the drawing: a value being edited
//! in place, the constraint (or pattern) menu, and the selected pattern.
//! Pure UI state; nothing here is persisted or enters the undo history.

use signex_sketch::array::{ArrayId, ArrayKind};
use signex_sketch::id::ConstraintId;

/// One value of a pattern that shows and edits on the drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatternField {
    /// The instance count of a linear or polar pattern.
    Count,
    /// A grid's count along x.
    Columns,
    /// A grid's count along y.
    Rows,
    /// The step along x.
    SpacingX,
    /// The step along y.
    SpacingY,
    /// A polar pattern's total sweep.
    Sweep,
}

impl PatternField {
    /// The fields `kind` has, in the order they show.
    pub fn of(kind: &ArrayKind) -> &'static [PatternField] {
        match kind {
            ArrayKind::Linear { .. } => &[Self::Count, Self::SpacingX, Self::SpacingY],
            ArrayKind::Grid { .. } => &[Self::Columns, Self::Rows, Self::SpacingX, Self::SpacingY],
            ArrayKind::Polar { .. } => &[Self::Count, Self::Sweep],
        }
    }

    /// Whether the field holds a count (rather than a length or an angle).
    pub fn is_count(self) -> bool {
        matches!(self, Self::Count | Self::Columns | Self::Rows)
    }

    /// The expression of this field in `kind`; `None` when `kind` has no
    /// such field.
    pub fn expr(self, kind: &ArrayKind) -> Option<&str> {
        match (kind, self) {
            (
                ArrayKind::Linear { count_expr, .. } | ArrayKind::Polar { count_expr, .. },
                Self::Count,
            ) => Some(count_expr.as_str()),
            (ArrayKind::Grid { nx_expr, .. }, Self::Columns) => Some(nx_expr.as_str()),
            (ArrayKind::Grid { ny_expr, .. }, Self::Rows) => Some(ny_expr.as_str()),
            (
                ArrayKind::Linear { dx_expr, .. } | ArrayKind::Grid { dx_expr, .. },
                Self::SpacingX,
            ) => Some(dx_expr.as_str()),
            (
                ArrayKind::Linear { dy_expr, .. } | ArrayKind::Grid { dy_expr, .. },
                Self::SpacingY,
            ) => Some(dy_expr.as_str()),
            (
                ArrayKind::Polar {
                    sweep_angle_expr, ..
                },
                Self::Sweep,
            ) => Some(sweep_angle_expr.as_str()),
            _ => None,
        }
    }

    /// Write `expr` into this field of `kind`. Returns whether it changed.
    pub fn set_expr(self, kind: &mut ArrayKind, expr: &str) -> bool {
        let slot = match (kind, self) {
            (
                ArrayKind::Linear { count_expr, .. } | ArrayKind::Polar { count_expr, .. },
                Self::Count,
            ) => count_expr,
            (ArrayKind::Grid { nx_expr, .. }, Self::Columns) => nx_expr,
            (ArrayKind::Grid { ny_expr, .. }, Self::Rows) => ny_expr,
            (
                ArrayKind::Linear { dx_expr, .. } | ArrayKind::Grid { dx_expr, .. },
                Self::SpacingX,
            ) => dx_expr,
            (
                ArrayKind::Linear { dy_expr, .. } | ArrayKind::Grid { dy_expr, .. },
                Self::SpacingY,
            ) => dy_expr,
            (
                ArrayKind::Polar {
                    sweep_angle_expr, ..
                },
                Self::Sweep,
            ) => sweep_angle_expr,
            _ => return false,
        };
        if slot.as_str() == expr {
            return false;
        }
        *slot = expr.to_string();
        true
    }
}

/// The value an in-place edit writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueTarget {
    /// A dimension constraint's target.
    Constraint(ConstraintId),
    /// One value of a pattern.
    Pattern(ArrayId, PatternField),
}

/// A value being edited on the drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueEdit {
    pub target: ValueTarget,
    /// The text typed so far.
    pub buffer: String,
    /// The last Enter found the text unreadable: the field stays open and
    /// draws in the error colour until the text changes.
    pub invalid: bool,
}

/// What a canvas menu acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuTarget {
    Constraint(ConstraintId),
    Pattern(ArrayId),
}

/// A menu opened by a right-click on a glyph or value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasMenu {
    pub target: MenuTarget,
    /// Where the click was, in canvas pixels from its top-left corner.
    pub x: f32,
    pub y: f32,
}

/// The sketch canvas's open overlays.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SketchOverlay {
    pub value_edit: Option<ValueEdit>,
    pub menu: Option<CanvasMenu>,
    /// The pattern whose glyph was clicked: its values show on the drawing
    /// and a click on one of its instances suppresses it.
    pub selected_pattern: Option<ArrayId>,
}

impl SketchOverlay {
    /// Close the menu, else the value edit, else drop the selected
    /// pattern: the innermost first, as Esc unwinds. Returns whether
    /// anything was open.
    pub fn close_innermost(&mut self) -> bool {
        if self.menu.take().is_some() {
            return true;
        }
        if self.value_edit.take().is_some() {
            return true;
        }
        self.selected_pattern.take().is_some()
    }

    /// Whether the value of `target` is being edited.
    pub fn is_editing(&self, target: ValueTarget) -> bool {
        self.value_edit.as_ref().is_some_and(|e| e.target == target)
    }
}
