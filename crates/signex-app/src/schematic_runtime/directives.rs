//! How the sheet's directives are drawn and hit (#2420, #1281, #1489): a Net
//! Blanket is a dashed outline, a Compile Mask a dotted one, a Differential
//! Pair directive two short parallel strokes, a Net Parameters directive a
//! flag on its wire. A Net Blanket with a class and a classed Differential
//! Pair carry a flag too. All use the directive colour the No-ERC diamond
//! uses (`no_connect`). Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use super::*;
use signex_types::schematic::SchDirective;

/// The outline stroke of a directive region, as thin as the No-ERC diamond.
const OUTLINE_MM: f32 = signex_types::schematic::SCHEMATIC_NO_CONNECT_STROKE_MM as f32;

/// How close a click must come to a region's edge to pick the directive, so
/// a click inside the region still reaches what it covers.
const EDGE_HIT_MM: f64 = 0.5;

// The flag's lines and box are shared with the exports (`signex-types`,
// `directive_flag`), so the canvas and the paper draw one flag.
pub(super) use signex_types::schematic::directive_flag as flag;
use signex_types::schematic::{DIRECTIVE_FLAG_TEXT_MM, directive_flag_line_origin};

/// Append the strokes of every directive in view to `wires`, with the
/// dash pattern of a region outline in `line_styles`.
pub(super) fn push_lines(
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    wires: &mut Vec<WireInput>,
    line_styles: &mut HashMap<u64, NetLineStyle>,
) {
    for directive in &snapshot.directives {
        let color = to_rgba(focus_color(
            to_iced(&colors.no_connect),
            focus_set,
            directive.uuid(),
        ));
        let base = renderer_id(directive.uuid());
        let segments = strokes(directive, transform.scale);
        let style = match directive {
            SchDirective::Blanket { .. } => NetLineStyle::Dashed,
            SchDirective::CompileMask { .. } => NetLineStyle::Dotted,
            SchDirective::DiffPair { .. } | SchDirective::NetParameters { .. } => {
                NetLineStyle::Solid
            }
        };
        // The region's own four edges take its dash; a flag is solid.
        let outline = directive.region().map_or(0, |_| 4);
        for (i, (p0, p1)) in segments.into_iter().enumerate() {
            let s0 = transform.world_to_screen((p0.x, p0.y));
            let s1 = transform.world_to_screen((p1.x, p1.y));
            if !line_visible(s0, s1, bounds) {
                continue;
            }
            let id = base.saturating_add(i as u64);
            wires.push(WireInput {
                id,
                p0: [p0.x as f32, p0.y as f32],
                p1: [p1.x as f32, p1.y as f32],
                width_mm: OUTLINE_MM,
                explicit_color: Some(color),
            });
            if style != NetLineStyle::Solid && i < outline {
                line_styles.insert(id, style);
            }
        }
    }
}

/// Append the text of every directive flag in view to `texts`, in the
/// directive colour.
pub(super) fn push_flag_texts(
    snapshot: &SchematicRenderSnapshot,
    transform: &ScreenTransform,
    colors: &CanvasColors,
    bounds: Rectangle,
    focus_set: Option<&HashSet<uuid::Uuid>>,
    document_family: Option<&str>,
    texts: &mut Vec<TextInput>,
) {
    for directive in &snapshot.directives {
        let Some(flag) = flag(directive) else {
            continue;
        };
        let min = transform.world_to_screen((flag.rect.min_x, flag.rect.min_y));
        let max = transform.world_to_screen((flag.rect.max_x, flag.rect.max_y));
        let rect_min = iced::Point::new(min.x.min(max.x), min.y.min(max.y));
        let size = iced::Size::new((max.x - min.x).abs(), (max.y - min.y).abs());
        if !rect_visible(rect_min, size, bounds) {
            continue;
        }
        let color = to_rgba(focus_color(
            to_iced(&colors.no_connect),
            focus_set,
            directive.uuid(),
        ));
        for (i, line) in flag.lines.iter().enumerate() {
            let at = directive_flag_line_origin(&flag, i);
            texts.push(TextInput {
                content: line.clone(),
                position: [at.x as f32, at.y as f32],
                size_mm: DIRECTIVE_FLAG_TEXT_MM as f32,
                color,
                bold: i == 0,
                italic: false,
                family: resolve_family(None, document_family),
                rotation_rad: 0.0,
                h_align: HAlign::Left,
                v_align: VAlign::Top,
            });
        }
    }
}

