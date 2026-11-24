//! The reset confirm (#76). It serves both footer buttons: the title and
//! the danger button follow the reset asked for (Reset All / Reset &
//! Renumber). The detail names the scope, and the warn banner says how undo
//! works, in the same words as the dialog footer, with the shortcut as a
//! key cap. Its glyph leads the banner, and the buttons sit on the footer
//! band under a rule (mockup row 76).

use super::super::*;
use iced::widget::text::Span;
use iced::widget::{column, container};
use iced::{Element, Length, Padding, Widget};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{rich_text, span};

use super::super::board_output::kit;
use super::super::widgets::{draggable_header, modal_title_header, os_title_bar_slot};
use super::{SP, TS};
use crate::styles::{DialogButtonKind, PillTone, dialog_body, dialog_button, dialog_footer_band};

impl Signex {
    pub(in crate::app::view) fn view_annotate_reset_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_annotate_reset_confirm_body_inner(true);
        self.wrap_dialog(
            crate::app::state::ModalId::AnnotateResetConfirm,
            dialog,
            super::super::sizes::ANNOTATE_RESET_CONFIRM,
        )
    }

    pub(in crate::app::view) fn view_annotate_reset_confirm_body(&self) -> Element<'_, Message> {
        self.view_annotate_reset_confirm_body_inner(false)
    }

    fn view_annotate_reset_confirm_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let ink = crate::styles::ti(tokens.text);
        let session = &self.ui_state.annotate_session;
        let mode = session.reset_mode;
        let renumber = session.resets_and_renumbers();
        let (title, action) = (session.reset_title(), session.reset_action());

        let header_content: Element<'_, Message> = modal_title_header(
            t!(i18n, title).into_string(),
            Message::Annotate(AnnotateMsg::CloseResetConfirm),
            tokens,
        );
        let header = if draggable {
            draggable_header(
                header_content,
                crate::app::state::ModalId::AnnotateResetConfirm,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        let yes_no = if renumber { "yes" } else { "no" };
        let scope = self.ui_state.annotate_preview.reset_scope(
            &self.ui_state.annotate_session.unticked,
            &self.ui_state.annotate_locked,
        );
        let detail = match scope {
            Some(scope) => t!(
                i18n,
                ids::DIALOG_ANNOTATE_RESET_DETAIL,
                "sheets" => scope.sheets,
                "symbols" => scope.symbols,
                "renumber" => yes_no
            ),
            None => t!(i18n, ids::DIALOG_ANNOTATE_RESET_DETAIL_LOADING, "renumber" => yes_no),
        };
        // AV-05: one statement of undo, the same as the dialog footer's.
        let undo_id = if self.annotate_undo_is_one_step() {
            ids::DIALOG_ANNOTATE_RESET_ONE_STEP
        } else {
            ids::DIALOG_ANNOTATE_RESET_PER_SHEET
        };
        let shortcut = crate::keymap::shown_in("Mod+Z", i18n);
        let undo = t!(i18n, undo_id, "undo" => shortcut.clone());
        let banner = kit::banner(
            PillTone::Warn,
            undo_caveat(undo, &shortcut, ink, tokens),
            None,
            tokens,
        );

        let body = column![
            crate::styles::dialog_intro(
                crate::styles::DialogIcon::Warn,
                t!(i18n, ids::DIALOG_ANNOTATE_RESET_QUESTION).into_string(),
                Some(detail.into_string()),
                tokens,
            ),
            // Under the text, clear of the icon tile, as in the mock.
            container(banner).padding(iced::Padding {
                left: crate::styles::DIALOG_ICON_TILE + SP.m + SP.xs,
                ..iced::Padding::ZERO
            }),
        ]
        .spacing(SP.m);

        let run = (!self.ui_state.annotate_runs.is_running())
            .then_some(Message::Annotate(AnnotateMsg::Run(mode)));
        let dialog = container(
            column![header, dialog_body(body), footer(action, run, i18n, tokens),].width(
                self.dialog_width(
                    crate::app::state::ModalId::AnnotateResetConfirm,
                    super::super::sizes::ANNOTATE_RESET_CONFIRM,
                ),
            ),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}

/// The confirm's footer: Cancel, then the reset asked for. There is no
/// "Save all first": a reset writes no file and one undo restores every
/// sheet, so saving first protects nothing (mockup row 76, Caner
/// 2026-11-24).
fn footer_buttons(action: MessageId) -> [(MessageId, DialogButtonKind, FooterAction); 2] {
    [
        (
            ids::DIALOG_BUTTON_CANCEL,
            DialogButtonKind::Cancel,
            FooterAction::Close,
        ),
        (action, DialogButtonKind::DangerSolid, FooterAction::Reset),
    ]
}

/// The footer band under its rule, with [`footer_buttons`] on the right.
/// `run` is the reset's message, `None` while a run is going.
fn footer<'a>(
    action: MessageId,
    run: Option<Message>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let buttons = footer_buttons(action)
        .into_iter()
        .map(|(label, kind, does)| {
            let on_press = match does {
                FooterAction::Reset => run.clone(),
                FooterAction::Close => Some(Message::Annotate(AnnotateMsg::CloseResetConfirm)),
            };
            dialog_button(t!(i18n, label), kind, on_press, tokens)
        })
        .collect();
    dialog_footer_band(
        Vec::new(),
        buttons,
        Padding::from([SP.m + SP.xs, SP.xl]),
        tokens,
    )
}

/// What a footer button of the confirm does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FooterAction {
    /// Closes the confirm and resets nothing.
    Close,
    /// Runs the reset asked for.
    Reset,
}

