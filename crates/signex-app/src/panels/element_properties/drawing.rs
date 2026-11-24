//! Properties of a drawing shape (mockup row 48): the line-art illustration at the
//! top (a rectangle's with the caption naming its sample wire), then
//! Location (Position and Size; Center for an arc or a circle; Start and
//! End for a line) → Properties (Border; Radius, Angles, Width) →
//! Appearance (Color, Fill, Placement). The panel header names the kind;
//! the State ▸ Locked section is the shared one `selected` adds. Paired
//! values share a row, every field carries its axis and unit inside it,
//! and every kind keeps its own fields.

use super::super::*;
use super::illustrations::drawing_shape::{
    DRAW_ANGLES, DRAW_BORDER, DRAW_CENTER, DRAW_ENDS, DRAW_FILL, DRAW_PLACEMENT, DRAW_POSITION,
    DRAW_RADIUS, DRAW_SIZE, DRAW_WIDTH, ShapeFacts, shape_illustration, value_text,
};
use super::rows::{PAIR_FIELD_MIN, field_insets, field_tags, mono, pair_line};
use crate::app::contracts::DrawingFieldEdit as E;
use crate::panels::form_look;
use crate::panels::multi_selection::{
    Cell, control_row, hover_linked, illustration_view, inset_caption, segmented_cells,
};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::schematic::{DrawingPlacement, FillType, SchDrawing, StrokeColor};
use signex_widgets::Label;
use signex_widgets::property_illustration::FieldId;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Average advance of an axis-tag letter, in captions.
const AXIS_ADVANCE_EM: f32 = 0.6;

/// The selection-info rows of the selected drawing, parsed.
struct Rows<'a> {
    info: &'a [(String, String)],
}

impl Rows<'_> {
    fn get(&self, key: &str) -> String {
        self.info
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    fn number(&self, key: &str) -> f64 {
        crate::numeric_field::parse_field_f64(&self.get(key)).unwrap_or(0.0)
    }

    fn pair(&self, key: &str) -> (f64, f64) {
        let text = self.get(key);
        let mut parts = text.split(',');
        let mut next = || {
            parts
                .next()
                .and_then(crate::numeric_field::parse_field_f64)
                .unwrap_or(0.0)
        };
        let x = next();
        (x, next())
    }

    fn fill(&self) -> FillType {
        match self.get("Fill").as_str() {
            "Outline" => FillType::Outline,
            "Background" => FillType::Background,
            _ => FillType::None,
        }
    }
}

/// What the illustration of the selected kind needs.
fn shape_facts(
    kind: &str,
    rows: &Rows<'_>,
    drawing: Option<&SchDrawing>,
    placement: DrawingPlacement,
) -> Option<ShapeFacts> {
    Some(match kind {
        "Line" => ShapeFacts::Line {
            start: rows.pair("Start"),
            end: rows.pair("End"),
        },
        "Rectangle" => ShapeFacts::Rect {
            position: rows.pair("Position"),
            size: (rows.number("Width"), rows.number("Height")),
            border: rows.number("Border"),
            fill: rows.fill(),
            placement,
        },
        "Circle" => ShapeFacts::Circle { fill: rows.fill() },
        "Arc" => ShapeFacts::Arc {
            start_deg: rows.number("Start Angle"),
            end_deg: rows.number("End Angle"),
        },
        "Polygon" => ShapeFacts::Polygon {
            points: match drawing {
                Some(SchDrawing::Polyline { points, .. }) => {
                    points.iter().map(|p| (p.x, p.y)).collect()
                }
                _ => Vec::new(),
            },
            fill: rows.fill(),
        },
        _ => return None,
    })
}

/// The text a numeric box shows: the draft while typed, else the value.
fn shown(field: DrawingFieldId, stored: f64, ctx: &PanelContext) -> String {
    ctx.drawing_edit_buf
        .get(&field)
        .cloned()
        .unwrap_or_else(|| value_text(stored))
}

