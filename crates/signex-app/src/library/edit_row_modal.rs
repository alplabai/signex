//! Edit Component Details modal — full row editor invoked by
//! the "Edit Details" button in the Library Browser action row.
//!
//! `view_delete_confirm` lives in this module too.

use iced::widget::{
    Space, Widget, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Element, Length};
use signex_library::{ComponentClass, DatasheetRef, LifecycleState, ParameterTemplate};
use signex_types::object_id::short_id;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::messages::{BrowserEditMsg, LibraryMessage};
use super::modal_chrome::{
    cancel_btn, danger_btn, footer_strip, header, header_title_only, primary_btn, secondary_btn,
};
use super::state::{DeleteConfirmState, EditRowModalState};
use crate::styles::{DialogIcon, dialog_intro};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

const CONFIRM_W: f32 = 480.0;

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

/// `pick_list` adapter for the Lifecycle State dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LifecyclePick(LifecycleState);

impl std::fmt::Display for LifecyclePick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

const LIFECYCLE_OPTIONS: [LifecyclePick; 5] = [
    LifecyclePick(LifecycleState::Draft),
    LifecyclePick(LifecycleState::InReview),
    LifecyclePick(LifecycleState::Released),
    LifecyclePick(LifecycleState::Deprecated),
    LifecyclePick(LifecycleState::Obsolete),
];

