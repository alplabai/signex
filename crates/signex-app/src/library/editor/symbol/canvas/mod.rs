//! Symbol-tab interactive canvas.
//!
//! The canvas reads the typed [`signex_library::Symbol`] primitive
//! directly. The body rectangle is derived from `Symbol.graphics`
//! (first `Rectangle` graphic), or defaults to a
//! `[-5.08, -2.54] .. [5.08, 2.54]` rectangle when the primitive
//! carries no body geometry yet.
//!
//! World-space convention mirrors the schematic editor: Standard y-axis
//! (positive going up; on screen y goes down so we flip). The
//! camera ([`crate::canvas::Camera`]) handles pan/zoom; the user
//! pans with right- or middle-button drag and zooms with the wheel.
//! Press Home (or click the Fit button) to fit the symbol bbox to
//! the viewport — also the implicit state on tab open.
//!
//! Background colour, grid size + visibility, snap, and the cursor
//! coordinate readout follow the same Altium-parity surface as the
//! schematic canvas: bg + grid colour come from the active theme's
//! `CanvasColors`; grid spacing follows `panel_ctx.grid_size_mm`;
//! the unit ([`signex_types::coord::Unit`]) drives the status
//! footer. Sheet colour is per-tab (Altium "Document Options")
//! and shifts the bg fill alpha so the user can pick Black / White
//! / Dark Gray / Light Gray / Cream per-symbol library.

use iced::Color;
use iced::Rectangle;
use iced::Renderer;
use iced::Theme;
use iced::event::Event;
use iced::mouse;
use iced::widget::canvas;
use signex_gfx::scene::{DirtyFlags, Scene};
use signex_library::{Symbol, SymbolGraphicKind, SymbolPin};
use signex_renderer::schematic::{
    ArcInput, JunctionInput, OverlayInputs, PolygonInput, SchematicRenderer,
    SchematicSnapshot as RendererSnapshot, ViewRenderer, WireInput,
};
use signex_renderer::theme::ResolvedTheme;
use signex_types::schematic::{HAlign, VAlign};
use std::collections::HashMap;

use super::state::{self, SymbolSelection};

mod draw;
mod geometry;
mod input;
mod pins;
mod types;

pub use types::{CanvasAction, CanvasState, RotatePivotMode, SymbolTool};

use pins::{PIN_TEXT_LAYOUT, PinRenderGeometry, SymbolPalette};

// Re-export the free math / coordinate helpers so the `input` and
// `draw` submodules keep resolving them via their `use super::super::*`
// globs. `pub(in …canvas)` matches their `pub(super)` definitions in
// `geometry.rs` — visible across the canvas subtree, nowhere wider.
pub(in crate::library::editor::symbol::canvas) use geometry::{
    circle_vertices, screen_px_to_world_mm, selection_anchor, stroke_px_at_zoom, stroke_world_mm,
    text_size_px_from_mm, to_rgba, unwrap_angle, world_for, world_unsnapped,
};

/// Builder for the per-render [`SymbolCanvas`] — all the inputs the
/// canvas needs from the surrounding state. The canvas itself is
/// constructed fresh on every iced view tick (see
/// `library/editor/standalone.rs::view_symbol_canvas`).
pub struct SymbolCanvas<'a> {
    pub symbol: &'a Symbol,
    pub selected: Option<SymbolSelection>,
    pub tool: SymbolTool,
    /// Active sub-part the canvas is filtering pins for. Pins with
    /// `part_number == 0` (Part Zero) render on every part; pins
    /// with `part_number == active_part` render on the active part
    /// only. Defaults to `1` (single-part components).
    pub active_part: u8,
    /// Pan/zoom state owned by the editor tab — see
    /// [`crate::app::SymbolEditorState::camera`].
    pub camera: &'a crate::canvas::Camera,
    /// Visible grid spacing in mm — sourced from
    /// `panel_ctx.grid_size_mm` so the schematic + library editors
    /// share the global grid setting.
    pub grid_size_mm: f64,
    /// Whether the grid is rendered. Sourced from
    /// `panel_ctx.grid_visible` (View ▸ Toggle Grid / status-bar
    /// click).
    pub grid_visible: bool,
    pub bg_color: Color,
    pub grid_color: Color,
    pub body_color: Color,
    pub pin_color: Color,
    pub selected_color: Color,
    pub text_color: Color,
    /// Stroke color for the X/Y axis lines through world (0, 0) —
    /// Altium-style "centre crosshair" so the symbol's anchor is
    /// always visible.
    pub axis_color: Color,
}