/// The error colour while `field` holds text it cannot store (#2767).
fn refused_ink(field: DrawingFieldId, ctx: &PanelContext) -> Option<Color> {
    ctx.drawing_edit_buf
        .get(&field)
        .filter(|typed| field.refuses(typed))
        .map(|_| crate::styles::ti(ctx.tokens.error))
}

/// The width a tag longer than one letter (Start, End) adds to its field's
/// left inset, past the one letter `field_insets` leaves room for.
fn long_tag_extra(axis: &Label<'_>) -> f32 {
    let extra = axis.as_str().chars().count().saturating_sub(1);
    extra as f32 * AXIS_ADVANCE_EM * TS.caption
}

/// The narrowest field of a pair tagged `a` and `b`: `PAIR_FIELD_MIN` plus
/// the room the longer tag takes past one letter, so a pair that cannot fit
/// its value past a Start or End tag stacks instead of clipping the value.
fn pair_floor(i18n: &I18n, a: MessageId, b: MessageId) -> f32 {
    let extra = |id: MessageId| long_tag_extra(&Label::from(t!(i18n, id)));
    PAIR_FIELD_MIN + extra(a).max(extra(b))
}

/// One drawing field: the box with its axis tag (when it has one) at the
/// left and its unit at the right, inside it (mockup row 48). A value the
/// box can store previews while typed and commits on Enter or on leaving
/// the box.
fn field_box<'a>(
    ctx: &PanelContext,
    axis: Option<MessageId>,
    field: DrawingFieldId,
    stored: f64,
    unit: MessageId,
) -> Element<'a, PanelMsg> {
    let axis = axis.map(|id| Label::from(t!(ctx.i18n, id)));
    let unit = Label::from(t!(ctx.i18n, unit));
    let (left, right) = field_insets(axis.is_some(), true);
    let left = left + axis.as_ref().map_or(0.0, long_tag_extra);
    let input = signex_widgets::text_input(
        signex_i18n::UserText::new(String::new()),
        shown(field, stored, ctx),
    )
    .size(TS.body)
    .font(mono())
    .padding(iced::Padding {
        top: SP.s,
        bottom: SP.s,
        left,
        right,
    })
    .width(Length::Fill)
    .on_input(move |s| PanelMsg::DrawingFieldTyping(field, s))
    .on_submit(PanelMsg::DrawingFieldLeft(field))
    .on_blur(PanelMsg::DrawingFieldLeft(field))
    .style(form_look::refusable_input_style(refused_ink(field, ctx)));
    iced::widget::stack![input, field_tags(axis, Some(unit))]
        .width(Length::Fill)
        .boxed()
}

/// A label | field row of one drawing field.
fn num_row<'a>(
    ctx: &PanelContext,
    label: MessageId,
    field: DrawingFieldId,
    stored: f64,
    unit: MessageId,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let control = field_box(ctx, None, field, stored, unit);
    control_row(t!(ctx.i18n, label), control, muted)
}

/// A paired row: `label` | first | second, linked to `link`.
fn pair<'a>(
    ctx: &'a PanelContext,
    link: FieldId,
    label: MessageId,
    (ax_a, field_a, a): (MessageId, DrawingFieldId, f64),
    (ax_b, field_b, b): (MessageId, DrawingFieldId, f64),
    unit: MessageId,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let both = pair_line(
        field_box(ctx, Some(ax_a), field_a, a, unit),
        field_box(ctx, Some(ax_b), field_b, b, unit),
        pair_floor(&ctx.i18n, ax_a, ax_b),
    );
    hover_linked(ctx, link, control_row(t!(ctx.i18n, label), both, muted))
}

/// One Properties section's rows, top to bottom.
type SectionRows<'a> = Vec<Element<'a, PanelMsg>>;