pub fn view<'a>(
    library_path: &'a std::path::Path,
    edit: &'a EditRowModalState,
    tokens: &'a ThemeTokens,
    windowed: bool,
    classes: Vec<crate::fonts::ComponentClassEntry>,
    template: Option<&'a ParameterTemplate>,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let title = format!("Edit Component — {}", edit.draft.internal_pn.as_str());
    let header = header(
        &title,
        LibraryMessage::BrowserEdit {
            library_path: library_path.to_path_buf(),
            msg: BrowserEditMsg::Cancel,
        },
        windowed,
        tokens,
    );

    let lib_for_msg = library_path.to_path_buf();
    let send = move |m: BrowserEditMsg| LibraryMessage::BrowserEdit {
        library_path: lib_for_msg.clone(),
        msg: m,
    };

    let pn_input = text_input("Internal PN", edit.draft.internal_pn.as_str())
        .style(crate::panels::form_look::text_input_style)
        .on_input({
            let send = send.clone();
            move |s| send(BrowserEditMsg::SetInternalPn(s))
        })
        .padding(6)
        .size(TS.label);

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
        .style(crate::panels::form_look::pick_list_style)
        .on_select({
            let send = send.clone();
            move |pick: ClassPick| send(BrowserEditMsg::SetClass(ComponentClass::new(pick.key)))
        })
        .placeholder("Select class…")
        .padding(6)
        .text_size(12);

    let selected_state = LIFECYCLE_OPTIONS
        .iter()
        .find(|p| p.0 == edit.draft.state)
        .copied();
    let state_picker = pick_list(
        selected_state,
        LIFECYCLE_OPTIONS.to_vec(),
        LifecyclePick::to_string,
    )
    .style(crate::panels::form_look::pick_list_style)
    .on_select({
        let send = send.clone();
        move |pick: LifecyclePick| send(BrowserEditMsg::SetState(pick.0))
    })
    .placeholder("Select lifecycle…")
    .padding(6)
    .text_size(12);

    // Only a URL ref is editable here. A pinned PDF is shown read-only so a
    // keystroke can never replace it (and its hash) with a bare URL.
    let datasheet_field: Element<'a, LibraryMessage> = match &edit.draft.datasheet {
        DatasheetRef::Url { url } => text_input("https://example.com/datasheet.pdf", url)
            .style(crate::panels::form_look::text_input_style)
            .on_input({
                let send = send.clone();
                move |s| send(BrowserEditMsg::SetDatasheetUrl(s))
            })
            .padding(6)
            .size(TS.label)
            .boxed(),
        DatasheetRef::HashPinned { filename, .. } => text(format!("Pinned PDF: {filename}"))
            .size(TS.label)
            .color(muted)
            .boxed(),
    };

    let mfr_input = text_input("Manufacturer", &edit.draft.primary_mpn.manufacturer)
        .style(crate::panels::form_look::text_input_style)
        .on_input({
            let send = send.clone();
            move |s| send(BrowserEditMsg::SetManufacturer(s))
        })
        .padding(6)
        .size(TS.label);

    let mpn_input = text_input("MPN", &edit.draft.primary_mpn.mpn)
        .style(crate::panels::form_look::text_input_style)
        .on_input({
            let send = send.clone();
            move |s| send(BrowserEditMsg::SetMpn(s))
        })
        .padding(6)
        .size(TS.label);

    let description_input = text_input("e.g. 100 nF 50 V X7R 0603", &edit.draft.description)
        .style(crate::panels::form_look::text_input_style)
        .on_input({
            let send = send.clone();
            move |s| send(BrowserEditMsg::SetDescription(s))
        })
        .padding(6)
        .size(TS.label);

    // Tags row (Stage 18) — comma-separated free-form tokens.
    // The buffer flushes to `parameters["tags"]` on Save (see
    // `handle_browser_edit_msg`); empty buffer drops the entry.
    let tags_input = text_input("e.g. low-noise, RoHS, qualified", &edit.tags_buf)
        .style(crate::panels::form_look::text_input_style)
        .on_input({
            let send = send.clone();
            move |s| send(BrowserEditMsg::SetTags(s))
        })
        .padding(6)
        .size(TS.label);

    // Symbol / Footprint ref rows.
    let symbol_short = short_id(&edit.draft.symbol_ref.uuid);
    let symbol_label = if edit.draft.symbol_ref.uuid.is_nil() {
        "Unbound — pick a symbol".to_string()
    } else {
        format!("symbol uuid {}", symbol_short)
    };
    let symbol_btn = secondary_btn(
        "Pick Symbol…",
        send.clone()(BrowserEditMsg::OpenSymbolPicker),
        tokens,
    );

    let footprint_label = match edit.draft.footprint_ref.as_ref() {
        Some(r) => format!("footprint uuid {}", short_id(&r.uuid)),
        None => "Unbound (optional)".to_string(),
    };
    let footprint_btn = secondary_btn(
        "Pick Footprint…",
        send.clone()(BrowserEditMsg::OpenFootprintPicker),
        tokens,
    );

    let labelled =
        |lbl: &'a str, body: Element<'a, LibraryMessage>| -> Element<'a, LibraryMessage> {
            column![
                text(lbl).size(TS.caption).color(muted),
                container(body).padding([SP.xs, 0.0])
            ]
            .spacing(SP.xs)
            .boxed()
        };

    let pn_class_state_row = row![
        labelled("Internal PN", pn_input.boxed()),
        Space::new().width(8),
        labelled("Class", class_picker.boxed()),
        Space::new().width(8),
        labelled("State", state_picker.boxed()),
    ]
    .spacing(0);

    let mfr_mpn_row = row![
        labelled("Manufacturer", mfr_input.boxed()),
        Space::new().width(8),
        labelled("MPN", mpn_input.boxed()),
    ]
    .spacing(0);

    let symbol_row = row![
        text(symbol_label)
            .size(TS.body)
            .color(text_c)
            .width(Length::Fill),
        symbol_btn,
    ]
    .align_y(iced::Alignment::Center)
    .padding([SP.s, 0.0]);

    let footprint_row = row![
        text(footprint_label)
            .size(TS.body)
            .color(text_c)
            .width(Length::Fill),
        footprint_btn,
    ]
    .align_y(iced::Alignment::Center)
    .padding([SP.s, 0.0]);

    // Parameters table.
    let params_section = super::edit_row_params::view(edit, template, tokens, send.clone());

    let form_body = column![
        pn_class_state_row,
        Space::new().height(8),
        labelled("Datasheet", datasheet_field),
        Space::new().height(8),
        mfr_mpn_row,
        Space::new().height(8),
        labelled("Description", description_input.boxed()),
        Space::new().height(8),
        labelled("Tags (comma-separated)", tags_input.boxed()),
        Space::new().height(12),
        text("Symbol ref").size(TS.caption).color(muted),
        symbol_row,
        Space::new().height(6),
        text("Footprint ref").size(TS.caption).color(muted),
        footprint_row,
        Space::new().height(12),
        params_section,
    ]
    .spacing(0)
    .padding([14.0, SP.xl]);

    let body = container(
        scrollable(form_body)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill);

    let error_row: Element<'_, LibraryMessage> = if let Some(err) = edit.error.as_ref() {
        container(
            text(err.clone())
                .size(TS.body)
                .color(crate::styles::tokens::danger(tokens)),
        )
        .padding([SP.s, SP.xl])
        .boxed()
    } else if let Some(note) = edit.note.as_ref() {
        container(text(note.clone()).size(TS.body).color(muted))
            .padding([SP.s, SP.xl])
            .boxed()
    } else {
        Space::new().height(0).boxed()
    };

    // Offered once a save was refused because the row changed on the
    // server; discarding the draft asks first.
    let plain_btn =
        |label: &'static str, msg: BrowserEditMsg| secondary_btn(label, send.clone()(msg), tokens);
    let reload: Element<'_, LibraryMessage> = if !edit.stale_base {
        Space::new().width(0).boxed()
    } else if edit.reload_confirming {
        row![
            text("Discard your edits and reload?")
                .size(TS.body)
                .color(text_c),
            crate::styles::dialog_button(
                "Reload",
                crate::styles::DialogButtonKind::Danger,
                Some(send.clone()(BrowserEditMsg::ReloadConfirmed)),
                tokens,
            ),
            plain_btn("Keep editing", BrowserEditMsg::ReloadCancelled),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
    } else {
        plain_btn("Reload saved version", BrowserEditMsg::ReloadRequested)
    };

    let footer = footer_strip(
        vec![reload],
        vec![
            cancel_btn("Cancel", send.clone()(BrowserEditMsg::Cancel), tokens),
            primary_btn("Save", send(BrowserEditMsg::Save), tokens),
        ],
        tokens,
    );

    container(
        column![header, body, error_row, footer]
            .width(super::card_len(windowed, super::modal_chrome::EDIT_ROW_W))
            .height(super::card_len(windowed, super::modal_chrome::EDIT_ROW_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// The delete confirm's intro title and secondary text; the window height
/// follows their wrapped line counts (`app/handlers/modal_sizing.rs`).
pub(crate) fn delete_intro_texts(internal_pn: &str, table: &str) -> (String, String) {
    (
        format!("Delete {internal_pn}?"),
        format!("Delete row {internal_pn} from {table}? This cannot be undone from the UI."),
    )
}

/// Delete-row confirm modal — sibling overlay launched from the
/// Library Browser action row's Delete Selected button.
pub fn view_delete_confirm<'a>(
    library_path: &'a std::path::Path,
    confirm: &'a DeleteConfirmState,
    tokens: &'a ThemeTokens,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let header = header_title_only("Delete Component?", windowed, tokens);

    let (intro_title, msg_label) = delete_intro_texts(&confirm.internal_pn, &confirm.table);

    // As tall as its text: the footer follows it, with no empty band.
    let body = container(
        column![dialog_intro(
            DialogIcon::Warn,
            intro_title,
            Some(msg_label),
            tokens,
        )]
        .padding([14.0, SP.xl])
        .width(Length::Fill),
    )
    .width(Length::Fill);

    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn(
                "Cancel",
                LibraryMessage::BrowserDeleteRowCancel {
                    library_path: library_path.to_path_buf(),
                },
                tokens,
            ),
            danger_btn(
                "Delete",
                LibraryMessage::BrowserDeleteRowConfirm {
                    library_path: library_path.to_path_buf(),
                    table: confirm.table.clone(),
                    row_id: confirm.row_id,
                },
                tokens,
            ),
        ],
        tokens,
    );

    let mut card = column![header, body];
    if windowed {
        card = card.push(Space::new().height(Length::Fill).boxed());
    }
    container(
        card.push(footer)
            .width(super::card_len(windowed, CONFIRM_W))
            .height(if windowed {
                Length::Fill
            } else {
                Length::Shrink
            }),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}
