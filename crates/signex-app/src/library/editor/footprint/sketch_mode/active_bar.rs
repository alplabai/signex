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

use iced::widget::{container, text, text_input, Space};
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

    let mk_tool = |label: &str,
                   tool: SketchTool,
                   icon: ActiveBarIcon|
     -> ActiveBarItem<LibraryMessage> {
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

    // Section 5 — Solve toggle.
    let auto_paused = editor.state.auto_pause.paused();
    let solve_path = path.clone();
    let solve_button = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Glyph(if auto_paused { "\u{25B6}" } else { "\u{23F8}" }),
        // ▶ = paused (click to resume); ⏸ = running (click to pause).
        tooltip: if auto_paused {
            "Resume live solve".into()
        } else {
            "Pause live solve".into()
        },
        enabled: true,
        selected: auto_paused,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: solve_path,
            msg: PrimitiveEditorMsg::FootprintSketchToggleAutoPause,
        }),
        ..ActiveBarButton::default()
    });

    // Section 4 — Dimension input as a Custom slot. Sized to fit
    // ~6 digits + "mm" hint inside the bar's vertical rhythm.
    let dim_input = build_dimension_input(editor, tokens);

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
            "Place Circle (centre + radius)",
            SketchTool::Circle,
            ActiveBarIcon::Svg(icons::icon_shape_circle(theme_id)),
        ),
        mk_tool(
            "Place Arc (centre + start + end)",
            SketchTool::Arc,
            ActiveBarIcon::Svg(icons::icon_shape_arc(theme_id)),
        ),
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
        // Section 5: Solve toggle
        solve_button,
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
            .primitive
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
