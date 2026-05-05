//! Standalone primitive editor tabs (`.snxsym` / `.snxfpt`).
//!
//! Renders `.snxsym` / `.snxfpt` files as main-window document tabs
//! using the canvas editor programs from
//! [`crate::library::editor::symbol`] and
//! [`crate::library::editor::footprint`] verbatim. Canvas events flow
//! through `LibraryMessage::PrimitiveEditorEvent { path, msg }` —
//! distinct from the Component Preview tab's
//! `LibraryMessage::EditorEvent { library_path, table, row_id, msg }`.
//!
//! Primitives are first-class document tabs alongside `.snxsch` /
//! `.snxpcb`. Dirty-tracking is keyed by `path`; persistence is plain
//! JSON via serde. The Save action lives on the parent dispatcher
//! (`save_primitive_tab_at`); the editor view only owns the widget
//! tree.

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Border, Element, Length, Theme};
use signex_types::coord::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::app::{FootprintEditorState, SymbolEditorState};
use crate::library::editor::footprint::canvas::FootprintCanvas;
use crate::library::editor::footprint::layers::FpLayer;
use crate::library::editor::symbol::canvas::{self as sym_canvas, SymbolCanvas};
use crate::library::editor::symbol::state as sym_state;
use crate::library::messages::{
    EditorMsg, GraphicHandleMsg, LibraryMessage, PrimitiveEditorMsg, SymbolSelectionMsg,
};
use crate::library::state::LibraryDisplaySettings;
use crate::panels::PanelContext;

// ── Symbol ──────────────────────────────────────────────────────────

/// Render the standalone Symbol editor for a `.snxsym` tab. Altium
/// SchLib parity: the canvas takes the full tab width; the right-dock
/// Properties panel renders symbol/pin properties driven by the
/// selection (see `panels::view_symbol_editor_properties`). The
/// in-tab properties column was retired in v0.9 phase 1 so the user
/// sees the same Properties surface whether editing a schematic or
/// a symbol library — single source of truth, no duplicated panes.
pub fn view_symbol<'a>(
    editor: &'a SymbolEditorState,
    panel_ctx: &'a PanelContext,
    display: LibraryDisplaySettings,
    theme_id: signex_types::theme::ThemeId,
) -> Element<'a, LibraryMessage> {
    let tokens = &panel_ctx.tokens;
    let toolbar = view_symbol_toolbar(editor, panel_ctx);
    let active_bar = container(crate::library::editor::symbol::active_bar::view(
        &editor.path,
        editor.tool,
        theme_id,
        tokens,
    ))
    .padding([4, 8])
    .center_x(Length::Fill);
    let canvas_widget = view_symbol_canvas(editor, panel_ctx, display);

    let body = column![toolbar, active_bar, canvas_widget]
        .spacing(6)
        .width(Length::Fill)
        .height(Length::Fill);

    let status_line = view_symbol_status(editor, panel_ctx, display);

    let outer = column![body, Space::new().height(4), status_line]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

    container(outer)
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .into()
}

/// Bottom status footer for the .snxsym tab — Altium SchLib parity.
/// X / Y in the active unit (mm / mil), zoom %, grid spacing,
/// pin count + a hint string. Mirrors the global schematic
/// status bar so a user editing a symbol library has the same
/// metadata at the same place on screen.
fn view_symbol_status<'a>(
    editor: &'a SymbolEditorState,
    panel_ctx: &'a PanelContext,
    display: LibraryDisplaySettings,
) -> Element<'a, LibraryMessage> {
    let tokens = &panel_ctx.tokens;
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let unit = display.unit;
    let coord_text = match editor.cursor_mm {
        Some((x, y)) => format_coord(x, y, unit),
        None => "X: -.--   Y: -.--".to_string(),
    };
    let zoom_text = format!("{:.0}%", editor.camera.zoom_percent());
    let pin_count = format!("{} pins", editor.primitive().pins.len());

    let sep = || text("|").size(10).color(muted);

    // Sheet / grid / unit live in Tools ▸ Document Options
    // (per Altium parity) — the footer is read-only quick info
    // (cursor / zoom / pin count + hint).
    container(
        row![
            text(coord_text).size(11).color(text_c),
            sep(),
            text(zoom_text).size(11).color(muted),
            sep(),
            text(pin_count).size(11).color(muted),
            Space::new().width(Length::Fill),
            text(if editor.selected.is_some() {
                "Del removes · drag to move · scroll zooms · right-drag pans · Home fits"
            } else {
                "Tools ▸ Document Options for sheet / grid / unit · scroll zooms · right-drag pans · Home fits"
            })
            .size(10)
            .color(muted),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 12])
    .style(crate::styles::status_bar(tokens))
    .width(Length::Fill)
    .into()
}

