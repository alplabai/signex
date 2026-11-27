//! The Move Selection dialog body (#66): ΔX / ΔY with the unit as a suffix,
//! a read-only line-art preview of the move and a grid-step help line. The
//! dialog lives in its own window, whose title bar carries the title and the
//! selection count. The net colour bodies are in `view/net_color/`.

use super::*;
use crate::app::handlers::editing_commands::move_selection_dialog::{
    MOVE_DX_INPUT_ID, PREVIEW_EM, grid_steps, offset_mm, stored_dy,
};
use crate::styles::{DialogButtonKind, dialog_body, dialog_button, dialog_footer};
use iced::{Point, Rectangle, Size};
use signex_i18n::{Arg, UserText, ids, t};
use signex_types::coord::Unit;
use signex_types::schematic::{
    Point as SheetPoint, SchematicDefaults, SchematicSheet, SelectedItem, SelectedKind, Symbol,
    SymbolPrimitive, arc_polyline, lower_placed_symbol,
};
use signex_widgets::LengthFormat;
use signex_widgets::property_illustration::{
    Anchor, Callout, Illustration, IllustrationHover, Ink, Paint, Part, Primitive, Style,
    TextLabel, TextRole, Tint, Weight,
};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Preview geometry, in the illustration's own view units: the span the
/// selection and its move are fitted to, the room kept for the callouts,
/// how far a name sits above its object and the moved selection's frame pad.
const SPAN: f32 = 140.0;
const MARGIN: f32 = 26.0;
const CALLOUT_GAP: f32 = 12.0;
const NAME_LIFT: f32 = 6.0;
const FRAME_PAD: f32 = 4.0;
/// How long a drawn name is at most.
const NAME_MAX_CHARS: usize = 8;
/// Sheet sizes, in mm, of the marks with no body to lower: a junction dot,
/// half a no-connect cross, half the stand-in box of a part with no library
/// body. The fitted span covers at least this many grid steps.
const JUNCTION_RADIUS_MM: f64 = 0.5;
const NO_CONNECT_HALF_MM: f64 = 0.635;
const MISSING_BODY_HALF_MM: f64 = 1.27;
const MIN_SPAN_STEPS: f64 = 4.0;

impl Signex {
    /// The Move Selection dialog: the two offsets, the preview, the help
    /// line, and Cancel / Move. Enter in either field moves.
    pub(super) fn view_move_selection_body(&self) -> Element<'_, Message> {
        use iced::widget::{Widget, column, container, row};
        let tokens = &self.document_view.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let ms = &self.ui_state.move_selection;
        let unit = self.ui_state.unit;
        let has_selection = !self.interaction_state.active_canvas().selected.is_empty();
        let preview = |text: &str| offset_mm(text, unit).unwrap_or(0.0);
        let (dx_mm, dy_mm) = (preview(&ms.dx), preview(&ms.dy));

        let fields = row![
            offset_field(
                t!(i18n, ids::DIALOG_MOVE_SELECTION_DX),
                &ms.dx,
                Some(MOVE_DX_INPUT_ID.clone()),
                unit,
                |s| Message::MoveSelection(MoveSelectionMsg::DxChanged(s)),
                self,
            ),
            offset_field(
                t!(i18n, ids::DIALOG_MOVE_SELECTION_DY),
                &ms.dy,
                None,
                unit,
                |s| Message::MoveSelection(MoveSelectionMsg::DyChanged(s)),
                self,
            ),
        ]
        .spacing(SP.l);

        let preview = self.move_selection_preview(dx_mm, dy_mm);
        let help = signex_widgets::text(UserText::new(self.move_selection_help(dx_mm, dy_mm)))
            .size(TS.caption)
            .color(crate::styles::tokens::resolve(tokens).ink3);

