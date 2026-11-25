//! Export-error and print / PDF preview modal builders.
//!
//! Extracted verbatim from `view/mod.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use crate::styles::{DialogButtonKind, dialog_body, dialog_button, dialog_footer_split};
use iced::widget::Widget;
use signex_i18n::{I18n, Localized, ids, t};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    /// The export-error dialog in its own OS window: the card filling the
    /// window surface.
    pub(super) fn view_error_notice(&self) -> Element<'_, Message> {
        match self.error_notice_card() {
            Some(card) => self.fill_dialog_window(card),
            None => iced::widget::Space::new().boxed(),
        }
    }

    /// The error card (Graphite mockup row 11): a header with the severity glyph, the
    /// heading and the close x; the readable sentence; the system part in a
    /// sunken mono block; on a choice card, what each choice does. A plain
    /// card's footer is Copy and OK; a choice card's is Cancel, then the
    /// choices. The in-window overlay hands it, unpositioned, to the dismiss
    /// helper so only a press outside the card closes it (#2377).
    pub(super) fn error_notice_card(&self) -> Option<Element<'_, Message>> {
        use iced::widget::{column, container};
        use signex_i18n::RenderTranslatable as _;
        let notice = self.document_state.error_notice.as_ref()?;

        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let detached = self.modal_detached(crate::app::state::ModalId::ErrorNotice);
        let choices = notice_choice_buttons(i18n, notice, tokens);
        let is_choice = !choices.is_empty();
        let footer = self.error_notice_footer(choices);

        let ink = crate::styles::ti(tokens.text);
        let line = |label: Localized| signex_widgets::text(label).size(TS.body).color(ink);
        let mut sentence: iced::widget::Column<Element<'_, Message>> =
            iced::widget::Column::new().push_boxed(line(notice.detail.localized(i18n)));
        // A failed save adds that the edits are still open (mockup row 11).
        if let Some(note) = notice.note {
            sentence = sentence.push_boxed(line(t!(i18n, note)));
        }
        let mut content: iced::widget::Column<Element<'_, Message>> = iced::widget::Column::new()
            .spacing(NOTICE_BODY_GAP)
            .push_boxed(
                container(iced::widget::scrollable(sentence)).height(Length::Fit.max(200.0)),
            );
        if let Some(system) = &notice.system {
            content = content.push(notice_system_block(system, tokens));
        }
        if let Some(rows) = notice_choice_effects(i18n, notice, &self.document_state.panel_ctx) {
            content = content.push(rows);
        }
        // In its own window the OS title bar carries the close.
        let close = (!detached).then(|| Message::Overlay(OverlayMsg::DismissErrorNotice));
        let body = column![
            notice_header(
                notice,
                i18n,
                close,
                self.ui_state.effective_theme_id(),
                tokens
            ),
            dialog_body(content),
            footer,
        ];

        let width = if detached {
            f32::MAX
        } else if is_choice {
            NOTICE_CHOICE_W
        } else {
            NOTICE_PLAIN_W
        };
        let card = container(body)
            .width(Length::Fit.max(width))
            .style(crate::styles::modal_card(tokens));
        Some(card.boxed())
    }

    /// The error card's footer: Cancel, then `choices` on a choice card;
    /// Copy and OK on a plain one.
    fn error_notice_footer<'a>(&self, choices: Vec<Element<'a, Message>>) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let dismiss = || Some(Message::Overlay(OverlayMsg::DismissErrorNotice));
        if !choices.is_empty() {
            let cancel = dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                dismiss(),
                tokens,
            );
            return dialog_footer_split(vec![cancel], choices);
        }
        let copy = crate::styles::dialog_icon_button(
            crate::icons::icon_chrome_copy(self.ui_state.effective_theme_id()),
            t!(i18n, ids::DIALOG_BUTTON_COPY),
            DialogButtonKind::Secondary,
            Some(Message::Overlay(OverlayMsg::ErrorNoticeCopyPressed)),
            tokens,
        );
        let ok = dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_OK),
            DialogButtonKind::Primary,
            dismiss(),
            tokens,
        );
        dialog_footer_split(vec![copy], vec![ok])
    }

    /// `card` centred in a dialog's own OS window, on the panel surface.
    fn fill_dialog_window<'a>(&self, card: Element<'a, Message>) -> Element<'a, Message> {
        use iced::widget::container;
        let panel_bg = crate::styles::ti(self.document_state.panel_ctx.tokens.panel_bg);
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |theme: &iced::Theme| {
                // In its own window the dialog fills the window surface.
                if crate::styles::is_dialog_window(theme) {
                    container::Style {
                        background: Some(panel_bg.into()),
                        ..container::Style::default()
                    }
                } else {
                    container::Style::default()
                }
            })
            .boxed()
    }

    /// The Export PDF dialog: one screen with the page list, the page and
    /// the settings column (`pdf_preview/`), Cancel and Export PDF… in the
    /// footer. Opened by File → Print (Ctrl+P) and File → Export → PDF;
    /// closed by the header X or Cancel (Close once an export finished). The
    /// in-window flavour wraps the body in `wrap_dialog` for backdrop +
    /// drag-to-position.
    pub(super) fn view_print_preview(&self) -> Element<'_, Message> {
        use crate::app::state::ModalId;
        // `pdf_modal_size` is already fitted; fitting again is a no-op.
        self.wrap_dialog(
            ModalId::PrintPreview,
            self.view_print_preview_inner(true),
            self.pdf_modal_size(),
        )
    }

    /// The in-window export modal at its design size, capped to the window
    /// (#1227).
    pub(in crate::app::view) fn pdf_modal_size(&self) -> (f32, f32) {
        crate::app::view::dialogs::fit_modal_size(
            self.ui_state.window_size,
            (Self::PDF_MODAL_W, Self::PDF_MODAL_H),
        )
    }

    /// Detached-window flavour — bare body, no backdrop, no in-window
    /// drag handler (the OS window-drag covers the header).
    pub(super) fn view_print_preview_body(&self) -> Element<'_, Message> {
        self.view_print_preview_inner(false)
    }

    fn view_print_preview_inner(&self, draggable: bool) -> Element<'_, Message> {
        use crate::app::state::ModalId;
        use crate::app::view::dialogs::{
            DIALOG_HEADER_H, draggable_header, graphite_header, os_title_bar_slot,
        };
        use iced::widget::{column, container};
        use signex_widgets::text;

        let preview = match &self.document_state.preview {
            Some(p) => p,
            None => return iced::widget::Space::new().boxed(),
        };

        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_muted = crate::styles::ti(tokens.text_secondary);

        // Header — the modal chrome, with the file and its sheet count.
        let header_content: Element<'_, Message> = graphite_header(
            i18n.message_plain(ids::DIALOG_PRINT_PREVIEW_EXPORT_PDF, &[]),
            Some(self.pdf_dialog_subtitle(preview)),
            None,
            Message::PrintPreview(PrintPreviewMsg::Close),
            DIALOG_HEADER_H,
            tokens,
        );
        let header = if draggable {
            draggable_header(
                header_content,
                ModalId::PrintPreview,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        // One screen: page list, page, settings column.
        let body = self.view_pdf_dialog_body(preview);

        // Footer — the status caption, then Cancel before the action and
        // Close once an export has finished, and Export PDF….
        let dismiss = if preview.export_status.is_some() {
            ids::DIALOG_BUTTON_CLOSE
        } else {
            ids::DIALOG_BUTTON_CANCEL
        };
        let footer = dialog_footer_split(
            vec![
                text(self.pdf_footer_caption(preview))
                    .size(TS.body)
                    .color(text_muted)
                    .boxed(),
            ],
            vec![
                dialog_button(
                    t!(i18n, dismiss),
                    DialogButtonKind::Cancel,
                    Some(Message::PrintPreview(PrintPreviewMsg::Close)),
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::PDF_EXPORT_PDF),
                    DialogButtonKind::Primary,
                    Some(Message::PrintPreview(PrintPreviewMsg::Export)),
                    tokens,
                ),
            ],
        );

        // Detached: the OS window owns the size, so fill it.
        let (modal_w, modal_h): (Length, Length) = if draggable {
            let (w, h) = self.pdf_modal_size();
            (Length::Fixed(w), Length::Fixed(h))
        } else {
            (Length::Fill, Length::Fill)
        };
        let dialog = container(column![header, body, footer].width(modal_w).height(modal_h))
            .style(crate::styles::modal_card(tokens))
            .clip(true);
        dialog.boxed()
    }
}

