//! v0.13.1 Phase 6.5 — sketch inspector (lite).
//!
//! Three sections, displayed as a horizontal strip below the
//! footprint toolbar when [`EditorMode::Sketch`](crate::library::editor::footprint::state::EditorMode::Sketch) is active:
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
    widget::{Space, column, container, pick_list, row, scrollable, text, text_input},
};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::app::FootprintEditorState;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::tokens::{self, Overlay};

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
    let strip_bg = tokens::overlay(tokens, Overlay::Faint);

    // v0.14.2: tool palette + constraint authoring picker have moved
    // into the Fusion-360-style floating Active Bar over the canvas
    // (`crate::library::editor::footprint::sketch_mode::active_bar`).
    // The strip below the canvas carries DOF / parameters / role /
    // warnings — Role is a v0.16.2 addition for selection-aware bake
    // attr assignment.
    let dof = view_dof(editor, text_c, muted);
    let params = view_params(editor, text_c, muted, border, tokens);
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
        background: Some(iced::Background::Color(strip_bg)),
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
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let input_bg = tokens::input_bg(tokens);
    let selection = tokens::selection_tint(tokens);
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
                            background: iced::Background::Color(input_bg),
                            border: Border {
                                width: 1.0,
                                radius: 2.0.into(),
                                color: border,
                            },
                            icon: iced::Color::TRANSPARENT,
                            placeholder: muted,
                            value: text_c,
                            selection,
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
