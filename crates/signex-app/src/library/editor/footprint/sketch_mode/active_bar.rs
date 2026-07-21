//! Sketch-mode Active Bar — floating toolbar over the footprint
//! canvas when the editor is in [`EditorMode::Sketch`].
//!
//! Mirrors the Fusion 360 sketch toolbar layout, including its
//! grouping: the geometry tools collapse into two dropdown triggers
//! (**Create ▾** and **Modify ▾**) while the constraints stay a flat,
//! selection-driven strip — exactly the split Fusion uses, and for the
//! same reason. A constraint is applied to a selection you already
//! made, so burying it costs a click on the most frequent action;
//! arming a drawing tool happens once and then you draw many, so the
//! extra click amortises to nothing.
//!
//! Layout (left → right):
//!
//! 1. **Select** — sketch entity selection tool.
//! 2. **Create ▾** — Line / Rectangle / Rounded Rectangle / Circle /
//!    Arc / Tangent Arc. The trigger borrows the armed tool's icon so
//!    the collapsed bar still shows what's in hand.
//! 3. **Modify ▾** — Fillet / Trim / Mirror / Offset / Rectangular +
//!    Circular Pattern / Make Pad from Profile.
//! 4. **Constrain** — 19 selection-aware constraint buttons. Only the
//!    ones the current selection actually permits are rendered, so
//!    this section is empty until something is selected. Enabled state
//!    derives from the kinds of the primary + secondary selection
//!    slots (+ the extra slot for the two 3-entity Symmetric
//!    constraints).
//! 5. **Dimension input** — `Custom` slot with a `text_input` for
//!    the `DistancePtPt` numeric value.
//! 6. **Linetype** — Normal / Construction / Centerline tri-state pill.
//!
//! Both dropdown menus live in
//! [`crate::library::editor::footprint::active_bar_dropdowns`]; this
//! module only builds their trigger buttons. The bar floats over the
//! canvas (Stack overlay layer) so it doesn't steal vertical space
//! from the drawing area.

use std::path::PathBuf;