/// The four edges of `r`.
fn edges(r: &Aabb) -> [(Point, Point); 4] {
    let corners = [
        Point::new(r.min_x, r.min_y),
        Point::new(r.max_x, r.min_y),
        Point::new(r.max_x, r.max_y),
        Point::new(r.min_x, r.max_y),
    ];
    [0, 1, 2, 3].map(|i| (corners[i], corners[(i + 1) % 4]))
}

/// The segments `directive` is drawn with, in world mm: a region's four
/// edges first, a Differential Pair's two strokes, then a flag's stem and
/// box. `scale` keeps the Differential Pair mark readable when zoomed out.
pub(super) fn strokes(directive: &SchDirective, scale: f32) -> Vec<(Point, Point)> {
    let mut out: Vec<(Point, Point)> = Vec::new();
    if let Some(r) = directive.region() {
        out.extend(edges(&r));
    } else if let SchDirective::DiffPair { .. } = directive {
        let half = signex_types::schematic::SCHEMATIC_RENDER_NO_CONNECT_HALF_LEN_MM.max(
            screen_px_to_world_mm(
                signex_types::schematic::SCHEMATIC_RENDER_NO_CONNECT_MIN_HALF_LEN_PX,
                scale,
            ),
        );
        let at = directive.anchor();
        let gap = half * 0.4;
        out.extend([-gap, gap].into_iter().map(|dy| {
            (
                Point::new(at.x - half, at.y + dy),
                Point::new(at.x + half, at.y + dy),
            )
        }));
    }
    if let Some(flag) = flag(directive) {
        out.extend(flag.stem);
        out.extend(edges(&flag.rect));
    }
    out
}

/// The box of each directive for selection: a region's own box, a point
/// directive's small square with its flag. Pushed after every other item,
/// like the No-ERC diamond, so a mark on a wire wins the click; a region is
/// hit only near its edge ([`hit`]), so a click inside it still reaches what
/// it covers.
pub(super) fn push_item_bounds(snapshot: &SchematicRenderSnapshot, out: &mut Vec<ItemBound>) {
    for directive in &snapshot.directives {
        let anchor = directive.anchor();
        let bbox = directive.region().unwrap_or_else(|| {
            let mark = Aabb::new(
                anchor.x - 0.5,
                anchor.y - 0.5,
                anchor.x + 0.5,
                anchor.y + 0.5,
            );
            flag(directive).map_or(mark, |f| mark.union(&f.rect))
        });
        out.push(ItemBound {
            item: SelectedItem::new(directive.uuid(), SelectedKind::Directive),
            bbox,
            anchor,
        });
    }
}

/// Where a click picks the directive `id`: the area, and for a region the
/// hole inside it that is left to what the region covers. `None` when the
/// sheet has no such directive. The one rule `hit` and the hover index share.
pub(super) fn pick_area(
    snapshot: &SchematicRenderSnapshot,
    id: uuid::Uuid,
) -> Option<(Aabb, Option<Aabb>)> {
    let directive = snapshot.directives.iter().find(|d| d.uuid() == id)?;
    Some(match directive.region() {
        Some(r) => {
            let inner = r.expand(-EDGE_HIT_MM);
            let hole = (inner.min_x < inner.max_x && inner.min_y < inner.max_y).then_some(inner);
            (r.expand(EDGE_HIT_MM), hole)
        }
        // The box every other point item is hit by: its bound plus 0.25 mm,
        // with the flag.
        None => {
            let at = directive.anchor();
            let mark = Aabb::new(at.x - 0.75, at.y - 0.75, at.x + 0.75, at.y + 0.75);
            (flag(directive).map_or(mark, |f| mark.union(&f.rect)), None)
        }
    })
}

/// Whether a click at `point` picks the directive `id`: near a region's
/// edge, or on a point directive's mark or flag.
pub(super) fn hit(snapshot: &SchematicRenderSnapshot, id: uuid::Uuid, point: Point) -> bool {
    pick_area(snapshot, id).is_some_and(|(area, hole)| {
        area.contains(point.x, point.y) && !hole.is_some_and(|h| h.contains(point.x, point.y))
    })
}