impl<'a> SymbolCanvas<'a> {
    /// Construct the per-frame canvas with the inputs from
    /// `SymbolEditorState` + the active theme + global grid/unit
    /// settings. See module-level docs for the parity rationale.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: &'a Symbol,
        selected: Option<SymbolSelection>,
        tool: SymbolTool,
        active_part: u8,
        camera: &'a crate::canvas::Camera,
        grid_size_mm: f64,
        grid_visible: bool,
        sheet_color: Color,
        accent_color: Color,
        _body_color_unused: Color,
        _text_color_unused: Color,
        _grid_color_unused: Color,
    ) -> Self {
        // Pick canvas-content colours based on sheet luminance —
        // Altium-style. Light sheets (Cream/White/LightGray) want
        // dark body strokes + black text; dark sheets keep the
        // signature yellow body + light text. Theme-text colours
        // are ignored — they're tuned for the surrounding panel
        // chrome and read as washed-out on a Cream sheet.
        let palette = SymbolPalette::for_sheet(sheet_color);
        Self {
            symbol,
            selected,
            tool,
            active_part,
            camera,
            grid_size_mm,
            grid_visible,
            bg_color: sheet_color,
            grid_color: palette.grid,
            body_color: palette.body,
            pin_color: palette.pin,
            selected_color: accent_color,
            text_color: palette.text,
            axis_color: palette.axis,
        }
    }

    /// True when `pin` should render on the currently-active part.
    /// Part Zero (`part_number == 0`) appears on every part; other
    /// pins only render when they match `active_part`.
    fn pin_visible_on_active_part(&self, pin: &SymbolPin) -> bool {
        pin.part_number == 0 || pin.part_number == self.active_part
    }

    /// Body rectangle, when present, derived from the first
    /// `SymbolGraphicKind::Rectangle` in `symbol.graphics`.
    fn body_rect(&self) -> Option<(f64, f64, f64, f64)> {
        for g in &self.symbol.graphics {
            if let SymbolGraphicKind::Rectangle { from, to } = &g.kind {
                return Some((from[0], from[1], to[0], to[1]));
            }
        }
        None
    }

    /// Bounding box around every visible symbol entity.
    ///
    /// If the symbol has no pins and no graphics, return a tiny box
    /// around world origin so Fit keeps the origin marker centered.
    pub(crate) fn bbox(&self) -> (f64, f64, f64, f64) {
        let mut bounds: Option<(f64, f64, f64, f64)> = None;
        let include_rect =
            |bounds: &mut Option<(f64, f64, f64, f64)>, x0: f64, y0: f64, x1: f64, y1: f64| {
                let rx0 = x0.min(x1);
                let ry0 = y0.min(y1);
                let rx1 = x0.max(x1);
                let ry1 = y0.max(y1);
                if let Some((min_x, min_y, max_x, max_y)) = bounds.as_mut() {
                    *min_x = (*min_x).min(rx0);
                    *min_y = (*min_y).min(ry0);
                    *max_x = (*max_x).max(rx1);
                    *max_y = (*max_y).max(ry1);
                } else {
                    *bounds = Some((rx0, ry0, rx1, ry1));
                }
            };

        if let Some((bx0, by0, bx1, by1)) = self.body_rect() {
            include_rect(
                &mut bounds,
                bx0.min(bx1) - 5.08,
                by0.min(by1) - 5.08,
                bx0.max(bx1) + 5.08,
                by0.max(by1) + 5.08,
            );
        }

        for pin in &self.symbol.pins {
            include_rect(
                &mut bounds,
                pin.position[0] - 1.27,
                pin.position[1] - 1.27,
                pin.position[0] + pin.length + 1.27,
                pin.position[1] + 1.27,
            );
        }

        // Include every graphic's extent so Fit doesn't leave shapes
        // off-screen.
        for g in &self.symbol.graphics {
            match &g.kind {
                SymbolGraphicKind::Rectangle { from, to }
                | SymbolGraphicKind::Line { from, to } => {
                    include_rect(&mut bounds, from[0], from[1], to[0], to[1]);
                }
                SymbolGraphicKind::Circle { center, radius }
                | SymbolGraphicKind::Arc { center, radius, .. } => {
                    include_rect(
                        &mut bounds,
                        center[0] - radius,
                        center[1] - radius,
                        center[0] + radius,
                        center[1] + radius,
                    );
                }
                SymbolGraphicKind::Text { position, size, .. } => {
                    include_rect(
                        &mut bounds,
                        position[0] - size,
                        position[1] - size,
                        position[0] + size,
                        position[1] + size,
                    );
                }
            }
        }

        bounds.unwrap_or((-1.27, -1.27, 1.27, 1.27))
    }
}