use iced::widget::{Space, container, text, text_input};
use iced::{Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::active_bar::{ActiveBarButton, ActiveBarIcon, ActiveBarItem};
use signex_widgets::theme_ext;

use crate::app::FootprintEditorState;
use crate::icons;
use crate::library::editor::footprint::state::{FpActiveBarMenu, SketchTool};
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
                         icon: iced::widget::svg::Handle|
     -> Option<ActiveBarItem<LibraryMessage>> {
        if !enabled[tag_index(tag)] {
            return None;
        }
        let p = path.clone();
        Some(ActiveBarItem::Button(ActiveBarButton {
            icon: ActiveBarIcon::Svg(icon),
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

    // Create ▾ / Modify ▾ group triggers. Both clicks open the menu —
    // there is no sensible "default action" for a group of six tools,
    // and guessing one would make the button do different things on
    // different days. The trigger instead reports state: it borrows
    // the armed tool's icon when the armed tool belongs to the group,
    // and paints selected while either that tool is armed or the menu
    // is open.
    let mk_group = |menu: FpActiveBarMenu,
                    label: &str,
                    fallback: iced::widget::svg::Handle|
     -> ActiveBarItem<LibraryMessage> {
        let tools = match menu {
            FpActiveBarMenu::SketchModify => FpActiveBarMenu::SKETCH_MODIFY_TOOLS,
            _ => FpActiveBarMenu::SKETCH_CREATE_TOOLS,
        };
        let owns_armed = tools.contains(&active_tool);
        let toggle = LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::ToggleActiveBarMenu(menu)),
        };
        ActiveBarItem::Button(ActiveBarButton {
            icon: ActiveBarIcon::Svg(if owns_armed {
                sketch_tool_icon(active_tool, theme_id)
            } else {
                fallback
            }),
            tooltip: label.to_string(),
            enabled: true,
            selected: owns_armed || editor.state.active_bar_menu == Some(menu),
            on_press: Some(toggle.clone()),
            on_right_press: Some(toggle),
            dropdown_indicator: Some(ActiveBarIcon::Svg(icons::icon_chevron_45(theme_id))),
        })
    };

    // Section 4 — Dimension input as a Custom slot. Sized to fit
    // ~6 digits + "mm" hint inside the bar's vertical rhythm.
    let dim_input = build_dimension_input(editor, tokens);

    let _ = tokens;

    // Bar shape: Select | Create ▾ Modify ▾ | <constraints the current
    // selection permits> | DimInput | Linetype. The two group triggers
    // replace what used to be twelve always-visible tool buttons; the
    // constraint section still collapses to nothing when no selection
    // is active, so the resting bar is six slots wide.
    //
    // NOTE — `unified_active_bar::dropdown_x_offset` mirrors the first
    // four slots below (Select, Separator, Create, Modify) to place the
    // Create / Modify dropdown panels. Reordering them without updating
    // that function puts the panels under the wrong button;
    // `sketch_group_triggers_sit_where_the_offsets_say` pins it.
    let mut items: Vec<ActiveBarItem<LibraryMessage>> = Vec::new();

    // Section 1: Select
    items.push(mk_tool(
        "Select",
        SketchTool::Select,
        ActiveBarIcon::Svg(icons::icon_select(theme_id)),
    ));
    items.push(ActiveBarItem::Separator);

    // Section 2 + 3: the Create / Modify group triggers.
    // v0.14 — Place Point is absent from Create by design. Intersections
    // snap automatically (Line×Line / Line×Arc / Arc×Arc, see snap.rs
    // SnapKind::Intersection) and the Line/Rect/Circle/Arc tools
    // auto-create their own endpoint Points, so a manual free-point tool
    // was clutter in the footprint context. The SketchTool::Point
    // variant + dispatch stay (constraints + pad auto-mint reference
    // Points internally).
    items.push(mk_group(
        FpActiveBarMenu::SketchCreate,
        "Create — Line / Rectangle / Rounded Rectangle / Circle / Arc / Tangent Arc",
        icons::icon_sk_create(theme_id),
    ));
    items.push(mk_group(
        FpActiveBarMenu::SketchModify,
        "Modify — Fillet / Trim / Mirror / Offset / Patterns / Make Pad from Profile",
        icons::icon_sk_modify(theme_id),
    ));

    // Section 4: Constrain — only the buttons whose precondition is
    // met by the current selection are rendered (mk_constraint
    // returns Option). Section disappears entirely when no
    // selection is active.
    let constraint_buttons: Vec<ActiveBarItem<LibraryMessage>> = [
        mk_constraint(
            SketchConstraintTag::Fixed,
            "Fix point in place (needs 1 Point)",
            icons::icon_sk_c_fixed(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::Coincident,
            "Coincident (needs 2 Points)",
            icons::icon_sk_c_coincident(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::DistancePtPt,
            "Distance between Points (needs 2 Points + dim input)",
            icons::icon_sk_c_distance_pt_pt(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::Horizontal,
            "Horizontal (needs 1 Line)",
            icons::icon_sk_c_horizontal(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::Vertical,
            "Vertical (needs 1 Line)",
            icons::icon_sk_c_vertical(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::Parallel,
            "Parallel (needs 2 Lines)",
            icons::icon_sk_c_parallel(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::Perpendicular,
            "Perpendicular (needs 2 Lines)",
            icons::icon_sk_c_perpendicular(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::EqualLength,
            "Equal length (needs 2 Lines)",
            icons::icon_sk_c_equal_length(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::PointOnLine,
            "Point on line (needs 1 Point + 1 Line)",
            icons::icon_sk_c_point_on_line(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::Midpoint,
            "Midpoint (needs 1 Point + 1 Line)",
            icons::icon_sk_c_midpoint(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::TangentLineArc,
            "Tangent (needs 1 Line + 1 Arc)",
            icons::icon_sk_c_tangent_line_arc(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::TangentArcArc,
            "Tangent (needs 2 Arcs)",
            icons::icon_sk_c_tangent_arc_arc(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::Angle,
            "Angle between Lines (needs 2 Lines + dim input, degrees)",
            icons::icon_sk_c_angle(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::EqualRadius,
            "Equal radius (needs 2 Circles/Arcs)",
            icons::icon_sk_c_equal_radius(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::PointOnArc,
            "Point on arc (needs 1 Point + 1 Arc)",
            icons::icon_sk_c_point_on_arc(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::DistancePtLine,
            "Distance Point↔Line (needs 1 Point + 1 Line + dim input)",
            icons::icon_sk_c_distance_pt_line(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::DistancePtCircle,
            "Distance Point↔Circle (needs 1 Point + 1 Circle/Arc + dim input)",
            icons::icon_sk_c_distance_pt_circle(theme_id),
        ),
        // v0.15 — 3-entity Symmetric constraints. Primary + secondary
        // hold the two Points; the third entity (mirror Line / centre
        // Point) comes from the extra slot, so these light up only
        // when a third entity is rubber-band-selected into it.
        mk_constraint(
            SketchConstraintTag::SymmetricAboutLine,
            "Symmetric about line (needs 2 Points + 1 Line in selection)",
            icons::icon_sk_c_symmetric_line(theme_id),
        ),
        mk_constraint(
            SketchConstraintTag::SymmetricAboutPoint,
            "Symmetric about point (needs 3 Points: 2 + a centre)",
            icons::icon_sk_c_symmetric_point(theme_id),
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
    items.push(ActiveBarItem::custom(dim_input, DIM_INPUT_W));
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

/// Icon for an armed sketch tool, so a collapsed group trigger can
/// show what's in hand instead of the generic group glyph. `Select`
/// and `Point` never reach a group trigger (neither belongs to one);
/// they fall back to the Create glyph.
fn sketch_tool_icon(
    tool: SketchTool,
    theme_id: signex_types::theme::ThemeId,
) -> iced::widget::svg::Handle {
    match tool {
        SketchTool::Line => icons::icon_shape_line(theme_id),
        SketchTool::Rectangle => icons::icon_shape_rect(theme_id),
        SketchTool::RoundedRectangle => icons::icon_sk_rounded_rect(theme_id),
        SketchTool::Circle => icons::icon_shape_circle(theme_id),
        SketchTool::Arc | SketchTool::TangentArc => icons::icon_shape_arc(theme_id),
        SketchTool::Fillet => icons::icon_sk_fillet(theme_id),
        SketchTool::Trim => icons::icon_sk_trim(theme_id),
        SketchTool::Mirror => icons::icon_sk_mirror(theme_id),
        SketchTool::Offset => icons::icon_sk_offset(theme_id),
        SketchTool::RectPattern => icons::icon_sk_rect_pattern(theme_id),
        SketchTool::CircularPattern => icons::icon_sk_circular_pattern(theme_id),
        // BreakTrack (#372) / DragTrackEnd (#361) land on this file via the
        // trunk merge; they are armed from the main bar's "Modify Tracks"
        // dropdown, not a sketch group trigger, so this icon is only a
        // fallback — use the Modify glyph, both being track-edit tools.
        SketchTool::BreakTrack | SketchTool::DragTrackEnd => icons::icon_sk_modify(theme_id),
        SketchTool::Select | SketchTool::Point => icons::icon_sk_create(theme_id),
    }
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

/// Declared width of the dimension-input slot, handed to
/// [`ActiveBarItem::custom`] so the bar can measure itself.
///
/// FIXED, not shrink-to-fit, and that is the point: `slot_offsets`
/// can't run a layout pass, so a Custom slot's width has to be stated
/// rather than discovered. Left to auto-size, this one's width would
/// depend on how wide the font renders "mm" — and because the bar is
/// centre-aligned, any error here moves *every* dropdown by half of it.
///
/// Content is `4 + "mm" + 4 + 58 px input + 4` ≈ 87 px; the value has
/// slack over that so a wider font can't clip the input.
pub(crate) const DIM_INPUT_W: f32 = 92.0;

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
    .width(Length::Fixed(DIM_INPUT_W))
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
