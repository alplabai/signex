//! Rotate / mirror a whole selection about a centre point
//! (`Command::TransformSelection`).
//!
//! Screen space is y-down. Label and text rotations are clockwise-positive
//! degrees (the renderer's `x cos - y sin` frame); a symbol's own `rotation`
//! is counter-clockwise-positive and is rotated, then mirrored, so its delta
//! is compensated the same way as `Command::RotateSelection`.

use signex_types::schematic::{
    HAlign, Point, SchDrawing, SchematicSheet, SelectedItem, SelectedKind, VAlign,
};

use super::{Engine, autoplace_fields, drawing_uuid, normalize_degrees};
use crate::command::{MirrorAxis, SelectionTransform};
use crate::error::EngineError;

impl SelectionTransform {
    fn is_mirror(self) -> bool {
        matches!(self, Self::Mirror(_))
    }

    /// The image of the offset `(dx, dy)` from the centre.
    fn vector(self, (dx, dy): (f64, f64)) -> (f64, f64) {
        match self {
            Self::RotateCounterclockwise => (dy, -dx),
            Self::RotateClockwise => (-dy, dx),
            Self::Mirror(MirrorAxis::Vertical) => (dx, -dy),
            Self::Mirror(MirrorAxis::Horizontal) => (-dx, dy),
        }
    }

    fn point(self, centre: Point, p: Point) -> Point {
        let (dx, dy) = self.vector((p.x - centre.x, p.y - centre.y));
        Point::new(centre.x + dx, centre.y + dy)
    }

    /// The image of a direction given as a clockwise-positive angle in
    /// degrees, kept in `[0, 360)`.
    fn angle(self, degrees: f64) -> f64 {
        match self {
            Self::RotateCounterclockwise => degrees - 90.0,
            Self::RotateClockwise => degrees + 90.0,
            Self::Mirror(MirrorAxis::Vertical) => -degrees,
            Self::Mirror(MirrorAxis::Horizontal) => 180.0 - degrees,
        }
        .rem_euclid(360.0)
    }

    /// Counter-clockwise-on-screen delta for a symbol's stored rotation.
    fn symbol_delta(self) -> f64 {
        match self {
            Self::RotateCounterclockwise => 90.0,
            Self::RotateClockwise => -90.0,
            Self::Mirror(_) => 0.0,
        }
    }
}

fn swapped_h(h: HAlign) -> HAlign {
    match h {
        HAlign::Left => HAlign::Right,
        HAlign::Right => HAlign::Left,
        HAlign::Center => HAlign::Center,
    }
}

fn swapped_v(v: VAlign) -> VAlign {
    match v {
        VAlign::Top => VAlign::Bottom,
        VAlign::Bottom => VAlign::Top,
        VAlign::Center => VAlign::Center,
    }
}

/// Carry a text's rotation and justification through `transform`. A mirror
/// flips the text's local y axis (swap vertical justification); a baseline
/// that would end up reading backwards or upside down is turned half a
/// revolution with both justifications swapped, so the text stays readable
/// and anchored on the same side.
fn retarget_text(
    transform: SelectionTransform,
    rotation: &mut f64,
    h: &mut HAlign,
    v: &mut VAlign,
) {
    let mut angle = transform.angle(*rotation);
    let (mut swap_h, mut swap_v) = (false, transform.is_mirror());
    let (sin, cos) = angle.to_radians().sin_cos();
    // World space is y-down and rotation is clockwise-positive on screen,
    // so 90 reads top-to-bottom; vertical text reads bottom-to-top (270),
    // readable from the right as on a drawing.
    let readable = cos > 1e-9 || (cos.abs() <= 1e-9 && sin < 0.0);
    if !readable {
        angle = (angle + 180.0).rem_euclid(360.0);
        swap_h = !swap_h;
        swap_v = !swap_v;
    }
    *rotation = angle;
    if swap_h {
        *h = swapped_h(*h);
    }
    if swap_v {
        *v = swapped_v(*v);
    }
}

fn is_flag(label: &signex_types::schematic::Label) -> bool {
    use signex_types::schematic::LabelType;
    matches!(
        label.label_type,
        LabelType::Global | LabelType::Hierarchical
    )
}

/// Sheet symbols and sheet pins refuse the whole action. A symbol's
/// ref/value field is not refused: box-select picks fields up next to their
/// symbol, and the field travels with its parent (a field selected on its
/// own matches nothing here and is skipped).
fn supported(kind: SelectedKind) -> bool {
    !matches!(kind, SelectedKind::ChildSheet | SelectedKind::SheetPin)
}

