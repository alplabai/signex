//! The Re-prefix to standard confirm (#1315, #77): three count tiles and
//! three short notes, the change table (part, old prefix, new prefix, note)
//! scrolling with its last row counting the rest, the parts left unchanged
//! as count pills, and Cancel / Re-prefix N parts.

use super::*;
use iced::widget::{Column, Row, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Element, Length, Theme, Widget};
use signex_i18n::{Localized, UserText, ids, t};
use signex_widgets::stat_tiles::{Density, StatTile, stat_tiles};
use signex_widgets::{raw, text};

use super::super::graphite as gr;
use super::super::graphite_annotate as ga;
use super::super::widgets::{draggable_header, os_title_bar_slot};
use super::controls::{pill, scroll_style, section_heading};
use crate::app::handlers::erc::ReprefixConfirm;
use crate::styles::{DialogButtonKind, PillTone, dialog_body, dialog_button, dialog_footer};

/// Column widths: the mock's 90 / 70 / 70 px at the 12 px body.
const PART_COL_W: f32 = TS.body * 90.0 / 12.0;
const PREFIX_COL_W: f32 = TS.body * 70.0 / 12.0;
/// Height of one table row.
const ROW_H: f32 = TS.body + 2.0 * SP.m;

/// A row's cells laid out on the table's columns.
fn table_row<'a>(
    part: Element<'a, Message>,
    prefix: Element<'a, Message>,
    new: Element<'a, Message>,
    note: Element<'a, Message>,
) -> Row<Element<'a, Message>> {
    row![
        container(part).width(Length::Fixed(PART_COL_W)),
        container(prefix).width(Length::Fixed(PREFIX_COL_W)),
        container(new).width(Length::Fixed(PREFIX_COL_W)),
        container(note).width(Length::Fill),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
}