/// Width of a plain error card and of a card that offers choices (mockup row 11).
const NOTICE_PLAIN_W: f32 = 460.0;
const NOTICE_CHOICE_W: f32 = 480.0;
/// Gap between the sentence, the system block and the choice list (10 px at
/// the default base).
const NOTICE_BODY_GAP: f32 = crate::styles::chrome_px(10.0 * 11.0 / 12.0);
/// Side of the severity glyph in the header (18 px at the default base).
const NOTICE_GLYPH: f32 = crate::styles::chrome_px(18.0 * 11.0 / 12.0);
/// Side of the header's close button (28 px at the default base).
const NOTICE_CLOSE: f32 = crate::styles::chrome_px(28.0 * 11.0 / 12.0);
/// Gaps of the choice list: between rows, and between a choice and its
/// effect (3 and 14 px at the default base).
const NOTICE_KV_ROW_GAP: f32 = crate::styles::chrome_px(3.0 * 11.0 / 12.0);
const NOTICE_KV_COLUMN_GAP: f32 = crate::styles::chrome_px(14.0 * 11.0 / 12.0);

/// The card's header: the severity glyph (red for a failure, amber for a
/// warning), the heading and, unless `close` is `None`, the close x.
fn notice_header<'a>(
    notice: &crate::app::state::ErrorNotice,
    i18n: &I18n,
    close: Option<Message>,
    theme: signex_types::theme::ThemeId,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    use crate::app::state::NoticeSeverity;
    use crate::styles::DialogIcon;
    use iced::widget::{Space, row};
    use signex_i18n::RenderTranslatable as _;
    let icon = match notice.severity() {
        NoticeSeverity::Error => DialogIcon::Error,
        NoticeSeverity::Warn => DialogIcon::Warn,
    };
    let title_font = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    };
    let mut line = row![
        crate::styles::dialog_glyph(icon, NOTICE_GLYPH, tokens),
        signex_widgets::text(notice.title.localized(i18n))
            .size(TS.title)
            .font(title_font)
            .color(crate::styles::ti(tokens.text))
            .wrapping(iced::widget::text::Wrapping::None),
        Space::new().width(Length::Fill),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    if let Some(close) = close {
        line = line.push(notice_close_button(close, theme, tokens));
    }
    notice_header_band(line.boxed(), tokens)
}

