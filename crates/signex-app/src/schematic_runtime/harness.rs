//! How signal harnesses are drawn and hit (#1280): a harness wire is a
//! thick line in the harness colour (thicker than a bus), a connector a body
//! with a flat side and a pointed tip in the soft harness fill, its entries
//! tabs on the flat side with their names and the type name above it. A
//! nested entry (its definition entry is a harness) is drawn heavier; a
//! harness-typed sheet entry is a harness tab with its type. Model and
//! rules: `docs/SIGNAL_HARNESSES.md`.

use super::*;
use signex_types::schematic::{
    HarnessConnector, SCHEMATIC_HARNESS_NESTED_OUTLINE_MM, SCHEMATIC_HARNESS_OUTLINE_MM,
    SCHEMATIC_HARNESS_STROKE_MM, SCHEMATIC_HIT_HARNESS_TOLERANCE_MM, SCHEMATIC_TEXT_MM,
};

/// What `push` appends the harness to.
pub(super) struct HarnessOut<'a> {
    pub(super) wires: &'a mut Vec<WireInput>,
    pub(super) polygons: &'a mut Vec<PolygonInput>,
    pub(super) texts: &'a mut Vec<TextInput>,
}

/// Append every harness object in view of `snapshot` to `out`.
pub(super) fn push(
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    out: HarnessOut<'_>,
) {
    let h = &snapshot.harness;
    if h.connectors.is_empty() && h.wires.is_empty() && h.entry_types.is_empty() {
        return;
    }
    let ink = to_iced(&colors.harness_color());
    let soft = to_rgba(to_iced(&colors.harness_soft_color()));
    let family = resolve_family(None, snapshot.font_family.as_deref());
    let text = |content: &str, at: Point, color: Color, v_align: VAlign| TextInput {
        content: content.to_string(),
        position: [at.x as f32, at.y as f32],
        size_mm: SCHEMATIC_TEXT_MM as f32,
        color: to_rgba(color),
        bold: true,
        italic: false,
        family: family.clone(),
        rotation_rad: 0.0,
        h_align: HAlign::Left,
        v_align,
    };

    for w in &h.wires {
        let p0 = transform.world_to_screen((w.start.x, w.start.y));
        let p1 = transform.world_to_screen((w.end.x, w.end.y));
        if !line_visible(p0, p1, bounds) {
            continue;
        }
        out.wires.push(WireInput {
            id: renderer_id(w.uuid),
            p0: [w.start.x as f32, w.start.y as f32],
            p1: [w.end.x as f32, w.end.y as f32],
            width_mm: SCHEMATIC_HARNESS_STROKE_MM as f32,
            explicit_color: Some(to_rgba(focus_color(ink, focus_set, w.uuid))),
        });
    }

    let nested = nested_entries(snapshot);
    for c in &h.connectors {
        let b = c.bounds();
        let min = transform.world_to_screen((b.min_x, b.min_y));
        let max = transform.world_to_screen((b.max_x, b.max_y));
        let rect_min = iced::Point::new(min.x.min(max.x), min.y.min(max.y));
        let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());
        if !rect_visible(rect_min, size, bounds) {
            continue;
        }
        let color = focus_color(ink, focus_set, c.uuid);
        out.polygons.push(PolygonInput {
            vertices: vertices(&c.outline()),
            fill_color: soft,
            stroke_color: Some(to_rgba(color)),
            stroke_width_mm: SCHEMATIC_HARNESS_OUTLINE_MM as f32,
        });
        for (i, e) in c.entries.iter().enumerate() {
            let heavy = nested.contains(&e.uuid);
            out.polygons.push(PolygonInput {
                vertices: vertices(&c.entry_tab(i)),
                fill_color: soft,
                stroke_color: Some(to_rgba(color)),
                stroke_width_mm: if heavy {
                    SCHEMATIC_HARNESS_NESTED_OUTLINE_MM
                } else {
                    SCHEMATIC_HARNESS_OUTLINE_MM
                } as f32,
            });
            let name_color = if heavy { color } else { to_iced(&colors.pin) };
            out.texts.push(text(
                &e.name,
                c.entry_label_point(i),
                name_color,
                VAlign::Center,
            ));
        }
        out.texts.push(text(
            &c.harness_type,
            Point::new(b.min_x, b.min_y - 0.5),
            color,
            VAlign::Bottom,
        ));
    }

    // A harness-typed sheet entry: a heavier tab in the harness colour and
    // its type name.
    for cs in &snapshot.child_sheets {
        for pin in &cs.pins {
            let Some(ty) = h.entry_type(pin.uuid) else {
                continue;
            };
            let color = focus_color(ink, focus_set, pin.uuid);
            let p = pin.position;
            out.polygons.push(PolygonInput {
                vertices: vertices(&sheet_entry_tab(p)),
                fill_color: soft,
                stroke_color: Some(to_rgba(color)),
                stroke_width_mm: SCHEMATIC_HARNESS_NESTED_OUTLINE_MM as f32,
            });
            out.texts
                .push(text(ty, Point::new(p.x + 1.8, p.y), color, VAlign::Center));
        }
    }
}

