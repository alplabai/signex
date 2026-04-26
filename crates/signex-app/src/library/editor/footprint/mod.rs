//! Footprint tab — minimum-viable editor with pad placement,
//! drag-move, delete, and an auto-fit courtyard.
//!
//! See `LIBRARY_PLAN.md §10` for the spec; this MVP wraps the
//! upcoming PCB footprint editor (pads + courtyard + silk + fab
//! layers) inside the Component Editor's tabbed surface.

pub mod canvas;
pub mod layers;
pub mod state;

#[cfg(test)]
mod tests;

use iced::widget::canvas::Cache;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use crate::library::messages::{EditorMsg, LibraryMessage};
use crate::library::state::ComponentEditorState;

use canvas::FootprintCanvas;
use layers::FpLayer;

/// Render the Footprint tab. Builds the layer toolbar, the canvas,
/// and a footer status line in a single column.
pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    // Panel-region background colour for the canvas — the same tone the
    // rest of the panels use so the editor doesn't look like a popup.
    // ThemeTokens doesn't carry canvas-specific colours, so we derive
    // a slightly darker shade from the editor body bg and use the
    // border colour for the grid lines.
    let bg = crate::styles::ti(tokens.bg);
    let grid = crate::styles::ti(tokens.text_secondary);

    let toolbar = layer_toolbar(editor, tokens, window_id);
    let canvas_area = canvas_area(editor, tokens, window_id, bg, grid);
    let footer = footer_status(editor, tokens, window_id);

    column![toolbar, canvas_area, footer]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Layer toggle bar — one pill per `FpLayer`. Each pill flips the
/// per-layer visibility flag.
fn layer_toolbar<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let layers = editor
        .footprint_state
        .as_ref()
        .map(|s| s.layer_visibility)
        .unwrap_or_default();
    let auto_fit_on = editor
        .footprint_state
        .as_ref()
        .map(|s| s.auto_fit_courtyard)
        .unwrap_or(true);

    let mut row_widget = row![text("Layers:").size(11).color(muted)]
        .spacing(6)
        .align_y(iced::Alignment::Center);
    for layer in FpLayer::ORDER {
        let on = layers.get(*layer);
        let swatch = layer.color();
        let label_color = if on { text_c } else { muted };
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
        .on_press(LibraryMessage::EditorEvent {
            window_id,
            msg: EditorMsg::FootprintToggleLayer(layer.standard_name().to_string()),
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
                    iced::Color { a: 0.6, ..swatch }
                } else {
                    border
                },
            },
            text_color: label_color,
            ..iced::widget::button::Style::default()
        });
        row_widget = row_widget.push(pill);
    }
    // Push the auto-fit toggle to the right.
    row_widget = row_widget.push(Space::new().width(Length::Fill));

    let auto_fit_label = if auto_fit_on {
        "Auto-fit courtyard: On"
    } else {
        "Auto-fit courtyard: Off"
    };
    let auto_fit_btn = button(
        container(text(auto_fit_label).size(11).color(text_c)).padding([3, 10]),
    )
    .on_press(LibraryMessage::EditorEvent {
        window_id,
        msg: EditorMsg::FootprintToggleAutoFit,
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
        text_color: text_c,
        ..iced::widget::button::Style::default()
    });
    row_widget = row_widget.push(auto_fit_btn);

    container(row_widget)
        .padding([6, 10])
        .style(crate::styles::tab_bar_strip(tokens))
        .into()
}

/// The canvas itself — wrapped in a container so the borders match
/// the rest of the editor surface, and surrounded by a key-listener
/// container that catches Delete.
fn canvas_area<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
    bg: iced::Color,
    grid: iced::Color,
) -> Element<'a, LibraryMessage> {
    let border = theme_ext::border_color(tokens);

    if let Some(fp_state) = editor.footprint_state.as_ref() {
        let cache = editor
            .footprint_canvas_cache
            .get_or_init(Cache::new);
        let prog = FootprintCanvas {
            state: fp_state,
            window_id,
            bg_color: bg,
            grid_color: grid,
            cache,
        };
        let canvas_widget = iced::widget::canvas(prog)
            .width(Length::Fill)
            .height(Length::Fill);
        container(canvas_widget)
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
    } else {
        // Footprint state not yet initialised — should be rare in
        // practice because mod.rs initialises it on the first tab
        // switch, but we handle the race for safety.
        container(text("Initialising footprint…").size(12))
            .padding(20)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}

/// Footer line — cursor readout in mm + selected-pad summary.
fn footer_status<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    _window_id: iced::window::Id,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let cursor_label = match editor.footprint_state.as_ref().and_then(|s| s.cursor_mm) {
        Some((x, y)) => format!("X {x:>+8.3} mm   Y {y:>+8.3} mm"),
        None => "X    -.--- mm   Y    -.--- mm".to_string(),
    };
    let pad_count = editor
        .footprint_state
        .as_ref()
        .map(|s| s.pads.len())
        .unwrap_or(0);
    let selected_label = match editor
        .footprint_state
        .as_ref()
        .and_then(|s| s.selected_pad.map(|i| (i, s)))
    {
        Some((i, s)) => match s.pads.get(i) {
            Some(pad) => format!(
                "Pad {} — {:.2} × {:.2} mm @ ({:+.3}, {:+.3})",
                pad.number,
                pad.size_mm.0,
                pad.size_mm.1,
                pad.position_mm.0,
                pad.position_mm.1
            ),
            None => format!("Pads: {pad_count}"),
        },
        None => format!("Pads: {pad_count}   ·   Click empty area to add, drag to move, Del to remove"),
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

