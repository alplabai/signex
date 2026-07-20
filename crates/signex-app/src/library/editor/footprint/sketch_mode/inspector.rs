//! v0.13.1 Phase 6.5 — sketch inspector (lite).
//!
//! Three sections, displayed as a horizontal strip below the
//! footprint toolbar when [`EditorMode::Sketch`] is active:
//!
//! 1. **DOF readout** — `state.len()` / constraint count / rank
//!    (free DoF) + last solve elapsed_ms + auto-pause status.
//! 2. **Parameter table** — list of file-local parameters; each row
//!    is editable in place. New rows are appended via the `+ Add`
//!    button.
//! 3. **Solve warnings** — pad-bake warnings surfaced from the most
//!    recent solve (Castellated bakes as Tht, Chamfered → RoundRect,
//!    PasteAperturePattern Grid/Custom deferred, etc.).
//!
//! Selection editing (per-entity coords, attached constraints with
//! delete buttons, auto-Coincident toggle) is deferred to v0.13.2 —
//! requires per-entity hit-testing in the canvas which lives in the
//! same patch as Task 6.3's tool palette.

use iced::{
    Border, Color, Element, Length, Theme,
    widget::{Space, button, column, container, pick_list, row, scrollable, text, text_input},
};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::app::FootprintEditorState;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};

/// Render the inspector strip. Returns an empty `Space` when not in
/// Sketch mode so the caller can unconditionally add it to the body
/// column without `if`-branching.
pub fn view<'a>(
    editor: &'a FootprintEditorState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    use crate::library::editor::footprint::state::EditorMode;
    if editor.state.mode != EditorMode::Sketch {
        return Space::new()
            .width(Length::Shrink)
            .height(Length::Shrink)
            .into();
    }

    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    // v0.14.2: tool palette + constraint authoring picker have moved
    // into the Fusion-360-style floating Active Bar over the canvas
    // (`crate::library::editor::footprint::sketch_mode::active_bar`).
    // The strip below the canvas carries DOF / parameters / role /
    // warnings — Role is a v0.16.2 addition for selection-aware bake
    // attr assignment.
    let dof = view_dof(editor, text_c, muted);
    let params = view_params(editor, text_c, muted, border);
    let role = view_role(editor, text_c, muted);
    let warnings = view_warnings(editor, text_c, muted);

    container(
        row![
            container(dof)
                .padding([6, 10])
                .width(Length::FillPortion(1)),
            container(params)
                .padding([6, 10])
                .width(Length::FillPortion(2)),
            container(role)
                .padding([6, 10])
                .width(Length::FillPortion(2)),
            container(warnings)
                .padding([6, 10])
                .width(Length::FillPortion(2)),
        ]
        .spacing(8),
    )
    .padding([4, 8])
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.02,
        ))),
        border: Border {
            width: 0.0,
            radius: 0.0.into(),
            color: Color::TRANSPARENT,
        },
        ..iced::widget::container::Style::default()
    })
    .into()
}