/// The undo caveat as one wrapping paragraph, its shortcut drawn as a key
/// cap where the sentence names it (mockup row 76). A translation that
/// leaves the shortcut out shows as plain words.
fn undo_caveat<'a>(
    undo: Localized,
    shortcut: &str,
    ink: iced::Color,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let spans = match undo.split_around(shortcut) {
        Some((before, cap, after)) => vec![
            span::<()>(before).color(ink),
            key_cap_span(cap, tokens),
            span::<()>(after).color(ink),
        ],
        None => vec![span::<()>(undo).color(ink)],
    };
    rich_text::<(), Message, iced::Theme>(spans)
        .size(TS.caption)
        .wrapping(iced::widget::text::Wrapping::Word)
        .width(Length::Fill)
        .boxed()
}

/// A shortcut inside a sentence on the key-cap face (`styles::key_cap`, the
/// face of every key cap) in the mono font. The padding only grows the
/// face, so it stays inside the spaces around the shortcut.
fn key_cap_span<'a>(cap: Localized, tokens: &ThemeTokens) -> Span<'a, ()> {
    // The key-cap style reads only the tokens, not the theme.
    let face = crate::styles::key_cap(tokens)(&iced::Theme::Light);
    span::<()>(cap)
        .font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ))
        .color_maybe(face.text_color)
        .background_maybe(face.background)
        .border(face.border)
        .padding([TS.caption * 0.1, TS.caption * 0.25])
}

#[cfg(test)]
mod tests {
    use signex_i18n::{I18n, ids, t};

    use crate::app::Signex;

    /// Mockup row 76: both undo caveats name the shortcut where the key cap
    /// goes, so neither falls back to plain words.
    #[test]
    fn both_undo_caveats_carry_the_shortcut_as_a_key_cap() {
        let i18n = I18n::en_us();
        let shortcut = crate::keymap::shown_in("Mod+Z", &i18n);
        for id in [
            ids::DIALOG_ANNOTATE_RESET_ONE_STEP,
            ids::DIALOG_ANNOTATE_RESET_PER_SHEET,
        ] {
            let undo = t!(i18n, id, "undo" => shortcut.clone());
            let (_, cap, _) = undo.split_around(&shortcut).expect("the shortcut");
            assert_eq!(cap.into_string(), shortcut, "{id:?}");
        }
    }

    /// Mockup row 76 (Caner 2026-11-24): the confirm offers Cancel and the
    /// reset only, with no "Save all first", for both resets.
    #[test]
    fn the_reset_confirm_offers_cancel_and_the_reset_only() {
        use super::{FooterAction, footer_buttons};
        use crate::styles::DialogButtonKind;
        for action in [
            ids::DIALOG_ANNOTATE_RESET_ALL,
            ids::DIALOG_ANNOTATE_RESET_RENUMBER,
        ] {
            assert_eq!(
                footer_buttons(action),
                [
                    (
                        ids::DIALOG_BUTTON_CANCEL,
                        DialogButtonKind::Cancel,
                        FooterAction::Close
                    ),
                    (action, DialogButtonKind::DangerSolid, FooterAction::Reset),
                ]
            );
        }
    }

    /// The confirm with its key-cap banner builds.
    #[test]
    fn the_reset_confirm_builds() {
        let (app, _task) = Signex::new();
        let _ = app.view_annotate_reset_confirm_body();
    }

