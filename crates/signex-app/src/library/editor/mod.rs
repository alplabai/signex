//! Component Editor — multi-window tabbed surface, one window per
//! open component. Each window's state lives in
//! `LibraryState.open_editors` keyed by `iced::window::Id`.
//!
//! WS-E (refactor): the editor surface is **trimmed back to Overview +
//! History + Where-Used + Submit-for-Review** while the rest of the
//! tabs are rewritten against the new primitives shape:
//!
//! - **WS-F** rewires Symbol / Footprint (and adds 3D body editing
//!   inside the Footprint tab).
//! - **WS-G** adds the new Pin Map tab.
//! - **WS-?** restores Sim against the new `SimModel` primitive.
//!
//! All non-trivial tabs render `placeholder_card` until those waves
//! land. The footer + tabs router still works so the user can move
//! around the editor without breakage.

pub mod footprint;
pub mod history;
pub mod overview;
// WS-J: Params tab
pub mod params;
pub mod pin_map;
// WS-L: Sim tab
pub mod sim;
pub mod submit_for_review;
pub mod supply; // WS-K: Supply tab
pub mod symbol;
pub mod where_used;

use iced::widget::{Space, button, column, container, row, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::{EditorMsg, LibraryMessage};
use super::state::{ComponentEditorState, EditorAddress, EditorTab, LibraryState};

// WS-I: tab-not-window
/// Render a Component Editor surface — same widget tree whether the
/// editor is hosted inline as a tab in the main window or detached
/// into its own window via the existing tab-undock flow. Editor
/// state is addressed by `(library_path, component_id)`; the address
/// is cloned into each sub-view by value so closures capture owned
/// copies rather than borrowing back into the local stack.
pub fn view<'a>(
    editor: &'a ComponentEditorState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let header = view_header(editor, tokens, address.clone());
    let tabs = view_tabs(editor.active_tab, tokens, address.clone());
    let body = view_active_tab(editor, library_state, tokens, address.clone());
    let footer = view_footer(editor, tokens, address.clone());

    let main: Element<'_, LibraryMessage> = column![header, tabs, body, footer]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

    if editor.review_dialog_open {
        let modal_card = submit_for_review::view(editor, tokens, address.clone());
        let backdrop: Element<'_, LibraryMessage> = container(modal_card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(|_: &Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.45,
                ))),
                ..Default::default()
            })
            .into();
        iced::widget::stack![main, backdrop].into()
    } else {
        main
    }
}

fn view_header<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let title = format!(
        "{}  v{}  —  {:?}",
        editor.display_internal_pn, editor.displayed_version, editor.draft.state,
    );
    container(
        row![
            text("Component Editor — ").size(13).color(muted),
            text(title).size(13).color(text_c),
            Space::new().width(Length::Fill),
            text("\u{1F512}").size(13).color(muted),
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
    active: EditorTab,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let mut row_widget = row![].spacing(0).align_y(iced::Alignment::Center);
    for tab in EditorTab::ORDER {
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
                component_id: address_for_msg.component_id,
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
    editor: &'a ComponentEditorState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let inner: Element<'_, LibraryMessage> = match editor.active_tab {
        EditorTab::Overview => overview::view(editor, tokens, address),
        EditorTab::Symbol => symbol::view(editor, tokens, address.clone()),
        EditorTab::Footprint => footprint::view(editor, tokens, address.clone()),
        EditorTab::PinMap => pin_map::view(
            editor,
            editor.symbol.as_ref().zip(editor.footprint.as_ref()),
            tokens,
            address,
        ),
        // WS-J: Params tab
        EditorTab::Params => params::view(editor, library_state, tokens, address.clone()),
        EditorTab::Supply => placeholder_card(
            "Supply",
            &[
                "Primary MPN + alternates editor — WS-F polishes the multi-row picker.",
                "Distributor listings live on `Revision::supply`.",
            ],
            tokens,
        ),
        // WS-L: Sim tab — replaces the placeholder with the real
        // SPICE deck editor + pin/node mapping table backed by the
        // typed `SimModel` primitive bound through `Revision::sim_ref`.
        EditorTab::Sim => sim::view(editor, tokens, address.clone()),
        EditorTab::History => history::view(editor, tokens, address),
        EditorTab::WhereUsed => where_used::view(editor, library_state, tokens),
    };
    container(inner)
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_footer<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);

    let primary = |label: &'static str, msg: EditorMsg, addr: &EditorAddress| {
        button(container(text(label).size(11).color(iced::Color::WHITE)).padding([4, 14]))
            .on_press(LibraryMessage::EditorEvent {
                library_path: addr.library_path.clone(),
                component_id: addr.component_id,
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
                component_id: addr.component_id,
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

    let mut footer_row = row![
        secondary("Save Draft", EditorMsg::SaveDraft, &address),
        Space::new().width(8),
        primary("Commit", EditorMsg::Commit, &address),
    ]
    .align_y(iced::Alignment::Center);
    if editor.review_required {
        footer_row = footer_row.push(Space::new().width(8));
        footer_row = footer_row.push(secondary(
            "Submit for Review",
            EditorMsg::SubmitForReview,
            &address,
        ));
    }
    footer_row = footer_row.push(Space::new().width(Length::Fill));
    footer_row = footer_row.push(secondary(
        "Where Used",
        EditorMsg::OpenWhereUsedTab,
        &address,
    ));

    container(footer_row)
        .padding([10, 14])
        .style(crate::styles::modal_footer_strip(tokens))
        .into()
}

fn close_btn<'a>(
    address: EditorAddress,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    button(container(text("\u{00D7}".to_string()).size(14).color(text_c)).padding([0, 6]))
        .on_press(LibraryMessage::EditorEvent {
            library_path: address.library_path,
            component_id: address.component_id,
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

/// Helper used by every placeholder tab — shared message and TODO list
/// pulled from the v0.9 refactor plan.
pub(crate) fn placeholder_card<'a>(
    title: &'a str,
    todos: &'a [&'a str],
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let mut col = column![
        text(format!("{title} — Coming in WS-F / WS-G"))
            .size(14)
            .color(text_c),
        Space::new().height(8),
    ]
    .spacing(4);
    for todo in todos {
        col = col.push(text(format!("• {todo}")).size(11).color(muted));
    }
    container(col)
        .padding(14)
        .style(crate::styles::modal_card(tokens))
        .into()
}
