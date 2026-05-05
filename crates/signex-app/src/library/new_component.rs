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

use iced::widget::{Space, button, column, container, pick_list, row, svg, text, text_input};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_library::{ComponentClass, PrimitiveKind};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::theme_ext;

use super::messages::LibraryMessage;
use super::state::{LibraryState, NewComponentState, PrimitivePickerTarget};
use crate::app::view::dialogs::{
    MODAL_CLOSE_X_HIT_H, MODAL_CLOSE_X_HIT_W, MODAL_CLOSE_X_HOVER, MODAL_CLOSE_X_ICON,
    MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE,
};

const MODAL_W: f32 = 520.0;
// No fixed height: the modal sizes to content so the Advanced ▾
// disclosure (which adds a Table-picker row) can grow vertically
// instead of clipping the Cancel / Create Row footer behind the
// modal-card's `clip(true)`. The header / form / footer column is
// `Length::Shrink` for the same reason; rounded-corner clipping
// continues to work because it's on the outer card, not the inner
// column. Width stays fixed so the form's inputs keep a predictable
// extent regardless of content.

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
    theme_id: ThemeId,
    classes: Vec<crate::fonts::ComponentClassEntry>,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    // Canonical modal header — same constants and shape every other
    // modal in the app uses (Rename, Remove, Project Options, …) so
    // the chrome stays in lockstep across surfaces.
    let header = container(
        row![
            text("New Component")
                .size(MODAL_HEADER_TITLE_SIZE)
                .color(text_c),
            Space::new().width(Length::Fill),
            close_x(LibraryMessage::CloseNewComponent, theme_id),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding(MODAL_HEADER_PADDING)
    .height(MODAL_HEADER_HEIGHT)
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

    // Table picker — populated from the selected library. We pull the
    // *actual* tables that exist in the .snxlib (`list_tables`) plus
    // the manifest's `[[tables]]` overrides, deduplicated. When the
    // library has none yet, fall back to the mechanical-plural slot
    // derived from the chosen class so the user always sees a
    // destination.
    let table_picks: Vec<TablePick> = nc
        .library_idx
        .and_then(|i| state.open_libraries.get(i))
        .and_then(|lib| state.set.get(lib.library_id))
        .map(|adapter| {
            let mut names: Vec<String> = Vec::new();
            if let Ok(actual) = adapter.list_tables() {
                names.extend(actual);
            }
            for cfg in adapter.manifest().tables() {
                if !names.iter().any(|n| n == &cfg.name) {
                    names.push(cfg.name.clone());
                }
            }
            if names.is_empty() {
                names.push(adapter.manifest().table_for_class(nc.class.as_str()));
            }
            names.into_iter().map(|name| TablePick { name }).collect()
        })
        .unwrap_or_default();
    let selected_table_pick = nc
        .table
        .as_deref()
        .and_then(|t| table_picks.iter().find(|p| p.name == t).cloned());
    let library_picked = nc.library_idx.is_some();
    let table_picker: Element<'_, LibraryMessage> = if !library_picked {
        text("Pick a library first; tables come from the library file.")
            .size(11)
            .color(muted)
            .into()
    } else if let Some(draft) = nc.creating_table.as_ref() {
        // Inline create-table form — replaces the picker while the
        // user is naming a fresh table. Confirm dispatches
        // `NewComponentConfirmCreateTable` which writes a `[tables.<name>]`
        // block via the adapter and re-points `nc.table` at the new
        // entry.
        let name_input = text_input("new_table_name", &draft.name)
            .on_input(LibraryMessage::NewComponentSetNewTableName)
            .on_submit(LibraryMessage::NewComponentConfirmCreateTable)
            .padding(6)
            .size(12);
        let confirm_btn = button(container(text("Create").size(11).color(text_c)).padding([4, 12]))
            .on_press(LibraryMessage::NewComponentConfirmCreateTable)
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.18, 0.36, 0.58,
                ))),
                text_color: iced::Color::WHITE,
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            });
        let cancel_btn = button(container(text("Cancel").size(11).color(text_c)).padding([4, 12]))
            .on_press(LibraryMessage::NewComponentCancelCreateTable)
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
            });
        let mut form = column![
            row![
                name_input,
                Space::new().width(6),
                cancel_btn,
                Space::new().width(6),
                confirm_btn,
            ]
            .align_y(iced::Alignment::Center),
        ]
        .spacing(4);
        if let Some(err) = draft.error.as_ref() {
            form = form.push(
                text(err.clone())
                    .size(11)
                    .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
            );
        }
        form.into()
    } else {
        let picker = pick_list(
            table_picks.clone(),
            selected_table_pick,
            |pick: TablePick| LibraryMessage::NewComponentSetTable(pick.name),
        )
        .placeholder("Select table…")
        .padding(6)
        .text_size(12);
        let new_table_btn =
            button(container(text("+ New Table…").size(11).color(text_c)).padding([4, 10]))
                .on_press(LibraryMessage::NewComponentBeginCreateTable)
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
                });
        row![picker, Space::new().width(6), new_table_btn,]
            .align_y(iced::Alignment::Center)
            .into()
    };

    // Class picker ───────────────────────────────────────────
    // Source of truth is the user's `prefs.json::component_classes`
    // list (seeded from `fonts::default_component_classes` on a fresh
    // install). Edits land via Preferences ▸ Component Classes; this
    // view just renders whatever the panel-ctx currently mirrors.
    let class_picks: Vec<ClassPick> = classes
        .iter()
        .map(|entry| ClassPick {
            key: entry.key.clone(),
            label: entry.label.clone(),
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

    // Pick Symbol / Pick Footprint rows. Optional — modal can submit
    // with unbound refs.
    let symbol_label = match nc.symbol_ref.as_ref() {
        None => "Unbound (optional)".to_string(),
        Some(r) => {
            let s = r.uuid.simple().to_string();
            let short = if s.len() >= 8 { &s[..8] } else { s.as_str() };
            format!("symbol uuid {}…", short)
        }
    };
    let pick_symbol_btn =
        button(container(text("Pick Symbol…").size(11).color(text_c)).padding([4, 12]))
            .on_press(LibraryMessage::OpenPrimitivePicker {
                kind: PrimitiveKind::Symbol,
                target: PrimitivePickerTarget::NewComponentForm,
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
            });
    let symbol_row = row![
        text(symbol_label)
            .size(11)
            .color(text_c)
            .width(Length::Fill),
        pick_symbol_btn,
    ]
    .align_y(iced::Alignment::Center);

    let footprint_label = match nc.footprint_ref.as_ref() {
        None => "Unbound (optional)".to_string(),
        Some(r) => {
            let s = r.uuid.simple().to_string();
            let short = if s.len() >= 8 { &s[..8] } else { s.as_str() };
            format!("footprint uuid {}…", short)
        }
    };
    let pick_footprint_btn =
        button(container(text("Pick Footprint…").size(11).color(text_c)).padding([4, 12]))
            .on_press(LibraryMessage::OpenPrimitivePicker {
                kind: PrimitiveKind::Footprint,
                target: PrimitivePickerTarget::NewComponentForm,
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
            });
    let footprint_row = row![
        text(footprint_label)
            .size(11)
            .color(text_c)
            .width(Length::Fill),
        pick_footprint_btn,
    ]
    .align_y(iced::Alignment::Center);

    // Advanced disclosure — for first-time users, the Table picker
    // is just noise: it auto-resolves from Class via
    // `manifest.table_for_class(class)` at submit time. Power users
    // who need custom routing (multi-class TSV / non-default
    // filename) flip Advanced open and pick a destination.
    let advanced_label = if nc.advanced_open {
        "Advanced ▴ — hide table routing"
    } else {
        "Advanced ▾ — pick a custom destination table"
    };
    let advanced_toggle =
        button(container(text(advanced_label).size(11).color(muted)).padding([2, 0]))
            .on_press(LibraryMessage::NewComponentToggleAdvanced)
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04),
                    )),
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: muted,
                    border: Border {
                        width: 0.0,
                        radius: 0.0.into(),
                        ..Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            });

    let mut form = column![
        labelled("Internal PN", pn_input.into()),
        Space::new().height(8),
        labelled("Library", lib_picker),
        Space::new().height(8),
        labelled("Class", class_picker),
        Space::new().height(8),
    ];
    if nc.advanced_open {
        form = form.push(labelled("Table", table_picker));
        form = form.push(Space::new().height(8));
    }
    let form = form.push(advanced_toggle).push(Space::new().height(8));

    let form = column![
        form,
        labelled("Symbol ref", symbol_row.into()),
        Space::new().height(8),
        labelled("Footprint ref", footprint_row.into()),
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

    container(column![header, form, error_row, footer].width(Length::Fixed(MODAL_W)))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
}

