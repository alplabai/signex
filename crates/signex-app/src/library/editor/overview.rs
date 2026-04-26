//! Overview tab — display name, internal PN, MPN, manufacturer,
//! description, datasheet, lifecycle state.

use iced::widget::{Space, column, container, pick_list, row, scrollable, text, text_input};
use iced::{Element, Length};
use signex_library::LifecycleState;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::ComponentEditorState;
use super::datasheet_picker;

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
            // LifecycleState is `non_exhaustive` — fall back to a
            // debug rendering for any future variant so the picker
            // still functions if signex-library adds states.
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

    let datasheet_block: Element<'a, LibraryMessage> = datasheet_picker::view(
        editor.draft.shared.datasheet.as_ref(),
        tokens,
        window_id,
    );

    let field = |label: &'static str,
                 value: &'a str,
                 placeholder: &'static str,
                 msg: fn(String) -> EditorMsg|
     -> Element<'a, LibraryMessage> {
        column![
            text(label).size(10).color(muted),
            text_input(placeholder, value)
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
        // Display name + internal PN side by side.
        row![
            field(
                "Display Name",
                &editor.display_internal_pn,
                "Friendly label",
                EditorMsg::OverviewSetDisplayName,
            ),
            Space::new().width(12),
            field(
                "Internal PN",
                editor.component.internal_pn.as_str(),
                "R0805_10k",
                EditorMsg::OverviewSetInternalPn,
            ),
        ]
        .align_y(iced::Alignment::Start),
        Space::new().height(10),
        row![
            field(
                "Manufacturer",
                &editor.draft.shared.manufacturer,
                "Yageo",
                EditorMsg::OverviewSetManufacturer,
            ),
            Space::new().width(12),
            field(
                "Manufacturer Part Number (MPN)",
                &editor.draft.shared.mpn,
                "RC0805FR-0710KL",
                EditorMsg::OverviewSetMpn,
            ),
        ],
        Space::new().height(10),
        field(
            "Description",
            &editor.draft.shared.description,
            "Resistor 10k 1% 0805",
            EditorMsg::OverviewSetDescription,
        ),
        Space::new().height(10),
        datasheet_block,
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
