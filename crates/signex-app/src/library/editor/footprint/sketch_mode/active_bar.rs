//! Sketch-mode Active Bar — floating toolbar over the footprint
//! canvas when the editor is in [`EditorMode::Sketch`].
//!
//! Mirrors the Fusion 360 sketch toolbar layout: grouped sections for
//! **Select / Create / Constrain / Dimension / Solve**, separated by
//! the [`signex_widgets::active_bar`] thin vertical separators. Each
//! section's enabled state derives from the editor's current
//! selection so disabled buttons grey out exactly when their
//! constraint can't apply.
//!
//! Layout (left → right):
//!
//! 1. **Select** — sketch entity selection tool.
//! 2. **Create** — Point / Line / Circle / Arc multi-click drawing
//!    tools.
//! 3. **Constrain** — 10 selection-aware constraint authoring
//!    buttons. Enabled state derives from the kinds of the primary
//!    + secondary selection slots.
//! 4. **Dimension input** — `Custom` slot with a `text_input` for
//!    the `DistancePtPt` numeric value.
//! 5. **Solve toggle** — pause / resume the live solver.
//!
//! The bar floats over the canvas (Stack overlay layer) so it
//! doesn't steal vertical space from the drawing area.

use std::path::PathBuf;

use iced::widget::{Space, container, text, text_input};
use iced::{Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};
use signex_widgets::theme_ext;

use crate::app::FootprintEditorState;
use crate::icons;
use crate::library::editor::footprint::state::SketchTool;
use crate::library::messages::{LibraryMessage, PrimitiveEditorMsg, SketchConstraintTag};