fn format_coord(x_mm: f64, y_mm: f64, unit: Unit) -> String {
    match unit {
        Unit::Mm => format!("X: {x_mm:>+8.3} mm   Y: {y_mm:>+8.3} mm"),
        Unit::Mil => format!(
            "X: {:>+8.1} mil  Y: {:>+8.1} mil",
            x_mm / 0.0254,
            y_mm / 0.0254
        ),
        Unit::Inch => format!("X: {:>+8.4} in   Y: {:>+8.4} in", x_mm / 25.4, y_mm / 25.4),
        Unit::Micrometer => format!(
            "X: {:>+8.0} um   Y: {:>+8.0} um",
            x_mm * 1000.0,
            y_mm * 1000.0
        ),
    }
}

fn view_symbol_toolbar<'a>(
    editor: &'a SymbolEditorState,
    panel_ctx: &'a PanelContext,
) -> Element<'a, LibraryMessage> {
    let tokens = &panel_ctx.tokens;
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let path = editor.path.clone();

    let save_path = path.clone();
    let save_btn = button(
        text(if editor.dirty { "Save *" } else { "Save" })
            .size(11)
            .color(text_c),
    )
    .padding([4, 10])
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: save_path,
        msg: PrimitiveEditorMsg::Save,
    })
    .style(symbol_tool_button_style(false, border));

    // Active Part picker. `← Part X / N →` reads + steps the active
    // sub-part. Arrows clamp at 1 / max — Tools ▸ New Part is the
    // way to add new parts, not the right arrow (mirrors Altium).
    let max_part = crate::library::editor::symbol::state::max_part_number(editor.primitive());
    let active_part = editor.active_part;
    let prev_path = path.clone();
    let next_path = path.clone();
    let prev_btn = button(text("\u{2190}").size(11).color(text_c))
        .padding([4, 8])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: prev_path,
            msg: PrimitiveEditorMsg::SymbolPrevPart,
        })
        .style(symbol_tool_button_style(false, border));
    let next_btn = button(text("\u{2192}").size(11).color(text_c))
        .padding([4, 8])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: next_path,
            msg: PrimitiveEditorMsg::SymbolNextPart,
        })
        .style(symbol_tool_button_style(false, border));
    let part_label = text(format!("Part {active_part} / {max_part}"))
        .size(11)
        .color(text_c);

    // Fit button stays on the in-tab strip — quick access to
    // viewport reset; tool selection moves to the floating Active Bar
    // below this strip. Sheet color picker moves to Properties ▸
    // Document Options (selection-driven UI).
    let fit_path = path.clone();
    let fit_btn = button(text("Fit").size(11).color(text_c))
        .padding([4, 8])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: fit_path,
            msg: PrimitiveEditorMsg::SymbolFit,
        })
        .style(symbol_tool_button_style(false, border));

    container(
        row![
            fit_btn,
            Space::new().width(Length::Fill),
            prev_btn,
            part_label,
            next_btn,
            Space::new().width(8),
            save_btn,
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 10])
    .style(crate::styles::tab_bar_strip(tokens))
    .into()
}

fn symbol_tool_button_style(
    active: bool,
    border: iced::Color,
) -> impl Fn(&Theme, iced::widget::button::Status) -> iced::widget::button::Style {
    move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(if active {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.02)
        })),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
        },
        ..iced::widget::button::Style::default()
    }
}

fn view_symbol_canvas<'a>(
    editor: &'a SymbolEditorState,
    panel_ctx: &'a PanelContext,
    display: LibraryDisplaySettings,
) -> Element<'a, LibraryMessage> {
    let tokens = &panel_ctx.tokens;
    let program = SymbolCanvas::new(
        editor.primitive(),
        editor.selected,
        editor.tool,
        editor.active_part,
        &editor.camera,
        display.grid_size_mm as f64,
        display.grid_visible,
        display.sheet_color.to_color(),
        crate::styles::ti(tokens.accent),
        crate::styles::ti(tokens.text),
        crate::styles::ti(tokens.text),
        iced::Color {
            a: 0.18,
            ..crate::styles::ti(tokens.text_secondary)
        },
    );
    let widget: Element<'a, sym_canvas::CanvasAction> = iced::widget::Canvas::new(program)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    let path = editor.path.clone();
    widget.map(move |action| LibraryMessage::PrimitiveEditorEvent {
        path: path.clone(),
        msg: symbol_action_to_primitive_msg(action),
    })
}

