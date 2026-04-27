//! Component Preview tab — read-only Symbol/Footprint render plus
//! template-validated forms for parameters / supply / datasheet /
//! simulation.
//!
//! Per `v0.9-refactor-2-plan.md` §11, the Component view is preview-
//! only. Symbol and Footprint editing happens via standalone
//! `.snxsym` / `.snxfpt` document tabs (WS-7); right-click on either
//! render in the Preview tab fires
//! [`crate::library::messages::LibraryMessage::OpenPrimitiveEditor`]
//! to open the standalone editor.
//!
//! Tab count is 5: Preview / Parameters / Supply / Datasheet /
//! Simulation. The History / Where-Used / Pin Map / Symbol / Footprint
//! tabs from the original v0.9 layout are dropped — Pin Map folds
//! into Preview as an inline subsection, History is reachable via
//! `git log`, Where-Used is a footer line on Preview.

pub mod datasheet_picker;
pub mod params;
pub mod preview;
pub mod sim;
pub mod supply;

// `editor/symbol/` and `editor/footprint/` STAY on disk for WS-7 to
// pick up as standalone document editors. Per plan §11 step 6.8, the
// Component context drops its references to them — we keep the file
// tree but stop declaring the modules from this `editor/mod.rs`. No
// `pub mod symbol;` / `pub mod footprint;` lines here.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::{EditorMsg, LibraryMessage};
use super::state::{ComponentPreviewState, EditorAddress, LibraryState, PreviewTab};

/// Render a Component Preview surface — five tabs (Preview / Parameters
/// / Supply / Datasheet / Simulation), all read-only for Symbol+
/// Footprint. The active tab body fills the panel; the header strip
/// and footer give the row's identity + save controls.
pub fn view<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let header = view_header(state, tokens, address.clone());
    let tabs = view_tabs(state.active_tab, tokens, address.clone());
    let body = view_active_tab(state, library_state, tokens, address.clone());
    let footer = view_footer(state, tokens, address);

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
    let row_id_label = format!("row: {}", state.row.row_id);
    container(
        row![
            text("Component Preview — ").size(13).color(muted),
            text(title).size(13).color(text_c),
            Space::new().width(Length::Fill),
            text(row_id_label).size(11).color(muted),
            Space::new().width(8),
            close_btn(address, tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([8, 14])
    .style(crate::styles::modal_header_strip(tokens))
    .into()
}

fn view_tabs<'a>(
    active: PreviewTab,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let mut row_widget = row![].spacing(0).align_y(iced::Alignment::Center);
    for tab in PreviewTab::ORDER {
        let is_active = *tab == active;
        let bg_color = if is_active {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
        } else {
            iced::Color::TRANSPARENT
        };
        let label = container(text(tab.label()).size(11).color(text_c)).padding([3, 10]);
        let address_for_msg = address.clone();
        let tab_for_msg = *tab;
        let inner_btn = button(label)
            .padding(0)
            .on_press_with(move || LibraryMessage::EditorEvent {
                library_path: address_for_msg.library_path.clone(),
                table: address_for_msg.table.clone(),
                row_id: address_for_msg.row_id,
                msg: EditorMsg::SelectTab(tab_for_msg),
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(bg_color)),
                text_color: text_c,
                border: Border::default(),
                ..iced::widget::button::Style::default()
            });
        row_widget = row_widget.push(inner_btn);
    }
    container(row_widget)
        .padding([2, 4])
        .width(Length::Fill)
        .style(crate::styles::tab_bar_strip(tokens))
        .into()
}

fn view_active_tab<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let inner: Element<'_, LibraryMessage> = match state.active_tab {
        PreviewTab::Preview => preview::view(state, library_state, tokens, address),
        PreviewTab::Parameters => params::view(state, library_state, tokens, address),
        PreviewTab::Supply => supply::view(state, tokens, address),
        PreviewTab::Datasheet => datasheet_picker::view(state, tokens, address),
        PreviewTab::Simulation => sim::view(state, tokens, address),
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
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let primary = |label: &'static str, msg: EditorMsg, addr: &EditorAddress| {
        button(container(text(label).size(11).color(iced::Color::WHITE)).padding([4, 14]))
            .on_press(LibraryMessage::EditorEvent {
                library_path: addr.library_path.clone(),
                table: addr.table.clone(),
                row_id: addr.row_id,
                msg,
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.00, 0.47, 0.84,
                ))),
                text_color: iced::Color::WHITE,
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            })
    };
    let secondary = |label: &'static str, msg: EditorMsg, addr: &EditorAddress| {
        button(container(text(label).size(11).color(text_c)).padding([4, 14]))
            .on_press(LibraryMessage::EditorEvent {
                library_path: addr.library_path.clone(),
                table: addr.table.clone(),
                row_id: addr.row_id,
                msg,
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            })
    };

    let dirty_marker: Element<'_, LibraryMessage> = if state.dirty {
        text("• unsaved").size(11).color(muted).into()
    } else {
        Space::new().width(0).into()
    };

    container(
        row![
            secondary("Save", EditorMsg::SaveDraft, &address),
            Space::new().width(8),
            primary("Save & Close", EditorMsg::Commit, &address),
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
    button(container(text("\u{00D7}".to_string()).size(14).color(text_c)).padding([0, 6]))
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
#[allow(dead_code)]
pub(crate) fn placeholder_card<'a>(
    title: &'a str,
    todos: &'a [&'a str],
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let mut col = column![text(title).size(14).color(text_c), Space::new().height(8),].spacing(4);
    for todo in todos {
        col = col.push(text(format!("• {todo}")).size(11).color(muted));
    }
    container(col)
        .padding(14)
        .style(crate::styles::modal_card(tokens))
        .into()
}