    use iced::{Color, Size};
    use signex_types::theme::{ThemeId, theme_tokens};

    use super::{SP, TS};
    use crate::layout_probe::{LaidOutText, draw_pixels, find, lay_out_texts};

    /// The window the confirm is laid out and drawn in.
    const WINDOW: Size = Size::new(TS.body * 60.0, TS.body * 40.0);

    /// The confirm on the `id` theme: its laid-out texts and its pixels.
    fn drawn(id: ThemeId) -> (Signex, Vec<LaidOutText>, Vec<u8>) {
        let (mut app, _task) = Signex::new();
        app.document_state.panel_ctx.tokens = theme_tokens(id);
        let texts = lay_out_texts(app.view_annotate_reset_confirm_body(), WINDOW);
        let pixels = draw_pixels(app.view_annotate_reset_confirm_body(), WINDOW);
        (app, texts, pixels)
    }

    fn pixel(pixels: &[u8], x: f32, y: f32) -> [u8; 3] {
        let i = (y as usize * WINDOW.width as usize + x as usize) * 4;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    }

    fn near(got: [u8; 3], want: Color, tolerance: u8) -> bool {
        let [r, g, b, _] = want.into_rgba8();
        got.iter()
            .zip([r, g, b])
            .all(|(g, w)| g.abs_diff(w) <= tolerance)
    }

    const THEMES: [ThemeId; 2] = [ThemeId::GraphiteLight, ThemeId::GraphiteDark];

    /// Mockup row 76: the reset is the solid danger button, so the fill
    /// left of its label is the danger colour, not a pale tint.
    #[test]
    fn the_reset_button_is_drawn_solid_danger() {
        for id in THEMES {
            let (app, texts, pixels) = drawn(id);
            let i18n = app.ui_state.i18n.as_ref();
            let words = t!(i18n, app.ui_state.annotate_session.reset_action()).to_string();
            // A dialog button's label fills the button, centred in it: its
            // left edge is the button's, and the label's ink starts later.
            let label = find(&texts, &words).bounds;
            let beside = pixel(&pixels, label.x + SP.s, label.center_y());
            let danger = crate::styles::tokens::danger(&app.document_state.panel_ctx.tokens);
            assert!(near(beside, danger, 2), "{id:?}: {beside:?}");
        }
    }

    /// Mockup row 76: the buttons sit on the footer band, its chrome fill
    /// reaching the dialog's left edge beside them.
    #[test]
    fn the_buttons_sit_on_the_footer_band() {
        for id in THEMES {
            let (app, texts, pixels) = drawn(id);
            let tokens = &app.document_state.panel_ctx.tokens;
            let cancel = find(&texts, "Cancel").bounds;
            let left = pixel(&pixels, SP.xl / 2.0, cancel.center_y());
            let band = crate::styles::ti(tokens.toolbar_bg);
            assert!(near(left, band, 2), "{id:?}: {left:?}");
            if band != crate::styles::ti(tokens.paper) {
                assert!(!near(left, crate::styles::ti(tokens.paper), 2), "{id:?}");
            }
        }
    }

    /// Mockup row 76: the warn banner opens with its glyph in the warn ink,
    /// left of the undo caveat, under the detail line.
    #[test]
    fn the_warn_banner_leads_with_its_glyph() {
        for id in THEMES {
            let (app, texts, pixels) = drawn(id);
            let i18n = app.ui_state.i18n.as_ref();
            let tokens = &app.document_state.panel_ctx.tokens;
            let question = t!(i18n, ids::DIALOG_ANNOTATE_RESET_QUESTION).to_string();
            let at = texts
                .iter()
                .position(|t| t.text == question)
                .expect("the question");
            // The detail follows the question; the banner is under it.
            let detail = texts[at + 1].bounds;
            let cancel = find(&texts, "Cancel").bounds;
            let ink = crate::styles::pill_colors(tokens, crate::styles::PillTone::Warn).1;
            let x_end = detail.x + SP.l + TS.label + SP.xs;
            let glyph = (detail.y + detail.height) as usize..cancel.y as usize;
            let found = glyph.into_iter().any(|y| {
                (detail.x as usize..x_end as usize)
                    .any(|x| near(pixel(&pixels, x as f32, y as f32), ink, 24))
            });
            assert!(found, "{id:?}: no warn glyph left of the caveat");
        }
    }
}