/// Location and Properties of one kind, as `(location, properties)` rows.
fn kind_rows<'a>(
    ctx: &'a PanelContext,
    kind: &str,
    rows: &Rows<'_>,
    muted: Color,
) -> Option<(SectionRows<'a>, SectionRows<'a>)> {
    use DrawingFieldId as F;
    let (x, y, w, h, mm, deg) = (
        ids::PROPERTIES_AXIS_X,
        ids::PROPERTIES_AXIS_Y,
        ids::PROPERTIES_AXIS_W,
        ids::PROPERTIES_AXIS_H,
        ids::PROPERTIES_UNIT_MM,
        ids::PROPERTIES_UNIT_DEGREES,
    );
    let linked = |link: FieldId, row: Element<'a, PanelMsg>| hover_linked(ctx, link, row);
    Some(match kind {
        "Line" => {
            let ((sx, sy), (ex, ey)) = (rows.pair("Start"), rows.pair("End"));
            let start = (x, F::LineStartX, sx);
            let end = (x, F::LineEndX, ex);
            (
                vec![
                    pair(
                        ctx,
                        DRAW_ENDS,
                        ids::PROPERTIES_DRAWING_START,
                        start,
                        (y, F::LineStartY, sy),
                        mm,
                        muted,
                    ),
                    pair(
                        ctx,
                        DRAW_ENDS,
                        ids::PROPERTIES_DRAWING_END,
                        end,
                        (y, F::LineEndY, ey),
                        mm,
                        muted,
                    ),
                ],
                vec![linked(
                    DRAW_WIDTH,
                    num_row(
                        ctx,
                        ids::PROPERTIES_DRAWING_WIDTH,
                        F::LineWidth,
                        rows.number("Width"),
                        mm,
                        muted,
                    ),
                )],
            )
        }
        "Rectangle" => {
            let (px, py) = rows.pair("Position");
            let (rw, rh) = (rows.number("Width"), rows.number("Height"));
            (
                vec![
                    pair(
                        ctx,
                        DRAW_POSITION,
                        ids::PROPERTIES_DRAWING_POSITION,
                        (x, F::RectStartX, px),
                        (y, F::RectStartY, py),
                        mm,
                        muted,
                    ),
                    pair(
                        ctx,
                        DRAW_SIZE,
                        ids::PROPERTIES_DRAWING_SIZE,
                        (w, F::RectWidth, rw),
                        (h, F::RectHeight, rh),
                        mm,
                        muted,
                    ),
                ],
                vec![linked(
                    DRAW_BORDER,
                    num_row(
                        ctx,
                        ids::PANELS_DRAWING_BORDER,
                        F::RectBorder,
                        rows.number("Border"),
                        mm,
                        muted,
                    ),
                )],
            )
        }
        "Circle" => {
            let (cx, cy) = rows.pair("Center");
            (
                vec![pair(
                    ctx,
                    DRAW_CENTER,
                    ids::PROPERTIES_DRAWING_CENTER,
                    (x, F::CircleCenterX, cx),
                    (y, F::CircleCenterY, cy),
                    mm,
                    muted,
                )],
                vec![
                    linked(
                        DRAW_RADIUS,
                        num_row(
                            ctx,
                            ids::PROPERTIES_DRAWING_RADIUS,
                            F::CircleRadius,
                            rows.number("Radius"),
                            mm,
                            muted,
                        ),
                    ),
                    linked(
                        DRAW_BORDER,
                        num_row(
                            ctx,
                            ids::PANELS_DRAWING_BORDER,
                            F::CircleBorder,
                            rows.number("Border"),
                            mm,
                            muted,
                        ),
                    ),
                ],
            )
        }
        "Arc" => {
            let (cx, cy) = rows.pair("Center");
            let angles = (
                (
                    ids::PROPERTIES_ANGLE_START,
                    F::ArcStartAngle,
                    rows.number("Start Angle"),
                ),
                (
                    ids::PROPERTIES_ANGLE_END,
                    F::ArcEndAngle,
                    rows.number("End Angle"),
                ),
            );
            (
                vec![pair(
                    ctx,
                    DRAW_CENTER,
                    ids::PROPERTIES_DRAWING_CENTER,
                    (x, F::ArcCenterX, cx),
                    (y, F::ArcCenterY, cy),
                    mm,
                    muted,
                )],
                vec![
                    linked(
                        DRAW_RADIUS,
                        num_row(
                            ctx,
                            ids::PROPERTIES_DRAWING_RADIUS,
                            F::ArcRadius,
                            rows.number("Radius"),
                            mm,
                            muted,
                        ),
                    ),
                    pair(
                        ctx,
                        DRAW_ANGLES,
                        ids::PROPERTIES_DRAWING_ANGLES,
                        angles.0,
                        angles.1,
                        deg,
                        muted,
                    ),
                    linked(
                        DRAW_WIDTH,
                        num_row(
                            ctx,
                            ids::PROPERTIES_DRAWING_WIDTH,
                            F::ArcWidth,
                            rows.number("Width"),
                            mm,
                            muted,
                        ),
                    ),
                ],
            )
        }
        "Polygon" => {
            let vertices = rows.number("Vertices") as i64;
            let ink = crate::styles::ti(ctx.tokens.text);
            (
                Vec::new(),
                vec![
                    prop_kv_row(
                        t!(ctx.i18n, ids::PROPERTIES_VERTICES),
                        &ctx.i18n.count(vertices),
                        muted,
                        ink,
                    ),
                    linked(
                        DRAW_BORDER,
                        num_row(
                            ctx,
                            ids::PANELS_DRAWING_BORDER,
                            F::PolyBorder,
                            rows.number("Border"),
                            mm,
                            muted,
                        ),
                    ),
                ],
            )
        }
        _ => return None,
    })
}

