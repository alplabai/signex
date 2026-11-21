//! New Library dialog (mockup row 133): the version-control options of a
//! library about to be created. See docs/architecture/library-dialogs.md,
//! "New Library dialog".

use iced::font::Weight;
use iced::widget::{Space, column, container, row};
use iced::{Border, Element, Font, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::enable_section::dependents_veil;
use signex_widgets::inert::Inert;
use signex_widgets::{ToggleSwitchStyle, button, raw, text, toggle_switch, tooltip};

use super::messages::LibraryMessage;
use super::modal_chrome::{
    cancel_btn, footer_strip, header_with_note_closing, notice_banner, primary_btn_if,
};
use super::state::LibraryCreateOptionsState;
use crate::styles::tokens::{accent, on_accent, resolve};
use crate::styles::{PillTone, Radii, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// How far the LFS card sits in from the Git card (the mock's 24 px on the
/// spacing scale).
const LFS_INDENT: f32 = SP.xl + SP.m;
/// The mock's 10 px on the spacing scale: between the cards, between a
/// card's switch and its words, and a card's top and bottom padding.
const CARD_GAP: f32 = SP.m + SP.xs;

pub fn view<'a>(
    state: &'a LibraryCreateOptionsState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    // The full target path is the note, so the user sees where it lands.
    let header = header_with_note_closing(
        &i18n.message_plain(ids::DIALOG_TITLE_LIBRARY_OPTIONS, &[]),
        Some(state.lib_path.display().to_string()),
        LibraryMessage::LibraryCreateOptionsCancel,
        windowed,
        tokens,
    );

    let git = option_card(
        state.enable_git,
        t!(i18n, ids::LIBRARY_CREATE_OPTIONS_GIT),
        t!(i18n, ids::LIBRARY_CREATE_OPTIONS_GIT_HELP),
        LibraryMessage::LibraryCreateOptionsToggleGit,
        tokens,
    );
    let lfs = option_card(
        state.use_lfs,
        t!(i18n, ids::LIBRARY_CREATE_OPTIONS_LFS),
        t!(i18n, ids::LIBRARY_CREATE_OPTIONS_LFS_HELP),
        LibraryMessage::LibraryCreateOptionsToggleLfs,
        tokens,
    );
    // Disable-when-off: LFS stays visible, greyed and inert, until Git is
    // on (the handler clears it when Git turns off).
    let lfs = match dependents_veil(ti(tokens.paper), state.enable_git) {
        Some(veil) => Inert::new(lfs, veil).boxed(),
        None => lfs,
    };

    let mut body = column![git, row![Space::new().width(LFS_INDENT), lfs]].spacing(CARD_GAP);
    if let Some(reason) = &state.refusal {
        body = body.push(refusal_banner(reason, i18n, tokens));
    }

    let refused = state.refusal.is_some();
    let create = primary_btn_if(
        t!(i18n, ids::LIBRARY_CREATE_OPTIONS_CREATE),
        LibraryMessage::LibraryCreateOptionsConfirm,
        !refused,
        tokens,
    );
    let create = if refused {
        tooltip(
            create,
            iced::widget::container(
                container(text(t!(i18n, ids::LIBRARY_CREATE_OPTIONS_REFUSED_TIP)).size(TS.body))
                    .padding([SP.xs, SP.s]),
            )
            .style(crate::styles::context_menu(tokens)),
            tooltip::Position::Top,
        )
        .boxed()
    } else {
        create
    };

    let footer = footer_strip(
        Vec::new(),
        vec![
            cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                LibraryMessage::LibraryCreateOptionsCancel,
                tokens,
            ),
            create,
        ],
        tokens,
    );

    container(
        column![
            header,
            container(body).padding(SP.xl).width(Length::Fill),
            footer
        ]
        .width(super::card_len(
            windowed,
            super::modal_chrome::CREATE_OPTIONS_W,
        )),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// Weight 500 of the default UI family: an option card's title.
fn title_font() -> Font {
    Font {
        weight: Weight::Medium,
        ..Font::DEFAULT
    }
}

/// An option card: a hairline-bordered card holding a switch, a title and
/// a help caption. A press anywhere on the card sends `on_toggle`, as the
/// switch does.
fn option_card<'a>(
    is_on: bool,
    title: Localized,
    help: Localized,
    on_toggle: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let line = ti(tokens.border);
    let switch = toggle_switch(
        is_on,
        ToggleSwitchStyle::new(accent(tokens), r.line_strong, on_accent(tokens)),
        Some(on_toggle.clone()),
    );
    let words = column![
        text(title).size(TS.body).font(title_font()).color(ink),
        text(help).size(TS.caption).color(r.ink3),
    ]
    .spacing(SP.xs)
    .width(Length::Fill);
    button(
        row![switch, words]
            .spacing(CARD_GAP)
            .align_y(iced::Alignment::Start),
    )
    .width(Length::Fill)
    .padding([CARD_GAP, SP.l])
    .on_press(on_toggle)
    .style(move |_: &Theme, _| signex_widgets::button::Style {
        background: None,
        text_color: ink,
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..signex_widgets::button::Style::default()
    })
    .boxed()
}

/// The refusal as an error banner, with the way out: choose another
/// location (the save-as picker again).
fn refusal_banner<'a>(
    reason: &str,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    // raw: the library guard words its own refusal, in English.
    let body = text(raw(reason.to_owned())).size(TS.body).boxed();
    notice_banner(
        PillTone::Err,
        body,
        vec![small_btn(
            t!(i18n, ids::LIBRARY_CREATE_OPTIONS_RELOCATE),
            LibraryMessage::LibraryCreateOptionsRelocateRequested,
            tokens,
        )],
        tokens,
    )
}