fn symbol_action_to_primitive_msg(action: sym_canvas::CanvasAction) -> PrimitiveEditorMsg {
    use sym_canvas::CanvasAction;
    match action {
        CanvasAction::AddPin { x, y } => PrimitiveEditorMsg::SymbolAddPin { x, y },
        CanvasAction::AddRectangle { x, y } => PrimitiveEditorMsg::SymbolAddRectangle { x, y },
        CanvasAction::AddLine { x, y } => PrimitiveEditorMsg::SymbolAddLine { x, y },
        CanvasAction::AddCircle { x, y } => PrimitiveEditorMsg::SymbolAddCircle { x, y },
        CanvasAction::AddArc { x, y } => PrimitiveEditorMsg::SymbolAddArc { x, y },
        CanvasAction::AddText { x, y } => PrimitiveEditorMsg::SymbolAddText { x, y },
        CanvasAction::Select(sel) => PrimitiveEditorMsg::SymbolSelect(symbol_selection_to_msg(sel)),
        CanvasAction::Deselect => PrimitiveEditorMsg::SymbolDeselect,
        CanvasAction::Move { x, y } => PrimitiveEditorMsg::SymbolMoveSelected { x, y },
        CanvasAction::MoveGraphicHandle { idx, handle, x, y } => {
            PrimitiveEditorMsg::SymbolMoveGraphicHandle {
                idx,
                handle: graphic_handle_to_msg(handle),
                x,
                y,
            }
        }
        CanvasAction::DeleteSelected => PrimitiveEditorMsg::SymbolDeleteSelected,
        CanvasAction::Pan { dx, dy } => PrimitiveEditorMsg::SymbolPan { dx, dy },
        CanvasAction::Zoom { sx, sy, delta } => PrimitiveEditorMsg::SymbolZoom { sx, sy, delta },
        CanvasAction::Fit => PrimitiveEditorMsg::SymbolFit,
        CanvasAction::CursorAt { x_mm, y_mm } => PrimitiveEditorMsg::SymbolCursorAt { x_mm, y_mm },
    }
}

fn graphic_handle_to_msg(handle: sym_state::GraphicHandle) -> GraphicHandleMsg {
    use sym_state::GraphicHandle;
    match handle {
        GraphicHandle::RectCorner(c) => GraphicHandleMsg::RectCorner(c),
        GraphicHandle::LineEndpoint(e) => GraphicHandleMsg::LineEndpoint(e),
        GraphicHandle::CircleRadius => GraphicHandleMsg::CircleRadius,
        GraphicHandle::ArcStart => GraphicHandleMsg::ArcStart,
        GraphicHandle::ArcEnd => GraphicHandleMsg::ArcEnd,
        GraphicHandle::TextAnchor => GraphicHandleMsg::TextAnchor,
    }
}

fn symbol_selection_to_msg(sel: sym_state::SymbolSelection) -> SymbolSelectionMsg {
    use sym_state::{FieldKey, SymbolSelection};
    match sel {
        SymbolSelection::Pin(idx) => SymbolSelectionMsg::Pin(idx),
        SymbolSelection::Field(FieldKey::Reference) => SymbolSelectionMsg::FieldReference,
        SymbolSelection::Field(FieldKey::Value) => SymbolSelectionMsg::FieldValue,
        SymbolSelection::Graphic(idx) => SymbolSelectionMsg::Graphic(idx),
    }
}

// ── Footprint ───────────────────────────────────────────────────────