/// A collapsible section holding `rows`; nothing when there are none.
fn section<'a>(
    ctx: &'a PanelContext,
    key: &str,
    title: MessageId,
    rows: Vec<Element<'a, PanelMsg>>,
) -> Option<Element<'a, PanelMsg>> {
    if rows.is_empty() {
        return None;
    }
    Some(
        ctx.collapsible(key, title, &ctx.collapsed_sections, move || {
            Column::with_children(rows).spacing(0).width(Length::Fill)
        })
        .boxed(),
    )
}

pub(in crate::panels) fn view_drawing_properties<'a>(
    ctx: &'a PanelContext,
    muted: Color,
    _primary: Color,
    _border_c: Color,
) -> Element<'a, PanelMsg> {
    let rows = Rows {
        info: &ctx.selection_info,
    };
    let kind = rows.get("Type");
    let ink = crate::styles::ti(ctx.tokens.text);
    // The panel header names the kind; the illustration comes first.
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);
    let placement = ctx.selected_drawing_placement;
    if let Some(facts) = shape_facts(&kind, &rows, ctx.selected_drawing.as_ref(), placement) {
        let has_sample_wire = matches!(facts, ShapeFacts::Rect { .. });
        col = col.push(illustration_view(
            ctx,
            shape_illustration(&facts, &ctx.i18n),
        ));
        if has_sample_wire {
            let caption = t!(ctx.i18n, ids::PROPERTIES_DRAWING_SAMPLE_WIRE);
            col = col.push(inset_caption(caption, muted));
        }
    }

    let Some((location, properties)) = kind_rows(ctx, &kind, &rows, muted) else {
        for (key, value) in &ctx.selection_info {
            if key != "Type" {
                // raw: the key is worded by signex-engine's selection info (#2060).
                col = col.push(prop_kv_row(raw(key.clone()), value, muted, ink));
            }
        }
        return col.boxed();
    };
    if let Some(s) = section(ctx, "draw_location", ids::PANELS_PLACED_LOCATION, location) {
        col = col.push(s);
    }
    if let Some(s) = section(
        ctx,
        "draw_properties",
        ids::PANELS_PLACED_PROPERTIES,
        properties,
    ) {
        col = col.push(s);
    }
    let has_fill = matches!(kind.as_str(), "Rectangle" | "Circle" | "Polygon");
    let mut appearance = vec![drawing_stroke_color_row(
        current_color(ctx.selected_drawing.as_ref()),
        ctx.wire_color_picker,
        drawing_hex(ctx),
        &ctx.i18n,
    )];
    if has_fill {
        appearance.push(hover_linked(
            ctx,
            DRAW_FILL,
            fill_row(rows.fill(), muted, &ctx.i18n),
        ));
    }
    appearance.push(hover_linked(
        ctx,
        DRAW_PLACEMENT,
        placement_row(placement, muted, &ctx.i18n),
    ));
    if let Some(s) = section(
        ctx,
        "draw_appearance",
        ids::PROPERTIES_SECTION_APPEARANCE,
        appearance,
    ) {
        col = col.push(s);
    }
    col.boxed()
}