        let body = dialog_body(column![fields, preview, help].spacing(SP.m));
        let footer = dialog_footer(vec![
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                Some(Message::MoveSelection(MoveSelectionMsg::Close)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::DIALOG_MOVE_SELECTION_MOVE),
                DialogButtonKind::Primary,
                has_selection.then_some(Message::MoveSelection(MoveSelectionMsg::Apply)),
                tokens,
            ),
        ]);

        container(column![body, footer].width(iced::Length::Fill))
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed()
    }

    /// The framed preview of the move, on the sheet grid (mockup row 66).
    fn move_selection_preview(&self, dx_mm: f64, dy_mm: f64) -> Element<'_, Message> {
        let tokens = &self.document_view.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let fmt = LengthFormat::new(i18n, self.ui_state.unit);
        let grid_mm = crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm);
        let art = self
            .document_state
            .active_engine()
            .map(|engine| {
                selection_art(
                    engine.document(),
                    &self.interaction_state.active_canvas().selected,
                    |symbol| self.shown_designator(symbol).to_owned(),
                )
            })
            .unwrap_or_default();
        let drawing = signex_widgets::property_illustration::property_illustration(
            move_preview(dx_mm, dy_mm, grid_mm, &art, &fmt),
            &IllustrationHover::default(),
            tokens,
            TS.caption,
        )
        .map(|_| Message::noop())
        .boxed();
        crate::app::view::dialogs::preview_frame(
            t!(i18n, ids::DIALOG_MOVE_SELECTION_PREVIEW),
            drawing,
            None,
            tokens,
        )
    }

    /// The dialog's subtitle: how many items the move takes.
    pub(crate) fn move_selection_subtitle(&self) -> String {
        let count = self.interaction_state.active_canvas().selected.len();
        self.ui_state.i18n.message_plain(
            ids::DIALOG_MOVE_SELECTION_COUNT,
            &[("count", Arg::from(count))],
        )
    }

    /// The help line: the axis directions and the move in whole grid steps,
    /// or "off grid" for a part step.
    pub(crate) fn move_selection_help(&self, dx_mm: f64, dy_mm: f64) -> String {
        let i18n = &self.ui_state.i18n;
        let unit = self.ui_state.unit;
        let grid_mm = crate::canvas::grid::pitch_mm_exact(self.ui_state.grid_size_mm);
        let grid = LengthFormat::new(i18n, unit).number(grid_mm);
        let unit = unit.to_string();
        match (grid_steps(dx_mm, grid_mm), grid_steps(dy_mm, grid_mm)) {
            (Some(x), Some(y)) => i18n.message_plain(
                ids::DIALOG_MOVE_SELECTION_HELP,
                &[
                    ("grid", Arg::from(grid)),
                    ("unit", Arg::from(unit)),
                    ("x", Arg::from(x)),
                    ("y", Arg::from(y)),
                ],
            ),
            _ => i18n.message_plain(
                ids::DIALOG_MOVE_SELECTION_HELP_OFF_GRID,
                &[("grid", Arg::from(grid)), ("unit", Arg::from(unit))],
            ),
        }
    }
}

/// One offset field: its caption over the input, the unit as a suffix
/// inside the field. Enter moves.
fn offset_field<'a>(
    label: signex_i18n::Localized,
    value: &str,
    id: Option<iced::widget::Id>,
    unit: Unit,
    on_input: fn(String) -> Message,
    app: &Signex,
) -> Element<'a, Message> {
    use iced::widget::{Widget, column, container, stack};
    use signex_widgets::{text, text_input};
    let tokens = &app.document_view.panel_ctx.tokens;
    let muted = crate::styles::ti(tokens.text_secondary);
    let ink3 = crate::styles::tokens::resolve(tokens).ink3;
    let suffix = UserText::new(unit.to_string());
    let refused = offset_mm(value, unit).is_err();
    let mut input = text_input(
        t!(app.ui_state.i18n, ids::PLACEHOLDER_MOVE_OFFSET),
        value.to_owned(),
    )
    .style(crate::panels::form_look::refusable_input_style(
        refused.then(|| crate::styles::ti(tokens.error)),
    ))
    .on_input(on_input)
    .on_submit(Message::MoveSelection(MoveSelectionMsg::Apply))
    .padding(iced::Padding {
        top: SP.s,
        bottom: SP.s,
        left: SP.m,
        // Room for the suffix: about three characters.
        right: SP.m + 3.0 * TS.caption,
    })
    .size(TS.label);
    if let Some(id) = id {
        input = input.id(id);
    }
    // The suffix sits on the field and takes no input, so presses reach it.
    let unit_suffix = container(text(suffix).size(TS.caption).color(ink3))
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .padding([0.0, SP.m])
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Center);
    column![
        text(label).size(TS.caption).color(muted),
        stack![input, unit_suffix],
    ]
    .spacing(SP.s)
    .width(iced::Length::Fill)
    .boxed()
}

