//! Library Browser — table/class master sidebar.
//!
//! The vertical master pane: one row per table, active-table highlight,
//! inline rename / delete / add-table forms, plus the (F20-gated)
//! Classes section. Extracted verbatim from the former single-file
//! `browser` module.

use super::*;
use iced::widget::column;

/// Vertical table sidebar — replaces the old horizontal tab strip
/// with a database-style master pane: each table is one row, the
/// active one highlights, and `+ Table` (plus the inline create
/// form) anchors at the bottom. Per-tab × delete still ships with
/// the next iteration; for now an empty table is selectable and the
/// user can drop rows individually before deletion lands.
pub(super) fn view_table_sidebar<'a>(
    library_path: &'a std::path::Path,
    lib: &'a OpenLibrary,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    const SIDEBAR_W: f32 = 200.0;
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let active_bg = iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10);

    let mut col = column![]
        .spacing(2)
        .padding(iced::Padding {
            top: 6.0,
            right: 0.0,
            bottom: 6.0,
            left: 0.0,
        })
        .width(Length::Fill);

    col = col.push(
        container(text("Classes").size(11).color(muted)).padding(iced::Padding {
            top: 0.0,
            right: 8.0,
            bottom: 4.0,
            left: 12.0,
        }),
    );

    // Hoist a single owned `PathBuf` out of every per-row loop.
    // Each table / class row's message constructors (`on_input` /
    // `on_press`) need an owned path inside their closure; calling
    // `library_path.to_path_buf()` per row burned 60+ heap allocs
    // per frame on a 20-table library. With this hoist the alloc
    // count is one per closure, sourced from the same `lib_pb`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    let mut names: Vec<&String> = lib.tables.keys().collect();
    names.sort();
    // Surface the most recent delete-table error at the top of the
    // list so the user can see why a `×` click bounced (typically
    // "table is not empty (N rows)").
    if let Some(err) = browser.delete_error.as_ref() {
        let library_for_dismiss = lib_pb.clone();
        let dismiss = button(text("×").size(BROWSER_TEXT_SIZE).color(iced::Color::WHITE))
            .padding([0, 6])
            .on_press(LibraryMessage::BrowserDismissDeleteError {
                library_path: library_for_dismiss,
            })
            .style(|_: &Theme, _| iced::widget::button::Style {
                background: None,
                text_color: iced::Color::WHITE,
                border: Border::default(),
                ..iced::widget::button::Style::default()
            });
        let banner = container(
            row![
                text(err.clone())
                    .size(BROWSER_TEXT_SIZE)
                    .color(iced::Color::WHITE)
                    .width(Length::Fill),
                dismiss,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([4, 8])
        .style(|_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.55, 0.18, 0.18,
            ))),
            border: Border {
                radius: 3.0.into(),
                ..Border::default()
            },
            ..iced::widget::container::Style::default()
        });
        col = col.push(container(banner).padding([2, 8]));
    }

    for name in &names {
        let is_active = browser.active_table.as_deref() == Some(name.as_str());
        let count = lib.tables.get(*name).map(|v| v.len()).unwrap_or(0);
        let is_renaming = browser
            .renaming_table
            .as_ref()
            .is_some_and(|(orig, _)| orig.as_str() == name.as_str());

        // Rename mode: replace the whole row with a text input +
        // confirm/cancel buttons. Submit on Enter; Esc closes from
        // the Library Browser tab's escape handler (TODO).
        if is_renaming {
            let buffer = browser
                .renaming_table
                .as_ref()
                .map(|(_, b)| b.as_str())
                .unwrap_or("");
            let library_for_input = lib_pb.clone();
            let library_for_confirm = lib_pb.clone();
            let library_for_cancel = lib_pb.clone();
            let name_input = text_input("table_name", buffer)
                .on_input(move |s| LibraryMessage::BrowserSetRenameName {
                    library_path: library_for_input.clone(),
                    value: s,
                })
                .on_submit(LibraryMessage::BrowserConfirmRenameTable {
                    library_path: library_for_confirm.clone(),
                })
                .padding(4)
                .size(BROWSER_TEXT_SIZE);
            let confirm = button(text("✓").size(BROWSER_TEXT_SIZE).color(iced::Color::WHITE))
                .padding([4, 8])
                .on_press(LibraryMessage::BrowserConfirmRenameTable {
                    library_path: library_for_confirm,
                })
                .style(|_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgb(
                        0.18, 0.36, 0.58,
                    ))),
                    text_color: iced::Color::WHITE,
                    border: Border {
                        width: 0.0,
                        radius: 2.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                });
            let cancel = button(text("×").size(BROWSER_TEXT_SIZE).color(text_c))
                .padding([4, 8])
                .on_press(LibraryMessage::BrowserCancelRenameTable {
                    library_path: library_for_cancel,
                })
                .style(|_: &Theme, _| iced::widget::button::Style {
                    background: None,
                    text_color: iced::Color::WHITE,
                    border: Border::default(),
                    ..iced::widget::button::Style::default()
                });
            let mut form = column![
                row![
                    name_input,
                    Space::new().width(4),
                    cancel,
                    Space::new().width(2),
                    confirm,
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(2)
            .padding([4, 8]);
            if let Some(err) = browser.rename_error.as_ref() {
                form = form.push(
                    text(err.clone())
                        .size(BROWSER_TEXT_SIZE)
                        .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
                );
            }
            col = col.push(form);
            continue;
        }

        let row_label = row![
            text((*name).clone())
                .size(BROWSER_TEXT_SIZE)
                .color(text_c)
                .width(Length::Fill),
            text(format!("{count}"))
                .size(BROWSER_TEXT_SIZE)
                .color(muted),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        let library_owned = lib_pb.clone();
        let table_owned = (*name).clone();
        let on_press = LibraryMessage::BrowserSelectTable {
            library_path: library_owned,
            table: table_owned.clone(),
        };
        let bg_color = if is_active { Some(active_bg) } else { None };
        let row_btn = button(row_label)
            .padding(iced::Padding {
                top: 5.0,
                right: 6.0,
                bottom: 5.0,
                left: 12.0,
            })
            .width(Length::Fill)
            .on_press(on_press)
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let bg = bg_color.or_else(|| match status {
                    iced::widget::button::Status::Hovered => {
                        Some(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.04))
                    }
                    _ => None,
                });
                iced::widget::button::Style {
                    background: bg.map(iced::Background::Color),
                    text_color: text_c,
                    border: Border {
                        width: 0.0,
                        radius: 0.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                }
            });

        // × delete button — sibling to the select button so click
        // routing isn't ambiguous (nested buttons confuse iced's
        // hit testing). Adapter refuses non-empty deletes; the
        // resulting Conflict error surfaces in the banner above.
        let library_for_del = lib_pb.clone();
        let table_for_del = (*name).clone();
        let delete_btn = button(text("×").size(BROWSER_TEXT_SIZE + 1.0).color(muted))
            .padding([3, 8])
            .on_press_maybe((!read_only).then(|| LibraryMessage::BrowserDeleteTable {
                library_path: library_for_del,
                table: table_for_del,
            }))
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let (bg, fg) = match status {
                    iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed => (
                        Some(iced::Background::Color(iced::Color::from_rgba(
                            0.78, 0.22, 0.22, 1.0,
                        ))),
                        iced::Color::WHITE,
                    ),
                    _ => (None, muted),
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: fg,
                    border: Border {
                        width: 0.0,
                        radius: 2.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                }
            });

        // ✎ rename trigger — sibling to × so click routing stays
        // unambiguous. Switches the row into the inline rename
        // form above.
        let library_for_rename = lib_pb.clone();
        let table_for_rename = (*name).clone();
        let rename_btn = button(text("\u{270E}").size(BROWSER_TEXT_SIZE).color(muted))
            .padding([3, 6])
            .on_press_maybe(
                (!read_only).then(|| LibraryMessage::BrowserBeginRenameTable {
                    library_path: library_for_rename,
                    table: table_for_rename,
                }),
            )
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let (bg, fg) = match status {
                    iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed => (
                        Some(iced::Background::Color(iced::Color::from_rgba(
                            1.0, 1.0, 1.0, 0.10,
                        ))),
                        iced::Color::WHITE,
                    ),
                    _ => (None, muted),
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: fg,
                    border: Border {
                        width: 0.0,
                        radius: 2.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                }
            });

        let row_with_actions = row![row_btn, rename_btn, delete_btn,]
            .align_y(iced::Alignment::Center)
            .width(Length::Fill);
        col = col.push(row_with_actions);
    }

    // Inline `+ Table` form / button — same lifecycle as before, now
    // anchored at the bottom of the sidebar.
    let bottom_section: Element<'a, LibraryMessage> = match browser.adding_table.as_ref() {
        Some(draft) => {
            let library_for_input = lib_pb.clone();
            let library_for_confirm = lib_pb.clone();
            let library_for_cancel = lib_pb.clone();
            let name_input = text_input("new_table_name", &draft.name)
                .on_input(move |s| LibraryMessage::BrowserSetNewTableName {
                    library_path: library_for_input.clone(),
                    value: s,
                })
                .on_submit(LibraryMessage::BrowserConfirmAddTable {
                    library_path: library_for_confirm.clone(),
                })
                .padding(4)
                .size(BROWSER_TEXT_SIZE);
            let confirm = button(
                text("Create")
                    .size(BROWSER_TEXT_SIZE)
                    .color(iced::Color::WHITE),
            )
            .padding([4, 10])
            .on_press(LibraryMessage::BrowserConfirmAddTable {
                library_path: library_for_confirm,
            })
            .style(|_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.18, 0.36, 0.58,
                ))),
                text_color: iced::Color::WHITE,
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    color: iced::Color::TRANSPARENT,
                },
                ..iced::widget::button::Style::default()
            });
            let cancel = button(text("Cancel").size(BROWSER_TEXT_SIZE).color(text_c))
                .padding([4, 10])
                .on_press(LibraryMessage::BrowserCancelAddTable {
                    library_path: library_for_cancel,
                })
                .style(|_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04,
                    ))),
                    text_color: iced::Color::WHITE,
                    border: Border {
                        width: 0.0,
                        radius: 3.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                });
            let mut form = column![
                name_input,
                row![cancel, Space::new().width(4), confirm,].align_y(iced::Alignment::Center),
            ]
            .spacing(4)
            .padding([6, 12]);
            if let Some(err) = draft.error.as_ref() {
                form = form.push(
                    text(err.clone())
                        .size(BROWSER_TEXT_SIZE)
                        .color(iced::Color::from_rgb(0.85, 0.3, 0.3)),
                );
            }
            form.into()
        }
        None => {
            let library_for_begin = lib_pb.clone();
            container(
                button(text("+ Class").size(BROWSER_TEXT_SIZE).color(if read_only {
                    muted
                } else {
                    text_c
                }))
                .padding([4, 10])
                .width(Length::Fill)
                .on_press_maybe((!read_only).then(|| LibraryMessage::BrowserBeginAddTable {
                    library_path: library_for_begin,
                }))
                .style(|_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.04,
                    ))),
                    text_color: iced::Color::WHITE,
                    border: Border {
                        width: 0.0,
                        radius: 3.0.into(),
                        color: iced::Color::TRANSPARENT,
                    },
                    ..iced::widget::button::Style::default()
                }),
            )
            .padding(iced::Padding {
                top: 4.0,
                right: 8.0,
                bottom: 6.0,
                left: 8.0,
            })
            .into()
        }
    };

    col = col.push(Space::new().height(Length::Fill));
    col = col.push(bottom_section);

    container(col)
        .width(Length::Fixed(SIDEBAR_W))
        .height(Length::Fill)
        .into()
}