// v0.14.2: replaced by `crate::library::editor::footprint::sketch_mode
// ::active_bar` — kept the body below temporarily as a doc-only ref;
// remove on next visual-pass commit.
#[allow(dead_code)]
fn view_tool_palette<'a>(
    editor: &'a FootprintEditorState,
    text_c: Color,
    muted: Color,
    border: Color,
) -> Element<'a, LibraryMessage> {
    use crate::library::editor::footprint::state::SketchTool;
    let active = editor.state.active_tool;
    let mk_pill = |label: &'static str, target: SketchTool| -> Element<'a, LibraryMessage> {
        let path = editor.path.clone();
        let on = active == target;
        let label_color = if on { text_c } else { muted };
        button(text(label).size(11).color(label_color))
            .padding([3, 8])
            .on_press(LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchSetTool(target)),
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: if on {
                    Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.10,
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
            .into()
    };

    use crate::library::editor::footprint::state::ToolPending;
    let pending_label: String = match editor.state.tool_pending {
        ToolPending::Idle => match active {
            SketchTool::Select => String::new(),
            SketchTool::Point => "click to place".into(),
            SketchTool::Line => "click first endpoint".into(),
            SketchTool::Rectangle => "click first corner".into(),
            SketchTool::RoundedRectangle => "click first corner (radius from input)".into(),
            SketchTool::Circle => "click centre".into(),
            SketchTool::Arc => "click centre".into(),
            SketchTool::Mirror => {
                "select a Line first, then click a Point to mirror".into()
            }
            SketchTool::Offset => {
                "select a Line / Arc / Circle first, then click on the side to offset (distance from input)".into()
            }
            SketchTool::RectPattern => {
                "click an entity to mint a 2×2 grid array (5 mm × 5 mm; edit via JSON or Properties)".into()
            }
            SketchTool::CircularPattern => {
                "click an entity to mint a 4-instance circular array (360°; centre 5 mm right of source)".into()
            }
            SketchTool::TangentArc => "click first endpoint of tangent arc".into(),
            SketchTool::Fillet => {
                "click the first Line — radius from input (default 0.5 mm)".into()
            }
            SketchTool::Trim => {
                "click a Line segment to remove it (cuts to nearest intersections)".into()
            }
            // #372 — single click splits a Line in two at the click.
            SketchTool::BreakTrack => {
                "click a Line to split it in two at the click point".into()
            }
            // #361 — endpoint-biased segment grab.
            SketchTool::DragTrackEnd => {
                "press anywhere on a track and drag its nearer end to a new position".into()
            }
        },
        ToolPending::LineFirst { .. } => "click second endpoint (Esc to cancel)".into(),
        ToolPending::RectangleFirst { .. } => "click opposite corner (Esc to cancel)".into(),
        ToolPending::RoundedRectangleFirst { .. } => "click opposite corner (Esc to cancel)".into(),
        ToolPending::CircleCenter { .. } => "click radius point (Esc to cancel)".into(),
        ToolPending::ArcCenter { .. } => "click start (Esc to cancel)".into(),
        ToolPending::ArcStart { .. } => "click end (Esc to cancel)".into(),
        ToolPending::RepickPolarCenter { .. } => {
            "click a Point to set polar centre (Esc to cancel)".into()
        }
        ToolPending::TangentArcFirst { .. } => "click second endpoint (Esc to cancel)".into(),
        ToolPending::FilletFirst { .. } => {
            "click the second Line that shares a corner (Esc to cancel)".into()
        }
    };

    row![
        text("Tool:").size(11).color(muted),
        mk_pill("Select", SketchTool::Select),
        mk_pill("Point", SketchTool::Point),
        mk_pill("Line", SketchTool::Line),
        mk_pill("Circle", SketchTool::Circle),
        mk_pill("Arc", SketchTool::Arc),
        Space::new().width(Length::Fixed(8.0)),
        text(pending_label).size(10).color(muted),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center)
    .into()
}

