//! The Identity section of the Edit Component modal: Internal PN, Class and
//! State, Manufacturer and MPN, Description, Datasheet and Tags.

use iced::widget::{Column, column, container, pick_list, row};
use iced::{Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_library::{ComponentClass, DatasheetRef, LifecycleState};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{PickListPlaceholder, text, text_input, theme_ext};

use super::super::messages::{BrowserEditMsg, LibraryMessage};
use super::super::state::EditRowModalState;
use crate::library::editor::kit::lifecycle_label;
use crate::panels::form_look::text_input_style;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width shares of the Internal PN / Class / State grid (1.4fr 1fr 1fr).
const PN_SHARE: u16 = 14;
const PICK_SHARE: u16 = 10;

/// `pick_list` adapter for the Class dropdown.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClassPick {
    key: String,
    label: String,
}

impl std::fmt::Display for ClassPick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// `pick_list` adapter for the Lifecycle State dropdown, shown with the
/// state's display label (the one its pill uses).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LifecyclePick(LifecycleState);

impl std::fmt::Display for LifecyclePick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(lifecycle_label(self.0))
    }
}

const LIFECYCLE_OPTIONS: [LifecyclePick; 5] = [
    LifecyclePick(LifecycleState::Draft),
    LifecyclePick(LifecycleState::InReview),
    LifecyclePick(LifecycleState::Released),
    LifecyclePick(LifecycleState::Deprecated),
    LifecyclePick(LifecycleState::Obsolete),
];

/// A field caption over its control.
fn labelled<'a>(
    label: Localized,
    body: Element<'a, LibraryMessage>,
    tokens: &ThemeTokens,
) -> Column<'a, LibraryMessage> {
    column![
        text(label)
            .size(TS.caption)
            .color(crate::styles::tokens::resolve(tokens).ink3),
        body
    ]
    .spacing(SP.xs + SP.xs)
}

/// A one-line form input in the form look, `mono` for part numbers.
fn input<'a>(
    placeholder: Localized,
    value: &'a str,
    mono: bool,
    on_input: impl Fn(String) -> LibraryMessage + 'a,
) -> iced::widget::TextInput<'a, LibraryMessage> {
    let field = text_input(placeholder, value)
        .style(text_input_style)
        .on_input(on_input)
        .padding(SP.s + SP.xs)
        .size(TS.label);
    if mono {
        field.font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ))
    } else {
        field
    }
}