/// The tab a harness-typed sheet entry is drawn with, centred on `p`.
fn sheet_entry_tab(p: Point) -> Vec<Point> {
    vec![
        Point::new(p.x - 1.27, p.y - 0.6),
        Point::new(p.x + 0.6, p.y - 0.6),
        Point::new(p.x + 1.27, p.y),
        Point::new(p.x + 0.6, p.y + 0.6),
        Point::new(p.x - 1.27, p.y + 0.6),
    ]
}

/// Entries whose point a harness wire or connector tip touches: a harness
/// nested in this one, drawn heavier.
fn nested_entries(snapshot: &SchematicRenderSnapshot) -> HashSet<uuid::Uuid> {
    let h = &snapshot.harness;
    let tips: Vec<Point> = h
        .connectors
        .iter()
        .map(HarnessConnector::tip_point)
        .collect();
    let mut out = HashSet::new();
    for c in &h.connectors {
        for (i, e) in c.entries.iter().enumerate() {
            let at = c.entry_point(i);
            let touched = signex_net::on_harness(snapshot, &at)
                || tips.iter().any(|t| t.um_key() == at.um_key());
            if touched {
                out.insert(e.uuid);
            }
        }
    }
    out
}

fn vertices(points: &[Point]) -> Vec<[f32; 2]> {
    points.iter().map(|p| [p.x as f32, p.y as f32]).collect()
}

/// The box of each harness object for selection: a connector's outline, an
/// entry's tab and a wire's segment. Pushed after the other items, so a
/// connector body never hides the wire ending on its entry.
pub(super) fn push_item_bounds(snapshot: &SchematicRenderSnapshot, out: &mut Vec<ItemBound>) {
    let h = &snapshot.harness;
    for c in &h.connectors {
        let b = c.bounds();
        out.push(ItemBound {
            item: SelectedItem::new(c.uuid, SelectedKind::HarnessConnector),
            bbox: b,
            anchor: c.tip_point(),
        });
    }
    // Entries after their bodies, so a click on a tab picks the entry.
    for c in &h.connectors {
        for (i, e) in c.entries.iter().enumerate() {
            let tab = c.entry_tab(i);
            let bbox = tab
                .iter()
                .fold(None::<Aabb>, |acc, p| {
                    let dot = Aabb::new(p.x, p.y, p.x, p.y);
                    Some(acc.map_or(dot, |a| a.union(&dot)))
                })
                .unwrap_or_else(|| Aabb::new(0.0, 0.0, 0.0, 0.0));
            out.push(ItemBound {
                item: SelectedItem::new(e.uuid, SelectedKind::HarnessEntry),
                bbox,
                anchor: c.entry_point(i),
            });
        }
    }
    for w in &h.wires {
        out.push(ItemBound {
            item: SelectedItem::new(w.uuid, SelectedKind::HarnessWire),
            bbox: Aabb::new(w.start.x, w.start.y, w.end.x, w.end.y),
            anchor: Point::new((w.start.x + w.end.x) * 0.5, (w.start.y + w.end.y) * 0.5),
        });
    }
}

/// The segment and tolerance a click on the harness wire `id` is measured
/// against; `None` when the sheet has no such wire.
pub(super) fn wire_segment(
    snapshot: &SchematicRenderSnapshot,
    id: uuid::Uuid,
) -> Option<(Point, Point, f64)> {
    let w = snapshot.harness.wire(id)?;
    let tolerance = (SCHEMATIC_HARNESS_STROKE_MM * 0.5).max(SCHEMATIC_HIT_HARNESS_TOLERANCE_MM);
    Some((w.start, w.end, tolerance))
}

#[cfg(test)]
#[path = "harness_tests.rs"]
mod tests;
