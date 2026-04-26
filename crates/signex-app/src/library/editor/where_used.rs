//! Where-Used tab — list every project / sheet / instance using
//! this component.
//!
//! Reads from [`LibraryState.where_used`] which is populated via
//! [`LibraryState::ingest_sheet`] whenever a sheet opens or saves.
//! For Phase 1 the index is empty until the future sheet-load flow
//! starts calling `ingest_sheet`; clicking a row fires
//! [`LibraryMessage::JumpToUseSite`] which `commands::jump_to_use_site`
//! turns into a `tracing::info!` stub.

use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Border, Element, Length, Theme};
use signex_library::UseSite;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::LibraryMessage;
use super::super::state::{ComponentEditorState, LibraryState};

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    let hover = crate::styles::ti(tokens.hover);

    let header = row![
        text("Project")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(3)),
        text("Sheet")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(3)),
        text("Instance")
            .size(10)
            .color(muted)
            .width(Length::Fixed(96.0)),
        text("Pinned Version")
            .size(10)
            .color(muted)
            .width(Length::Fixed(120.0)),
    ]
    .padding([4, 6]);

    // Phase 1: read live use-sites from the LibraryState index. The
    // list is empty until the sheet-load flow calls
    // `LibraryState::ingest_sheet` (TODO(v0.9-phase-3)).
    let sites = library_state.where_used_for(editor.component_id, None);

    let mut col = column![header].spacing(1);
    if sites.is_empty() {
        col = col.push(
            container(
                text("No use sites yet for this component.")
                    .size(11)
                    .color(muted),
            )
            .padding([10, 6]),
        );
    } else {
        // Stable ordering by (project, sheet, instance) so re-renders
        // don't shuffle rows.
        let mut sorted = sites.clone();
        sorted.sort_by(|a, b| {
            (
                a.project_path.as_path(),
                a.sheet_path.as_path(),
                a.instance_id.as_str(),
            )
                .cmp(&(
                    b.project_path.as_path(),
                    b.sheet_path.as_path(),
                    b.instance_id.as_str(),
                ))
        });
        for site in sorted {
            col = col.push(use_site_row(site, text_c, hover, border));
        }
    }

    container(scrollable(col).height(Length::Fill).width(Length::Fill))
        .style(crate::styles::modal_card(tokens))
        .padding(14)
        .into()
}

fn use_site_row<'a>(
    site: UseSite,
    text_c: iced::Color,
    hover: iced::Color,
    border: iced::Color,
) -> Element<'a, LibraryMessage> {
    let project_label = site
        .project_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| site.project_path.display().to_string());
    let sheet_label = site
        .sheet_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| site.sheet_path.display().to_string());
    let instance_label = site.instance_id.clone();
    let version_label = site.version_pinned.to_string();
    let click_payload = site;

    let row_inner = row![
        text(project_label)
            .size(11)
            .color(text_c)
            .width(Length::FillPortion(3)),
        text(sheet_label)
            .size(11)
            .color(text_c)
            .width(Length::FillPortion(3)),
        text(instance_label)
            .size(11)
            .color(text_c)
            .width(Length::Fixed(96.0)),
        text(version_label)
            .size(11)
            .color(text_c)
            .width(Length::Fixed(120.0)),
    ]
    .padding([3, 6]);

    button(container(row_inner))
        .padding(0)
        .width(Length::Fill)
        .on_press(LibraryMessage::JumpToUseSite(click_payload))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(hover)),
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                text_color: text_c,
                border: Border {
                    width: 0.5,
                    radius: 0.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}