/// Render the standalone Footprint editor for a `.snxfpt` tab. Mirrors
/// the in-Component Editor footprint surface (toolbar + canvas +
/// footer) but skips the right-column Body 3D / 3D preview / STEP
/// attach panel — those edit Component-level fields that live on the
/// Footprint primitive's `body_3d` and `step_attachment` slots and the
/// view tree for them is reused via the Component Editor surface.
/// Pure pad-layout standalone editing is what `.snxfpt` needs first.
pub fn view_footprint<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
    theme_id: signex_types::theme::ThemeId,
    custom_filter_presets: &'a [crate::active_bar::CustomFilterPreset],
) -> Element<'a, LibraryMessage> {
    use crate::library::editor::footprint::state::EditorMode;
    // v0.16.2.2 — footprint canvas uses Altium PCB-editor colours
    // regardless of the active app theme. Background pure black,
    // grid dark navy. Pads / silk / courtyard already render via
    // `FpLayer::color()` which carries Altium-flavoured tints (red
    // FCu / blue BCu / off-white silk / yellow Edge.Cuts), so this
    // change rounds out the Altium look on the canvas chrome itself.
    // Symbol (schematic) canvas stays theme-driven — Altium's
    // schematic editor uses a different (cream-ish) palette.
    let bg = iced::Color::BLACK;
    let grid = iced::Color::from_rgba(0.32, 0.36, 0.55, 1.0);

    let canvas_area = view_footprint_canvas(editor, tokens, bg, grid);
    let footer = view_footprint_footer(editor, tokens);

    // v0.14.2 — no top strip. Mode segments live INSIDE each active
    // bar (left edge), so the only horizontal chrome is the floating
    // active bar + the layer strip at the bottom. Save action is
    // still reachable via Ctrl+S / File menu / tab right-click; the
    // tab's dirty asterisk is the visual save indicator.
    let layers_strip = view_footprint_layers_strip(editor, tokens);

    // Active bar floats OVER the canvas via Stack so the canvas
    // drawing area extends edge-to-edge behind it instead of being
    // clipped under the bar's bottom. Mirrors Fusion 360 / Altium.
    // v0.14.2 — mode switcher is its own floating widget at the
    // canvas's top-LEFT, separate from the centered active bar.
    let mode_switcher =
        crate::library::editor::footprint::pads_active_bar::mode_switcher_overlay(editor, tokens);
    // v0.18.11.1 — canvas-overlay multi-footprint tab strip removed.
    // The Footprint Library left-dock panel (v0.18.8) renders the
    // same list (internal `file.footprints[i]`) with proper
    // Place/Add/Delete/Edit buttons, so the redundant on-canvas
    // chip was just visual noise. Keep `mode_switcher` (Sketch /
    // Pads / 3D) at top-right since it has no panel equivalent.

    // v0.18.14 — single unified active bar carries mode-keyed
    // tools (left half) + the eight Selection Filter pills (right
    // half) regardless of mode. Replaces the per-mode
    // pads_active_bar::view / sketch_mode::active_bar::view
    // mounting that lived here through v0.18.13.
    let unified_bar = container(crate::library::editor::footprint::unified_active_bar::view(
        editor,
        theme_id,
        tokens,
        custom_filter_presets,
    ))
    .padding([6, 10])
    .center_x(Length::Fill)
    .align_y(iced::alignment::Vertical::Top);
    let body: Element<'a, LibraryMessage> = match editor.state.mode {
        EditorMode::Sketch => {
            let canvas_with_bar = iced::widget::Stack::new()
                .push(canvas_area)
                .push(unified_bar)
                .push(mode_switcher);
            column![canvas_with_bar, layers_strip, footer]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
        EditorMode::Normal => {
            let canvas_with_bar = iced::widget::Stack::new()
                .push(canvas_area)
                .push(unified_bar)
                .push(mode_switcher);
            column![canvas_with_bar, layers_strip, footer]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
        EditorMode::View3d => {
            let canvas_with_bar = iced::widget::Stack::new()
                .push(canvas_area)
                .push(mode_switcher);
            column![canvas_with_bar, layers_strip, footer]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    };

    body
}

/// Minimal Sketch-mode top bar — replaces the layer-heavy
/// `view_footprint_toolbar` so the workspace reads as "you are now
/// editing the sketch, not the pads". Carries a "Sketch" title, an
/// **Exit Sketch** button (returns to Normal mode), and Save.
/// v0.14.2 — fixed top strip rendered above the active bar in every
/// mode. Hosts the mode segmented control + Save button. Stays the
/// same height across mode switches so the active bar's vertical
/// position is constant.
#[allow(dead_code)]
fn view_footprint_top_strip<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    use crate::library::editor::footprint::state::EditorMode;
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let accent = theme_ext::to_color(&tokens.accent);

    // Segmented control — three connected segments. Active segment
    // paints with the accent background; inactive segments stay
    // muted with a hover affordance.
    let mode_segment =
        |label: &'static str, target: EditorMode, active: bool| -> Element<'a, LibraryMessage> {
            let path = editor.path.clone();
            let label_color = if active { iced::Color::WHITE } else { text_c };
            button(
                text(label)
                    .size(11)
                    .color(label_color)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .padding([5, 14])
            .on_press(LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEditorMsg::FootprintSetMode(target),
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: if active {
                    Some(iced::Background::Color(accent))
                } else {
                    Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04,
                    )))
                },
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: if active { accent } else { border },
                },
                ..iced::widget::button::Style::default()
            })
            .into()
        };

    let mode = editor.state.mode;
    let mode_segments = row![
        text("Mode").size(11).color(muted),
        Space::new().width(6),
        mode_segment(
            "Pads",
            EditorMode::Normal,
            matches!(mode, EditorMode::Normal)
        ),
        mode_segment(
            "Sketch",
            EditorMode::Sketch,
            matches!(mode, EditorMode::Sketch)
        ),
        mode_segment("3D", EditorMode::View3d, matches!(mode, EditorMode::View3d)),
    ]
    .spacing(2)
    .align_y(iced::Alignment::Center);

    let save_path = editor.path.clone();
    let save_btn = button(
        text(if editor.dirty { "Save *" } else { "Save" })
            .size(11)
            .color(text_c),
    )
    .padding([5, 12])
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: save_path,
        msg: PrimitiveEditorMsg::Save,
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.04,
        ))),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
        },
        ..iced::widget::button::Style::default()
    });

    let auto_fit_path = editor.path.clone();
    let auto_fit_on = editor.state.auto_fit_courtyard;
    let auto_fit_label = if auto_fit_on {
        "Auto-fit Courtyard \u{2713}"
    } else {
        "Auto-fit Courtyard"
    };
    let auto_fit_btn = button(text(auto_fit_label).size(11).color(text_c))
        .padding([5, 12])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: auto_fit_path,
            msg: PrimitiveEditorMsg::FootprintToggleAutoFit,
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.04,
            ))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        });

    let row_widget = row![
        mode_segments,
        Space::new().width(Length::Fill),
        auto_fit_btn,
        Space::new().width(8),
        save_btn,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    container(row_widget)
        .padding([6, 10])
        .style(crate::styles::tab_bar_strip(tokens))
        .into()
}