/// A small bordered action inside a banner (the mock's `btn sm`).
fn small_btn<'a>(
    label: Localized,
    message: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let look = crate::styles::outline_button(tokens);
    button(text(label).size(TS.caption))
        .padding([SP.xs, SP.m])
        .on_press(message)
        .style(move |theme: &Theme, status| {
            let mut style = look(theme, status);
            style.border.radius = Radii::GRAPHITE.small.into();
            style
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::ThemeId;

    fn state(enable_git: bool, refusal: Option<&str>) -> LibraryCreateOptionsState {
        LibraryCreateOptionsState {
            project_path: "p.snxprj".into(),
            lib_path: "/work/libs/passives.snxlib".into(),
            enable_git,
            use_lfs: false,
            refusal: refusal.map(str::to_owned),
        }
    }

    /// The dialog builds in both Graphite themes, Git off and on, with and
    /// without a refusal, inline and windowed.
    #[test]
    fn the_view_builds_in_both_graphite_themes() {
        let i18n = I18n::en_us();
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = signex_types::theme::theme_tokens(id);
            for s in [
                state(false, None),
                state(true, None),
                state(true, Some("already has its own history")),
            ] {
                for windowed in [false, true] {
                    let _ = view(&s, &tokens, &i18n, windowed);
                }
            }
        }
    }

    /// A refusal grows the dialog's own window so the banner and its action
    /// stay above the footer; a longer reason grows it more.
    #[test]
    fn a_refusal_grows_the_window() {
        use crate::app::ModalId;
        let (mut app, _boot) = crate::app::Signex::new();
        app.library.create_options = Some(state(true, None));
        let plain = app.modal_window_size(ModalId::LibraryCreateOptions);
        app.library.create_options = Some(state(true, Some("short reason")));
        let short = app.modal_window_size(ModalId::LibraryCreateOptions);
        let long_reason = "a long reason ".repeat(40);
        app.library.create_options = Some(state(true, Some(&long_reason)));
        let long = app.modal_window_size(ModalId::LibraryCreateOptions);
        assert_eq!(plain.width, short.width);
        assert!(short.height > plain.height);
        assert!(long.height > short.height);
    }

    /// The dialog is titled New Library, in the window title too.
    #[test]
    fn the_dialog_is_titled_new_library() {
        let i18n = I18n::en_us();
        assert_eq!(
            i18n.message_plain(ids::DIALOG_TITLE_LIBRARY_OPTIONS, &[]),
            "New Library"
        );
    }
}
