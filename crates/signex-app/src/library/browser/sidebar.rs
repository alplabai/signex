//! Library Browser — the 210 px sidebar (#118): the LIBRARIES group (see
//! `tree.rs`) over the TABLES group, whose header carries the add-table
//! action as a small ghost button. One row per table with its row count,
//! the active one in the neutral selection, and the inline rename / delete
//! / add-table forms.

use super::look;
use super::*;
use iced::widget::column;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of the sidebar.
pub(super) const SIDEBAR_W: f32 = 210.0;

/// The whole sidebar: libraries, then tables, scrolling together, on the
/// panel fill with a line on its right edge.
pub(super) fn view_sidebar<'a>(
    library_path: &'a std::path::Path,
    library_state: &'a LibraryState,
    lib: &'a OpenLibrary,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let body = column![
        super::tree::view_libraries_tree(library_path, library_state, tokens, i18n),
        view_table_list(library_path, lib, browser, tokens, i18n, read_only),
    ]
    .spacing(SP.s)
    .padding([SP.s, SP.xs])
    .width(Length::Fill);
    let panel = crate::styles::ti(tokens.panel_bg);
    let line = theme_ext::border_color(tokens);
    container(
        scrollable(body)
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(tokens))
            .height(Length::Fill),
    )
    .width(Length::Fixed(SIDEBAR_W))
    .height(Length::Fill)
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(panel)),
        border: Border {
            color: line,
            right: iced::border::Side::default().width(1.0),
            ..Border::default()
        },
        ..iced::widget::container::Style::default()
    })
    .boxed()
}

/// The message the TABLES header's add button sends; `None` on a
/// read-only library.
pub(super) fn add_table_message(
    library_path: &std::path::Path,
    read_only: bool,
) -> Option<LibraryMessage> {
    (!read_only).then(|| LibraryMessage::BrowserBeginAddTable {
        library_path: library_path.to_path_buf(),
    })
}

/// The TABLES header: the section label and, at its right, the add-table
/// action as a small ghost button in the accent text (today's wording).
fn tables_header<'a>(
    library_path: &std::path::Path,
    tokens: &ThemeTokens,
    i18n: &I18n,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let (accent_text, disabled) = (r.accent_text, r.ink4);
    let hover = look::hover_fill(tokens);
    let add = button(text(t!(i18n, ids::LIBRARY_BROWSER_ADD_CLASS)).size(TS.caption))
        .padding([0.0, SP.s])
        .on_press_maybe(add_table_message(library_path, read_only))
        .style(move |_: &Theme, status| iced::widget::button::Style {
            background: matches!(status, iced::widget::button::Status::Hovered)
                .then_some(iced::Background::Color(hover)),
            text_color: if status == iced::widget::button::Status::Disabled {
                disabled
            } else {
                accent_text
            },
            border: Border {
                radius: crate::styles::Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        });
    container(
        row![
            text(t!(i18n, ids::LIBRARY_BROWSER_TABLES))
                .size(TS.section)
                .style(crate::styles::section_label(tokens))
                .boxed(),
            Space::new().width(Length::Fill).boxed(),
            add.boxed(),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding(iced::Padding {
        top: SP.m,
        right: SP.m,
        bottom: SP.xs,
        left: SP.m,
    })
    .boxed()
}

/// The TABLES group: header, the add-table form while it is open, the last
/// delete error, then one row per table.
fn view_table_list<'a>(
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
    let row_radius = crate::styles::Radii::GRAPHITE.small;

    let mut col: Column<Element<'a, LibraryMessage>> =
        column![tables_header(library_path, tokens, i18n, read_only)]
            .spacing(SP.xs)
            .width(Length::Fill);

    // Hoist a single owned `PathBuf` out of every per-row loop: each row's
    // message constructors need an owned path inside their closure.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    // The inline add-table form opens under the header.
    if let Some(draft) = browser.adding_table.as_ref() {
        let library_for_input = lib_pb.clone();
        let library_for_confirm = lib_pb.clone();
        let library_for_cancel = lib_pb.clone();
        let name_input = text_input(
            t!(i18n, ids::LIBRARY_BROWSER_NEW_TABLE_NAME_PLACEHOLDER),
            &draft.name,
        )
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
        .padding([SP.xs, SP.m]);
        if let Some(err) = draft.error.as_ref() {
            form = form.push(
                text(raw(err.clone()))
                    .size(BROWSER_TEXT_SIZE)
                    .color(theme_ext::error_color(tokens))
                    .boxed(),
            );
        }
        col = col.push(form.boxed());
    }

    let mut names: Vec<&String> = lib.tables.keys().collect();
    names.sort();
    // The most recent delete-table error sits above the list, so the user
    // sees why a `×` click bounced (typically "table is not empty").
    if let Some(err) = browser.delete_error.as_ref() {
        let (fill, ink) = crate::styles::pill_colors(tokens, crate::styles::PillTone::Err);
        let dismiss = button(text(raw("×")).size(BROWSER_TEXT_SIZE).color(ink))
            .padding([0, 6])
            .on_press(LibraryMessage::BrowserDismissDeleteError {
                library_path: lib_pb.clone(),
            })
            .style(move |_: &Theme, _| iced::widget::button::Style {
                background: None,
                text_color: ink,
                border: Border::default(),
                ..iced::widget::button::Style::default()
            });
        let banner = container(
            row![
                text(raw(err.clone()))
                    .size(BROWSER_TEXT_SIZE)
                    .color(ink)
                    .width(Length::Fill),
                dismiss,
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([SP.s, SP.m])
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(fill)),
            border: Border {
                radius: row_radius.into(),
                ..Border::default()
            },
            ..iced::widget::container::Style::default()
        });
        col = col.push(container(banner).padding([SP.xs, SP.xs]).boxed());
    }

    for name in &names {
        let is_active = browser.active_table.as_deref() == Some(name.as_str());
        let count = lib.tables.get(*name).map(|v| v.len()).unwrap_or(0);
        let is_renaming = browser
            .renaming_table
            .as_ref()
            .is_some_and(|(orig, _)| orig.as_str() == name.as_str());

        // Rename mode: the row becomes a text input with confirm / cancel.
        // Submit on Enter; Esc closes from the tab's escape handler.
        if is_renaming {
            col = col.push(rename_form(&lib_pb, browser, tokens, i18n));
            continue;
        }

        let row_label = row![
            text(signex_i18n::UserText::new((*name).clone()))
                .size(BROWSER_TEXT_SIZE)
                .color(text_c)
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Length::Fill),
            text(t!(i18n, ids::LIBRARY_BROWSER_ROW_COUNT, "count" => count))
                .size(TS.caption)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT
                ))
                .color(muted),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        let on_press = LibraryMessage::BrowserSelectTable {
            library_path: lib_pb.clone(),
            table: (*name).clone(),
        };
        let bg_color = if is_active { Some(active_bg) } else { None };
        let row_btn = button(row_label)
            .padding([5.0, SP.m])
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
                        radius: row_radius.into(),
                        ..Border::default()
                    },
                    ..iced::widget::button::Style::default()
                }
            });

        // ✎ rename and × delete are siblings of the select button so click
        // routing stays unambiguous (nested buttons confuse hit testing).
        // The adapter refuses a non-empty delete; the error shows above.
        let danger = theme_ext::error_color(tokens);
        let delete_btn = button(text(raw("×")).size(BROWSER_TEXT_SIZE + 1.0).color(muted))
            .padding([3.0, SP.s])
            .on_press_maybe((!read_only).then(|| LibraryMessage::BrowserDeleteTable {
                library_path: lib_pb.clone(),
                table: (*name).clone(),
            }))
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let hot = matches!(
                    status,
                    iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
                );
                iced::widget::button::Style {
                    background: None,
                    text_color: if hot { danger } else { muted },
                    border: Border::default(),
                    ..iced::widget::button::Style::default()
                }
            });
        let rename_btn = button(text(raw("\u{270E}")).size(BROWSER_TEXT_SIZE).color(muted))
            .padding([3.0, SP.xs])
            .on_press_maybe(
                (!read_only).then(|| LibraryMessage::BrowserBeginRenameTable {
                    library_path: lib_pb.clone(),
                    table: (*name).clone(),
                }),
            )
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let hot = matches!(
                    status,
                    iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
                );
                iced::widget::button::Style {
                    background: None,
                    text_color: if hot { wash_ink } else { muted },
                    border: Border::default(),
                    ..iced::widget::button::Style::default()
                }
            });

        col = col.push(
            row![row_btn, rename_btn, delete_btn]
                .align_y(iced::Alignment::Center)
                .width(Length::Fill)
                .boxed(),
        );
    }
    col.boxed()
}