/// v0.14.2 — Altium PCB-Library-style layer tab strip at the bottom
/// of the canvas (above the footer). Each layer is a clickable pill
/// with a colour swatch + label. Click toggles visibility (existing
/// `FootprintToggleLayer` message).
///
/// Replaces the heavy layer pills that used to sit at the top of the
/// editor; moving them below the canvas keeps the top compact and
/// matches Altium's bottom-of-canvas layer tab pattern.
fn view_footprint_layers_strip<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let layers = editor.state.layer_visibility;

    let mut row_widget = row![text("Layers").size(10).color(muted)]
        .spacing(4)
        .align_y(iced::Alignment::Center);

    for layer in FpLayer::ORDER {
        let on = layers.get(*layer);
        let swatch = layer.color();
        let label_color = if on { text_c } else { muted };
        let toggle_path = editor.path.clone();
        let layer_standard = layer.standard_name().to_string();
        let pill = button(
            row![
                container(text("").size(10))
                    .width(Length::Fixed(8.0))
                    .height(Length::Fixed(8.0))
                    .style(move |_: &Theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(swatch)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: iced::Color { a: 0.5, ..swatch },
                        },
                        ..iced::widget::container::Style::default()
                    }),
                text(layer.label()).size(10).color(label_color),
            ]
            .spacing(5)
            .align_y(iced::Alignment::Center),
        )
        .padding([3, 7])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: toggle_path,
            msg: PrimitiveEditorMsg::FootprintToggleLayer(layer_standard),
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: if on {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.05,
                )))
            } else {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.01,
                )))
            },
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: if on {
                    swatch
                } else {
                    iced::Color { a: 0.5, ..border }
                },
            },
            ..iced::widget::button::Style::default()
        });
        row_widget = row_widget.push(pill);
    }

    container(
        scrollable(row_widget).direction(iced::widget::scrollable::Direction::Horizontal(
            iced::widget::scrollable::Scrollbar::default()
                .width(0)
                .margin(0)
                .scroller_width(0),
        )),
    )
    .padding([4, 10])
    .width(Length::Fill)
    .style(crate::styles::tab_bar_strip(tokens))
    .into()
}