/// One piece of the selection's line art, in sheet mm (y down).
#[derive(Debug, Clone, PartialEq)]
enum Art {
    /// A part outline or pin, or a wire or bus (`is_wire`), through `points`.
    Path {
        points: Vec<SheetPoint>,
        is_closed: bool,
        is_wire: bool,
    },
    /// A part's circle or a junction dot.
    Circle { center: SheetPoint, radius: f64 },
    /// A part's designator over its body, or a net label's text (`is_net`).
    Name {
        at: SheetPoint,
        text: String,
        is_net: bool,
    },
}

/// The line art of each selected object, once per object (mockup row 66):
/// parts with their designators, wires, buses, junctions, no-connects and
/// net labels. Other kinds draw nothing.
fn selection_art(
    sheet: &SchematicSheet,
    selected: &[SelectedItem],
    designator: impl Fn(&Symbol) -> String,
) -> Vec<Art> {
    let mut seen = std::collections::HashSet::new();
    let mut art = Vec::new();
    for item in selected.iter().filter(|item| seen.insert(item.uuid)) {
        item_art(sheet, item, &designator, &mut art);
    }
    art
}

/// The art of one selected object, appended to `out`.
fn item_art(
    sheet: &SchematicSheet,
    item: &SelectedItem,
    designator: &impl Fn(&Symbol) -> String,
    out: &mut Vec<Art>,
) {
    let wire = |a: SheetPoint, b: SheetPoint| Art::Path {
        points: vec![a, b],
        is_closed: false,
        is_wire: true,
    };
    let id = item.uuid;
    match item.kind {
        SelectedKind::Symbol | SelectedKind::SymbolRefField | SelectedKind::SymbolValField => {
            if let Some(symbol) = sheet.symbols.iter().find(|s| s.uuid == id) {
                let name = item_name(sheet, item, designator);
                symbol_art(sheet, symbol, name, out);
            }
        }
        SelectedKind::Wire => {
            if let Some(w) = sheet.wires.iter().find(|w| w.uuid == id) {
                out.push(wire(w.start, w.end));
            }
        }
        SelectedKind::Bus => {
            if let Some(b) = sheet.buses.iter().find(|b| b.uuid == id) {
                out.push(wire(b.start, b.end));
            }
        }
        SelectedKind::Junction => {
            if let Some(j) = sheet.junctions.iter().find(|j| j.uuid == id) {
                out.push(Art::Circle {
                    center: j.position,
                    radius: JUNCTION_RADIUS_MM,
                });
            }
        }
        SelectedKind::NoConnect => {
            if let Some(nc) = sheet.no_connects.iter().find(|nc| nc.uuid == id) {
                let (p, h) = (nc.position, NO_CONNECT_HALF_MM);
                for s in [-1.0, 1.0] {
                    out.push(Art::Path {
                        points: vec![
                            SheetPoint::new(p.x - h, p.y - s * h),
                            SheetPoint::new(p.x + h, p.y + s * h),
                        ],
                        is_closed: false,
                        is_wire: false,
                    });
                }
            }
        }
        SelectedKind::Label => {
            let label = sheet.labels.iter().find(|l| l.uuid == id);
            if let (Some(label), Some(text)) = (label, item_name(sheet, item, designator)) {
                out.push(Art::Name {
                    at: label.position,
                    text: shorten(&text),
                    is_net: true,
                });
            }
        }
        _ => {}
    }
}