/// World-space mm grid the symbol canvas snaps cursor positions
/// to when the user is placing/moving things. Independent of the
/// visible grid (which follows `panel_ctx.grid_size_mm`) so a 0.635
/// mm visible grid still snaps to 1.27 mm — Altium's "smaller grid
/// for visual precision, larger for commit". Future toolbar work
/// could expose a separate snap-grid picker.
const SNAP_GRID_MM: f64 = 1.27;
const ORIGIN_MARKER_MM: f32 = 1.27;
const MM_PER_EM: f32 = 0.72;
const SYMBOL_AXIS_STROKE_PX_AT_100: f32 = 1.0;
const SYMBOL_GRAPHIC_STROKE_PX_AT_100: f32 = 1.5;
const SYMBOL_GRAPHIC_SELECTED_STROKE_PX_AT_100: f32 = 2.5;
const SYMBOL_RECT_STROKE_PX_AT_100: f32 = 2.0;
const SYMBOL_RECT_SELECTED_STROKE_PX_AT_100: f32 = 2.5;
const SYMBOL_HANDLE_STROKE_PX_AT_100: f32 = 1.0;
const SYMBOL_PIN_SELECTION_HALO_STROKE_PX_AT_100: f32 = 1.0;

/// Returns `true` when `item` (a single-element selection) belongs to the
/// multi-element `group` selection. Used to decide whether a click on an
/// already-selected item should start a group drag rather than replace the
/// selection.
fn item_in_selection(group: &SymbolSelection, item: &SymbolSelection) -> bool {
    match (group, item) {
        (SymbolSelection::All, _) => true,
        (SymbolSelection::Multiple { pin_indices, .. }, SymbolSelection::Pin(idx)) => {
            pin_indices.contains(idx)
        }
        (
            SymbolSelection::Multiple {
                graphic_indices, ..
            },
            SymbolSelection::Graphic(idx),
        ) => graphic_indices.contains(idx),
        _ => false,
    }
}

/// Returns `true` when the graphic at `idx` should be drawn in the
/// selection colour. Handles single-graphic, Multiple, and All selections.
fn is_graphic_selected(sel: &Option<SymbolSelection>, idx: usize) -> bool {
    match sel {
        Some(SymbolSelection::Graphic(i)) => *i == idx,
        Some(SymbolSelection::Multiple {
            graphic_indices, ..
        }) => graphic_indices.contains(&idx),
        Some(SymbolSelection::All) => true,
        _ => false,
    }
}

impl<'a> canvas::Program<CanvasAction> for SymbolCanvas<'a> {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasAction>> {
        // Thin dispatcher — each event kind routes to its extracted
        // `impl SymbolCanvas` method in `input::{tools, pointer,
        // camera, keys}`. Arm order + patterns are identical to the
        // pre-split god-function; behaviour is byte-identical.
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                self.on_left_press(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
            | Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                self.on_secondary_press(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right))
            | Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                self.on_secondary_release(state)
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                self.on_cursor_moved(state, bounds, cursor)
            }
            Event::Mouse(mouse::Event::CursorLeft) => self.on_cursor_left(state),
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                self.on_wheel_scrolled(delta, bounds, cursor)
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                self.on_left_release(state)
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                self.on_key_pressed(state, key, modifiers)
            }
            _ => None,
        }
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.panning {
            return mouse::Interaction::Grabbing;
        }
        // While actively dragging a resize handle, keep the cursor that
        // matches the handle so it doesn't flicker back to Crosshair.
        if let Some((_, handle)) = state.dragging_handle {
            return state::handle_interaction(handle);
        }
        if state.dragging {
            return mouse::Interaction::Grab;
        }
        // Hover detection: change the cursor when the pointer is close
        // to any graphic handle. Tolerance is expressed in screen pixels
        // so it feels the same at all zoom levels.
        if self.tool == SymbolTool::Select {
            if let Some(pos) = cursor.position_in(bounds) {
                let (wx, wy) = world_unsnapped(self, pos.x, pos.y, bounds);
                let tol_mm = (8.0_f32 / self.camera.scale.max(0.01)).clamp(0.5, 4.0) as f64;
                if let Some((_, handle)) =
                    state::hit_test_graphic_handle(self.symbol, wx, wy, tol_mm)
                {
                    return state::handle_interaction(handle);
                }
            }
        }
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        // Layers are painted bottom-to-top; this z-order is
        // load-bearing and identical to the pre-split god-function.
        // Each layer lives in the `draw` submodule, except the symbol
        // body which flows through `draw_symbol_with_renderer`.
        self.draw_background(&mut frame, bounds);
        self.draw_grid(&mut frame, bounds);
        self.draw_origin_marker(&mut frame, bounds);
        self.draw_symbol_with_renderer(&mut frame, &self.selected, self.camera.scale);
        self.draw_resize_handles(&mut frame);
        self.draw_tool_hint(&mut frame);
        self.draw_box_select_overlay(&mut frame, state);
        self.draw_line_preview(&mut frame, state);
        self.draw_circle_preview(&mut frame, state);
        self.draw_arc_preview(&mut frame, state);

        vec![frame.into_geometry()]
    }
}