#[allow(dead_code)]
fn view_footprint_sketch_toolbar<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    use crate::library::editor::footprint::state::EditorMode;
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    let accent = theme_ext::to_color(&tokens.accent);

    let exit_path = editor.path.clone();
    let exit_btn = button(
        row![
            text("\u{2715}").size(11).color(text_c),
            Space::new().width(4),
            text("Exit Sketch").size(11).color(text_c),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 10])
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: exit_path,
        msg: PrimitiveEditorMsg::FootprintSetMode(EditorMode::Normal),
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            accent.r, accent.g, accent.b, 0.18,
        ))),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: iced::Color { a: 0.6, ..accent },
        },
        ..iced::widget::button::Style::default()
    });

    let save_path = editor.path.clone();
    let save_btn = button(
        text(if editor.dirty { "Save *" } else { "Save" })
            .size(11)
            .color(text_c),
    )
    .padding([4, 10])
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: save_path,
        msg: PrimitiveEditorMsg::Save,
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.04,
        ))),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
        },
        ..iced::widget::button::Style::default()
    });

    let row_widget = row![
        text("Sketch").size(13).color(text_c),
        text("·").size(13).color(muted),
        text("authoring parametric geometry").size(11).color(muted),
        Space::new().width(Length::Fill),
        save_btn,
        Space::new().width(8),
        exit_btn,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    container(row_widget)
        .padding([6, 10])
        .style(crate::styles::tab_bar_strip(tokens))
        .into()
}

#[allow(dead_code)]
fn view_footprint_toolbar<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let layers = editor.state.layer_visibility;
    let auto_fit_on = editor.state.auto_fit_courtyard;

    let mut row_widget = row![text("Layers:").size(11).color(muted)]
        .spacing(6)
        .align_y(iced::Alignment::Center);

    for layer in FpLayer::ORDER {
        let on = layers.get(*layer);
        let swatch = layer.color();
        let label_color = if on { text_c } else { muted };
        let toggle_path = editor.path.clone();
        let layer_standard = layer.standard_name().to_string();
        let pill = button(
            row![
                container(text("").size(11))
                    .width(Length::Fixed(8.0))
                    .height(Length::Fixed(8.0))
                    .style(move |_: &Theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(swatch)),
                        border: Border {
                            width: 1.0,
                            radius: 2.0.into(),
                            color: iced::Color { a: 0.5, ..swatch },
                        },
                        ..iced::widget::container::Style::default()
                    }),
                text(layer.label()).size(11).color(label_color),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        )
        .padding([3, 8])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: toggle_path,
            msg: PrimitiveEditorMsg::FootprintToggleLayer(layer_standard),
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: if on {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.05,
                )))
            } else {
                Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.01,
                )))
            },
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: if on {
                    swatch
                } else {
                    iced::Color { a: 0.5, ..border }
                },
            },
            ..iced::widget::button::Style::default()
        });
        row_widget = row_widget.push(pill);
    }

    let auto_fit_path = editor.path.clone();
    let auto_fit_label = if auto_fit_on {
        "Auto-fit Courtyard \u{2713}"
    } else {
        "Auto-fit Courtyard"
    };
    let auto_fit_btn = button(text(auto_fit_label).size(11).color(text_c))
        .padding([3, 8])
        .on_press(LibraryMessage::PrimitiveEditorEvent {
            path: auto_fit_path,
            msg: PrimitiveEditorMsg::FootprintToggleAutoFit,
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.02,
            ))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..iced::widget::button::Style::default()
        });

    let save_path = editor.path.clone();
    let save_btn = button(
        text(if editor.dirty { "Save *" } else { "Save" })
            .size(11)
            .color(text_c),
    )
    .padding([3, 8])
    .on_press(LibraryMessage::PrimitiveEditorEvent {
        path: save_path,
        msg: PrimitiveEditorMsg::Save,
    })
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.02,
        ))),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
        },
        ..iced::widget::button::Style::default()
    });

    // v0.13.1 — Sketch mode toggle. Three pills (Normal / Sketch /
    // 3D View) sit between the layer toggles and the auto-fit /
    // save buttons. The active pill is highlighted via the same
    // pattern the layer toggles use.
    let mode_pill = |label: &'static str,
                     target: crate::library::editor::footprint::state::EditorMode,
                     active: bool| {
        let path = editor.path.clone();
        let label_color = if active { text_c } else { muted };
        button(text(label).size(11).color(label_color))
            .padding([3, 8])
            .on_press(LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEditorMsg::FootprintSetMode(target),
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: if active {
                    Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.08,
                    )))
                } else {
                    Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.02,
                    )))
                },
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            })
    };
    let mode = editor.state.mode;
    let mode_row = row![
        text("Mode:").size(11).color(muted),
        mode_pill(
            "Normal",
            crate::library::editor::footprint::state::EditorMode::Normal,
            matches!(
                mode,
                crate::library::editor::footprint::state::EditorMode::Normal
            ),
        ),
        mode_pill(
            "Sketch",
            crate::library::editor::footprint::state::EditorMode::Sketch,
            matches!(
                mode,
                crate::library::editor::footprint::state::EditorMode::Sketch
            ),
        ),
        mode_pill(
            "3D View",
            crate::library::editor::footprint::state::EditorMode::View3d,
            matches!(
                mode,
                crate::library::editor::footprint::state::EditorMode::View3d
            ),
        ),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    row_widget = row_widget
        .push(Space::new().width(Length::Fill))
        .push(mode_row)
        .push(Space::new().width(8))
        .push(auto_fit_btn)
        .push(Space::new().width(8))
        .push(save_btn);

    container(row_widget)
        .padding([6, 10])
        .style(crate::styles::tab_bar_strip(tokens))
        .into()
}

