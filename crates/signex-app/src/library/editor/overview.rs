//! Overview tab — display name, internal PN, MPN, manufacturer,
//! description, datasheet, lifecycle state.
//!
//! WS-E (refactor): rewired against the new `Revision` shape — the
//! description / datasheet / mpn / manufacturer fields are no longer
//! grouped under `draft.shared`; they live on `draft.primary_mpn` and
//! `draft.datasheet` directly.

use iced::widget::{Space, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};
use signex_library::{DatasheetRef, LifecycleState};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::ComponentEditorState;

const LIFECYCLE_OPTS: [LifecycleState; 5] = [
    LifecycleState::Draft,
    LifecycleState::InReview,
    LifecycleState::Released,
    LifecycleState::Deprecated,
    LifecycleState::Obsolete,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LifecyclePick(LifecycleState);

impl std::fmt::Display for LifecyclePick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self.0 {
            LifecycleState::Draft => "Draft",
            LifecycleState::InReview => "In Review",
            LifecycleState::Released => "Released",
            LifecycleState::Deprecated => "Deprecated",
            LifecycleState::Obsolete => "Obsolete",
            other => return write!(f, "{other:?}"),
        };
        f.write_str(s)
    }
}

pub fn view<'a>(
    editor: &'a ComponentEditorState,
    tokens: &'a ThemeTokens,
    window_id: iced::window::Id,
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
        column![
            text(label).size(10).color(muted),
            text_input(placeholder, &value)
                .on_input(move |s| LibraryMessage::EditorEvent {
                    window_id,
                    msg: msg(s),
                })
                .padding([4, 8])
                .size(12),
        ]
        .spacing(4)
        .width(Length::Fill)
        .into()
    };

    let lifecycle_picker = pick_list(
        LIFECYCLE_OPTS.map(LifecyclePick),
        Some(LifecyclePick(editor.draft.state)),
        move |LifecyclePick(s)| LibraryMessage::EditorEvent {
            window_id,
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
        row![
            field(
                "Display Name",
                editor.display_internal_pn.clone(),
                "Friendly label",
                EditorMsg::OverviewSetDisplayName,
            ),
            Space::new().width(12),
            field(
                "Internal PN",
                editor.component.internal_pn.as_str().to_string(),
                "R0805_10k",
                EditorMsg::OverviewSetInternalPn,
            ),
        ]
        .align_y(iced::Alignment::Start),
        Space::new().height(10),
        row![
            field(
                "Manufacturer",
                editor.draft.primary_mpn.manufacturer.clone(),
                "Yageo",
                EditorMsg::OverviewSetManufacturer,
            ),
            Space::new().width(12),
            field(
                "Manufacturer Part Number (MPN)",
                editor.draft.primary_mpn.mpn.clone(),
                "RC0805FR-0710KL",
                EditorMsg::OverviewSetMpn,
            ),
        ],
        Space::new().height(10),
        field(
            "Description / Notes",
            description,
            "Resistor 10k 1% 0805",
            EditorMsg::OverviewSetDescription,
        ),
        Space::new().height(10),
        field(
            "Datasheet URL",
            datasheet_url,
            "https://example.com/datasheet.pdf",
            EditorMsg::OverviewSetDatasheet,
        ),
        Space::new().height(10),
        lifecycle_block,
    ]
    .spacing(0)
    .width(Length::Fill);

    container(scrollable(body).width(Length::Fill).height(Length::Fill))
        .style(crate::styles::modal_card(tokens))
        .padding(14)
        .into()
}