/// The stored stroke colour of the selected drawing.
fn current_color(drawing: Option<&SchDrawing>) -> Option<StrokeColor> {
    drawing.and_then(|d| match d {
        SchDrawing::Line { stroke_color, .. }
        | SchDrawing::Rect { stroke_color, .. }
        | SchDrawing::Circle { stroke_color, .. }
        | SchDrawing::Arc { stroke_color, .. }
        | SchDrawing::Polyline { stroke_color, .. } => *stroke_color,
        SchDrawing::Bezier(b) => b.stroke_color,
        SchDrawing::Dimension(d) => d.stroke_color,
        SchDrawing::Table(_) => None,
    })
}

/// The hex box of the selected drawing's Color row. With no drawing id the
/// nil id stands in: the row then edits nothing anyway.
fn drawing_hex(ctx: &PanelContext) -> crate::panels::HexEntry {
    let id = ctx.selected_uuid.unwrap_or_else(uuid::Uuid::nil);
    crate::panels::HexEntry::of(
        crate::panels::ColorRowKey::DrawingStroke(id),
        ctx.color_hex_draft.as_ref(),
    )
}

/// The stroke colour field (mockup row 48): a swatch, the code or Auto (the
/// theme's drawing colour) in a hex box that takes a typed code, and the
/// chevron opening the shared preset popover. Each pick is one undoable
/// edit.
fn drawing_stroke_color_row<'a>(
    current: Option<StrokeColor>,
    picker: crate::panels::WireColorPicker,
    hex: crate::panels::HexEntry,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    use crate::panels::wire_style_rows::{PopoverColorRow, popover_color_row_named};
    let on_pick: std::rc::Rc<dyn Fn([u8; 4]) -> PanelMsg + 'static> =
        std::rc::Rc::new(|[r, g, b, a]: [u8; 4]| {
            PanelMsg::UpdateDrawingEdit(E::StrokeColor(Some(StrokeColor { r, g, b, a })))
        });
    popover_color_row_named(
        PopoverColorRow {
            label: t!(i18n, ids::PANELS_DRAWING_COLOR),
            current: current.map(|c| [c.r, c.g, c.b, c.a]),
            picker,
            hex,
            on_toggle: PanelMsg::ToggleWireColorPicker,
            on_advanced: PanelMsg::OpenWireColorAdvancedPicker,
            on_cancel: PanelMsg::CancelWireColorPicker,
            on_pick,
            on_clear: current
                .is_some()
                .then_some(PanelMsg::UpdateDrawingEdit(E::StrokeColor(None))),
        },
        ids::PANELS_DRAWING_COLOR_AUTO,
        i18n,
    )
}

/// Placement: Behind circuit / Above circuit. Drawings default to behind,
/// so a highlight or callout laid over symbols and wires is the one that
/// needs choosing (#1583).
fn placement_row<'a>(
    current: DrawingPlacement,
    muted: Color,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let cells = vec![
        Cell::new(
            DrawingPlacement::Behind,
            t!(i18n, ids::PANELS_DRAWING_PLACEMENT_BEHIND),
        ),
        Cell::new(
            DrawingPlacement::Above,
            t!(i18n, ids::PANELS_DRAWING_PLACEMENT_ABOVE),
        ),
    ];
    control_row(
        t!(i18n, ids::PANELS_DRAWING_PLACEMENT),
        segmented_cells(cells, Some(current), 2, |p| {
            PanelMsg::UpdateDrawingEdit(E::Placement(p))
        }),
        muted,
    )
}

