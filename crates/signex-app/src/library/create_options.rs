//! New Library dialog (mockup row 133): the version-control options of a
//! library about to be created. See docs/architecture/library-dialogs.md,
//! "New Library dialog".

use iced::font::Weight;
use iced::widget::{Space, column, container, row};
use iced::{Border, Color, Element, Font, Length, Theme, Widget};
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
        true,
        t!(i18n, ids::LIBRARY_CREATE_OPTIONS_GIT),
        t!(i18n, ids::LIBRARY_CREATE_OPTIONS_GIT_HELP),
        LibraryMessage::LibraryCreateOptionsToggleGit,
        tokens,
    );
    let lfs = option_card(
        state.use_lfs,
        state.enable_git,
        t!(i18n, ids::LIBRARY_CREATE_OPTIONS_LFS),
        t!(i18n, ids::LIBRARY_CREATE_OPTIONS_LFS_HELP),
        LibraryMessage::LibraryCreateOptionsToggleLfs,
        tokens,
    );
    // Disable-when-off: LFS stays visible, greyed and inert, until Git is
    // on (the handler clears it when Git turns off). The veil dims the
    // card's quads; text draws above it, so the card greys its own words.
    let lfs = match dependents_veil(ti(tokens.paper), state.enable_git) {
        Some(veil) => Inert::new(lfs, veil).boxed(),
        None => lfs,
    };

    let mut body = column![git, row![Space::new().width(LFS_INDENT), lfs]].spacing(CARD_GAP);
    if let Some(reason) = &state.refusal {
        body = body.push(refusal_banner(reason, state.existing_library, i18n, tokens));
    }

    let refused = state.refusal.is_some();
    let create = primary_btn_if(
        t!(i18n, ids::LIBRARY_CREATE_OPTIONS_CREATE),
        LibraryMessage::LibraryCreateOptionsConfirm,
        !refused,
        tokens,
    );
    let create = if refused {
        let tip = if state.existing_library {
            ids::LIBRARY_CREATE_OPTIONS_EXISTS_TIP
        } else if state.path_refused {
            ids::LIBRARY_CREATE_OPTIONS_PATH_TIP
        } else {
            ids::LIBRARY_CREATE_OPTIONS_REFUSED_TIP
        };
        tooltip(
            create,
            container(text(t!(i18n, tip)).size(TS.body)).padding([SP.xs, SP.s]),
            tooltip::Position::Top,
        )
        .style(crate::styles::context_menu(tokens))
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
        ..signex_widgets::ui_font::regular()
    }
}

/// The title and help inks of an option card; a card that is not live has
/// both in the faintest ink.
fn option_inks(is_live: bool, tokens: &ThemeTokens) -> (Color, Color) {
    let r = resolve(tokens);
    if is_live {
        (ti(tokens.text), r.ink3)
    } else {
        (r.ink4, r.ink4)
    }
}

/// An option card: a hairline-bordered card holding a switch, a title and
/// a help caption. A press anywhere on the card sends `on_toggle`, as the
/// switch does.
fn option_card<'a>(
    is_on: bool,
    is_live: bool,
    title: Localized,
    help: Localized,
    on_toggle: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let (ink, help_ink) = option_inks(is_live, tokens);
    let line = ti(tokens.border);
    let switch = toggle_switch(
        is_on,
        ToggleSwitchStyle::new(accent(tokens), r.line_strong, on_accent(tokens)),
        Some(on_toggle.clone()),
    );
    let words = column![
        text(title).size(TS.body).font(title_font()).color(ink),
        text(help).size(TS.caption).color(help_ink),
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

/// The refusal as an error banner, with the ways out: choose another
/// location (the save-as picker again) and, when a library is already
/// there, open it.
fn refusal_banner<'a>(
    reason: &str,
    existing_library: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    // raw: the reason is already worded, by the handler or the library guard.
    let body = text(raw(reason.to_owned())).size(TS.body).boxed();
    let mut actions = vec![small_btn(
        t!(i18n, ids::LIBRARY_CREATE_OPTIONS_RELOCATE),
        LibraryMessage::LibraryCreateOptionsRelocateRequested,
        tokens,
    )];
    if existing_library {
        actions.push(small_btn(
            t!(i18n, ids::LIBRARY_CREATE_OPTIONS_OPEN_EXISTING),
            LibraryMessage::LibraryCreateOptionsOpenExistingRequested,
            tokens,
        ));
    }
    notice_banner(PillTone::Err, body, actions, tokens)
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
            existing_library: false,
            path_refused: false,
        }
    }

    /// Mockup row 133: while Git is off the LFS card's title and help are
    /// in the faintest ink, in both themes; a live card keeps its inks.
    #[test]
    fn a_card_that_is_not_live_greys_its_words() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = signex_types::theme::theme_tokens(id);
            let r = resolve(&tokens);
            assert_eq!(option_inks(false, &tokens), (r.ink4, r.ink4), "{id:?}");
            assert_eq!(
                option_inks(true, &tokens),
                (ti(tokens.text), r.ink3),
                "{id:?}"
            );
        }
    }

    /// The banner for a library already at the path builds, in both themes.
    #[test]
    fn the_view_builds_with_a_library_already_at_the_path() {
        let i18n = I18n::en_us();
        let refused = LibraryCreateOptionsState {
            existing_library: true,
            ..state(false, Some("A library already exists"))
        };
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = signex_types::theme::theme_tokens(id);
            let _ = view(&refused, &tokens, &i18n, false);
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