/// The inline rename form of the table being renamed.
fn rename_form<'a>(
    lib_pb: &std::path::Path,
    browser: &'a LibraryBrowserState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let wash_ink = look::ink_over_wash(tokens);
    let buffer = browser
        .renaming_table
        .as_ref()
        .map(|(_, b)| b.as_str())
        .unwrap_or("");
    let library_for_input = lib_pb.to_path_buf();
    let name_input = text_input(
        t!(i18n, ids::LIBRARY_BROWSER_TABLE_NAME_PLACEHOLDER),
        buffer,
    )
    .on_input(move |s| LibraryMessage::BrowserSetRenameName {
        library_path: library_for_input.clone(),
        value: s,
    })
    .on_submit(LibraryMessage::BrowserConfirmRenameTable {
        library_path: lib_pb.to_path_buf(),
    })
    .padding(SP.s)
    .size(BROWSER_TEXT_SIZE);
    let confirm =
        button(text(t!(i18n, ids::LIBRARY_BROWSER_RENAME_CONFIRM)).size(BROWSER_TEXT_SIZE))
            .padding([SP.s, SP.m])
            .on_press(LibraryMessage::BrowserConfirmRenameTable {
                library_path: lib_pb.to_path_buf(),
            })
            .style(look::confirm_style(tokens));
    let cancel = button(text(raw("×")).size(BROWSER_TEXT_SIZE).color(text_c))
        .padding([SP.s, SP.m])
        .on_press(LibraryMessage::BrowserCancelRenameTable {
            library_path: lib_pb.to_path_buf(),
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
                .color(theme_ext::error_color(tokens))
                .boxed(),
        );
    }
    form.boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #118: the TABLES header's add button opens the existing add-table
    /// form, and is off on a read-only library.
    #[test]
    fn the_tables_header_adds_a_table_unless_read_only() {
        let path = std::path::Path::new("parts.snxlib");
        match add_table_message(path, false) {
            Some(LibraryMessage::BrowserBeginAddTable { library_path }) => {
                assert_eq!(library_path, path);
            }
            other => panic!("expected BrowserBeginAddTable, got {other:?}"),
        }
        assert!(add_table_message(path, true).is_none());
    }

    #[test]
    fn the_sidebar_is_the_mock_width() {
        assert_eq!(SIDEBAR_W, 210.0);
    }
}
