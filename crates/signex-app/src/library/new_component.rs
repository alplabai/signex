//! "New Component" modal — opened from File ▸ Library ▸ New
//! Component… and from the project tree's library-node right-click
//! menu.
//!
//! Components are rows in category tables (DBLib model). The modal
//! collects PN + library + table + class. On submit the dispatcher
//! calls `commands::create_component_row` which mints Symbol +
//! Footprint primitives, builds a `ComponentRow` with the binding
//! refs, and inserts it into the chosen table. Success fires
//! `LibraryMessage::OpenComponentRow` so the new row opens as a
//! Component Preview tab.
//!
//! Shape (plan §13):
//!
//! ```text
//! ┌─[ New Component ]──────────────────────────────┐
//! │ Internal PN [_______________________________]  │
//! │ Library     [▾ MyComponents              ]     │
//! │ Table       [▾ Resistors                 ]     │
//! │ Class       [▾ resistor                  ]     │
//! │             [ Cancel ]  [ Create Row ]         │
//! └────────────────────────────────────────────────┘
//! ```
//!
//! When the manifest declares no `[[tables]]` overrides we still
//! surface the table pick_list with a single "<class>s" placeholder
//! option so the user always sees the destination filename.

use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::ComponentClass;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::state::{BUILTIN_CLASSES, LibraryState, NewComponentState};

const MODAL_W: f32 = 520.0;
const MODAL_H: f32 = 420.0;

/// `pick_list` adapter for the library dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LibraryPick {
    idx: usize,
    label: String,
}

impl std::fmt::Display for LibraryPick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// `pick_list` adapter for the class dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClassPick {
    /// Canonical class string ("resistor", "opamp", …) — what gets
    /// stored on `ComponentRow::class`.
    key: String,
    /// Display label.
    label: String,
}

impl std::fmt::Display for ClassPick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// `pick_list` adapter for the table dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TablePick {
    /// Filename stem (no extension) — what gets stored on
    /// `NewComponentState.table`.
    name: String,
}

impl std::fmt::Display for TablePick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)
    }
}