/// The Identity fields, top to bottom. `pn_conflict` is the table of
/// another row that already holds the draft's Internal PN: the field gets
/// the error outline and the reason under it.
pub(super) fn view<'a>(
    edit: &'a EditRowModalState,
    classes: Vec<crate::fonts::ComponentClassEntry>,
    pn_conflict: Option<&str>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    send: impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a,
) -> Element<'a, LibraryMessage> {
    let gap = SP.m;

    let error = theme_ext::error_color(tokens);
    let conflicting = pn_conflict.is_some();
    let pn_input = input(
        t!(i18n, ids::LIBRARY_EDIT_ROW_INTERNAL_PN),
        edit.draft.internal_pn.as_str(),
        true,
        {
            let send = send.clone();
            move |s| send(BrowserEditMsg::SetInternalPn(s))
        },
    )
    .style(move |theme: &Theme, status| {
        let mut style = text_input_style(theme, status);
        if conflicting {
            style.border.color = error;
        }
        style
    });
    let mut pn_cell = labelled(
        t!(i18n, ids::LIBRARY_EDIT_ROW_INTERNAL_PN),
        pn_input.boxed(),
        tokens,
    );
    if let Some(table) = pn_conflict {
        pn_cell = pn_cell.push(
            text(t!(
                i18n,
                ids::LIBRARY_EDIT_ROW_PN_CONFLICT,
                "pn" => edit.draft.internal_pn.as_str().trim(),
                "table" => table
            ))
            .size(TS.caption)
            .color(error),
        );
    }

    let class_picks: Vec<ClassPick> = classes
        .iter()
        .map(|entry| ClassPick {
            key: entry.key.clone(),
            label: entry.label.clone(),
        })
        .collect();
    let selected_class = class_picks
        .iter()
        .find(|p| p.key == edit.draft.class.as_str())
        .cloned();
    let class_picker = pick_list(selected_class, class_picks, ClassPick::to_string)
        .graphite()
        .on_select({
            let send = send.clone();
            move |pick: ClassPick| send(BrowserEditMsg::SetClass(ComponentClass::new(pick.key)))
        })
        .placeholder_label(t!(i18n, ids::LIBRARY_EDIT_ROW_CLASS_PLACEHOLDER))
        .width(Length::Fill)
        .padding(SP.s + SP.xs);

    let selected_state = LIFECYCLE_OPTIONS
        .iter()
        .find(|p| p.0 == edit.draft.state)
        .copied();
    let state_picker = pick_list(
        selected_state,
        LIFECYCLE_OPTIONS.to_vec(),
        LifecyclePick::to_string,
    )
    .graphite()
    .on_select({
        let send = send.clone();
        move |pick: LifecyclePick| send(BrowserEditMsg::SetState(pick.0))
    })
    .placeholder_label(t!(i18n, ids::LIBRARY_EDIT_ROW_STATE_PLACEHOLDER))
    .width(Length::Fill)
    .padding(SP.s + SP.xs);

    let pn_class_state = row![
        pn_cell.width(Length::FillPortion(PN_SHARE)),
        labelled(
            t!(i18n, ids::LIBRARY_EDIT_ROW_CLASS),
            class_picker.boxed(),
            tokens
        )
        .width(Length::FillPortion(PICK_SHARE)),
        labelled(
            t!(i18n, ids::LIBRARY_EDIT_ROW_STATE),
            state_picker.boxed(),
            tokens
        )
        .width(Length::FillPortion(PICK_SHARE)),
    ]
    .spacing(gap);

    let mfr_mpn = row![
        labelled(
            t!(i18n, ids::LIBRARY_EDIT_ROW_MANUFACTURER),
            input(
                t!(i18n, ids::LIBRARY_EDIT_ROW_MANUFACTURER),
                &edit.draft.primary_mpn.manufacturer,
                false,
                {
                    let send = send.clone();
                    move |s| send(BrowserEditMsg::SetManufacturer(s))
                },
            )
            .boxed(),
            tokens,
        )
        .width(Length::FillPortion(1)),
        labelled(
            t!(i18n, ids::LIBRARY_EDIT_ROW_MPN),
            input(
                t!(i18n, ids::LIBRARY_EDIT_ROW_MPN),
                &edit.draft.primary_mpn.mpn,
                true,
                {
                    let send = send.clone();
                    move |s| send(BrowserEditMsg::SetMpn(s))
                },
            )
            .boxed(),
            tokens,
        )
        .width(Length::FillPortion(1)),
    ]
    .spacing(gap);

    let description = labelled(
        t!(i18n, ids::LIBRARY_EDIT_ROW_DESCRIPTION),
        input(
            t!(i18n, ids::LIBRARY_EDIT_ROW_DESCRIPTION_PLACEHOLDER),
            &edit.draft.description,
            false,
            {
                let send = send.clone();
                move |s| send(BrowserEditMsg::SetDescription(s))
            },
        )
        .boxed(),
        tokens,
    );

    // Only a URL ref is editable here. A pinned PDF is shown read-only so a
    // keystroke can never replace it (and its hash) with a bare URL.
    let datasheet_field: Element<'a, LibraryMessage> = match &edit.draft.datasheet {
        DatasheetRef::Url { url } => input(
            t!(i18n, ids::LIBRARY_EDIT_ROW_DATASHEET_PLACEHOLDER),
            url,
            false,
            {
                let send = send.clone();
                move |s| send(BrowserEditMsg::SetDatasheetUrl(s))
            },
        )
        .boxed(),
        DatasheetRef::HashPinned { filename, .. } => container(
            text(t!(
                i18n,
                ids::LIBRARY_EDIT_ROW_DATASHEET_PINNED,
                "file" => filename.as_str()
            ))
            .size(TS.label)
            .color(theme_ext::text_secondary(tokens)),
        )
        .padding([SP.s + SP.xs, 0.0])
        .boxed(),
    };
    let datasheet = labelled(
        t!(i18n, ids::LIBRARY_EDIT_ROW_DATASHEET),
        datasheet_field,
        tokens,
    );

    // Comma-separated free-form tokens. The buffer flushes to
    // `parameters["tags"]` on Save; an empty buffer drops the entry.
    let tags = labelled(
        t!(i18n, ids::LIBRARY_EDIT_ROW_TAGS),
        input(
            t!(i18n, ids::LIBRARY_EDIT_ROW_TAGS_PLACEHOLDER),
            &edit.tags_buf,
            false,
            move |s| send(BrowserEditMsg::SetTags(s)),
        )
        .boxed(),
        tokens,
    );

    column![pn_class_state, mfr_mpn, description, datasheet, tags]
        .spacing(SP.m + SP.xs)
        .width(Length::Fill)
        .boxed()
}