fn view_footprint_canvas<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
    bg: iced::Color,
    grid: iced::Color,
) -> Element<'a, LibraryMessage> {
    let border = theme_ext::border_color(tokens);

    // The canvas program publishes `LibraryMessage::EditorEvent { ...,
    // msg: EditorMsg::Footprint* }`; we translate those into the
    // standalone primitive-editor envelope via `Element::map`. The
    // `EditorAddress` we stamp on the program is a sentinel — its
    // `library_path` field is the tab path (so dirty-tracking still
    // resolves), and the `table` / `row_id` are nil-shaped since
    // standalone tabs don't carry a Component.
    let address = crate::library::state::EditorAddress::new(
        editor.path.clone(),
        String::new(),
        signex_library::RowId::from_uuid(uuid::Uuid::nil()),
    );
    let prog = FootprintCanvas {
        state: &editor.state,
        address,
        bg_color: bg,
        grid_color: grid,
        cache: &editor.canvas_cache,
        sketch: editor.primitive().sketch.as_ref(),
        silk_f: editor.primitive().silk_f.as_slice(),
        silk_b: editor.primitive().silk_b.as_slice(),
    };
    let canvas_widget: Element<'a, LibraryMessage> = iced::widget::canvas(prog)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    let path = editor.path.clone();
    let translated = canvas_widget.map(move |msg| match msg {
        LibraryMessage::EditorEvent { msg, .. } => LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: editor_msg_to_primitive_msg(msg),
        },
        other => other,
    });

    container(translated)
        .padding(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 0.0.into(),
                color: border,
            },
            ..iced::widget::container::Style::default()
        })
        .into()
}