fn drawing_points(d: &SchDrawing) -> Vec<Point> {
    match d {
        SchDrawing::Line { start, end, .. } | SchDrawing::Rect { start, end, .. } => {
            vec![*start, *end]
        }
        SchDrawing::Circle { center, .. } => vec![*center],
        SchDrawing::Arc {
            start, mid, end, ..
        } => vec![*start, *mid, *end],
        SchDrawing::Polyline { points, .. } => points.clone(),
    }
}

impl Engine {
    /// Centre of the bounding box of the anchor points of the transformable
    /// items in `items` (symbol origins, wire / bus ends, label, junction,
    /// no-connect and text positions, drawing vertices). `None` when none of
    /// the items is present and transformable. Not snapped.
    pub fn selection_centre(&self, items: &[SelectedItem]) -> Option<Point> {
        let mut bounds: Option<(f64, f64, f64, f64)> = None;
        for item in items.iter().filter(|i| supported(i.kind)) {
            for p in self.item_anchor_points(item) {
                bounds = Some(match bounds {
                    None => (p.x, p.y, p.x, p.y),
                    Some((a, b, c, d)) => (a.min(p.x), b.min(p.y), c.max(p.x), d.max(p.y)),
                });
            }
        }
        bounds.map(|(a, b, c, d)| Point::new((a + c) / 2.0, (b + d) / 2.0))
    }

