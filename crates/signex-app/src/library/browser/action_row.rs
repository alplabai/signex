//! Library Browser — the grid action row (Add / Edit Details / Delete Selected).
//!
//! Extracted verbatim from the former single-file `browser` module.

use super::*;

/// Default type / spacing scales; equal to the old literals at the default size.
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

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
    let control_bg = super::look::control_fill(tokens);
    let inert_bg = super::look::inert_fill(tokens);
    let radius = super::look::control_radius();
    // Hoisted owned path — see comment in `view_table_sidebar`.
    let lib_pb: std::path::PathBuf = library_path.to_path_buf();

    // Edit Details opens the full-row Edit Component Details modal for
    // the selected row. Cells in the grid are text inputs that capture
    // clicks, so a row double-click is not a reliable trigger; the
    // action row is the entry point.
    let edit_btn: Element<'a, LibraryMessage> =
        if let Some(open_edit) = edit_details_message(library_path, table, selected, read_only) {
            button(text("Edit Details").size(BROWSER_HEADER_SIZE).color(text_c))
                .padding([SP.xs, SP.m])
                .on_press(open_edit)
                .style(move |_: &Theme, _| iced::widget::button::Style {
                    background: Some(iced::Background::Color(control_bg)),
                    text_color: text_c,
                    border: Border {
                        width: 1.0,
                        radius: radius.into(),
                        color: border,
                        ..Border::default()
                    },
                    ..iced::widget::button::Style::default()
                })
                .boxed()
        } else {
            container(text("Edit Details").size(BROWSER_HEADER_SIZE).color(muted))
                .padding([SP.xs, SP.m])
                .style(move |_: &Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(inert_bg)),
                    border: Border {
                        width: 1.0,
                        radius: radius.into(),
                        color: border,
                        ..Border::default()
                    },
                    ..Default::default()
                })
                .boxed()
        };

    let delete_btn: Element<'a, LibraryMessage> = if let Some(row_id) = selected
        && !read_only
    {
        let library_for_del = lib_pb.clone();
        let table_for_del = table.to_string();
        button(
            text("Delete Selected")
                .size(BROWSER_HEADER_SIZE)
                .color(text_c),
        )
        .padding([SP.xs, SP.m])
        .on_press(LibraryMessage::BrowserDeleteRowRequest {
            library_path: library_for_del,
            table: table_for_del,
            row_id,
        })
        .style(move |_: &Theme, _| iced::widget::button::Style {
            background: Some(iced::Background::Color(control_bg)),
            text_color: text_c,
            border: Border {
                width: 1.0,
                radius: radius.into(),
                color: border,
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        })
        .boxed()
    } else {
        // Greyed-out placeholder when no row is selected.
        container(
            text("Delete Selected")
                .size(BROWSER_HEADER_SIZE)
                .color(muted),
        )
        .padding([SP.xs, SP.m])
        .style(move |_: &Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(inert_bg)),
            border: Border {
                width: 1.0,
                radius: radius.into(),
                color: border,
                ..Border::default()
            },
            ..Default::default()
        })
        .boxed()
    };

    // Add Component is the header's primary button.
    let mut actions = row![edit_btn, Space::new().width(SP.s), delete_btn]
        .spacing(0)
        .align_y(iced::Alignment::Center);
    if read_only {
        actions = actions.push(Space::new().width(SP.m).boxed()).push(
            text("This library ships with Signex and is not editable.")
                .size(BROWSER_HEADER_SIZE)
                .color(muted)
                .boxed(),
        );
    }
    container(actions).padding([SP.s, SP.s]).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_details_targets_the_selected_row_and_is_inert_otherwise() {
        let path = std::path::Path::new("parts.snxlib");
        let row_id = RowId::from_uuid(uuid::Uuid::now_v7());
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
