//! Edit Component modal (mockup row 130): the row editor the Library
//! Browser's "Edit Details" opens. Identity and bindings on the left, the
//! parameters on the right; Save waits while the draft has a problem (lib-07:
//! a field error outline, an error banner at the foot of the Parameters
//! column, and a footer summary). Update refuses that Save too.
//!
//! The delete-row confirm (mockup row 137) lives here too, in `delete_confirm`.

mod bindings;
mod checks;
mod delete_confirm;
mod identity;
#[cfg(test)]
mod layout_tests;

use iced::widget::{Space, column, container, row, scrollable};
use iced::{Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_library::ParameterTemplate;
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use super::messages::{BrowserEditMsg, LibraryMessage};
use super::modal_chrome::{
    cancel_btn, footer_strip, header_with_note_closing, notice_banner, primary_btn_if,
    secondary_btn,
};
use super::state::{EditRowModalState, LibraryState};

pub use bindings::Binding;
pub(crate) use checks::pn_conflict;
pub use delete_confirm::view_delete_confirm;
pub(crate) use delete_confirm::{DELETE_CAPTION_GAP, delete_intro_texts};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of the Identity / Bindings column; the Parameters column takes
/// the rest of the card.
const LEFT_COLUMN_W: f32 = 380.0;
/// Padding of both columns (14 / 16 px at the default size).
const COLUMN_PAD: [f32; 2] = [SP.l + SP.xs, SP.xl];

/// What the modal shows beyond its own state, read from the Model by the
/// caller.
pub struct EditRowContext<'a> {
    /// The classes the Class dropdown offers.
    pub classes: Vec<crate::fonts::ComponentClassEntry>,
    /// The class template of the draft's class.
    pub template: Option<&'a ParameterTemplate>,
    /// Display name of the library the row lives in.
    pub library_name: Option<&'a str>,
    pub symbol: Binding<'a>,
    pub footprint: Binding<'a>,
    /// The table of another row that already holds the draft's Internal PN.
    pub pn_conflict: Option<String>,
}

impl<'a> EditRowContext<'a> {
    /// Resolve the context of the modal editing `edit` in the library at
    /// `path`.
    pub fn resolve(
        lib: &'a LibraryState,
        path: &std::path::Path,
        edit: &EditRowModalState,
        classes: Vec<crate::fonts::ComponentClassEntry>,
    ) -> Self {
        let open = lib.library_at(path);
        let draft = &edit.draft;
        Self {
            classes,
            template: lib.row_template(path, draft.class.as_str()),
            library_name: open.map(|l| l.display_name.as_str()),
            symbol: Binding::resolve(lib, Some(&draft.symbol_ref), |l| {
                l.cached_symbols.as_slice()
            }),
            footprint: Binding::resolve(lib, draft.footprint_ref.as_ref(), |l| {
                l.cached_footprints.as_slice()
            }),
            pn_conflict: open
                .and_then(|l| pn_conflict(l, draft.internal_pn.as_str(), edit.address.row_id)),
        }
    }
}

/// A section heading inside a column: caption size, faint ink; the catalog
/// writes it in capitals.
fn mini_header<'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    text(label)
        .size(TS.caption)
        .color(crate::styles::tokens::resolve(tokens).ink4)
        .boxed()
}