/// Fill: None / Outline / Solid, each with a tooltip; Solid fills with the
/// sheet background.
fn fill_row<'a>(current: FillType, muted: Color, i18n: &I18n) -> Element<'a, PanelMsg> {
    let cells = vec![
        Cell::new(FillType::None, t!(i18n, ids::PANELS_SHAPE_FILL_NONE))
            .tip(t!(i18n, ids::PROPERTIES_FILL_NONE_TIP)),
        Cell::new(FillType::Outline, t!(i18n, ids::PANELS_SHAPE_FILL_OUTLINE))
            .tip(t!(i18n, ids::PROPERTIES_FILL_OUTLINE_TIP)),
        Cell::new(FillType::Background, t!(i18n, ids::PROPERTIES_FILL_SOLID))
            .tip(t!(i18n, ids::PROPERTIES_FILL_SOLID_TIP)),
    ];
    control_row(
        t!(i18n, ids::PANELS_SHAPE_FILL),
        segmented_cells(cells, Some(current), 3, |f| {
            PanelMsg::UpdateDrawingEdit(E::Fill(f))
        }),
        muted,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(kind: &str, info: &[(&str, &str)]) -> Vec<String> {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selection_info = std::iter::once(("Type".to_string(), kind.to_string()))
            .chain(info.iter().map(|(k, v)| (k.to_string(), v.to_string())))
            .collect();
        let view = view_drawing_properties(&ctx, Color::WHITE, Color::WHITE, Color::BLACK);
        crate::layout_probe::lay_out_texts(view, iced::Size::new(400.0, 3000.0))
            .into_iter()
            .map(|t| t.text)
            .collect()
    }

    fn order(texts: &[String], wanted: &[&str]) -> Vec<usize> {
        wanted
            .iter()
            .map(|w| {
                texts
                    .iter()
                    .position(|t| t == w)
                    .unwrap_or_else(|| panic!("{w:?} not in {texts:?}"))
            })
            .collect()
    }

    /// Mockup row 48: Location → Properties → Appearance, paired rows, the Fill
    /// segment reads Solid.
    #[test]
    fn a_rectangle_shows_its_sections_in_order() {
        let shown = texts(
            "Rectangle",
            &[
                ("Position", "1, 2"),
                ("Width", "10"),
                ("Height", "5"),
                ("Border", "0.2"),
            ],
        );
        let at = order(
            &shown,
            &[
                "LOCATION",
                "Position",
                "Size",
                "PROPERTIES",
                "Border",
                "APPEARANCE",
                "Fill",
                "Solid",
            ],
        );
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{shown:?}");
    }

    /// Mockup rows 46 and 48: Color is one field whose hex box reads Auto,
    /// beside its label; its presets stay in the popover until it is opened.
    #[test]
    fn the_color_row_is_one_field_reading_auto() {
        let info = [
            ("Type", "Rectangle"),
            ("Position", "1, 2"),
            ("Border", "0.2"),
        ];
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selection_info = (info.iter())
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let size = iced::Size::new(400.0, 3000.0);
        let view = || view_drawing_properties(&ctx, Color::WHITE, Color::WHITE, Color::BLACK);
        let texts = crate::layout_probe::lay_out_texts(view(), size);
        let boxes = crate::layout_probe::lay_out_inputs(view(), size);
        let label = crate::layout_probe::find(&texts, "Color").bounds;
        let auto = crate::layout_probe::find(&boxes, "Auto").bounds;
        assert!(auto.x > label.x, "{label:?} {auto:?}");
        assert!(
            (auto.center_y() - label.center_y()).abs() < label.height,
            "the box sits beside its label: {label:?} {auto:?}"
        );
        assert!(
            !texts.iter().any(|t| t.text == "Preset Colours"),
            "{texts:?}"
        );
    }

    /// Mockup row 48: an arc keeps Center, Radius, Angles (Start / End on one row)
    /// and Width, and has no Fill.
    #[test]
    fn an_arc_keeps_its_own_fields() {
        let shown = texts(
            "Arc",
            &[
                ("Center", "0, 0"),
                ("Radius", "5"),
                ("Start Angle", "0"),
                ("End Angle", "90"),
                ("Width", "0.2"),
            ],
        );
        let at = order(
            &shown,
            &["Center", "Radius", "Angles", "Start", "End", "Width"],
        );
        assert!(at.windows(2).all(|w| w[0] < w[1]), "{shown:?}");
        assert!(!shown.iter().any(|t| t == "Fill"), "{shown:?}");
    }

    const RECTANGLE: [(&str, &str); 4] = [
        ("Position", "1, 2"),
        ("Width", "10"),
        ("Height", "5"),
        ("Border", "0.2"),
    ];

    /// Mockup row 48: the panel header names the kind, so the drawing's
    /// rows start with the illustration, no second icon-and-name line;
    /// under a rectangle's illustration a caption says what its sample wire
    /// shows. An arc has no sample wire and no caption.
    #[test]
    fn the_rectangle_has_its_caption_and_no_second_name_line() {
        let caption = "The sample wire shows Behind / Above";
        let shown = texts("Rectangle", &RECTANGLE);
        assert!(!shown.iter().any(|t| t == "Rectangle"), "{shown:?}");
        assert!(shown.iter().any(|t| t == caption), "{shown:?}");
        let arc = texts("Arc", &[("Center", "0, 0"), ("Radius", "5")]);
        assert!(!arc.iter().any(|t| t == "Arc" || t == caption), "{arc:?}");
    }

    /// Mockup row 48: the X / W axis tags and the mm unit sit inside their
    /// fields, as on every placed object's Position and Size.
    #[test]
    fn the_axis_and_unit_sit_inside_each_field() {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_state.panel_ctx.clone();
        ctx.selection_info = std::iter::once(("Type", "Rectangle"))
            .chain(RECTANGLE)
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let size = iced::Size::new(400.0, 3000.0);
        let view = || view_drawing_properties(&ctx, Color::WHITE, Color::WHITE, Color::BLACK);
        let texts = crate::layout_probe::lay_out_texts(view(), size);
        let boxes = crate::layout_probe::lay_out_inputs(view(), size);
        let inside = |tag: &str, value: &str| {
            let field = crate::layout_probe::find(&boxes, value).bounds;
            texts.iter().any(|t| {
                t.text == tag
                    && t.bounds.x >= field.x
                    && t.bounds.x + t.bounds.width <= field.x + field.width
                    && (t.bounds.center_y() - field.center_y()).abs() < field.height / 2.0
            })
        };
        for (tag, value) in [("X", "1.000"), ("W", "10.000"), ("mm", "10.000")] {
            assert!(inside(tag, value), "{tag} outside {value}: {texts:?}");
        }
        assert!(inside("mm", "0.200"), "the Border unit: {texts:?}");
    }

    /// Mockup row 48: a pair tagged Start / End keeps the value's room past
    /// its longer tag, so it stacks where an X / Y pair still fits; an X / Y
    /// pair keeps the one-letter floor.
    #[test]
    fn a_long_tagged_pair_keeps_room_for_its_value() {
        let i18n = I18n::en_us();
        let xy = pair_floor(&i18n, ids::PROPERTIES_AXIS_X, ids::PROPERTIES_AXIS_Y);
        assert_eq!(xy, PAIR_FIELD_MIN);
        let angles = pair_floor(
            &i18n,
            ids::PROPERTIES_ANGLE_START,
            ids::PROPERTIES_ANGLE_END,
        );
        let start = long_tag_extra(&Label::from(t!(i18n, ids::PROPERTIES_ANGLE_START)));
        assert!(start > 0.0, "Start is longer than one letter");
        assert_eq!(angles, PAIR_FIELD_MIN + start);
    }
}