/// Build the Active Bar items for the given editor state. Theme is
/// pulled from `editor.path` → `themes::current_id()` lookup at the
/// caller's site (same pattern as the SchLib editor).
pub fn items<'a>(
    editor: &'a FootprintEditorState,
    theme_id: signex_types::theme::ThemeId,
    tokens: &'a ThemeTokens,
) -> Vec<ActiveBarItem<LibraryMessage>> {
    let path = editor.path.clone();
    let active_tool = editor.state.active_tool;

    let mk_tool =
        |label: &str, tool: SketchTool, icon: ActiveBarIcon| -> ActiveBarItem<LibraryMessage> {
            let p = path.clone();
            ActiveBarItem::Button(ActiveBarButton {
                icon,
                tooltip: label.to_string(),
                enabled: true,
                selected: active_tool == tool,
                on_press: Some(LibraryMessage::PrimitiveEditorEvent {
                    path: p,
                    msg: PrimitiveEditorMsg::FootprintSketchSetTool(tool),
                }),
                ..ActiveBarButton::default()
            })
        };

    // Compute which constraint tags apply to the current selection.
    let enabled = constraint_enable_matrix(editor);

    let mk_constraint = |tag: SketchConstraintTag,
                         tooltip: &str,
                         glyph: &'static str|
     -> ActiveBarItem<LibraryMessage> {
        let p = path.clone();
        let on = enabled[tag_index(tag)];
        ActiveBarItem::Button(ActiveBarButton {
            icon: ActiveBarIcon::Glyph(glyph),
            tooltip: tooltip.to_string(),
            enabled: on,
            selected: false,
            on_press: if on {
                Some(LibraryMessage::PrimitiveEditorEvent {
                    path: p,
                    msg: PrimitiveEditorMsg::FootprintSketchAddConstraintForSelection(tag),
                })
            } else {
                None
            },
            ..ActiveBarButton::default()
        })
    };

    // v0.16.1 — Construction-mode toggle. Sticky pill: while on, every
    // newly-minted entity gets `construction = true` (rendered dashed-
    // grey, skipped by bake). Useful for guides + symmetry without
    // affecting the baked silk / pad / courtyard output.
    let construction_on = editor.state.construction_mode;
    let construction_path = path.clone();
    let construction_button = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Glyph("\u{2504}"), // ┄ dashed line glyph
        tooltip: if construction_on {
            "Construction mode: ON (new geometry won't bake)".into()
        } else {
            "Construction mode: OFF".into()
        },
        enabled: true,
        selected: construction_on,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: construction_path,
            msg: PrimitiveEditorMsg::FootprintSketchToggleConstruction,
        }),
        ..ActiveBarButton::default()
    });

    // v0.22 Phase A5 — Centerline-mode toggle. Mirrors construction-
    // mode but stamps `entity.centerline = true` instead. Renders as a
    // long-dash gold pattern in the canvas; bake skips it identically
    // to construction. Mutually exclusive with construction-mode at
    // the dispatcher level.
    let centerline_on = editor.state.centerline_mode;
    let centerline_path = path.clone();
    let centerline_button = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Glyph("\u{2501}"), // ━ heavy long-dash hint
        tooltip: if centerline_on {
            "Centerline mode: ON (new geometry is a centerline)".into()
        } else {
            "Centerline mode: OFF".into()
        },
        enabled: true,
        selected: centerline_on,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: centerline_path,
            msg: PrimitiveEditorMsg::FootprintSketchToggleCenterline,
        }),
        ..ActiveBarButton::default()
    });

    // v0.22 Phase D4 — Make Pad from Profile button. One-shot action
    // (not a tool mode): converts the closed-loop profile that
    // includes the currently-selected Line into a Custom-shape pad.
    // Enabled only when a Line is selected; the dispatcher itself
    // verifies the loop closes and pushes a warning otherwise.
    let make_pad_path = path.clone();
    let make_pad_enabled = matches!(
        editor
            .state
            .selected_sketch
            .and_then(|id| editor.primitive().sketch.as_ref()?.entities.iter().find(|e| e.id == id))
            .map(|e| &e.kind),
        Some(signex_sketch::entity::EntityKind::Line { .. })
    );
    let make_pad_button = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Glyph("\u{2B22}"), // ⬢ black hexagon (custom polygon → pad)
        tooltip: if make_pad_enabled {
            "Make Pad from Profile — walk the closed loop containing the selected Line and convert it into a Custom-shape pad"
                .into()
        } else {
            "Make Pad from Profile (select a Line that's part of a closed loop)".into()
        },
        enabled: make_pad_enabled,
        selected: false,
        on_press: if make_pad_enabled {
            Some(LibraryMessage::PrimitiveEditorEvent {
                path: make_pad_path,
                msg: PrimitiveEditorMsg::FootprintSketchMakePadFromProfile,
            })
        } else {
            None
        },
        ..ActiveBarButton::default()
    });

    // Section 4 — Dimension input as a Custom slot. Sized to fit
    // ~6 digits + "mm" hint inside the bar's vertical rhythm.
    let dim_input = build_dimension_input(editor, tokens);

    let _ = tokens;
    vec![
        // Section 1: Select
        mk_tool(
            "Select",
            SketchTool::Select,
            ActiveBarIcon::Svg(icons::icon_select(theme_id)),
        ),
        ActiveBarItem::Separator,
        // Section 2: Create
        mk_tool(
            "Place Point",
            SketchTool::Point,
            ActiveBarIcon::Glyph("\u{2022}"), // •
        ),
        mk_tool(
            "Place Line (2 clicks)",
            SketchTool::Line,
            ActiveBarIcon::Svg(icons::icon_shape_line(theme_id)),
        ),
        mk_tool(
            "Place Rectangle (corner + opposite corner)",
            SketchTool::Rectangle,
            ActiveBarIcon::Svg(icons::icon_shape_rect(theme_id)),
        ),
        mk_tool(
            "Place Rounded Rectangle (corner + opposite corner; radius from dim input)",
            SketchTool::RoundedRectangle,
            // No bespoke icon yet — show a glyph that hints at the
            // shape until the per-theme registry gains a rrect entry.
            ActiveBarIcon::Glyph("\u{25A2}"), // ▢
        ),
        mk_tool(
            "Place Circle (centre + radius)",
            SketchTool::Circle,
            ActiveBarIcon::Svg(icons::icon_shape_circle(theme_id)),
        ),
        mk_tool(
            "Place Arc (centre + start + end)",
            SketchTool::Arc,
            ActiveBarIcon::Svg(icons::icon_shape_arc(theme_id)),
        ),
        mk_tool(
            "Mirror — pre-select a Line with the Select tool, then click a Point to mirror",
            SketchTool::Mirror,
            ActiveBarIcon::Glyph("\u{29B5}"), // ⦵
        ),
        mk_tool(
            "Offset — pre-select a Line / Arc / Circle, then click on the side to offset (distance from dim input, default 0.5 mm)",
            SketchTool::Offset,
            ActiveBarIcon::Glyph("\u{29C8}"), // ⧈
        ),
        mk_tool(
            "Rectangular Pattern — click an entity to mint a 2×2 grid array (5 mm × 5 mm)",
            SketchTool::RectPattern,
            ActiveBarIcon::Glyph("\u{229E}"), // ⊞ squared plus (grid)
        ),
        mk_tool(
            "Circular Pattern — click an entity to mint a 4-instance polar array (360°)",
            SketchTool::CircularPattern,
            ActiveBarIcon::Glyph("\u{233E}"), // ⌾ apl functional symbol circle jot
        ),
        make_pad_button,
        ActiveBarItem::Separator,
        // Section 3: Constrain (selection-aware)
        mk_constraint(
            SketchConstraintTag::Fixed,
            "Fix point in place (needs 1 Point)",
            "\u{2693}", // ⚓
        ),
        mk_constraint(
            SketchConstraintTag::Coincident,
            "Coincident (needs 2 Points)",
            "\u{2299}", // ⊙
        ),
        mk_constraint(
            SketchConstraintTag::DistancePtPt,
            "Distance between Points (needs 2 Points + dim input)",
            "\u{27F7}", // ⟷
        ),
        mk_constraint(
            SketchConstraintTag::Horizontal,
            "Horizontal (needs 1 Line)",
            "\u{2500}", // ─
        ),
        mk_constraint(
            SketchConstraintTag::Vertical,
            "Vertical (needs 1 Line)",
            "\u{2502}", // │
        ),
        mk_constraint(
            SketchConstraintTag::Parallel,
            "Parallel (needs 2 Lines)",
            "\u{2225}", // ∥
        ),
        mk_constraint(
            SketchConstraintTag::Perpendicular,
            "Perpendicular (needs 2 Lines)",
            "\u{27C2}", // ⟂
        ),
        mk_constraint(
            SketchConstraintTag::EqualLength,
            "Equal length (needs 2 Lines)",
            "\u{2261}", // ≡
        ),
        mk_constraint(
            SketchConstraintTag::PointOnLine,
            "Point on line (needs 1 Point + 1 Line)",
            "\u{22A2}", // ⊢
        ),
        mk_constraint(
            SketchConstraintTag::Midpoint,
            "Midpoint (needs 1 Point + 1 Line)",
            "\u{25C7}", // ◇
        ),
        ActiveBarItem::Separator,
        // Section 4: Dimension input
        ActiveBarItem::Custom(dim_input),
        ActiveBarItem::Separator,
        // Section 5: Linetype toggles. Construction (dashed grey) and
        // Centerline (long-dash gold) are mutually exclusive — both
        // skipped by bake. Solver is always live in v0.16.1 — the
        // pause toggle was retired.
        construction_button,
        centerline_button,
    ]
}

