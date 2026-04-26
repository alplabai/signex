//! Library left-dock panel.
//!
//! Shape:
//!
//! ```text
//! ┌───────────────────────────┐
//! │ [Search components…]      │
//! ├───────────────────────────┤
//! │ ▼ MyComponents (12)       │  ← library node (toggleable)
//! │   • R0805_10k             │
//! │   • C0805_100n            │
//! │   …                       │
//! │ ▶ AlpLab Lib              │
//! │ ─────────────────────────  │
//! │ [+ Open Library…]          │
//! └───────────────────────────┘
//! ```
//!
//! No drag-and-drop yet — Phase 2 lights up `OpenEditor` on
//! double-click and adds drag-into-canvas placement. Today the panel
//! is a read surface plus a "double-click → editor" affordance.

use iced::widget::{Column, button, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::state::LibraryState;

const LIB_PANEL_TEXT_SIZE: f32 = 11.0;
const LIB_PANEL_HEADER_SIZE: f32 = 10.0;
const LIB_PANEL_ROW_PADDING: u16 = 4;

/// Render the Library left-dock panel.
pub fn view<'a>(state: &'a LibraryState, tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let search = text_input("Search components…", &state.panel_search)
        .on_input(|s| {
            // Phase 1 shares the picker filter buffer because the
            // picker modal is the only consumer that actually
            // narrows. Wiring the panel's own filter view lands in
            // Phase 2.
            LibraryMessage::Picker(super::messages::PickerMsg::FilterChanged(s))
        })
        .padding(LIB_PANEL_ROW_PADDING)
        .size(LIB_PANEL_TEXT_SIZE);

    let mut col: Column<'a, LibraryMessage> = column![]
        .spacing(2)
        .width(Length::Fill)
        .push(container(search).padding([4, 4]));

    if state.open_libraries.is_empty() {
        col = col.push(
            container(
                text("No libraries open. Use File ▸ Library ▸ Open Library… to open a *.snxlib/.")
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(muted),
            )
            .padding([12, 8]),
        );
    } else {
        for (idx, lib) in state.open_libraries.iter().enumerate() {
            let expanded = state.expanded.get(idx).copied().unwrap_or(true);
            let chevron = if expanded { "▼" } else { "▶" };
            let header = button(
                row![
                    text(format!(
                        "{}  {}  ({})",
                        chevron,
                        lib.display_name,
                        lib.cached_components.len()
                    ))
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(text_c),
                ]
                .padding([2, 4]),
            )
            .padding(0)
            .width(Length::Fill)
            .on_press(LibraryMessage::ToggleLibraryTreeNode(idx))
            .style(crate::styles::menu_item(tokens));
            col = col.push(header);

            if expanded {
                if lib.cached_components.is_empty() {
                    col = col.push(
                        container(text("(empty)").size(LIB_PANEL_HEADER_SIZE).color(muted))
                            .padding([2, 18]),
                    );
                } else {
                    for c in &lib.cached_components {
                        // Each row opens an editor on click. Phase 2
                        // upgrades to "double-click opens editor; single
                        // click previews".
                        let label = format!("• {}  ({})", c.internal_pn.as_str(), c.head);
                        let row_btn = button(
                            row![text(label).size(LIB_PANEL_TEXT_SIZE).color(text_c)]
                                .padding([1, 4]),
                        )
                        .padding(0)
                        .width(Length::Fill)
                        .on_press(LibraryMessage::OpenEditor {
                            library_path: lib.root.clone(),
                            component_id: c.uuid,
                        })
                        .style(crate::styles::menu_item(tokens));
                        col = col.push(container(row_btn).padding([0, 12]));
                    }
                }
            }
        }
    }

    let footer = container(
        row![
            button(
                text("+ Open Library…")
                    .size(LIB_PANEL_TEXT_SIZE)
                    .color(text_c)
            )
            .padding([4, 8])
            .on_press(LibraryMessage::OpenLibraryDialog)
            .style(crate::styles::menu_item(tokens)),
        ]
        .spacing(4),
    )
    .padding([6, 4]);

    let body = container(col).style(move |_: &Theme| iced::widget::container::Style {
        background: None,
        border: Border {
            color: border,
            width: 0.0,
            ..Border::default()
        },
        ..Default::default()
    });

    column![
        scrollable(body).width(Length::Fill).height(Length::Fill),
        footer
    ]
    .spacing(0)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