/// Translate a Footprint canvas `EditorMsg` into the standalone
/// primitive-editor envelope. Only the canvas-emitted variants are
/// ever produced here — non-footprint variants fall through to a
/// no-op `Save` (the dispatcher discards on path-keyed lookup
/// mismatch anyway).
fn editor_msg_to_primitive_msg(msg: EditorMsg) -> PrimitiveEditorMsg {
    match msg {
        EditorMsg::FootprintAddPad { x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintAddPad { x_mm, y_mm }
        }
        EditorMsg::FootprintAddHole { x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintAddHole { x_mm, y_mm }
        }
        EditorMsg::FootprintAddText { x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintAddText { x_mm, y_mm }
        }
        EditorMsg::FootprintTrackClick { x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintTrackClick { x_mm, y_mm }
        }
        EditorMsg::FootprintTrackCancel => PrimitiveEditorMsg::FootprintTrackCancel,
        EditorMsg::FootprintArcClick { x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintArcClick { x_mm, y_mm }
        }
        EditorMsg::FootprintArcCancel => PrimitiveEditorMsg::FootprintArcCancel,
        EditorMsg::FootprintPolygonClick { x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintPolygonClick { x_mm, y_mm }
        }
        EditorMsg::FootprintPolygonCommit => PrimitiveEditorMsg::FootprintPolygonCommit,
        EditorMsg::FootprintPolygonCancel => PrimitiveEditorMsg::FootprintPolygonCancel,
        EditorMsg::FootprintSelectSilkF(sel) => PrimitiveEditorMsg::FootprintSelectSilkF(sel),
        EditorMsg::FootprintDeleteSilkF => PrimitiveEditorMsg::FootprintDeleteSilkF,
        EditorMsg::FootprintSketchPlacePoint { x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintSketchPlacePoint { x_mm, y_mm }
        }
        EditorMsg::FootprintSketchToolClick {
            x_mm,
            y_mm,
            snap_id,
        } => PrimitiveEditorMsg::FootprintSketchToolClick {
            x_mm,
            y_mm,
            snap_id,
        },
        EditorMsg::FootprintSketchToolEscape => PrimitiveEditorMsg::FootprintSketchToolEscape,
        EditorMsg::FootprintSketchSelect { id, shift } => {
            PrimitiveEditorMsg::FootprintSketchSelect { id, shift }
        }
        EditorMsg::FootprintSketchMovePoint { id, dx, dy } => {
            PrimitiveEditorMsg::FootprintSketchMovePoint { id, dx, dy }
        }
        EditorMsg::FootprintMovePad { idx, x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintMovePad { idx, x_mm, y_mm }
        }
        EditorMsg::FootprintCursorAt { x_mm, y_mm } => {
            PrimitiveEditorMsg::FootprintCursorAt { x_mm, y_mm }
        }
        EditorMsg::FootprintSelectPad(sel) => PrimitiveEditorMsg::FootprintSelectPad(sel),
        EditorMsg::FootprintDeleteSelected => PrimitiveEditorMsg::FootprintDeleteSelected,
        EditorMsg::FootprintToggleLayer(name) => PrimitiveEditorMsg::FootprintToggleLayer(name),
        EditorMsg::FootprintToggleAutoFit => PrimitiveEditorMsg::FootprintToggleAutoFit,
        EditorMsg::FootprintSetPadsTool(t) => PrimitiveEditorMsg::FootprintSetPadsTool(t),
        EditorMsg::FootprintSketchSetTool(t) => PrimitiveEditorMsg::FootprintSketchSetTool(t),
        EditorMsg::FootprintSketchToggleConstruction => {
            PrimitiveEditorMsg::FootprintSketchToggleConstruction
        }
        EditorMsg::FootprintTogglePlacementPause => {
            PrimitiveEditorMsg::FootprintTogglePlacementPause
        }
        EditorMsg::FootprintSketchSetRole { id, role } => {
            PrimitiveEditorMsg::FootprintSketchSetRole { id, role }
        }
        // Anything not emitted by the footprint canvas is dropped via
        // a benign "save of the wrong tab" — the path-keyed dispatcher
        // ignores mismatches.
        _ => PrimitiveEditorMsg::Save,
    }
}

fn view_footprint_footer<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let cursor_label = match editor.state.cursor_mm {
        Some((x, y)) => format!("X {x:>+8.3} mm   Y {y:>+8.3} mm"),
        None => "X    -.--- mm   Y    -.--- mm".to_string(),
    };
    let pad_count = editor.state.pads.len();
    let selected_label = match editor.state.selected_pad {
        Some(i) => match editor.state.pads.get(i) {
            Some(pad) => format!(
                "Pad {} — {:.2} × {:.2} mm @ ({:+.3}, {:+.3})",
                pad.number, pad.size_mm.0, pad.size_mm.1, pad.position_mm.0, pad.position_mm.1
            ),
            None => format!("Pads: {pad_count}"),
        },
        None => {
            format!("Pads: {pad_count}   ·   Click empty area to add, drag to move, Del to remove")
        }
    };

    container(
        row![
            text(cursor_label).size(11).color(muted),
            Space::new().width(20),
            text(selected_label).size(11).color(text_c),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 12])
    .style(crate::styles::modal_footer_strip(tokens))
    .width(Length::Fill)
    .into()
}
