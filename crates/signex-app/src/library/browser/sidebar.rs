//! Library Browser — table master sidebar.
//!
//! The vertical master pane: one row per table, active-table highlight,
//! inline rename / delete / add-table forms, plus the (F20-gated)
//! Classes section. Extracted verbatim from the former single-file
//! `browser` module.

use super::look;
use super::*;
use iced::widget::column;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of the table sidebar.
pub(super) const SIDEBAR_W: f32 = 200.0;

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
    i18n: &I18n,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let active_bg = look::active_fill(tokens);
    let hover_bg = look::row_hover_fill(tokens);
    let wash_ink = look::ink_over_wash(tokens);
    let control_bg = look::control_fill(tokens);

    let mut col: Column<Element<'a, LibraryMessage>> = column![]
        .spacing(SP.xs)
        .padding(iced::Padding {
            top: 6.0,
            right: 0.0,
            bottom: 6.0,
            left: 0.0,
        })
        .width(Length::Fill);

    col = col.push(
        container(
            text(t!(i18n, ids::LIBRARY_BROWSER_TABLES))
                .size(TS.section)
                .style(crate::styles::section_label(tokens)),
        )
        .padding(iced::Padding {
            top: 0.0,
            right: 8.0,
            bottom: 4.0,
            left: 12.0,
        })
        .boxed(),
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
        let dismiss = button(
            text(raw("×"))
                .size(BROWSER_TEXT_SIZE)
                .color(iced::Color::WHITE),
        )
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
                text(raw(err.clone()))
                    .size(BROWSER_TEXT_SIZE)
                    .color(iced::Color::WHITE)
                    .width(Length::Fill),
                dismiss,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.s, SP.m])
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
        col = col.push(container(banner).padding([SP.xs, SP.m]).boxed());
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
        // the Library Browser tab's escape handler.
        if is_renaming {
            let buffer = browser
                .renaming_table
                .as_ref()
                .map(|(_, b)| b.as_str())
                .unwrap_or("");
            let library_for_input = lib_pb.clone();
            let library_for_confirm = lib_pb.clone();
            let library_for_cancel = lib_pb.clone();
            let name_input = text_input(raw("table_name"), buffer)
                .on_input(move |s| LibraryMessage::BrowserSetRenameName {
                    library_path: library_for_input.clone(),
                    value: s,
                })
                .on_submit(LibraryMessage::BrowserConfirmRenameTable {
                    library_path: library_for_confirm.clone(),
                })
                .padding(SP.s)
                .size(BROWSER_TEXT_SIZE);
            let confirm =
                button(text(t!(i18n, ids::LIBRARY_BROWSER_RENAME_CONFIRM)).size(BROWSER_TEXT_SIZE))
                    .padding([SP.s, SP.m])
                    .on_press(LibraryMessage::BrowserConfirmRenameTable {
                        library_path: library_for_confirm,
                    })
                    .style(look::confirm_style(tokens));
            let cancel = button(text(raw("×")).size(BROWSER_TEXT_SIZE).color(text_c))
                .padding([SP.s, SP.m])
                .on_press(LibraryMessage::BrowserCancelRenameTable {
                    library_path: library_for_cancel,
                })
                .style(move |_: &Theme, _| iced::widget::button::Style {
                    background: None,
                    text_color: wash_ink,
                    border: Border::default(),
                    ..iced::widget::button::Style::default()
                });
            let mut form: Column<Element<'a, LibraryMessage>> = column![
                row![
                    name_input,
                    Space::new().width(4),
                    cancel,
                    Space::new().width(2),
                    confirm,
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(SP.xs)
            .padding([SP.s, SP.m]);
            if let Some(err) = browser.rename_error.as_ref() {
                form = form.push(
                    text(raw(err.clone()))
                        .size(BROWSER_TEXT_SIZE)
                        .color(iced::Color::from_rgb(0.85, 0.3, 0.3))
                        .boxed(),
                );
            }
            col = col.push(form.boxed());
            continue;
        }

        let row_label = row![
            text(signex_i18n::UserText::new((*name).clone()))
                .size(BROWSER_TEXT_SIZE)
                .color(text_c)
                .width(Length::Fill),
            text(t!(i18n, ids::LIBRARY_BROWSER_ROW_COUNT, "count" => count))
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
                let bg = bg_color.or(match status {
                    iced::widget::button::Status::Hovered => Some(hover_bg),
                    _ => None,
                });
                iced::widget::button::Style {
                    background: bg.map(iced::Background::Color),
                    text_color: text_c,
                    border: Border {
                        width: 0.0,
                        radius: 0.0.into(),
                        color: iced::Color::TRANSPARENT,
                        ..Border::default()
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
        let delete_btn = button(text(raw("×")).size(BROWSER_TEXT_SIZE + 1.0).color(muted))
            .padding([3.0, SP.m])
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
                        ..Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            });

        // ✎ rename trigger — sibling to × so click routing stays
        // unambiguous. Switches the row into the inline rename
        // form above.
        let library_for_rename = lib_pb.clone();
        let table_for_rename = (*name).clone();
        let rename_btn = button(text(raw("\u{270E}")).size(BROWSER_TEXT_SIZE).color(muted))
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
                    | iced::widget::button::Status::Pressed => {
                        (Some(iced::Background::Color(active_bg)), wash_ink)
                    }
                    _ => (None, muted),
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: fg,
                    border: Border {
                        width: 0.0,
                        radius: 2.0.into(),
                        color: iced::Color::TRANSPARENT,
                        ..Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            });

        let row_with_actions = row![row_btn, rename_btn, delete_btn,]
            .align_y(iced::Alignment::Center)
            .width(Length::Fill);
        col = col.push(row_with_actions.boxed());
    }

    // Inline `+ Table` form / button — same lifecycle as before, now
    // anchored at the bottom of the sidebar.
    let bottom_section: Element<'a, LibraryMessage> = match browser.adding_table.as_ref() {
        Some(draft) => {
            let library_for_input = lib_pb.clone();
            let library_for_confirm = lib_pb.clone();
            let library_for_cancel = lib_pb.clone();
            let name_input = text_input(raw("new_table_name"), &draft.name)
                .on_input(move |s| LibraryMessage::BrowserSetNewTableName {
                    library_path: library_for_input.clone(),
                    value: s,
                })
                .on_submit(LibraryMessage::BrowserConfirmAddTable {
                    library_path: library_for_confirm.clone(),
                })
                .padding(SP.s)
                .size(BROWSER_TEXT_SIZE);
            let confirm =
                button(text(t!(i18n, ids::LIBRARY_BROWSER_CREATE_TABLE)).size(BROWSER_TEXT_SIZE))
                    .padding([SP.s, 10.0])
                    .on_press(LibraryMessage::BrowserConfirmAddTable {
                        library_path: library_for_confirm,
                    })
                    .style(look::confirm_style(tokens));
            let cancel = button(
                text(t!(i18n, ids::LIBRARY_BROWSER_CANCEL))
                    .size(BROWSER_TEXT_SIZE)
                    .color(text_c),
            )
            .padding([SP.s, 10.0])
            .on_press(LibraryMessage::BrowserCancelAddTable {
                library_path: library_for_cancel,
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(control_bg)),
                text_color: wash_ink,
                border: Border {
                    width: 0.0,
                    radius: 3.0.into(),
                    color: iced::Color::TRANSPARENT,
                    ..Border::default()
                },
                ..iced::widget::button::Style::default()
            });
            let mut form: Column<Element<'a, LibraryMessage>> = column![
                name_input,
                row![cancel, Space::new().width(4), confirm,].align_y(iced::Alignment::Center),
            ]
            .spacing(SP.s)
            .padding([6.0, SP.l]);
            if let Some(err) = draft.error.as_ref() {
                form = form.push(
                    text(raw(err.clone()))
                        .size(BROWSER_TEXT_SIZE)
                        .color(iced::Color::from_rgb(0.85, 0.3, 0.3))
                        .boxed(),
                );
            }
            form.boxed()
        }
        None => {
            let library_for_begin = lib_pb.clone();
            container(
                button(
                    text(raw("+ Class"))
                        .size(BROWSER_TEXT_SIZE)
                        .color(if read_only { muted } else { text_c }),
                )
                .padding([SP.s, 10.0])
                .width(Length::Fill)
                .on_press_maybe((!read_only).then(|| LibraryMessage::BrowserBeginAddTable {
                    library_path: library_for_begin,
                }))
                .style(move |_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(control_bg)),
                    text_color: wash_ink,
                    border: Border {
                        width: 0.0,
                        radius: 3.0.into(),
                        color: iced::Color::TRANSPARENT,
                        ..Border::default()
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
            .boxed()
        }
    };

    col = col.push(Space::new().height(Length::Fill).boxed());
    col = col.push(bottom_section);

    container(col)
        .width(Length::Fixed(SIDEBAR_W))
        .height(Length::Fill)
        .boxed()
}
