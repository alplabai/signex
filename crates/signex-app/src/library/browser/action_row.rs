//! Library Browser — the grid action row (Add / Edit Details / Delete Selected).
//!
//! Extracted verbatim from the former single-file `browser` module.

use super::*;

/// The message the "Edit Details" button fires: opens the Edit Component
/// Details modal for the selected row. `None` while nothing is selected or
/// the library is read-only, which leaves the button inert.
fn edit_details_message(
    library_path: &std::path::Path,
    table: &str,
    selected: Option<RowId>,
    read_only: bool,
) -> Option<LibraryMessage> {
    let row_id = selected.filter(|_| !read_only)?;
    Some(LibraryMessage::BrowserOpenEditModal {
        library_path: library_path.to_path_buf(),
        table: table.to_string(),
        row_id,
    })
}

pub(super) fn view_action_row<'a>(
    library_path: &'a std::path::Path,
    table: &str,
    selected: Option<RowId>,
    tokens: &'a ThemeTokens,
    read_only: bool,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);
    // Hoisted owned path — see comment in `view_table_sidebar`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    let library_for_add = lib_pb.clone();
    let table_for_add = Some(table.to_string());
    let add_btn = button(
        text("Add Component")
            .size(BROWSER_TEXT_SIZE)
            .color(iced::Color::WHITE),
    )
    .padding([4, 12])
    .on_press_maybe((!read_only).then(|| LibraryMessage::BrowserAddComponent {
        library_path: library_for_add,
        table: table_for_add,
    }))
    .style(move |_: &Theme, _| iced::widget::button::Style {
        background: Some(iced::Background::Color(if read_only {
            iced::Color::from_rgb(0.35, 0.35, 0.38)
        } else {
            iced::Color::from_rgb(0.00, 0.47, 0.84)
        })),
        text_color: iced::Color::WHITE,
        border: Border {
            width: 0.0,
            radius: 3.0.into(),
            color: iced::Color::TRANSPARENT,
        },
        ..iced::widget::button::Style::default()
    });

    // Edit Details opens the full-row Edit Component Details modal for
    // the selected row. Cells in the grid are text inputs that capture
    // clicks, so a row double-click is not a reliable trigger; the
    // action row is the entry point.
    let edit_btn: Element<'a, LibraryMessage> =
        if let Some(open_edit) = edit_details_message(library_path, table, selected, read_only) {
            button(text("Edit Details").size(BROWSER_TEXT_SIZE).color(text_c))
                .padding([4, 12])
                .on_press(open_edit)
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
                .into()
        } else {
            container(text("Edit Details").size(BROWSER_TEXT_SIZE).color(muted))
                .padding([4, 12])
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        1.0, 1.0, 1.0, 0.02,
                    ))),
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border,
                    },
                    ..Default::default()
                })
                .into()
        };

    let delete_btn: Element<'a, LibraryMessage> = if let Some(row_id) = selected
        && !read_only
    {
        let library_for_del = lib_pb.clone();
        let table_for_del = table.to_string();
        button(
            text("Delete Selected")
                .size(BROWSER_TEXT_SIZE)
                .color(text_c),
        )
        .padding([4, 12])
        .on_press(LibraryMessage::BrowserDeleteRowRequest {
            library_path: library_for_del,
            table: table_for_del,
            row_id,
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
        .into()
    } else {
        // Greyed-out placeholder when no row is selected.
        container(text("Delete Selected").size(BROWSER_TEXT_SIZE).color(muted))
            .padding([4, 12])
            .style(move |_: &Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.02,
                ))),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..Default::default()
            })
            .into()
    };

    let mut actions = row![
        add_btn,
        Space::new().width(8),
        edit_btn,
        Space::new().width(8),
        delete_btn
    ]
    .spacing(0)
    .align_y(iced::Alignment::Center);
    if read_only {
        actions = actions.push(Space::new().width(12)).push(
            text("This library ships with Signex and is not editable.")
                .size(BROWSER_TEXT_SIZE)
                .color(muted),
        );
    }
    container(actions).padding([8, 4]).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_details_targets_the_selected_row_and_is_inert_otherwise() {
        let path = std::path::Path::new("parts.snxlib");
        let row_id = RowId::from_uuid(uuid::Uuid::new_v4());
        match edit_details_message(path, "R", Some(row_id), false) {
            Some(LibraryMessage::BrowserOpenEditModal {
                library_path,
                table,
                row_id: got,
            }) => {
                assert_eq!(library_path, path);
                assert_eq!(table, "R");
                assert_eq!(got, row_id);
            }
            other => panic!("expected BrowserOpenEditModal, got {other:?}"),
        }
        assert!(edit_details_message(path, "R", None, false).is_none());
        assert!(edit_details_message(path, "R", Some(row_id), true).is_none());
    }
}
