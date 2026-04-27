//! Overview tab — WS-E PENDING placeholder.
//!
//! The pre-refactor Overview tab edited `SchematicSide` /
//! `SharedSide` / `PcbSide` fields directly. WS-B re-shaped `Component`
//! into a binding record; WS-E owns the rebuild against the new
//! `Revision { primary_mpn, alternates, supply, datasheet, parameters,
//! pin_map_overrides, plm }` layout. WS-F here only ships the
//! Symbol/Footprint/Body3D editor surfaces, so this tab renders a
//! "WS-E pending" placeholder until the merger lands.
//!
//! TODO(merge-with-WS-E): replace this stub with the full Overview
//! form — display name, Internal PN, Manufacturer + MPN row, alternate
//! MPN list, supply / datasheet / lifecycle / parameter validation.

use iced::widget::{Space, column, container, text};
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentEditorState, EditorAddress};

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);

    let datasheet_url: String = match &editor.draft.datasheet {
        DatasheetRef::Url { url } => url.clone(),
        DatasheetRef::HashPinned { filename, .. } => filename.clone(),
    };

    let description: String = editor
        .draft
        .primary_mpn
        .notes
        .clone()
        .unwrap_or_default();

    let field = |label: &'static str,
                 value: String,
                 placeholder: &'static str,
                 msg: fn(String) -> EditorMsg|
     -> Element<'a, LibraryMessage> {
        let lib_path_for_input = address.library_path.clone();
        let component_id_for_input = address.component_id;
        column![
            text(label).size(10).color(muted),
            text_input(placeholder, &value)
                .on_input(move |s| LibraryMessage::EditorEvent {
                    library_path: lib_path_for_input.clone(),
                    component_id: component_id_for_input,
                    msg: msg(s),
                })
                .padding([4, 8])
                .size(12),
        ]
        .spacing(4)
        .width(Length::Fill)
        .into()
    };

    let lib_path_for_lifecycle = address.library_path.clone();
    let component_id_for_lifecycle = address.component_id;
    let lifecycle_picker = pick_list(
        LIFECYCLE_OPTS.map(LifecyclePick),
        Some(LifecyclePick(editor.draft.state)),
        move |LifecyclePick(s)| LibraryMessage::EditorEvent {
            library_path: lib_path_for_lifecycle.clone(),
            component_id: component_id_for_lifecycle,
            msg: EditorMsg::OverviewSetLifecycle(s),
        },
    )
    .text_size(12)
    .padding([4, 8]);

    let lifecycle_block: Element<'a, LibraryMessage> = column![
        text("Lifecycle State").size(10).color(muted),
        lifecycle_picker,
    ]
    .spacing(4)
    .into();

    let body = column![
        text("Overview").size(13).color(text_c),
        Space::new().height(6),
        text(format!("Internal PN: {}", editor.display_internal_pn))
            .size(12)
            .color(text_c),
        text(format!("Class: {}", editor.component.class.as_str()))
            .size(11)
            .color(muted),
        Space::new().height(8),
        text("Overview tab rebuild lives in WS-E (binding-record fields:")
            .size(11)
            .color(muted),
        text("primary_mpn / alternates / supply / datasheet / lifecycle / parameters).")
            .size(11)
            .color(muted),
    ]
    .spacing(0);

    container(body)
        .padding(14)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::modal_card(tokens))
        .into()
}