    fn item_anchor_points(&self, item: &SelectedItem) -> Vec<Point> {
        let doc = &self.document;
        let id = item.uuid;
        match item.kind {
            SelectedKind::Symbol => doc
                .symbols
                .iter()
                .filter(|s| s.uuid == id)
                .map(|s| s.position)
                .collect(),
            SelectedKind::Wire => doc
                .wires
                .iter()
                .filter(|w| w.uuid == id)
                .flat_map(|w| [w.start, w.end])
                .collect(),
            SelectedKind::Bus => doc
                .buses
                .iter()
                .filter(|b| b.uuid == id)
                .flat_map(|b| [b.start, b.end])
                .collect(),
            SelectedKind::BusEntry => doc
                .bus_entries
                .iter()
                .filter(|b| b.uuid == id)
                .flat_map(|b| {
                    [
                        b.position,
                        Point::new(b.position.x + b.size.0, b.position.y + b.size.1),
                    ]
                })
                .collect(),
            SelectedKind::Label => doc
                .labels
                .iter()
                .filter(|l| l.uuid == id)
                .map(|l| l.position)
                .collect(),
            SelectedKind::Junction => doc
                .junctions
                .iter()
                .filter(|j| j.uuid == id)
                .map(|j| j.position)
                .collect(),
            SelectedKind::NoConnect => doc
                .no_connects
                .iter()
                .filter(|n| n.uuid == id)
                .map(|n| n.position)
                .collect(),
            SelectedKind::NoErcDirective => doc
                .no_erc_directives
                .iter()
                .filter(|n| n.uuid == id)
                .map(|n| n.position)
                .collect(),
            SelectedKind::TextNote => doc
                .text_notes
                .iter()
                .filter(|t| t.uuid == id)
                .map(|t| t.position)
                .collect(),
            SelectedKind::Drawing => doc
                .drawings
                .iter()
                .filter(|d| drawing_uuid(d) == id)
                .flat_map(drawing_points)
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Apply `transform` about `centre` to every present item, all or
    /// nothing: an unsupported item refuses the whole selection before any
    /// geometry changes. Returns whether anything moved.
    pub(crate) fn transform_selection_about(
        &mut self,
        items: &[SelectedItem],
        centre: Point,
        transform: SelectionTransform,
    ) -> Result<bool, EngineError> {
        if let Some(bad) = items
            .iter()
            .find(|i| !supported(i.kind) && self.contains_selected_item(i))
        {
            return Err(EngineError::SelectionTransformUnsupported {
                what: if bad.kind == SelectedKind::ChildSheet {
                    "a sheet symbol"
                } else {
                    "a sheet pin"
                }
                .to_string(),
            });
        }

        let present: Vec<&SelectedItem> = items
            .iter()
            .filter(|i| self.contains_selected_item(i))
            .collect();
        if present.is_empty() {
            return Ok(false);
        }

        let doc = &mut self.document;
        let has = |kind: SelectedKind, id: uuid::Uuid| {
            present.iter().any(|i| i.kind == kind && i.uuid == id)
        };
        let at = |p: &mut Point| *p = transform.point(centre, *p);

        for s in doc
            .symbols
            .iter_mut()
            .filter(|s| has(SelectedKind::Symbol, s.uuid))
        {
            at(&mut s.position);
            for text in [s.ref_text.as_mut(), s.val_text.as_mut()]
                .into_iter()
                .flatten()
            {
                at(&mut text.position);
            }
            match transform {
                SelectionTransform::Mirror(MirrorAxis::Vertical) => s.mirror_x = !s.mirror_x,
                SelectionTransform::Mirror(MirrorAxis::Horizontal) => s.mirror_y = !s.mirror_y,
                _ => {
                    // Rotate-then-mirror render order: one mirror axis turns
                    // a positive stored angle clockwise on screen.
                    let delta = if s.mirror_x != s.mirror_y {
                        -transform.symbol_delta()
                    } else {
                        transform.symbol_delta()
                    };
                    s.rotation = normalize_degrees(s.rotation + delta);
                }
            }
        }
        for w in doc
            .wires
            .iter_mut()
            .filter(|w| has(SelectedKind::Wire, w.uuid))
        {
            at(&mut w.start);
            at(&mut w.end);
        }
        for b in doc
            .buses
            .iter_mut()
            .filter(|b| has(SelectedKind::Bus, b.uuid))
        {
            at(&mut b.start);
            at(&mut b.end);
        }
        for e in doc
            .bus_entries
            .iter_mut()
            .filter(|e| has(SelectedKind::BusEntry, e.uuid))
        {
            let far = Point::new(e.position.x + e.size.0, e.position.y + e.size.1);
            let (near, far) = (
                transform.point(centre, e.position),
                transform.point(centre, far),
            );
            e.position = near;
            e.size = (far.x - near.x, far.y - near.y);
        }
        for j in doc
            .junctions
            .iter_mut()
            .filter(|j| has(SelectedKind::Junction, j.uuid))
        {
            at(&mut j.position);
        }
        for n in doc
            .no_connects
            .iter_mut()
            .filter(|n| has(SelectedKind::NoConnect, n.uuid))
        {
            at(&mut n.position);
        }
        for n in doc
            .no_erc_directives
            .iter_mut()
            .filter(|n| has(SelectedKind::NoErcDirective, n.uuid))
        {
            at(&mut n.position);
        }
        for l in doc
            .labels
            .iter_mut()
            .filter(|l| has(SelectedKind::Label, l.uuid))
        {
            at(&mut l.position);
            if is_flag(l) {
                l.rotation = transform.angle(l.rotation);
            } else {
                retarget_text(transform, &mut l.rotation, &mut l.justify, &mut l.justify_v);
            }
        }
        for t in doc
            .text_notes
            .iter_mut()
            .filter(|t| has(SelectedKind::TextNote, t.uuid))
        {
            at(&mut t.position);
            retarget_text(
                transform,
                &mut t.rotation,
                &mut t.justify_h,
                &mut t.justify_v,
            );
        }
        for d in doc
            .drawings
            .iter_mut()
            .filter(|d| has(SelectedKind::Drawing, drawing_uuid(d)))
        {
            match d {
                SchDrawing::Line { start, end, .. } | SchDrawing::Rect { start, end, .. } => {
                    at(start);
                    at(end);
                }
                SchDrawing::Circle { center, .. } => at(center),
                SchDrawing::Arc {
                    start, mid, end, ..
                } => {
                    at(start);
                    at(mid);
                    at(end);
                }
                SchDrawing::Polyline { points, .. } => points.iter_mut().for_each(at),
            }
        }

        self.autoplace_symbol_fields(&present_symbol_ids(items));
        Ok(true)
    }

    /// Re-run field autoplace for the transformed symbols against the sheet
    /// as it now stands (symbols with user-placed fields are skipped by
    /// `autoplace_fields` itself).
    fn autoplace_symbol_fields(&mut self, ids: &[uuid::Uuid]) {
        let snapshot: SchematicSheet = self.document.clone();
        for symbol in self
            .document
            .symbols
            .iter_mut()
            .filter(|s| ids.contains(&s.uuid))
        {
            if let Some(lib) = snapshot.lib_symbols.get(&symbol.lib_id) {
                autoplace_fields(symbol, lib, &snapshot);
            }
        }
    }
}

fn present_symbol_ids(items: &[SelectedItem]) -> Vec<uuid::Uuid> {
    items
        .iter()
        .filter(|i| i.kind == SelectedKind::Symbol)
        .map(|i| i.uuid)
        .collect()
}