pub fn view<'a>(
    library_path: &'a std::path::Path,
    edit: &'a EditRowModalState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
    context: EditRowContext<'a>,
) -> Element<'a, LibraryMessage> {
    let lib_for_msg = library_path.to_path_buf();
    let send = move |m: BrowserEditMsg| LibraryMessage::BrowserEdit {
        library_path: lib_for_msg.clone(),
        msg: m,
    };

    let title = t!(i18n, ids::LIBRARY_EDIT_ROW_TITLE).into_string();
    let note = t!(
        i18n,
        ids::LIBRARY_EDIT_ROW_NOTE,
        "pn" => edit.draft.internal_pn.as_str(),
        "library" => context.library_name.unwrap_or_default(),
        "table" => edit.address.table.as_str()
    )
    .into_string();
    let header = header_with_note_closing(
        &title,
        Some(note),
        send(BrowserEditMsg::Cancel),
        windowed,
        tokens,
    );

    let binding_texts =
        |title: MessageId, unbound: MessageId, pick: MessageId| bindings::CardTexts {
            title: t!(i18n, title),
            unbound: t!(i18n, unbound),
            pick: t!(i18n, pick),
        };
    let left = column![
        mini_header(t!(i18n, ids::LIBRARY_EDIT_ROW_IDENTITY), tokens),
        identity::view(
            edit,
            context.classes.clone(),
            context.pn_conflict.is_some(),
            tokens,
            i18n,
            send.clone(),
        ),
        container(mini_header(
            t!(i18n, ids::LIBRARY_EDIT_ROW_BINDINGS),
            tokens
        ))
        .padding(iced::Padding::ZERO.top(SP.s + SP.xs)),
        bindings::card(
            context.symbol,
            binding_texts(
                ids::LIBRARY_EDIT_ROW_SYMBOL_REF,
                ids::LIBRARY_EDIT_ROW_SYMBOL_UNBOUND,
                ids::PANELS_LIBRARY_ROW_PICK_SYMBOL,
            ),
            send(BrowserEditMsg::OpenSymbolPicker),
            tokens,
            i18n,
        ),
        bindings::card(
            context.footprint,
            binding_texts(
                ids::LIBRARY_EDIT_ROW_FOOTPRINT_REF,
                ids::LIBRARY_EDIT_ROW_FOOTPRINT_UNBOUND,
                ids::PANELS_LIBRARY_ROW_PICK_FOOTPRINT,
            ),
            send(BrowserEditMsg::OpenFootprintPicker),
            tokens,
            i18n,
        ),
    ]
    .spacing(SP.m + SP.xs)
    .padding(COLUMN_PAD);

    let right = super::edit_row_params::view(
        edit,
        context.template,
        class_label(&context),
        tokens,
        i18n,
        send.clone(),
    );

    let line = theme_ext::border_color(tokens);
    let separator = container(Space::new())
        .width(crate::styles::HAIRLINE)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(iced::Background::Color(line)),
            ..container::Style::default()
        });
    // The Internal PN conflict is the error banner pinned to the foot of
    // the Parameters column, under the scrolling parameters (mockup row 130).
    let mut right_column = column![
        scrollable(container(right).padding(COLUMN_PAD).width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    ]
    .width(Length::Fill)
    .height(Length::Fill);
    if let Some(table) = context.pn_conflict.as_deref() {
        right_column = right_column.push(
            container(pn_conflict_banner(edit, table, tokens, i18n))
                .padding(iced::Padding {
                    top: 0.0,
                    right: COLUMN_PAD[1],
                    bottom: COLUMN_PAD[0],
                    left: COLUMN_PAD[1],
                })
                .boxed(),
        );
    }
    let body = row![
        scrollable(left)
            .width(Length::Fixed(LEFT_COLUMN_W))
            .height(Length::Fill),
        separator,
        right_column,
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    let footer = footer_strip(
        vec![
            reload_control(edit, tokens, i18n, &send),
            summary(edit, &context, tokens, i18n),
        ],
        vec![
            cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                send(BrowserEditMsg::Cancel),
                tokens,
            ),
            primary_btn_if(
                t!(i18n, ids::DIALOG_BUTTON_SAVE),
                send(BrowserEditMsg::Save),
                save_enabled(&context),
                tokens,
            ),
        ],
        tokens,
    );

    container(
        column![header, body, footer]
            .width(super::card_len(windowed, super::modal_chrome::EDIT_ROW_W))
            .height(super::card_len(windowed, super::modal_chrome::EDIT_ROW_H)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// The err banner (warn glyph, then the reason) for an Internal PN that
/// another row in `table` already holds.
fn pn_conflict_banner<'a>(
    edit: &EditRowModalState,
    table: &str,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let reason = text(t!(
        i18n,
        ids::LIBRARY_EDIT_ROW_PN_CONFLICT,
        "pn" => edit.draft.internal_pn.as_str().trim(),
        "table" => table
    ))
    .size(TS.body)
    .boxed();
    notice_banner(crate::styles::PillTone::Err, reason, Vec::new(), tokens)
}

/// Save waits while the draft has a problem the modal can see; the
/// backend's own refusal stays the backstop for the rest.
fn save_enabled(context: &EditRowContext<'_>) -> bool {
    context.pn_conflict.is_none()
}

/// The display label of the template's class, else its key.
fn class_label(context: &EditRowContext<'_>) -> Option<String> {
    let class = context.template?.class.as_str();
    Some(
        context
            .classes
            .iter()
            .find(|c| c.key == class)
            .map_or_else(|| class.to_string(), |c| c.label.clone()),
    )
}

/// "Reload saved version": a quiet action at the footer's left at all
/// times, the conflict state included (mockup row 130). Discarding the
/// draft asks first, inline.
fn reload_control<'a>(
    edit: &EditRowModalState,
    tokens: &ThemeTokens,
    i18n: &I18n,
    send: &impl Fn(BrowserEditMsg) -> LibraryMessage,
) -> Element<'a, LibraryMessage> {
    if !edit.reload_confirming {
        return crate::styles::dialog_icon_button(
            crate::icons::icon_chrome_refresh(signex_types::theme::ThemeId::GraphiteDark),
            t!(i18n, ids::LIBRARY_EDIT_ROW_RELOAD_SAVED),
            crate::styles::DialogButtonKind::Ghost,
            Some(send(BrowserEditMsg::ReloadRequested)),
            tokens,
        );
    }
    row![
        text(t!(i18n, ids::LIBRARY_EDIT_ROW_RELOAD_CONFIRM))
            .size(TS.body)
            .color(theme_ext::text_primary(tokens)),
        crate::styles::dialog_button(
            t!(i18n, ids::LIBRARY_EDIT_ROW_RELOAD),
            crate::styles::DialogButtonKind::Danger,
            Some(send(BrowserEditMsg::ReloadConfirmed)),
            tokens,
        ),
        secondary_btn(
            t!(i18n, ids::LIBRARY_EDIT_ROW_KEEP_EDITING),
            send(BrowserEditMsg::ReloadCancelled),
            tokens,
        ),
    ]
    .spacing(SP.s)
    .align_y(iced::Alignment::Center)
    .boxed()
}

/// The footer summary: the problems that hold Save, else the reason the
/// last save failed. Error-toned; nothing when there is neither.
fn summary<'a>(
    edit: &EditRowModalState,
    context: &EditRowContext<'_>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let line = if context.pn_conflict.is_some() {
        let fields = t!(i18n, ids::LIBRARY_EDIT_ROW_INTERNAL_PN).into_string();
        text(t!(
            i18n,
            ids::LIBRARY_EDIT_ROW_PROBLEMS,
            "count" => 1,
            "fields" => fields
        ))
    } else if let Some(err) = edit.error.as_ref() {
        text(UserText::new(err.clone()))
    } else {
        return Space::new().width(0).boxed();
    };
    // Fill: a long save error wraps in the footer's free space instead of
    // pushing the buttons out.
    line.size(TS.body)
        .color(theme_ext::error_color(tokens))
        .width(Length::Fill)
        .boxed()
}
