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
//! 3. **Constrain** — 19 selection-aware constraint authoring
//!    buttons. Enabled state derives from the kinds of the primary
//!    + secondary selection slots (+ the extra slot for the two
//!    3-entity Symmetric constraints).
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
use crate::library::messages::{
    FootprintEditorMsg, LibraryMessage, PrimitiveEdit, SketchConstraintTag,
};

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
                    msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchSetTool(tool)),
                }),
                ..ActiveBarButton::default()
            })
        };

    // Compute which constraint tags apply to the current selection.
    let enabled = constraint_enable_matrix(editor);

    // v0.22 — `mk_constraint` now returns `Option<...>`: only emits a
    // button when the tag's slot is enabled by the current selection.
    // Inert constraint buttons used to occupy real width even when
    // they did nothing; the bar reads as a dynamic context-sensitive
    // strip now. Combined with the modify-section gating below, this
    // removes ~10 buttons from the no-selection state.
    let mk_constraint = |tag: SketchConstraintTag,
                         tooltip: &str,
                         glyph: &'static str|
     -> Option<ActiveBarItem<LibraryMessage>> {
        if !enabled[tag_index(tag)] {
            return None;
        }
        let p = path.clone();
        Some(ActiveBarItem::Button(ActiveBarButton {
            icon: ActiveBarIcon::Glyph(glyph),
            tooltip: tooltip.to_string(),
            enabled: true,
            selected: false,
            on_press: Some(LibraryMessage::PrimitiveEditorEvent {
                path: p,
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchAddConstraintForSelection(
                    tag,
                )),
            }),
            ..ActiveBarButton::default()
        }))
    };

    // v0.22 — Linetype tri-state pill. Combines the v0.16.1
    // Construction toggle and v0.22 Phase A5 Centerline toggle into a
    // single button that cycles Normal → Construction → Centerline →
    // Normal on left-click. The icon + tooltip update to reflect the
    // current state; the bar saves a button slot vs. two separate
    // pills with mutual-exclusion logic in the dispatcher.
    //
    // Cycle mapping:
    // - Normal      → ToggleConstruction (turns Construction on)
    // - Construction → ToggleCenterline   (turns Centerline on; the
    //                                      handler clears Construction
    //                                      because of the existing
    //                                      mutual-exclusivity rule)
    // - Centerline  → ToggleCenterline   (turns Centerline off → Normal)
    let construction_on = editor.state.construction_mode;
    let centerline_on = editor.state.centerline_mode;
    let linetype_path = path.clone();
    let (linetype_glyph, linetype_tooltip, linetype_msg) = if centerline_on {
        (
            "\u{2501}",
            "Linetype: Centerline (click → Normal)".to_string(),
            FootprintEditorMsg::SketchToggleCenterline,
        )
    } else if construction_on {
        (
            "\u{2504}",
            "Linetype: Construction (click → Centerline)".to_string(),
            FootprintEditorMsg::SketchToggleCenterline,
        )
    } else {
        (
            "\u{2501}\u{0307}", // ━̇ — solid line with overdot hint
            "Linetype: Normal (click → Construction)".to_string(),
            FootprintEditorMsg::SketchToggleConstruction,
        )
    };
    let linetype_button = ActiveBarItem::Button(ActiveBarButton {
        icon: ActiveBarIcon::Glyph(linetype_glyph),
        tooltip: linetype_tooltip,
        enabled: true,
        selected: construction_on || centerline_on,
        on_press: Some(LibraryMessage::PrimitiveEditorEvent {
            path: linetype_path,
            msg: PrimitiveEdit::Footprint(linetype_msg),
        }),
        ..ActiveBarButton::default()
    });

    // v0.22 Phase D4 — Make Pad from Profile button. One-shot action
    // (not a tool mode): converts the closed-loop profile that
    // includes the currently-selected Line into a Custom-shape pad.
    // Enabled only when a Line is selected; the dispatcher itself
    // verifies the loop closes and pushes a warning otherwise.
    let make_pad_path = path.clone();
    // v0.27 — accept any selection (primary, secondary, or extras)
    // that touches a Line directly OR a Point that's incident to a
    // Line. The dispatcher itself walks the closed loop and warns
    // if the seed isn't on one. Always-enabled when the sketch has
    // ≥1 Line so a "no selection — just convert the only loop"
    // workflow also works.
    let make_pad_enabled = {
        let sketch_ref = editor.primitive().sketch.as_ref();
        let any_line_in_sketch = sketch_ref
            .map(|s| {
                s.entities
                    .iter()
                    .any(|e| matches!(e.kind, signex_sketch::entity::EntityKind::Line { .. }))
            })
            .unwrap_or(false);
        let mut selection_iter = editor
            .state
            .selected_sketch
            .into_iter()
            .chain(editor.state.selected_sketch_secondary)
            .chain(editor.state.selected_sketch_extra.iter().copied());
        let selection_has_line_or_pointed_line = selection_iter.any(|id| {
            let Some(s) = sketch_ref else { return false };
            let Some(ent) = s.entities.iter().find(|e| e.id == id) else {
                return false;
            };
            match ent.kind {
                signex_sketch::entity::EntityKind::Line { .. } => true,
                signex_sketch::entity::EntityKind::Point { .. } => s.entities.iter().any(|other| {
                    matches!(
                        other.kind,
                        signex_sketch::entity::EntityKind::Line { start, end }
                            if start == id || end == id
                    )
                }),
                _ => false,
            }
        });
        any_line_in_sketch
            && (selection_has_line_or_pointed_line
                || editor.state.selected_sketch.is_none()
                    && editor.state.selected_sketch_secondary.is_none()
                    && editor.state.selected_sketch_extra.is_empty())
    };
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
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchMakePadFromProfile),
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

    // v0.22 — Build the bar in three groups: always-visible Create,
    // selection-gated Modify, and selection-driven Constrain. Empty
    // groups collapse cleanly so the no-selection bar reads compact:
    // Select | Point Line Rect RRect Circle Arc | DimInput | Linetype.
    let mut items: Vec<ActiveBarItem<LibraryMessage>> = Vec::new();

    // Section 1: Select
    items.push(mk_tool(
        "Select",
        SketchTool::Select,
        ActiveBarIcon::Svg(icons::icon_select(theme_id)),
    ));
    items.push(ActiveBarItem::Separator);

    // Section 2: Create — primitive geometry tools.
    // v0.14 — Place Point removed from the palette. Intersections snap
    // automatically (Line×Line / Line×Arc / Arc×Arc, see snap.rs
    // SnapKind::Intersection) and the Line/Rect/Circle/Arc tools
    // auto-create their own endpoint Points, so a manual free-point tool
    // was clutter in the footprint context. The SketchTool::Point
    // variant + dispatch stay (constraints + pad auto-mint reference
    // Points internally).
    items.push(mk_tool(
        "Place Line (2 clicks)",
        SketchTool::Line,
        ActiveBarIcon::Svg(icons::icon_shape_line(theme_id)),
    ));
    items.push(mk_tool(
        "Place Rectangle (corner + opposite corner)",
        SketchTool::Rectangle,
        ActiveBarIcon::Svg(icons::icon_shape_rect(theme_id)),
    ));
    items.push(mk_tool(
        "Place Rounded Rectangle (corner + opposite corner; radius from dim input)",
        SketchTool::RoundedRectangle,
        ActiveBarIcon::Glyph("\u{25A2}"), // ▢
    ));
    items.push(mk_tool(
        "Place Circle (centre + radius)",
        SketchTool::Circle,
        ActiveBarIcon::Svg(icons::icon_shape_circle(theme_id)),
    ));
    items.push(mk_tool(
        "Place Arc (centre + start + end)",
        SketchTool::Arc,
        ActiveBarIcon::Svg(icons::icon_shape_arc(theme_id)),
    ));
    // v0.24 Track C — Tangent Arc. Two-click chained arc segment that
    // mints an Arc tangent to whatever Line ends at the first click.
    // Mirrors the Arc entry visually (same arc SVG glyph) but emits a
    // distinct SketchTool variant + adds a TangentLineArc constraint
    // when committed so the tangency survives further edits.
    items.push(mk_tool(
        "Place Tangent Arc (chains tangent to previous Line)",
        SketchTool::TangentArc,
        ActiveBarIcon::Svg(icons::icon_shape_arc(theme_id)),
    ));

    // v0.27 — Fillet + Trim. Always available (no pre-selection
    // required). Fillet picks two adjacent Lines via two clicks
    // and rounds the corner with a tangent arc; Trim removes the
    // segment of a Line/Arc bounded by its nearest intersections
    // with other sketch entities. These are EDA-shaped — typical
    // use is rounding silk / courtyard corners + cleaning up
    // overlapping outline geometry.
    items.push(ActiveBarItem::Separator);
    items.push(mk_tool(
        "Fillet — click two adjacent Lines to round the corner with a tangent arc",
        SketchTool::Fillet,
        ActiveBarIcon::Glyph("\u{231C}"), // ⌜
    ));
    items.push(mk_tool(
        "Trim — click a segment to remove it up to its nearest intersections",
        SketchTool::Trim,
        ActiveBarIcon::Glyph("\u{2702}"), // ✂
    ));

    // Section 3: Modify — only visible when an entity is selected
    // (these tools all consume `editor.state.selected_sketch`). With
    // nothing selected they would all be silent no-ops with warnings,
    // so hiding them removes 5 buttons of width from the most-common
    // bar state and surfaces them right when they're useful.
    // v0.27 — also count rubber-band extras / secondary as a
    // selection. The Modify section was hiding when the user
    // rubber-banded a closed shape because the primary
    // `selected_sketch` could be empty even with 4 extras present.
    let any_selection = editor.state.selected_sketch.is_some()
        || editor.state.selected_sketch_secondary.is_some()
        || !editor.state.selected_sketch_extra.is_empty();
    if any_selection {
        items.push(ActiveBarItem::Separator);
        items.push(mk_tool(
            "Mirror — pre-select a Line, then click a Point/Line/Arc/Circle to mirror",
            SketchTool::Mirror,
            ActiveBarIcon::Glyph("\u{29B5}"), // ⦵
        ));
        items.push(mk_tool(
            "Offset — pre-select a Line / Arc / Circle, then click on the side to offset (distance from dim input)",
            SketchTool::Offset,
            ActiveBarIcon::Glyph("\u{29C8}"), // ⧈
        ));
        items.push(mk_tool(
            "Rectangular Pattern — click an entity to mint a 2×2 grid array (5 mm × 5 mm)",
            SketchTool::RectPattern,
            ActiveBarIcon::Glyph("\u{229E}"), // ⊞
        ));
        items.push(mk_tool(
            "Circular Pattern — click an entity to mint a 4-instance polar array (360°)",
            SketchTool::CircularPattern,
            ActiveBarIcon::Glyph("\u{233E}"), // ⌾
        ));
        items.push(make_pad_button);
    }

    // Section 4: Constrain — only the buttons whose precondition is
    // met by the current selection are rendered (mk_constraint
    // returns Option). Section disappears entirely when no
    // selection is active.
    let constraint_buttons: Vec<ActiveBarItem<LibraryMessage>> = [
        mk_constraint(
            SketchConstraintTag::Fixed,
            "Fix point in place (needs 1 Point)",
            "\u{2693}",
        ),
        mk_constraint(
            SketchConstraintTag::Coincident,
            "Coincident (needs 2 Points)",
            "\u{2299}",
        ),
        mk_constraint(
            SketchConstraintTag::DistancePtPt,
            "Distance between Points (needs 2 Points + dim input)",
            "\u{27F7}",
        ),
        mk_constraint(
            SketchConstraintTag::Horizontal,
            "Horizontal (needs 1 Line)",
            "\u{2500}",
        ),
        mk_constraint(
            SketchConstraintTag::Vertical,
            "Vertical (needs 1 Line)",
            "\u{2502}",
        ),
        mk_constraint(
            SketchConstraintTag::Parallel,
            "Parallel (needs 2 Lines)",
            "\u{2225}",
        ),
        mk_constraint(
            SketchConstraintTag::Perpendicular,
            "Perpendicular (needs 2 Lines)",
            "\u{27C2}",
        ),
        mk_constraint(
            SketchConstraintTag::EqualLength,
            "Equal length (needs 2 Lines)",
            "\u{2261}",
        ),
        mk_constraint(
            SketchConstraintTag::PointOnLine,
            "Point on line (needs 1 Point + 1 Line)",
            "\u{22A2}",
        ),
        mk_constraint(
            SketchConstraintTag::Midpoint,
            "Midpoint (needs 1 Point + 1 Line)",
            "\u{25C7}",
        ),
        mk_constraint(
            SketchConstraintTag::TangentLineArc,
            "Tangent (needs 1 Line + 1 Arc)",
            "T",
        ),
        mk_constraint(
            SketchConstraintTag::TangentArcArc,
            "Tangent (needs 2 Arcs)",
            "T",
        ),
        mk_constraint(
            SketchConstraintTag::Angle,
            "Angle between Lines (needs 2 Lines + dim input, degrees)",
            "\u{2220}", // ∠
        ),
        mk_constraint(
            SketchConstraintTag::EqualRadius,
            "Equal radius (needs 2 Circles/Arcs)",
            "\u{2261}R", // ≡R
        ),
        mk_constraint(
            SketchConstraintTag::PointOnArc,
            "Point on arc (needs 1 Point + 1 Arc)",
            "\u{2312}", // ⌒
        ),
        mk_constraint(
            SketchConstraintTag::DistancePtLine,
            "Distance Point↔Line (needs 1 Point + 1 Line + dim input)",
            "\u{27F7}", // ⟷
        ),
        mk_constraint(
            SketchConstraintTag::DistancePtCircle,
            "Distance Point↔Circle (needs 1 Point + 1 Circle/Arc + dim input)",
            "\u{27F7}", // ⟷
        ),
        // v0.15 — 3-entity Symmetric constraints. Primary + secondary
        // hold the two Points; the third entity (mirror Line / centre
        // Point) comes from the extra slot, so these light up only
        // when a third entity is rubber-band-selected into it.
        mk_constraint(
            SketchConstraintTag::SymmetricAboutLine,
            "Symmetric about line (needs 2 Points + 1 Line in selection)",
            "\u{25C3}\u{25B9}", // ◃▹
        ),
        mk_constraint(
            SketchConstraintTag::SymmetricAboutPoint,
            "Symmetric about point (needs 3 Points: 2 + a centre)",
            "\u{25C3}\u{25B9}", // ◃▹
        ),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !constraint_buttons.is_empty() {
        items.push(ActiveBarItem::Separator);
        items.extend(constraint_buttons);
    }

    // Section 5: Dimension input + Linetype tri-state pill — always
    // visible.
    items.push(ActiveBarItem::Separator);
    items.push(ActiveBarItem::Custom(dim_input));
    items.push(ActiveBarItem::Separator);
    items.push(linetype_button);

    items
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
fn constraint_enable_matrix(editor: &FootprintEditorState) -> [bool; 19] {
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
    // v0.15 — kind of the first extra-slot entity, used as the third
    // entity for the 3-entity Symmetric constraints.
    let extra = editor
        .state
        .selected_sketch_extra
        .first()
        .copied()
        .and_then(kind_of);
    let mut m = [false; 19];
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
            // 3-entity Symmetric constraints need a third entity in
            // the extra slot — gate the button on the extra kind.
            if extra == Some("Line") {
                m[tag_index(SketchConstraintTag::SymmetricAboutLine)] = true;
            }
            if extra == Some("Point") {
                m[tag_index(SketchConstraintTag::SymmetricAboutPoint)] = true;
            }
        }
        (Some("Line"), Some("Line")) => {
            m[tag_index(SketchConstraintTag::Parallel)] = true;
            m[tag_index(SketchConstraintTag::Perpendicular)] = true;
            m[tag_index(SketchConstraintTag::EqualLength)] = true;
            m[tag_index(SketchConstraintTag::Angle)] = true;
        }
        (Some("Point"), Some("Line")) | (Some("Line"), Some("Point")) => {
            m[tag_index(SketchConstraintTag::PointOnLine)] = true;
            m[tag_index(SketchConstraintTag::Midpoint)] = true;
            m[tag_index(SketchConstraintTag::DistancePtLine)] = true;
        }
        (Some("Line"), Some("Arc")) | (Some("Arc"), Some("Line")) => {
            m[tag_index(SketchConstraintTag::TangentLineArc)] = true;
        }
        (Some("Arc"), Some("Arc")) => {
            m[tag_index(SketchConstraintTag::TangentArcArc)] = true;
            m[tag_index(SketchConstraintTag::EqualRadius)] = true;
        }
        (Some("Point"), Some("Arc")) | (Some("Arc"), Some("Point")) => {
            m[tag_index(SketchConstraintTag::PointOnArc)] = true;
            m[tag_index(SketchConstraintTag::DistancePtCircle)] = true;
        }
        (Some("Point"), Some("Circle")) | (Some("Circle"), Some("Point")) => {
            m[tag_index(SketchConstraintTag::DistancePtCircle)] = true;
        }
        // EqualRadius spans any two of Circle / Arc.
        (Some("Circle"), Some("Circle"))
        | (Some("Circle"), Some("Arc"))
        | (Some("Arc"), Some("Circle")) => {
            m[tag_index(SketchConstraintTag::EqualRadius)] = true;
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
        SketchConstraintTag::TangentLineArc => 10,
        SketchConstraintTag::TangentArcArc => 11,
        SketchConstraintTag::Angle => 12,
        SketchConstraintTag::EqualRadius => 13,
        SketchConstraintTag::PointOnArc => 14,
        SketchConstraintTag::DistancePtLine => 15,
        SketchConstraintTag::DistancePtCircle => 16,
        SketchConstraintTag::SymmetricAboutLine => 17,
        SketchConstraintTag::SymmetricAboutPoint => 18,
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
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchDimensionInput(s)),
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