impl Signex {
    pub(in crate::app::view) fn view_reprefix_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_reprefix_confirm_body_inner(true);
        self.wrap_dialog(
            crate::app::state::ModalId::ReprefixConfirm,
            dialog,
            super::super::sizes::REPREFIX_CONFIRM,
        )
    }

    pub(in crate::app::view) fn view_reprefix_confirm_body(&self) -> Element<'_, Message> {
        self.view_reprefix_confirm_body_inner(false)
    }

    fn view_reprefix_confirm_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let Some(confirm) = &self.ui_state.reprefix_confirm else {
            return container(Space::new()).boxed();
        };
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_view.panel_ctx.tokens;

        let header_content: Element<'_, Message> = gr::titled_header(
            t!(i18n, ids::DIALOG_TITLE_REPREFIX).into_string(),
            Some(t!(i18n, ids::DIALOG_REPREFIX_QUESTION).into_string()),
            Message::Annotate(AnnotateMsg::ReprefixCancelled),
            tokens,
        );
        let header = if draggable {
            draggable_header(
                header_content,
                crate::app::state::ModalId::ReprefixConfirm,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        let mut body: Column<Element<'_, Message>> = column![
            self.reprefix_tiles(confirm),
            self.reprefix_notes(confirm),
            self.reprefix_caption(confirm),
            self.reprefix_table(confirm),
        ]
        .spacing(SP.m + SP.xs)
        .height(Length::Fill);
        if !confirm.skips.is_empty() {
            body = body.push(self.reprefix_skips(confirm));
        }

        let size = super::super::sizes::REPREFIX_CONFIRM;
        let height = if draggable {
            Length::Fixed(size.1)
        } else {
            Length::Fill
        };
        let dialog = container(
            column![
                header,
                container(dialog_body(body)).height(Length::Fill),
                dialog_footer(vec![
                    dialog_button(
                        t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                        DialogButtonKind::Cancel,
                        Some(Message::Annotate(AnnotateMsg::ReprefixCancelled)),
                        tokens,
                    ),
                    dialog_button(
                        t!(i18n, ids::DIALOG_ANNOTATE_REPREFIX_PARTS, "count" => confirm.parts),
                        DialogButtonKind::Primary,
                        Some(Message::Annotate(AnnotateMsg::ReprefixConfirmed)),
                        tokens,
                    ),
                ]),
            ]
            .width(self.dialog_width(crate::app::state::ModalId::ReprefixConfirm, size))
            .height(height),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }

    /// Parts, sheets and the standard, as three read-only tiles.
    fn reprefix_tiles<'a>(&'a self, confirm: &ReprefixConfirm) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        stat_tiles(
            vec![
                StatTile::count(
                    count(confirm.parts),
                    t!(i18n, ids::DIALOG_REPREFIX_TILE_PARTS, "count" => confirm.parts),
                ),
                StatTile::count(
                    count(confirm.sheets),
                    t!(i18n, ids::DIALOG_REPREFIX_TILE_SHEETS, "count" => confirm.sheets),
                ),
                StatTile::text(
                    UserText::new(confirm.standard.to_string()),
                    t!(i18n, ids::DIALOG_REPREFIX_TILE_STANDARD),
                ),
            ],
            Density::Regular,
            &self.document_view.panel_ctx.tokens,
            TS.body,
            i18n,
        )
    }

    /// The three short notes: one undo step, unsaved sheets, symbol matches.
    fn reprefix_notes<'a>(&'a self, confirm: &ReprefixConfirm) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let muted = crate::styles::tokens::resolve(&self.document_view.panel_ctx.tokens).ink3;
        let bullet = |line: Localized| -> Element<'a, Message> {
            row![
                // raw: a bullet glyph, not language.
                text(raw("\u{2022}")).size(TS.body).color(muted),
                text(line).size(TS.body).color(muted).width(Length::Fill),
            ]
            .spacing(SP.s)
            .boxed()
        };
        let mut notes: Column<Element<'a, Message>> = column![
            bullet(t!(
                i18n,
                ids::DIALOG_REPREFIX_BULLET_UNDO,
                "undo" => crate::keymap::shown_in("Mod+Z", i18n)
            )),
            bullet(t!(i18n, ids::DIALOG_REPREFIX_BULLET_UNSAVED)),
        ]
        .spacing(SP.xs);
        if confirm.by_symbol > 0 {
            notes = notes.push(bullet(t!(
                i18n,
                ids::DIALOG_REPREFIX_BULLET_BY_SYMBOL,
                "count" => confirm.by_symbol
            )));
        }
        notes.boxed()
    }

    /// The caption over the list: how many changes it names.
    fn reprefix_caption<'a>(&'a self, confirm: &ReprefixConfirm) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let faint = crate::styles::tokens::resolve(&self.document_view.panel_ctx.tokens).ink4;
        let caption = if confirm.more > 0 {
            t!(
                i18n,
                ids::DIALOG_REPREFIX_CAPTION_NAMED,
                "named" => confirm.changes.len(),
                "total" => confirm.parts
            )
        } else {
            t!(i18n, ids::DIALOG_REPREFIX_CAPTION_ALL, "count" => confirm.parts)
        };
        row![
            text(caption)
                .size(TS.caption)
                .color(faint)
                .width(Length::Fill),
            text(t!(i18n, ids::DIALOG_REPREFIX_SCROLL))
                .size(TS.caption)
                .color(faint),
        ]
        .spacing(SP.s)
        .boxed()
    }

    /// The change table: the old prefix in mono, the new one in accent ink,
    /// a pill for a part matched by symbol, and a last row for the rest.
    fn reprefix_table<'a>(&'a self, confirm: &ReprefixConfirm) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_view.panel_ctx.tokens;
        let r = crate::styles::tokens::resolve(tokens);
        let ink = crate::styles::ti(tokens.text);
        let mono = |s: String, color: iced::Color| -> Element<'a, Message> {
            text(UserText::new(s))
                .size(TS.body)
                .color(color)
                .font(ga::mono())
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed()
        };
        let head_cell =
            |id: signex_i18n::MessageId| text(t!(i18n, id)).size(TS.caption).color(r.ink4).boxed();
        let sunken = r.sunken;
        let head = container(table_row(
            head_cell(ids::DIALOG_REPREFIX_COL_PART),
            head_cell(ids::DIALOG_REPREFIX_COL_PREFIX),
            head_cell(ids::DIALOG_REPREFIX_COL_NEW),
            head_cell(ids::DIALOG_REPREFIX_COL_NOTE),
        ))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            ..container::Style::default()
        });
        let mut rows: Column<Element<'a, Message>> = column![].width(Length::Fill);
        for change in &confirm.changes {
            let note: Element<'a, Message> = if change.by_symbol {
                pill(
                    t!(i18n, ids::DIALOG_REPREFIX_NOTE_BY_SYMBOL),
                    PillTone::Neutral,
                    tokens,
                )
            } else {
                Space::new().boxed()
            };
            let line = table_row(
                mono(change.reference.clone(), ink),
                mono(change.old_prefix.clone(), r.ink4),
                mono(change.new_prefix.clone(), gr::changed_ink(tokens)),
                note,
            );
            rows = rows.push(
                container(line)
                    .padding([0.0, SP.m])
                    .height(Length::Fixed(ROW_H))
                    .align_y(iced::alignment::Vertical::Center)
                    .boxed(),
            );
        }
        if confirm.more > 0 {
            rows = rows.push(
                container(
                    text(t!(i18n, ids::DIALOG_REPREFIX_MORE, "count" => confirm.more))
                        .size(TS.body)
                        .color(r.ink3),
                )
                .padding([0.0, SP.m])
                .height(Length::Fixed(ROW_H))
                .align_y(iced::alignment::Vertical::Center)
                .boxed(),
            );
        }
        container(column![
            head,
            scrollable(rows)
                .height(Length::Fill)
                .style(scroll_style(tokens)),
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(crate::styles::card(tokens))
        .boxed()
    }

    /// The parts left as they are: one count pill per reason.
    fn reprefix_skips<'a>(&'a self, confirm: &ReprefixConfirm) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_view.panel_ctx.tokens;
        let faint = crate::styles::tokens::resolve(tokens).ink4;
        let mut pills: Row<Element<'a, Message>> =
            Row::new().spacing(SP.s).align_y(Alignment::Center);
        for skip in &confirm.skips {
            pills = pills.push(pill(
                t!(i18n, skip.label, "count" => skip.count),
                PillTone::Neutral,
                tokens,
            ));
        }
        pills = pills.push(
            text(t!(i18n, ids::DIALOG_REPREFIX_LISTED_IN_MESSAGES))
                .size(TS.caption)
                .color(faint)
                .boxed(),
        );
        column![
            section_heading(i18n, ids::DIALOG_REPREFIX_LEFT_UNCHANGED, tokens),
            pills,
        ]
        .spacing(SP.s)
        .boxed()
    }
}