impl<'a> SymbolCanvas<'a> {
    fn draw_symbol_with_renderer(
        &self,
        frame: &mut canvas::Frame,
        selected: &Option<SymbolSelection>,
        scale: f32,
    ) {
        let snapshot = self.build_symbol_renderer_snapshot(selected, scale);
        if snapshot.wires.is_empty()
            && snapshot.junctions.is_empty()
            && snapshot.arcs.is_empty()
            && snapshot.polygons.is_empty()
            && snapshot.labels.is_empty()
            && snapshot.pin_texts.is_empty()
        {
            return;
        }

        let mut scene = Scene::default();
        SchematicRenderer::build_scene(
            &snapshot,
            &ResolvedTheme::from_canvas_colors(signex_types::theme::canvas_colors(
                signex_types::theme::ThemeId::Signex,
            )),
            DirtyFlags::LINES
                | DirtyFlags::CIRCLES
                | DirtyFlags::ARCS
                | DirtyFlags::POLYGONS
                | DirtyFlags::TEXT,
            &mut scene,
        );

        let ox = self.camera.offset.x;
        let oy = self.camera.offset.y;
        crate::renderer_scene_canvas::draw_scene_with_world_to_screen(
            frame,
            &scene,
            |point| iced::Point::new(ox + point[0] * scale, oy - point[1] * scale),
            crate::renderer_scene_canvas::SceneDrawOptions {
                scale_px_per_mm: scale,
                min_stroke_px: signex_types::schematic::SCHEMATIC_RENDER_MIN_STROKE_PX,
                text_mm_per_em: MM_PER_EM,
                text_min_px: 2.0,
                text_max_px: 96.0,
            },
        );
    }

    fn build_symbol_renderer_snapshot(
        &self,
        selected: &Option<SymbolSelection>,
        scale: f32,
    ) -> RendererSnapshot {
        let mut wires = Vec::new();
        let mut junctions = Vec::new();
        let mut arcs = Vec::new();
        let mut polygons = Vec::new();
        let mut labels = Vec::new();
        let mut pin_texts = Vec::new();

        let mut body_drawn = false;
        for (i, g) in self.symbol.graphics.iter().enumerate() {
            let is_selected = is_graphic_selected(selected, i);
            let stroke_color = if is_selected {
                self.selected_color
            } else {
                self.body_color
            };
            let stroke_w = if is_selected {
                SYMBOL_GRAPHIC_SELECTED_STROKE_PX_AT_100
            } else {
                SYMBOL_GRAPHIC_STROKE_PX_AT_100
            };
            let rect_w = if is_selected {
                SYMBOL_RECT_SELECTED_STROKE_PX_AT_100
            } else {
                SYMBOL_RECT_STROKE_PX_AT_100
            };

            match &g.kind {
                SymbolGraphicKind::Rectangle { from, to } => {
                    let x0 = from[0] as f32;
                    let y0 = from[1] as f32;
                    let x1 = to[0] as f32;
                    let y1 = to[1] as f32;
                    let fill = if !body_drawn {
                        body_drawn = true;
                        to_rgba(Color {
                            a: 0.16,
                            ..self.body_color
                        })
                    } else {
                        [0.0, 0.0, 0.0, 0.0]
                    };

                    polygons.push(PolygonInput {
                        vertices: vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
                        fill_color: fill,
                        stroke_color: Some(to_rgba(stroke_color)),
                        stroke_width_mm: stroke_world_mm(rect_w, scale),
                    });
                }
                SymbolGraphicKind::Line { from, to } => {
                    wires.push(WireInput {
                        id: i as u64,
                        p0: [from[0] as f32, from[1] as f32],
                        p1: [to[0] as f32, to[1] as f32],
                        width_mm: stroke_world_mm(stroke_w, scale),
                        explicit_color: Some(to_rgba(stroke_color)),
                    });
                }
                SymbolGraphicKind::Circle { center, radius } => {
                    polygons.push(PolygonInput {
                        vertices: circle_vertices(*center, *radius as f32, 40),
                        fill_color: [0.0, 0.0, 0.0, 0.0],
                        stroke_color: Some(to_rgba(stroke_color)),
                        stroke_width_mm: stroke_world_mm(stroke_w, scale),
                    });
                }
                SymbolGraphicKind::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                } => {
                    arcs.push(ArcInput {
                        center: [center[0] as f32, center[1] as f32],
                        radius_mm: *radius as f32,
                        start_angle_rad: (*start_deg as f32).to_radians(),
                        end_angle_rad: (*end_deg as f32).to_radians(),
                        width_mm: stroke_world_mm(stroke_w, scale),
                        color: to_rgba(stroke_color),
                    });
                }
                SymbolGraphicKind::Text {
                    position,
                    content,
                    size,
                } => {
                    labels.push(signex_renderer::schematic::TextInput {
                        content: content.clone(),
                        position: [position[0] as f32, position[1] as f32],
                        size_mm: (*size as f32).max(0.1),
                        color: to_rgba(if is_selected {
                            self.selected_color
                        } else {
                            self.text_color
                        }),
                        bold: false,
                        italic: false,
                        rotation_rad: 0.0,
                        h_align: HAlign::Left,
                        v_align: VAlign::Top,
                    });
                }
            }
        }