/// Convenience wrapper — build items + render via
/// [`signex_widgets::active_bar::view`].
pub fn view<'a>(
    editor: &'a FootprintEditorState,
    theme_id: signex_types::theme::ThemeId,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    signex_widgets::active_bar::view(items(editor, theme_id, tokens), tokens)
}

/// Compute the per-tag enable state from the current selection slots.
/// Returns a fixed-length array indexed by [`tag_index`].
fn constraint_enable_matrix(editor: &FootprintEditorState) -> [bool; 10] {
    use signex_sketch::entity::EntityKind;
    let primary = editor.state.selected_sketch;
    let secondary = editor.state.selected_sketch_secondary;
    let kind_of = |id: signex_sketch::id::SketchEntityId| -> Option<&'static str> {
        editor
            .primitive()
            .sketch
            .as_ref()?
            .entities
            .iter()
            .find(|e| e.id == id)
            .map(|e| match e.kind {
                EntityKind::Point { .. } => "Point",
                EntityKind::Line { .. } => "Line",
                EntityKind::Arc { .. } => "Arc",
                EntityKind::Circle { .. } => "Circle",
            })
    };
    let p = primary.and_then(kind_of);
    let s = secondary.and_then(kind_of);
    let mut m = [false; 10];
    match (p, s) {
        (Some("Point"), None) => {
            m[tag_index(SketchConstraintTag::Fixed)] = true;
        }
        (Some("Line"), None) => {
            m[tag_index(SketchConstraintTag::Horizontal)] = true;
            m[tag_index(SketchConstraintTag::Vertical)] = true;
        }
        (Some("Point"), Some("Point")) => {
            m[tag_index(SketchConstraintTag::Coincident)] = true;
            m[tag_index(SketchConstraintTag::DistancePtPt)] = true;
        }
        (Some("Line"), Some("Line")) => {
            m[tag_index(SketchConstraintTag::Parallel)] = true;
            m[tag_index(SketchConstraintTag::Perpendicular)] = true;
            m[tag_index(SketchConstraintTag::EqualLength)] = true;
        }
        (Some("Point"), Some("Line")) | (Some("Line"), Some("Point")) => {
            m[tag_index(SketchConstraintTag::PointOnLine)] = true;
            m[tag_index(SketchConstraintTag::Midpoint)] = true;
        }
        _ => {}
    }
    m
}