/// Same SVG glyph + hover footprint the shared `close_x_button` uses
/// (`view::dialogs::close_x_button`), but generic over message type so
/// it composes into a `LibraryMessage` element. Matches the OS chrome
/// close: white glyph, full header-height hit-box, Windows-native red
/// hover. Kept local because the shared helper is `Message`-typed
/// only.
fn close_x<'a>(message: LibraryMessage, theme_id: ThemeId) -> Element<'a, LibraryMessage> {
    let handle = crate::icons::icon_chrome_window_close(theme_id);
    button(
        container(
            svg(handle)
                .width(MODAL_CLOSE_X_ICON)
                .height(MODAL_CLOSE_X_ICON)
                .style(move |_: &Theme, _| svg::Style {
                    color: Some(Color::WHITE),
                }),
        )
        .width(MODAL_CLOSE_X_HIT_W)
        .height(MODAL_CLOSE_X_HIT_H)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center),
    )
    .padding(0)
    .on_press(message)
    .style(move |_: &Theme, status: iced::widget::button::Status| {
        let hovered = matches!(
            status,
            iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
        );
        iced::widget::button::Style {
            background: if hovered {
                Some(Background::Color(MODAL_CLOSE_X_HOVER))
            } else {
                None
            },
            text_color: Color::WHITE,
            border: Border {
                radius: iced::border::Radius {
                    top_left: 0.0,
                    top_right: 4.0,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                },
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        }
    })
    .into()
}
