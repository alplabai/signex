//! "Copy into library?" (mockup row 131, decision lib-10): the card that
//! takes the picker's place while a chosen symbol outside the row's library
//! waits for an answer (#1841). See docs/architecture/library-dialogs.md,
//! "Copy into library confirm".

use iced::widget::{Space, column, container};
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::list::lib_name;
use crate::library::loose_symbols::LooseSymbol;
use crate::library::messages::{LibraryMessage, PrimitivePickerMsg};
use crate::library::modal_chrome::{
    footer_strip, header, header_title_only, primary_btn_if, secondary_btn,
};
use crate::library::state::{LibraryState, PrimitivePickerState};
use crate::styles::{DialogButtonKind, DialogIcon, dialog_body, dialog_button, dialog_intro};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of the card.
pub(super) const COPY_CONFIRM_W: f32 = 480.0;

fn msg(m: PrimitivePickerMsg) -> LibraryMessage {
    LibraryMessage::PrimitivePicker(m)
}

/// The name of the library the row lives in, as the buttons and the
/// question name it.
fn target_name(state: &LibraryState, picker: &PrimitivePickerState, i18n: &I18n) -> String {
    picker
        .target
        .library_path()
        .and_then(|path| state.library_at(path))
        .map_or_else(
            || i18n.message_plain(ids::LIBRARY_PRIMITIVE_PICKER_THIS_LIBRARY, &[]),
            |lib| lib.display_name.clone(),
        )
}