const fn tag_index(tag: SketchConstraintTag) -> usize {
    match tag {
        SketchConstraintTag::Fixed => 0,
        SketchConstraintTag::Coincident => 1,
        SketchConstraintTag::DistancePtPt => 2,
        SketchConstraintTag::Horizontal => 3,
        SketchConstraintTag::Vertical => 4,
        SketchConstraintTag::Parallel => 5,
        SketchConstraintTag::Perpendicular => 6,
        SketchConstraintTag::EqualLength => 7,
        SketchConstraintTag::PointOnLine => 8,
        SketchConstraintTag::Midpoint => 9,
    }
}

/// Build the inline dimension `text_input` slot. Sized to read like
/// the rest of the bar (matches the BTN_SIZE vertical rhythm).
fn build_dimension_input<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'static, LibraryMessage> {
    let path = editor.path.clone();
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let value: String = editor.state.dimension_input.clone();

    let input = text_input("0.0", &value)
        .size(11)
        .padding(2)
        .width(Length::Fixed(58.0))
        .style(move |_: &Theme, _| iced::widget::text_input::Style {
            background: iced::Background::Color(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border,
            },
            icon: iced::Color::TRANSPARENT,
            placeholder: muted,
            value: text_c,
            selection: iced::Color::from_rgba(0.4, 0.6, 1.0, 0.4),
        })
        .on_input(move |s| LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEditorMsg::FootprintSketchDimensionInput(s),
        });

    container(
        iced::widget::row![
            text("mm").size(10).color(muted),
            Space::new().width(Length::Fixed(4.0)),
            input,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([2, 4])
    .style(move |_: &Theme| iced::widget::container::Style {
        background: None,
        border: Border {
            width: 0.0,
            radius: 0.0.into(),
            color: Color::TRANSPARENT,
        },
        ..iced::widget::container::Style::default()
    })
    .into()
}
