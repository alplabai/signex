//! Component Preview tab — read-only Symbol/Footprint render plus
//! template-validated forms for parameters / supply / datasheet /
//! simulation.
//!
//! In the v0.9-refactor-2 (DBLib) model, components are TSV rows
//! addressed by [`crate::library::state::EditorAddress`]
//! (`library_path + table + row_id`). The state lives on
//! [`crate::library::state::ComponentPreviewState`].
//!
//! The Component view is preview-only: Symbol and Footprint render
//! read-only. Editing happens via standalone `.snxsym` / `.snxfpt`
//! document tabs in the main window — right-click on either render
//! in the Preview tab fires
//! [`crate::library::messages::LibraryMessage::OpenPrimitiveEditor`]
//! to open the standalone editor.
//!
//! Tab count is 5: Preview / Parameters / Supply / Datasheet /
//! Simulation. Pin Map folds into Preview as an inline subsection;
//! History is reachable via `git log`; Where-Used is a footer line
//! on Preview.

pub mod alternate_symbols;
pub mod datasheet_picker;
pub mod footprint;
pub mod inert;
mod measurement_cell;
pub mod params;
pub mod preview;
pub mod sim;
pub mod standalone;
pub mod supply;
pub mod symbol;
pub mod text_extent;

use iced::widget::{Space, button, column, container, row, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::theme_ext;

use super::messages::{EditorMsg, LibraryMessage};
use super::state::{ComponentPreviewState, EditorAddress, LibraryState, PreviewTab};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Render a Component Preview surface — five tabs (Preview / Parameters
/// / Supply / Datasheet / Simulation), all read-only for Symbol+
/// Footprint. The active tab body fills the panel; the header strip
/// and footer give the row's identity + save controls.
pub fn view<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    canvas_colors: &CanvasColors,
    device_px_per_logical_px: f32,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let read_only = library_state.is_read_only(&state.library_path);
    let header = view_header(state, tokens, address.clone());
    let tabs = view_tabs(state.active_tab, tokens, address.clone());
    let body = view_active_tab(
        state,
        library_state,
        tokens,
        canvas_colors,
        device_px_per_logical_px,
        address.clone(),
    );
    let footer = view_footer(state, tokens, address, read_only);

    column![header, tabs, body, footer]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_header<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let title = format!(
        "{}  —  class: {}  —  {:?}",
        state.row.internal_pn.as_str(),
        state.row.class.as_str(),
        state.row.state,
    );
    // The row id is an internal key; the internal PN in the title is the
    // name a user knows the part by.
    container(
        row![
            text("Component Preview — ").size(TS.title).color(muted),
            text(title).size(TS.title).color(text_c),
            Space::new().width(Length::Fill),
            close_btn(address, tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.m, 14.0])
    .style(crate::styles::modal_header_strip(tokens))
    .into()
}

fn view_tabs<'a>(
    active: PreviewTab,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    // The shared `TabPill`, so these tabs read like the document and dock
    // tabs: bordered pills, the active one filled with an accent line.
    let tab_fill = crate::styles::ti(tokens.hover);
    let border = crate::styles::ti(tokens.border);
    let accent = crate::styles::ti(tokens.accent);
    let last = PreviewTab::ORDER.len().saturating_sub(1);
    let mut row_widget = row![].spacing(0).align_y(iced::Alignment::End);
    for (i, tab) in PreviewTab::ORDER.iter().enumerate() {
        let is_active = *tab == active;
        let (fill, text_c) = if is_active {
            (tab_fill, theme_ext::text_primary(tokens))
        } else {
            (
                iced::Color {
                    a: tab_fill.a * 0.35,
                    ..tab_fill
                },
                theme_ext::text_secondary(tokens),
            )
        };
        let label = container(
            text(tab.label())
                .size(TS.body)
                .color(text_c)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .padding([SP.s, 10.0]);
        let style = signex_widgets::tab_pill::TabPillStyle {
            fill,
            border,
            accent,
            is_active,
            is_last: i == last,
            accent_position: signex_widgets::tab_pill::AccentPosition::Bottom,
        };
        let pill = signex_widgets::tab_pill::TabPill::new(label, style);
        let tab_msg = LibraryMessage::EditorEvent {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
            msg: EditorMsg::SelectTab(*tab),
        };
        row_widget = row_widget.push(iced::widget::mouse_area(pill).on_press(tab_msg));
    }
    container(row_widget)
        .padding(iced::Padding {
            top: SP.xs,
            right: SP.s,
            bottom: 0.0,
            left: SP.s,
        })
        .width(Length::Fill)
        .style(crate::styles::tab_bar_strip(tokens))
        .into()
}

fn view_active_tab<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    canvas_colors: &CanvasColors,
    device_px_per_logical_px: f32,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    // A read-only library (the stock library) shows every editing tab
    // dimmed and inert; the Preview tab guards its own editing parts
    // because its Open Editor buttons must stay live.
    let read_only = library_state.is_read_only(&state.library_path);
    let inner: Element<'_, LibraryMessage> = match state.active_tab {
        PreviewTab::Preview => preview::view(
            state,
            library_state,
            tokens,
            canvas_colors,
            device_px_per_logical_px,
            address,
        ),
        PreviewTab::Parameters => inert::unless_read_only(
            read_only,
            params::view(state, library_state, tokens, address),
        ),
        PreviewTab::Supply => {
            inert::unless_read_only(read_only, supply::view(state, tokens, address))
        }
        PreviewTab::Datasheet => {
            inert::unless_read_only(read_only, datasheet_picker::view(state, tokens, address))
        }
        PreviewTab::Simulation => {
            inert::unless_read_only(read_only, sim::view(state, tokens, address))
        }
    };
    container(inner)
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_footer<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);

    if read_only {
        return container(
            text("Read-only library - this component cannot be changed.")
                .size(TS.body)
                .color(muted),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(crate::styles::modal_footer_strip(tokens))
        .into();
    }

    // Themed like every other dialog's actions: accent for the primary,
    // outlined for the rest (`styles::buttons`), so they follow the theme.
    let primary = |label: &'static str, msg: EditorMsg, addr: &EditorAddress| {
        button(container(text(label).size(TS.body)).padding([SP.s, 14.0]))
            .on_press(LibraryMessage::EditorEvent {
                library_path: addr.library_path.clone(),
                table: addr.table.clone(),
                row_id: addr.row_id,
                msg,
            })
            .style(crate::styles::accent_button(tokens))
    };
    let secondary = |label: &'static str, msg: EditorMsg, addr: &EditorAddress| {
        button(container(text(label).size(TS.body)).padding([SP.s, 14.0]))
            .on_press(LibraryMessage::EditorEvent {
                library_path: addr.library_path.clone(),
                table: addr.table.clone(),
                row_id: addr.row_id,
                msg,
            })
            .style(crate::styles::outline_button(tokens))
    };
    let dirty_marker: Element<'_, LibraryMessage> = if state.dirty {
        text("• unsaved").size(TS.body).color(muted).into()
    } else {
        Space::new().width(0).into()
    };

    // Offered once a save was refused because the row changed on the
    // server; discarding the working copy asks first.
    let reload: Element<'_, LibraryMessage> = if !state.stale_base {
        Space::new().width(0).into()
    } else if state.reload_confirming {
        row![
            Space::new().width(12),
            text("Discard your edits and reload?")
                .size(TS.body)
                .color(muted),
            Space::new().width(8),
            primary("Reload", EditorMsg::ReloadConfirmed, &address),
            Space::new().width(8),
            secondary("Keep editing", EditorMsg::ReloadCancelled, &address),
        ]
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        row![
            Space::new().width(12),
            secondary("Reload saved version", EditorMsg::ReloadRequested, &address),
        ]
        .into()
    };

    container(
        row![
            secondary("Save", EditorMsg::SaveDraft, &address),
            Space::new().width(8),
            primary("Save & Close", EditorMsg::Commit, &address),
            reload,
            Space::new().width(Length::Fill),
            dirty_marker,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens))
    .into()
}

fn close_btn<'a>(address: EditorAddress, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    button(container(text("\u{00D7}".to_string()).size(TS.heading).color(text_c)).padding([0, 6]))
        .on_press(LibraryMessage::EditorEvent {
            library_path: address.library_path,
            table: address.table,
            row_id: address.row_id,
            msg: EditorMsg::CloseEditor,
        })
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10),
                )),
                _ => Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.03,
                ))),
            };
            iced::widget::button::Style {
                background: bg,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                text_color: text_c,
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

/// Helper for placeholder-card layouts — kept around for tabs that
/// still bottom out in TODO state during the Wave-3 refactor (sim-
/// without-binding, e.g.).
#[expect(
    dead_code,
    reason = "kept for tabs still bottoming out in TODO state during the Wave-3 refactor"
)]
pub(crate) fn placeholder_card<'a>(
    title: &'a str,
    todos: &'a [&'a str],
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let mut col = column![
        text(title).size(TS.heading).color(text_c),
        Space::new().height(8),
    ]
    .spacing(SP.s);
    for todo in todos {
        col = col.push(text(format!("• {todo}")).size(TS.body).color(muted));
    }
    container(col)
        .padding(14)
        .style(crate::styles::modal_card(tokens))
        .into()
}