pub(super) fn view<'a>(
    state: &'a LibraryState,
    picker: &'a PrimitivePickerState,
    symbol: &'a LooseSymbol,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let target = target_name(state, picker, i18n);
    let owner = symbol.library.as_deref().map(lib_name);
    let busy = picker.copying;

    // While the copy runs there is nothing to go back to: the copy binds
    // when it lands.
    let title = i18n.message_plain(ids::LIBRARY_PRIMITIVE_PICKER_COPY_TITLE, &[]);
    let header = if busy {
        header_title_only(&title, windowed, tokens)
    } else {
        header(
            &title,
            msg(PrimitivePickerMsg::CopyDeclined),
            windowed,
            tokens,
        )
    };

    let name = symbol.name.as_str();
    let mut body: iced::widget::Column<Element<'a, LibraryMessage>> = column![].spacing(SP.m);
    match owner.as_deref() {
        None => {
            let question = t!(
                i18n,
                ids::LIBRARY_PRIMITIVE_PICKER_COPY_LOOSE,
                "name" => name,
                "library" => target.as_str()
            );
            let detail = t!(
                i18n,
                ids::LIBRARY_PRIMITIVE_PICKER_COPY_LOOSE_DETAIL,
                "library" => target.as_str()
            );
            body = body.push(dialog_intro(
                DialogIcon::Copy,
                question.into_string(),
                Some(detail.into_string()),
                tokens,
            ));
        }
        Some(owner) => {
            let question = t!(
                i18n,
                ids::LIBRARY_PRIMITIVE_PICKER_COPY_HELD,
                "name" => name,
                "library" => owner
            );
            body = body.push(dialog_intro(
                DialogIcon::Copy,
                question.into_string(),
                None,
                tokens,
            ));
            let caption = t!(
                i18n,
                ids::LIBRARY_PRIMITIVE_PICKER_LINK_CAPTION,
                "library" => owner
            );
            body = body.push(text(caption).size(TS.caption).color(r.ink4).boxed());
        }
    }

    let cancel = dialog_button(
        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
        DialogButtonKind::Cancel,
        (!busy).then(|| msg(PrimitivePickerMsg::CopyDeclined)),
        tokens,
    );
    let footer = if picker.opening_library.is_some() {
        // The other library is being opened for the link; Cancel drops the
        // wait and the link.
        let opening = text(t!(i18n, ids::LIBRARY_PRIMITIVE_PICKER_OPENING))
            .size(TS.caption)
            .color(r.ink4)
            .boxed();
        footer_strip(vec![opening], vec![cancel], tokens)
    } else {
        let copy_label = if busy {
            t!(i18n, ids::LIBRARY_PRIMITIVE_PICKER_COPYING)
        } else {
            t!(
                i18n,
                ids::LIBRARY_PRIMITIVE_PICKER_COPY_INTO,
                "library" => target.as_str()
            )
        };
        let mut buttons = vec![cancel];
        if owner.is_some() && !busy {
            buttons.push(secondary_btn(
                t!(i18n, ids::LIBRARY_PRIMITIVE_PICKER_LINK),
                msg(PrimitivePickerMsg::OpenLibraryConfirmed),
                tokens,
            ));
        }
        buttons.push(primary_btn_if(
            copy_label,
            msg(PrimitivePickerMsg::CopyConfirmed),
            !busy,
            tokens,
        ));
        footer_strip(Vec::new(), buttons, tokens)
    };

    let mut card: iced::widget::Column<Element<'a, LibraryMessage>> =
        column![header, dialog_body(body)];
    // A detached window is sized for the picker: the footer stays at the
    // bottom of it.
    if windowed {
        card = card.push(Space::new().height(Length::Fill).boxed());
    }
    container(
        card.push(footer)
            .width(crate::library::card_len(windowed, COPY_CONFIRM_W))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::state::PrimitivePickerTarget;
    use signex_library::{PrimitiveKind, RowId};
    use signex_types::theme::ThemeId;

    fn picker(symbol: &LooseSymbol) -> PrimitivePickerState {
        PrimitivePickerState {
            kind: PrimitiveKind::Symbol,
            target: PrimitivePickerTarget::BrowserRow(crate::library::state::EditorAddress::new(
                "/libs/passives.snxlib".into(),
                "generics".into(),
                RowId::from_uuid(uuid::Uuid::nil()),
            )),
            filter: String::new(),
            error: None,
            scroll_y: 0.0,
            loose: vec![symbol.clone()],
            copy_confirm: Some(symbol.clone()),
            copying: false,
            opening_library: None,
            selection: Default::default(),
        }
    }

    /// The card builds in each of its states, both symbol cases, on every
    /// built-in theme (Graphite Light and Dark included).
    #[test]
    fn the_card_builds_in_every_state_and_theme() {
        let i18n = I18n::en_us_ref();
        let state = LibraryState::default();
        for library in [None, Some("/vendor/power-ics.snxlib".into())] {
            let symbol = LooseSymbol {
                file: "/work/sense.snxsym".into(),
                uuid: uuid::Uuid::nil(),
                name: "R_Shunt".into(),
                library,
            };
            for (copying, opening) in [(false, false), (true, false), (false, true)] {
                let mut p = picker(&symbol);
                p.copying = copying;
                p.opening_library = opening.then(|| "/vendor/power-ics.snxlib".into());
                for id in ThemeId::BUILTINS {
                    let tokens = signex_types::theme::theme_tokens(*id);
                    for windowed in [false, true] {
                        let _ = view(&state, &p, &symbol, &tokens, i18n, windowed);
                    }
                }
            }
        }
    }

    /// The question and the buttons name the libraries (lib-10).
    #[test]
    fn the_words_name_both_libraries() {
        let i18n = I18n::en_us_ref();
        let plain = |id, args: &[(&str, signex_i18n::Arg)]| i18n.message_plain(id, args);
        assert_eq!(
            plain(
                ids::LIBRARY_PRIMITIVE_PICKER_COPY_LOOSE,
                &[("name", "R_Shunt".into()), ("library", "passives".into())]
            ),
            "Copy R_Shunt into library passives?"
        );
        assert_eq!(
            plain(
                ids::LIBRARY_PRIMITIVE_PICKER_COPY_HELD,
                &[("name", "R_Shunt".into()), ("library", "power-ics".into())]
            ),
            "R_Shunt is in library power-ics, which is not open."
        );
        assert_eq!(
            plain(
                ids::LIBRARY_PRIMITIVE_PICKER_LINK_CAPTION,
                &[("library", "power-ics".into())]
            ),
            "A link breaks if power-ics is not open."
        );
        assert_eq!(
            plain(
                ids::LIBRARY_PRIMITIVE_PICKER_COPY_INTO,
                &[("library", "passives".into())]
            ),
            "Copy into passives"
        );
        assert_eq!(
            plain(ids::LIBRARY_PRIMITIVE_PICKER_LINK, &[]),
            "Link without copying"
        );
    }
}