/// The header's close x: a muted glyph on a ghost button.
fn notice_close_button<'a>(
    close: Message,
    theme: signex_types::theme::ThemeId,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    use iced::widget::container;
    let glyph = crate::styles::tinted_glyph(
        crate::icons::icon_close(theme),
        NOTICE_GLYPH * 0.75,
        crate::styles::tokens::muted_text(tokens),
    );
    signex_widgets::button(
        container(glyph)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .padding(0)
    .width(Length::Fixed(NOTICE_CLOSE))
    .height(Length::Fixed(NOTICE_CLOSE))
    .on_press(close)
    .style(crate::styles::ghost_button(tokens))
    .boxed()
}

/// The header band under `line`: the paper fill over a hairline, its upper
/// corners following the card's in the main window, flat in its own one.
fn notice_header_band<'a>(
    line: Element<'a, Message>,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    use iced::widget::container;
    let (paper, rule) = (
        crate::styles::ti(tokens.paper),
        crate::styles::tokens::resolve(tokens).line_soft,
    );
    let corner = crate::styles::Radii::GRAPHITE.dialog;
    container(line)
        .padding(iced::Padding {
            top: 0.0,
            right: SP.m,
            bottom: 0.0,
            left: SP.xl,
        })
        .width(Length::Fill)
        .height(Length::Fixed(crate::app::view::dialogs::DIALOG_HEADER_H))
        .align_y(iced::alignment::Vertical::Center)
        .style(move |theme: &iced::Theme| {
            let flat = crate::styles::is_dialog_window(theme);
            let mut border = iced::Border {
                color: rule,
                radius: if flat {
                    iced::border::Radius::default()
                } else {
                    iced::border::Radius::default()
                        .top_left(corner)
                        .top_right(corner)
                },
                ..iced::Border::default()
            };
            border.bottom = iced::border::Side::default().width(1.0);
            iced::widget::container::Style {
                background: Some(iced::Background::Color(paper)),
                border,
                ..iced::widget::container::Style::default()
            }
        })
        .boxed()
}