/// v0.13.3 — selection-aware constraint submenu. Renders a row of
/// applicable-constraint pills based on the current
/// `selected_sketch` + `selected_sketch_secondary` slots. Empty
/// when no entity is selected. Includes the Dimension tool's inline
/// numeric value-entry field for `DistancePtPt`.
///
/// v0.14.2: superseded by the Active Bar in
/// `crate::library::editor::footprint::sketch_mode::active_bar`.
#[allow(dead_code)]
fn view_constraint_submenu<'a>(
    editor: &'a FootprintEditorState,
    text_c: Color,
    muted: Color,
    border: Color,
) -> Element<'a, LibraryMessage> {
    use crate::library::messages::SketchConstraintTag;
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
    let p_kind = primary.and_then(kind_of);
    let s_kind = secondary.and_then(kind_of);

    let mut tags: Vec<(&'static str, SketchConstraintTag)> = Vec::new();
    let needs_dim_input;
    match (p_kind, s_kind) {
        (Some("Point"), None) => {
            tags.push(("Fix", SketchConstraintTag::Fixed));
            needs_dim_input = false;
        }
        (Some("Line"), None) => {
            tags.push(("Horizontal", SketchConstraintTag::Horizontal));
            tags.push(("Vertical", SketchConstraintTag::Vertical));
            needs_dim_input = false;
        }
        (Some("Point"), Some("Point")) => {
            tags.push(("Coincident", SketchConstraintTag::Coincident));
            tags.push(("Distance", SketchConstraintTag::DistancePtPt));
            needs_dim_input = true;
        }
        (Some("Line"), Some("Line")) => {
            tags.push(("Parallel", SketchConstraintTag::Parallel));
            tags.push(("Perpendicular", SketchConstraintTag::Perpendicular));
            tags.push(("Equal length", SketchConstraintTag::EqualLength));
            needs_dim_input = false;
        }
        (Some("Point"), Some("Line")) | (Some("Line"), Some("Point")) => {
            tags.push(("On line", SketchConstraintTag::PointOnLine));
            tags.push(("Midpoint", SketchConstraintTag::Midpoint));
            needs_dim_input = false;
        }
        _ => {
            needs_dim_input = false;
        }
    }

    let header_label = match (p_kind, s_kind) {
        (None, _) => "Selection: (none) — click a sketch entity in Sketch mode",
        (Some(a), None) => match a {
            "Point" => "Selection: 1 Point",
            "Line" => "Selection: 1 Line",
            "Arc" => "Selection: 1 Arc",
            "Circle" => "Selection: 1 Circle",
            _ => "Selection: 1 entity",
        },
        (Some(_), Some(_)) => "Selection: 2 entities",
    };

    let mut pill_row = row![text(header_label).size(11).color(muted)]
        .spacing(6)
        .align_y(iced::Alignment::Center);

    for (label, tag) in &tags {
        let path = editor.path.clone();
        let t = *tag;
        let pill = button(text(*label).size(11).color(text_c))
            .padding([3, 8])
            .on_press(LibraryMessage::PrimitiveEditorEvent {
                path,
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchAddConstraintForSelection(
                    t,
                )),
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
        pill_row = pill_row.push(pill);
    }

    if needs_dim_input {
        let path = editor.path.clone();
        pill_row = pill_row.push(Space::new().width(Length::Fixed(8.0)));
        pill_row = pill_row.push(text("mm:").size(11).color(muted));
        let input = text_input("0.0", &editor.state.dimension_input)
            .size(11)
            .padding(2)
            .width(Length::Fixed(80.0))
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
        pill_row = pill_row.push(input);
    }

    if let Some(_) = primary {
        let clear_path = editor.path.clone();
        pill_row = pill_row.push(Space::new().width(Length::Fixed(8.0)));
        let clear = button(text("Deselect").size(10).color(muted))
            .padding([2, 6])
            .on_press(LibraryMessage::PrimitiveEditorEvent {
                path: clear_path,
                msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchSelect {
                    id: None,
                    shift: false,
                }),
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
        pill_row = pill_row.push(clear);
    }

    container(pill_row)
        .padding([4, 10])
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                1.0, 1.0, 1.0, 0.01,
            ))),
            border: Border {
                width: 0.0,
                radius: 0.0.into(),
                color: Color::TRANSPARENT,
            },
            ..iced::widget::container::Style::default()
        })
        .into()
}

fn view_dof<'a>(
    editor: &'a FootprintEditorState,
    text_c: Color,
    muted: Color,
) -> Element<'a, LibraryMessage> {
    let last = editor.state.last_solve.as_ref();
    let n_state = last.map(|o| o.result.state.len()).unwrap_or(0);
    let m_residual = last
        .map(|o| o.jacobian.iter().map(|_| 1usize).sum::<usize>())
        .unwrap_or(0);
    let n_over = last.map(|o| o.over_constraints.len()).unwrap_or(0);
    let elapsed = last.map(|o| o.result.elapsed_ms).unwrap_or(0);
    let iters = last.map(|o| o.result.iterations).unwrap_or(0);

    column![
        text("DOF").size(11).color(text_c),
        text(format!(
            "state={n_state}  residual={m_residual}  over={n_over}"
        ))
        .size(10)
        .color(muted),
        text(format!("iters={iters}  elapsed={elapsed}ms"))
            .size(10)
            .color(muted),
    ]
    .spacing(2)
    .into()
}