/// A part's lowered body and pins, and its name centred over them. A part
/// with no library body draws a small stand-in box at its place.
fn symbol_art(sheet: &SchematicSheet, symbol: &Symbol, name: Option<String>, out: &mut Vec<Art>) {
    let first = out.len();
    let lowered = lower_placed_symbol(sheet.body_of(symbol), symbol, &SchematicDefaults::BUILT_IN);
    for primitive in &lowered {
        let path = |points: Vec<SheetPoint>, is_closed: bool| Art::Path {
            points,
            is_closed,
            is_wire: false,
        };
        out.push(match primitive {
            SymbolPrimitive::Path { points, closed, .. } => path(points.clone(), *closed),
            SymbolPrimitive::Circle { center, radius, .. } => Art::Circle {
                center: *center,
                radius: *radius,
            },
            SymbolPrimitive::Arc {
                start, mid, end, ..
            } => path(arc_polyline(*start, *mid, *end), false),
            SymbolPrimitive::Pin { tip, body, .. } => path(vec![*tip, *body], false),
            SymbolPrimitive::Text { .. } => continue,
        });
    }
    if art_bounds(&out[first..]).is_none() {
        let (p, h) = (symbol.position, MISSING_BODY_HALF_MM);
        out.push(Art::Path {
            points: vec![
                SheetPoint::new(p.x - h, p.y - h),
                SheetPoint::new(p.x + h, p.y - h),
                SheetPoint::new(p.x + h, p.y + h),
                SheetPoint::new(p.x - h, p.y + h),
            ],
            is_closed: true,
            is_wire: false,
        });
    }
    if let (Some(text), Some((lo, hi))) = (name, art_bounds(&out[first..])) {
        out.push(Art::Name {
            at: SheetPoint::new((lo.x + hi.x) / 2.0, lo.y),
            text: shorten(&text),
            is_net: false,
        });
    }
}

/// `text` cut to the longest name the preview draws.
fn shorten(text: &str) -> String {
    text.chars().take(NAME_MAX_CHARS).collect()
}