pub fn view<'a>(
    state: &'a LibraryState,
    nc: &'a NewComponentState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = container(
        row![
            text("New Component").size(14).color(text_c),
            Space::new().width(Length::Fill),
            close_x(LibraryMessage::CloseNewComponent, tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_header_strip(tokens));

    // Internal PN ────────────────────────────────────────────
    let pn_input = text_input("e.g. R0805_10k", &nc.internal_pn)
        .on_input(LibraryMessage::NewComponentSetInternalPn)
        .padding(6)
        .size(12);

    // Library picker ─────────────────────────────────────────
    let lib_picks: Vec<LibraryPick> = state
        .open_libraries
        .iter()
        .enumerate()
        .map(|(i, lib)| LibraryPick {
            idx: i,
            label: lib.display_name.clone(),
        })
        .collect();
    let selected_pick = nc
        .library_idx
        .and_then(|i| lib_picks.iter().find(|p| p.idx == i).cloned());

    let lib_picker: Element<'_, LibraryMessage> = if lib_picks.is_empty() {
        text("No open libraries — open one via File ▸ Library ▸ Open Library… first.")
            .size(11)
            .color(muted)
            .into()
    } else {
        pick_list(lib_picks.clone(), selected_pick, |pick: LibraryPick| {
            LibraryMessage::NewComponentSetLibrary(pick.idx)
        })
        .placeholder("Select library…")
        .padding(6)
        .text_size(12)
        .into()
    };

    // Table picker — populated from the selected library's
    // manifest. When the manifest declares no `[[tables]]` overrides
    // we still surface a single placeholder row carrying the
    // default-pluralised filename (`<class>s`) so the user always
    // sees the destination.
    let table_picks: Vec<TablePick> = nc
        .library_idx
        .and_then(|i| state.open_libraries.get(i))
        .and_then(|lib| state.set.get(lib.library_id))
        .map(|adapter| {
            let configured = adapter.manifest().tables();
            if configured.is_empty() {
                vec![TablePick {
                    name: adapter.manifest().table_for_class(nc.class.as_str()),
                }]
            } else {
                configured
                    .iter()
                    .map(|cfg| TablePick {
                        name: cfg.name.clone(),
                    })
                    .collect()
            }
        })
        .unwrap_or_default();
    let selected_table_pick = nc
        .table
        .as_deref()
        .and_then(|t| table_picks.iter().find(|p| p.name == t).cloned());
    let table_picker: Element<'_, LibraryMessage> = if table_picks.is_empty() {
        text("Pick a library first; tables come from the library manifest.")
            .size(11)
            .color(muted)
            .into()
    } else {
        pick_list(
            table_picks.clone(),
            selected_table_pick,
            |pick: TablePick| LibraryMessage::NewComponentSetTable(pick.name),
        )
        .placeholder("Select table…")
        .padding(6)
        .text_size(12)
        .into()
    };

    // Class picker ───────────────────────────────────────────
    let class_picks: Vec<ClassPick> = BUILTIN_CLASSES
        .iter()
        .map(|(key, label)| ClassPick {
            key: (*key).to_string(),
            label: (*label).to_string(),
        })
        .collect();
    let selected_class_pick = class_picks
        .iter()
        .find(|p| p.key == nc.class.as_str())
        .cloned();
    let class_picker: Element<'_, LibraryMessage> = pick_list(
        class_picks.clone(),
        selected_class_pick,
        |pick: ClassPick| LibraryMessage::NewComponentSetClass(ComponentClass::new(pick.key)),
    )
    .placeholder("Select class…")
    .padding(6)
    .text_size(12)
    .into();

    // Form layout ─────────────────────────────────────────────
    let labelled =
        |lbl: &'a str, body: Element<'a, LibraryMessage>| -> Element<'a, LibraryMessage> {
            column![
                text(lbl).size(11).color(muted),
                container(body).padding([2, 0])
            ]
            .spacing(4)
            .into()
        };

    let form = column![
        labelled("Internal PN", pn_input.into()),
        Space::new().height(8),
        labelled("Library", lib_picker),
        Space::new().height(8),
        labelled("Table", table_picker),
        Space::new().height(8),
        labelled("Class", class_picker),
    ]
    .spacing(0)
    .padding([16, 16]);

    let error_row: Element<'_, LibraryMessage> = if let Some(err) = nc.error.as_ref() {
        container(
            text(err.clone())
                .size(11)
                .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
        )
        .padding([0, 16])
        .into()
    } else {
        Space::new().height(0).into()
    };

    let submit_enabled = !nc.internal_pn.trim().is_empty() && nc.library_idx.is_some();
    let submit_bg = if submit_enabled {
        iced::Color::from_rgb(0.00, 0.47, 0.84)
    } else {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04)
    };
    let submit_fg = if submit_enabled {
        iced::Color::WHITE
    } else {
        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.4)
    };
    let mut submit_btn = button(
        container(text("Create Row").size(11).color(submit_fg)).padding([4, 14]),
    )
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(submit_bg)),
        text_color: submit_fg,
        border: Border {
            width: 0.0,
            radius: 3.0.into(),
            ..Border::default()
        },
        ..iced::widget::button::Style::default()
    });
    if submit_enabled {
        submit_btn = submit_btn.on_press(LibraryMessage::NewComponentSubmit);
    }

    let footer = container(
        row![
            Space::new().width(Length::Fill),
            button(container(text("Cancel").size(11).color(text_c)).padding([4, 14]))
                .on_press(LibraryMessage::CloseNewComponent)
                .style(move |_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04
                    ))),
                    text_color: text_c,
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border,
                    },
                    ..iced::widget::button::Style::default()
                }),
            Space::new().width(8),
            submit_btn,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .style(crate::styles::modal_footer_strip(tokens));

    container(
        column![
            header,
            form,
            error_row,
            Space::new().height(Length::Fill),
            footer
        ]
        .width(Length::Fixed(MODAL_W))
        .height(Length::Fixed(MODAL_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .into()
}

fn close_x<'a>(message: LibraryMessage, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    button(container(text("\u{00D7}".to_string()).size(14).color(text_c)).padding([0, 6]))
        .on_press(message)
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