/// The system part of a notice (a path, an OS error) in a sunken mono block,
/// apart from the readable sentence; long paths break anywhere.
fn notice_system_block<'a>(
    system: &signex_i18n::UserText,
    tokens: &signex_types::theme::ThemeTokens,
) -> Element<'a, Message> {
    use iced::widget::container;
    let sunken = crate::styles::tokens::resolve(tokens).sunken;
    let ink2 = crate::styles::ti(tokens.text_secondary);
    container(
        signex_widgets::text(system.clone())
            .font(crate::fonts::iced_font_for_family(
                crate::fonts::CHROME_MONO_FONT,
            ))
            .size(TS.caption)
            .color(ink2)
            .wrapping(iced::widget::text::Wrapping::Glyph)
            .width(Length::Fill),
    )
    .padding([SP.m, NOTICE_BODY_GAP])
    .width(Length::Fill)
    .style(move |_: &iced::Theme| container::Style {
        background: Some(iced::Background::Color(sunken)),
        border: iced::Border {
            radius: crate::styles::Radii::GRAPHITE.button.into(),
            ..iced::Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// What each choice of a File Changed on Disk card does, as a two-column
/// list (choice, effect); `None` for every other card.
fn notice_choice_effects<'a>(
    i18n: &I18n,
    notice: &crate::app::state::ErrorNotice,
    ctx: &crate::panels::PanelContext,
) -> Option<Element<'a, Message>> {
    use crate::app::state::NoticeAction;
    use iced::widget::{Column, row};
    let Some(NoticeAction::DiskChanged {
        drift: crate::persist::DiskDrift::Changed,
        ..
    }) = &notice.action
    else {
        return None;
    };
    let tokens = &ctx.tokens;
    let (key_ink, effect_ink) = (
        crate::styles::tokens::resolve(tokens).ink4,
        crate::styles::ti(tokens.text_secondary),
    );
    let pairs: Vec<(Localized, Localized)> =
        crate::app::state::disk_changed_choices(notice.offers_save_as())
            .into_iter()
            .map(|(label, effect)| (t!(i18n, label), t!(i18n, effect)))
            .collect();
    // The choice column is as wide as its longest label.
    let family = signex_types::font::nonblank_family(&ctx.ui_font_name);
    let key_w = pairs
        .iter()
        .map(|(label, _)| {
            signex_types::font::estimate_text_width_px(&label.to_string(), TS.body, family)
        })
        .fold(0.0, f32::max)
        .ceil();
    let mut rows: Column<Element<'a, Message>> = Column::new().spacing(NOTICE_KV_ROW_GAP);
    for (label, effect) in pairs {
        rows = rows.push_boxed(
            row![
                signex_widgets::text(label)
                    .size(TS.body)
                    .color(key_ink)
                    .width(Length::Fixed(key_w)),
                signex_widgets::text(effect)
                    .size(TS.body)
                    .color(effect_ink)
                    .width(Length::Fill),
            ]
            .spacing(NOTICE_KV_COLUMN_GAP),
        );
    }
    Some(rows.boxed())
}

/// The choices a notice offers after its leading Cancel (#1826). On File
/// Changed on Disk both data-losing choices take the same danger outline and
/// Save As…, which loses nothing, is the primary (mockup row 11); a board has no
/// Save As, so its card has no primary. A read-only project's refused save
/// offers Save Project As… (#2908).
fn notice_choice_buttons<'a>(
    i18n: &I18n,
    notice: &crate::app::state::ErrorNotice,
    tokens: &signex_types::theme::ThemeTokens,
) -> Vec<Element<'a, Message>> {
    use crate::app::state::NoticeAction;
    use crate::persist::DiskDrift;
    let drift = match &notice.action {
        Some(NoticeAction::DiskChanged { drift, .. }) => drift,
        Some(NoticeAction::SaveProjectAs { project }) => {
            let requested = crate::app::SaveProjectAsMsg::Requested {
                project: Some(*project),
            };
            return vec![dialog_button(
                t!(i18n, signex_i18n::command_ids::CMD_MENU_SAVE_PROJECT_AS),
                DialogButtonKind::Primary,
                Some(Message::from(requested)),
                tokens,
            )];
        }
        None => return Vec::new(),
    };
    let choice =
        |label: Localized, kind: DialogButtonKind, message: OverlayMsg| -> Element<'a, Message> {
            dialog_button(label, kind, Some(Message::Overlay(message)), tokens)
        };
    match drift {
        DiskDrift::Changed => {
            let mut buttons = vec![
                choice(
                    t!(i18n, ids::DIALOG_DISK_CONFLICT_RELOAD),
                    DialogButtonKind::Danger,
                    OverlayMsg::DiskConflictReloadChosen,
                ),
                choice(
                    t!(i18n, ids::DIALOG_OVERWRITE),
                    DialogButtonKind::Danger,
                    OverlayMsg::DiskConflictOverwriteChosen,
                ),
            ];
            if notice.offers_save_as() {
                buttons.push(choice(
                    t!(i18n, ids::DIALOG_DISK_CONFLICT_SAVE_AS),
                    DialogButtonKind::Primary,
                    OverlayMsg::DiskConflictSaveAsChosen,
                ));
            }
            buttons
        }
        DiskDrift::Missing => vec![choice(
            t!(i18n, ids::DIALOG_DISK_CONFLICT_RECREATE),
            DialogButtonKind::Primary,
            OverlayMsg::DiskConflictOverwriteChosen,
        )],
        // Writing would have to re-create the folder, which a save refuses.
        DiskDrift::FolderMissing => Vec::new(),
    }
}