/// The sheet box `art` covers, corners `(min, max)`; `None` for no art.
fn art_bounds(art: &[Art]) -> Option<(SheetPoint, SheetPoint)> {
    let mut points = Vec::new();
    for piece in art {
        match piece {
            Art::Path { points: p, .. } => points.extend_from_slice(p),
            Art::Circle { center, radius } => {
                points.push(SheetPoint::new(center.x - radius, center.y - radius));
                points.push(SheetPoint::new(center.x + radius, center.y + radius));
            }
            Art::Name { at, .. } => points.push(*at),
        }
    }
    let first = *points.first()?;
    Some(points.iter().fold((first, first), |(lo, hi), p| {
        (
            SheetPoint::new(lo.x.min(p.x), lo.y.min(p.y)),
            SheetPoint::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    }))
}

/// A part's shown designator (a power port's net name) or a label's text.
fn item_name(
    sheet: &SchematicSheet,
    item: &SelectedItem,
    designator: &impl Fn(&Symbol) -> String,
) -> Option<String> {
    let name = match item.kind {
        SelectedKind::Symbol | SelectedKind::SymbolRefField | SelectedKind::SymbolValField => {
            let symbol = sheet.symbols.iter().find(|s| s.uuid == item.uuid)?;
            if symbol.is_power {
                symbol.value.clone()
            } else {
                designator(symbol)
            }
        }
        SelectedKind::Label => sheet
            .labels
            .iter()
            .find(|l| l.uuid == item.uuid)?
            .text
            .clone(),
        _ => return None,
    };
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// The preview: on the sheet grid, each selected object drawn as a dashed
/// ghost at its place and drawn with its name at its new place, the moved
/// copy framed, with a ΔX and a ΔY callout (mockup row 66). Read-only line
/// art. With no art (nothing drawable selected) a stand-in box moves. A
/// positive ΔY draws upward; the callout shows the value as typed.
fn move_preview(
    dx_mm: f64,
    dy_mm: f64,
    grid_mm: f64,
    art: &[Art],
    fmt: &LengthFormat<'_>,
) -> Illustration {
    let step = if grid_mm.is_finite() && grid_mm > 0.0 {
        grid_mm
    } else {
        1.0
    };
    let stand_in: Vec<Art>;
    let art = if art.is_empty() {
        stand_in = stand_in_art(dx_mm, dy_mm, step);
        &stand_in[..]
    } else {
        art
    };
    let shift = SheetPoint::new(dx_mm, stored_dy(dy_mm));
    let (lo, hi) = art_bounds(art).unwrap_or((SheetPoint::ZERO, SheetPoint::ZERO));
    let (min_x, max_x) = (lo.x.min(lo.x + shift.x), hi.x.max(hi.x + shift.x));
    let (min_y, max_y) = (lo.y.min(lo.y + shift.y), hi.y.max(hi.y + shift.y));
    let span = (max_x - min_x)
        .max(max_y - min_y)
        .max(step * MIN_SPAN_STEPS);
    let k = f64::from(SPAN) / span;
    let view = |p: SheetPoint| {
        Point::new(
            MARGIN + ((p.x - min_x) * k) as f32,
            MARGIN + ((p.y - min_y) * k) as f32,
        )
    };
    let view_w = ((max_x - min_x) * k) as f32 + 2.0 * MARGIN;
    let view_h = ((max_y - min_y) * k) as f32 + 2.0 * MARGIN;
    let sheet_origin = view(SheetPoint::ZERO);
    let drawing = Illustration::new(view_w, view_h)
        .height_em(PREVIEW_EM)
        .parts(crate::paste_array::preview_grid(
            step * k,
            f64::from(sheet_origin.x),
            f64::from(sheet_origin.y),
            f64::from(view_w),
            f64::from(view_h),
        ))
        .parts(art.iter().map(|piece| art_part(piece, &view, k, true)));
    if dx_mm == 0.0 && dy_mm == 0.0 {
        return drawing;
    }
    let moved = |p: SheetPoint| view(SheetPoint::new(p.x + shift.x, p.y + shift.y));
    let (frame_lo, frame_hi) = (moved(lo), moved(hi));
    let frame = Primitive::RoundedRect {
        rect: Rectangle::new(
            Point::new(frame_lo.x - FRAME_PAD, frame_lo.y - FRAME_PAD),
            Size::new(
                frame_hi.x - frame_lo.x + 2.0 * FRAME_PAD,
                frame_hi.y - frame_lo.y + 2.0 * FRAME_PAD,
            ),
        ),
        radius: FRAME_PAD / 2.0,
    };
    let frame_style = Style::dashed()
        .with_stroke_paint(Paint::Ink(Ink::Accent))
        .with_fill(Paint::Ink(Ink::Accent), Tint::Wash);
    let (dx, dy) = ((shift.x * k) as f32, (shift.y * k) as f32);
    drawing
        .part(Part::new(frame, frame_style))
        .parts(art.iter().map(|piece| art_part(piece, &moved, k, false)))
        .parts(offset_callouts(
            view(SheetPoint::new(lo.x, hi.y)),
            view(SheetPoint::new(hi.x, lo.y)),
            (dx, dy),
            (dx_mm, dy_mm),
            (
                view(SheetPoint::new(0.0, max_y)).y + CALLOUT_GAP,
                view(SheetPoint::new(max_x, 0.0)).x + CALLOUT_GAP,
            ),
            fmt,
        ))
}

/// What moves when nothing drawable is selected: a box sized from the
/// offset, so the move still reads.
fn stand_in_art(dx_mm: f64, dy_mm: f64, step: f64) -> Vec<Art> {
    let w = (dx_mm.abs().max(dy_mm.abs()) * 0.75).max(step * 3.0);
    let h = w * 0.55;
    vec![Art::Path {
        points: vec![
            SheetPoint::new(0.0, 0.0),
            SheetPoint::new(w, 0.0),
            SheetPoint::new(w, h),
            SheetPoint::new(0.0, h),
        ],
        is_closed: true,
        is_wire: false,
    }]
}

/// One piece of art as a drawn part at `at`'s place, `k` view units per mm.
/// A ghost is dashed in the construction ink; the moved copy draws parts in
/// the edge ink and wires and net names in the accent.
fn art_part(piece: &Art, at: &impl Fn(SheetPoint) -> Point, k: f64, is_ghost: bool) -> Part {
    let stroke = |is_wire: bool| match (is_ghost, is_wire) {
        (true, _) => Style::dashed(),
        (false, true) => Style::outline(Ink::Accent, Weight::Regular),
        (false, false) => Style::edge(),
    };
    match piece {
        Art::Path {
            points,
            is_closed,
            is_wire,
        } => Part::new(
            Primitive::Polyline {
                points: points.iter().map(|p| at(*p)).collect(),
                is_closed: *is_closed,
            },
            stroke(*is_wire),
        ),
        Art::Circle { center, radius } => Part::new(
            Primitive::Circle {
                center: at(*center),
                radius: (radius * k) as f32,
            },
            stroke(false),
        ),
        Art::Name {
            at: place,
            text,
            is_net,
        } => {
            let p = at(*place);
            let ink = match (is_ghost, is_net) {
                (true, _) => Ink::Faint,
                (false, true) => Ink::Accent,
                (false, false) => Ink::Secondary,
            };
            Part::label(
                TextLabel {
                    at: Point::new(p.x, p.y - NAME_LIFT),
                    text: UserText::new(text.clone()).into(),
                    anchor: Anchor::Middle,
                    role: TextRole::Mono,
                },
                ink,
            )
        }
    }
}

/// The ΔX callout along `bottom` from the selection's lower-left corner
/// `x_from`, and the ΔY callout along `right` from its upper-right corner
/// `y_from`, in view units; an axis that does not move has none.
fn offset_callouts(
    x_from: Point,
    y_from: Point,
    (dx, dy): (f32, f32),
    (dx_mm, dy_mm): (f64, f64),
    (bottom, right): (f32, f32),
    fmt: &LengthFormat<'_>,
) -> Vec<Part> {
    let label = |mm: f64| UserText::new(format!("{} {}", fmt.number(mm), fmt.unit));
    let mut parts = Vec::new();
    if dx != 0.0 {
        let (from, to) = (x_from.x, x_from.x + dx);
        parts.push(Part::callout(
            Callout::new(Point::new(from, bottom), Point::new(to, bottom))
                .label(label(dx_mm))
                .extension(x_from, Point::new(from, bottom))
                .extension(Point::new(to, x_from.y + dy), Point::new(to, bottom)),
        ));
    }
    if dy != 0.0 {
        let (from, to) = (y_from.y, y_from.y + dy);
        parts.push(Part::callout(
            Callout::new(Point::new(right, from), Point::new(right, to))
                .label(label(dy_mm))
                .extension(y_from, Point::new(right, from))
                .extension(Point::new(y_from.x + dx, to), Point::new(right, to)),
        ));
    }
    parts
}

#[cfg(test)]
mod tests {
    use crate::app::state::ModalId;
    use signex_types::schematic::{SelectedItem, SelectedKind};

    fn app_with_selection(count: usize) -> crate::app::Signex {
        let (mut app, _boot) = crate::app::Signex::new();
        app.interaction_state.active_canvas_mut().selected = (0..count)
            .map(|_| SelectedItem::new(uuid::Uuid::now_v7(), SelectedKind::Wire))
            .collect();
        app
    }

    /// The count moves to the header subtitle with a real plural (#66): the
    /// locale groups it as `I18n::count` does. The fields name the axes
    /// without the unit, which is their suffix.
    #[test]
    fn move_selection_names_the_count_and_the_offsets() {
        let app = app_with_selection(1200);
        assert_eq!(
            app.detached_modal_subtitle(ModalId::MoveSelection)
                .as_deref(),
            Some("1,200 items selected")
        );
        assert_eq!(
            app_with_selection(1).move_selection_subtitle(),
            "1 item selected"
        );
        let texts: Vec<String> = crate::layout_probe::lay_out_texts(
            app.view_move_selection_body(),
            iced::Size::new(360.0, 400.0),
        )
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
        for expected in ["ΔX", "ΔY", "Cancel", "Move"] {
            assert!(
                texts.iter().any(|t| t == expected),
                "{expected:?} not in {texts:?}"
            );
        }
    }

    /// A sheet with part R3 (no library body), net label VOUT and a
    /// no-connect, and the selection of all three plus R3's designator and a
    /// wire the sheet no longer holds.
    fn sheet_and_selection() -> (signex_types::schematic::SchematicSheet, Vec<SelectedItem>) {
        use signex_types::schematic::{Label, LabelType, NoConnect, Point, SchematicSheet, Symbol};
        let id = uuid::Uuid::from_u128;
        let sheet = SchematicSheet {
            symbols: vec![Symbol {
                uuid: id(1),
                reference: "R3".into(),
                position: Point::new(10.16, 5.08),
                ..Default::default()
            }],
            labels: vec![Label {
                uuid: id(2),
                ..Label::new("VOUT", Point::new(20.32, 5.08), LabelType::Net)
            }],
            no_connects: vec![NoConnect {
                uuid: id(4),
                position: Point::new(25.4, 7.62),
            }],
            ..Default::default()
        };
        let selected = vec![
            SelectedItem::new(id(1), SelectedKind::Symbol),
            SelectedItem::new(id(1), SelectedKind::SymbolRefField),
            SelectedItem::new(id(2), SelectedKind::Label),
            SelectedItem::new(id(3), SelectedKind::Wire),
            SelectedItem::new(id(4), SelectedKind::NoConnect),
        ];
        (sheet, selected)
    }

    /// Mockup row 66: the preview draws each selected object, once per
    /// object, each with its own name; no summary of the selection.
    #[test]
    fn the_preview_draws_each_selected_object_once_with_its_name() {
        use super::Art;
        let (sheet, selected) = sheet_and_selection();
        let art = super::selection_art(&sheet, &selected, |s| s.reference.clone());
        let names: Vec<(&str, bool)> = art
            .iter()
            .filter_map(|piece| match piece {
                Art::Name { text, is_net, .. } => Some((text.as_str(), *is_net)),
                _ => None,
            })
            .collect();
        assert_eq!(names, [("R3", false), ("VOUT", true)]);
        let paths = art
            .iter()
            .filter(|piece| matches!(piece, Art::Path { .. }))
            .count();
        assert_eq!(paths, 3, "R3's stand-in box and the two no-connect strokes");
        let only_wire = super::selection_art(&sheet, &selected[3..4], |s| s.reference.clone());
        assert!(only_wire.is_empty(), "a wire the sheet lost draws nothing");
    }

    /// Mockup row 66 (#2847): every object is drawn twice, a dashed ghost at
    /// its place and a solid copy at its new place, each copy with its name;
    /// one frame marks the moved copy. With no offset only the ghosts draw.
    #[test]
    fn the_preview_draws_a_ghost_and_a_moved_copy_of_each_object() {
        use signex_widgets::property_illustration::Primitive;
        let en = signex_i18n::I18n::en_us();
        let fmt = signex_widgets::LengthFormat::new(&en, signex_types::coord::Unit::Mm);
        let (sheet, selected) = sheet_and_selection();
        let art = super::selection_art(&sheet, &selected, |s| s.reference.clone());
        let polylines = |ill: &super::Illustration, dashed: bool| {
            ill.visible_parts()
                .filter(|p| matches!(p.primitive, Primitive::Polyline { .. }))
                .filter(|p| p.style.stroke.is_some_and(|s| s.is_dashed == dashed))
                .count()
        };
        let labels = |ill: &super::Illustration, text: &str| {
            ill.visible_parts()
                .filter(|p| matches!(&p.primitive, Primitive::Label(l) if l.text.as_str() == text))
                .count()
        };
        let frames = |ill: &super::Illustration| {
            ill.visible_parts()
                .filter(|p| matches!(p.primitive, Primitive::RoundedRect { .. }))
                .count()
        };
        let moved = super::move_preview(5.08, -2.54, 1.27, &art, &fmt);
        assert_eq!(polylines(&moved, true), 3, "a ghost per drawn stroke");
        assert_eq!(polylines(&moved, false), 3, "a moved copy per drawn stroke");
        assert_eq!(labels(&moved, "R3"), 2);
        assert_eq!(labels(&moved, "VOUT"), 2);
        assert_eq!(frames(&moved), 1, "the moved copy is framed");
        let still = super::move_preview(0.0, 0.0, 1.27, &art, &fmt);
        assert_eq!(polylines(&still, true), 3);
        assert_eq!(polylines(&still, false), 0, "nothing moves");
        assert_eq!(labels(&still, "R3"), 1);
        assert_eq!(frames(&still), 0);
    }

    /// Mockup row 66: the drawing has the sheet grid and the name on the
    /// moved object, and with no offset (and nothing drawable selected)
    /// still shows the grid and the ghost.
    #[test]
    fn the_preview_draws_the_grid_and_the_names() {
        use signex_widgets::property_illustration::Primitive;
        let en = signex_i18n::I18n::en_us();
        let fmt = signex_widgets::LengthFormat::new(&en, signex_types::coord::Unit::Mm);
        let lines = |ill: &super::Illustration| {
            ill.visible_parts()
                .filter(|p| matches!(p.primitive, Primitive::Line { .. }))
                .count()
        };
        let (sheet, selected) = sheet_and_selection();
        let art = super::selection_art(&sheet, &selected[..1], |s| s.reference.clone());
        let moved = super::move_preview(5.08, 2.54, 1.27, &art, &fmt);
        assert!(lines(&moved) > 0, "the sheet grid is drawn");
        assert!(moved.visible_parts().any(|p| match &p.primitive {
            Primitive::Label(l) => l.text.as_str() == "R3",
            _ => false,
        }));
        let still = super::move_preview(0.0, 0.0, 1.27, &[], &fmt);
        assert!(
            lines(&still) > 0,
            "the grid shows before an offset is typed"
        );
        assert!(
            still
                .visible_parts()
                .any(|p| matches!(p.primitive, Primitive::Polyline { .. })),
            "a stand-in ghost shows with nothing drawable selected"
        );
    }

    /// The help line counts whole grid steps per axis and says "off grid"
    /// for a part step.
    #[test]
    fn the_help_line_counts_grid_steps() {
        let mut app = app_with_selection(3);
        app.ui_state.unit = signex_types::coord::Unit::Mm;
        app.ui_state.grid_size_mm = 1.27;
        assert_eq!(
            app.move_selection_help(5.08, -2.54),
            "Positive X moves right, positive Y moves up. Grid 1.27 mm: 4 × 2 steps."
        );
        assert_eq!(
            app.move_selection_help(1.0, 0.0),
            "Positive X moves right, positive Y moves up. Grid 1.27 mm: off grid."
        );
    }
}