        for (i, pin) in self.symbol.pins.iter().enumerate() {
            if !self.pin_visible_on_active_part(pin) {
                continue;
            }

            let geom = PinRenderGeometry::compute(pin);
            let selected = match selected {
                Some(SymbolSelection::Pin(j)) => *j == i,
                Some(SymbolSelection::Multiple { pin_indices, .. }) => pin_indices.contains(&i),
                Some(SymbolSelection::All) => true,
                _ => false,
            };
            let stroke_color = if selected {
                self.selected_color
            } else {
                self.pin_color
            };

            let tip_f32 = [geom.tip.x as f32, geom.tip.y as f32];
            let body_f32 = [geom.body_end.x as f32, geom.body_end.y as f32];

            wires.push(WireInput {
                id: 100_000 + i as u64,
                p0: tip_f32,
                p1: body_f32,
                width_mm: stroke_world_mm(
                    if selected {
                        signex_types::schematic::PIN_STROKE_SELECTED_PX
                    } else {
                        signex_types::schematic::PIN_STROKE_PX
                    },
                    scale,
                ),
                explicit_color: Some(to_rgba(stroke_color)),
            });

            junctions.push(JunctionInput {
                center: tip_f32,
                radius_mm: screen_px_to_world_mm(2.5, scale),
                color: to_rgba(stroke_color),
            });

            if selected {
                polygons.push(PolygonInput {
                    vertices: circle_vertices(
                        [geom.tip.x, geom.tip.y],
                        screen_px_to_world_mm(5.0, scale),
                        40,
                    ),
                    fill_color: [0.0, 0.0, 0.0, 0.0],
                    stroke_color: Some(to_rgba(self.selected_color)),
                    stroke_width_mm: stroke_world_mm(
                        SYMBOL_PIN_SELECTION_HALO_STROKE_PX_AT_100,
                        scale,
                    ),
                });
            }

            pin_texts.push(signex_renderer::schematic::TextInput {
                content: pin.number.clone(),
                position: [geom.number_pos.x as f32, geom.number_pos.y as f32],
                size_mm: PIN_TEXT_LAYOUT.number_size_mm,
                color: to_rgba(self.text_color),
                bold: false,
                italic: false,
                rotation_rad: geom.text_rotation,
                h_align: HAlign::Center,
                v_align: VAlign::Bottom,
            });

            pin_texts.push(signex_renderer::schematic::TextInput {
                content: pin.name.clone(),
                position: [geom.name_pos.x as f32, geom.name_pos.y as f32],
                size_mm: PIN_TEXT_LAYOUT.name_size_mm,
                color: to_rgba(Color {
                    a: 0.85,
                    ..self.text_color
                }),
                bold: false,
                italic: false,
                rotation_rad: geom.text_rotation,
                h_align: geom.name_h_align,
                v_align: VAlign::Center,
            });
        }

        RendererSnapshot {
            wires,
            junctions,
            arcs,
            polygons,
            labels,
            pin_texts,
            reference_value_texts: Vec::new(),
            parameter_texts: Vec::new(),
            overlays: OverlayInputs::default(),
            erc_markers: Vec::new(),
            wire_color_overrides: HashMap::new(),
        }
    }
}