fn view_params<'a>(
    editor: &'a FootprintEditorState,
    text_c: Color,
    muted: Color,
    border: Color,
) -> Element<'a, LibraryMessage> {
    let mut col = column![text("Parameters").size(11).color(text_c)].spacing(2);

    if let Some(sketch) = editor.primitive().sketch.as_ref() {
        if sketch.parameters.is_empty() {
            col = col.push(
                text("(no parameters yet — add via Inspector v0.13.2)")
                    .size(10)
                    .color(muted),
            );
        } else {
            for (name, src) in sketch.parameters.iter() {
                let name_clone = name.clone();
                let path = editor.path.clone();
                let row = row![
                    text(name)
                        .size(10)
                        .color(text_c)
                        .width(Length::Fixed(110.0)),
                    text_input("expression…", src)
                        .size(10)
                        .padding(2)
                        .style(move |_: &Theme, _| iced::widget::text_input::Style {
                            background: iced::Background::Color(iced::Color::from_rgba(
                                1.0, 1.0, 1.0, 0.04,
                            )),
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
                        .on_input(move |new_expr| LibraryMessage::PrimitiveEditorEvent {
                            path: path.clone(),
                            msg: PrimitiveEdit::Footprint(
                                FootprintEditorMsg::SketchEditParameter {
                                    name: name_clone.clone(),
                                    expr: new_expr,
                                }
                            ),
                        }),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center);
                col = col.push(row);
            }
        }
    } else {
        col = col.push(
            text("(no sketch — switch to Sketch mode)")
                .size(10)
                .color(muted),
        );
    }

    scrollable(col).height(Length::Fixed(80.0)).into()
}

fn view_warnings<'a>(
    editor: &'a FootprintEditorState,
    text_c: Color,
    muted: Color,
) -> Element<'a, LibraryMessage> {
    let mut col = column![text("Solve warnings").size(11).color(text_c)].spacing(2);
    if editor.state.solve_warnings.is_empty() {
        col = col.push(text("(none)").size(10).color(muted));
    } else {
        for w in editor.state.solve_warnings.iter().take(8) {
            col = col.push(text(w).size(9).color(muted));
        }
        if editor.state.solve_warnings.len() > 8 {
            col = col.push(
                text(format!("… +{} more", editor.state.solve_warnings.len() - 8))
                    .size(9)
                    .color(muted),
            );
        }
    }
    scrollable(col).height(Length::Fixed(80.0)).into()
}

/// v0.16.2 — Role-assignment dropdown. Visible only when a sketch
/// entity is selected; pick_list value mirrors the entity's
/// currently-attached `*Attr` slot (or `Unassigned`). Picking a new
/// value emits [`FootprintEditorMsg::SketchSetRole`] which
/// the dispatcher routes through `apply_sketch_role_with_warnings`
/// (clears all attrs, sets the matching one with defaults, runs
/// solve + bake).
fn view_role<'a>(
    editor: &'a FootprintEditorState,
    text_c: Color,
    muted: Color,
) -> Element<'a, LibraryMessage> {
    use crate::library::editor::footprint::sketch_dispatch::current_role_of;
    use crate::library::messages::RoleTag;
    use signex_sketch::entity::EntityKind;

    let primary = editor.state.selected_sketch;
    let selected_entity = primary.and_then(|id| {
        editor
            .primitive()
            .sketch
            .as_ref()?
            .entities
            .iter()
            .find(|e| e.id == id)
    });

    let mut col = column![text("Role").size(11).color(text_c)].spacing(4);

    let entity = match selected_entity {
        Some(e) => e,
        None => {
            col = col.push(
                text("(select a sketch entity in Sketch mode)")
                    .size(10)
                    .color(muted),
            );
            return col.into();
        }
    };

    let id = primary.unwrap();
    let current = current_role_of(entity);
    let path = editor.path.clone();

    let kind_label = match entity.kind {
        EntityKind::Point { .. } => "Point",
        EntityKind::Line { .. } => "Line",
        EntityKind::Arc { .. } => "Arc",
        EntityKind::Circle { .. } => "Circle",
    };
    col = col.push(
        text(format!("Selection: {kind_label}"))
            .size(10)
            .color(muted),
    );

    let dropdown = pick_list(RoleTag::ALL, Some(current), move |new_role| {
        LibraryMessage::PrimitiveEditorEvent {
            path: path.clone(),
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchSetRole { id, role: new_role }),
        }
    })
    .text_size(11)
    .padding([3, 8])
    .width(Length::Fill);

    col = col.push(dropdown);

    // Inline hint: Pad role only valid on Points; flag the silent
    // no-op path so the user knows why "Pad" doesn't take effect on
    // a Line or Arc selection.
    let is_point = matches!(entity.kind, EntityKind::Point { .. });
    if !is_point {
        col = col.push(text("Pad role applies to Points only").size(9).color(muted));
    }

    col.into()
}
