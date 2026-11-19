//! The Annotate dialog's options column (#75): the order of processing with
//! its one-line description, the sheets table with real ticks over every
//! project sheet, and the folded "Fixed options" group.

use super::super::*;
use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Element, Length, Widget};
use signex_i18n::{UserText, ids, t};
use signex_widgets::{button, text};

use super::super::graphite_annotate as ga;
use super::controls::{order_dropdown, order_help, pill, scroll_style, section_heading};
use super::{SP, TS};
use crate::styles::PillTone;

/// Width of a key in the Fixed options list.
const KV_KEY_W: f32 = TS.body * 13.0;

impl Signex {
    /// The options column: order, sheets and fixed options, scrolling.
    pub(super) fn annotate_options_column(&self) -> Element<'_, Message> {
        let blocks = column![
            self.annotate_order_block(),
            self.annotate_sheets_block(),
            self.annotate_fixed_options_block(),
        ]
        .spacing(SP.xl)
        .width(Length::Fill);
        scrollable(blocks)
            .height(Length::Fill)
            .style(scroll_style(&self.document_state.panel_ctx.tokens))
            .boxed()
    }

    /// Order of Processing: the dropdown with what the order does under it,
    /// beside the traversal diagram.
    fn annotate_order_block(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let faint = crate::styles::tokens::resolve(tokens).ink4;
        let order = self.ui_state.annotate_order;
        column![
            section_heading(i18n, ids::DIALOG_ANNOTATE_ORDER_HEADING, tokens),
            row![
                column![
                    order_dropdown(i18n, order),
                    text(t!(i18n, order_help(order)))
                        .size(TS.caption)
                        .color(faint)
                        .width(Length::Fill),
                ]
                .spacing(SP.s)
                .width(Length::Fill),
                ga::order_diagram(tokens, order),
            ]
            .spacing(SP.l)
            .align_y(Alignment::Center),
        ]
        .spacing(SP.s)
        .boxed()
    }

    /// Schematic Sheets To Annotate: every project sheet (a reused sheet once
    /// per placement) with a tick, the read-only Start, and the count ticked.
    fn annotate_sheets_block(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let faint = crate::styles::tokens::resolve(tokens).ink4;
        let preview = &self.ui_state.annotate_preview;
        let rows = preview.sheet_rows(
            &self.ui_state.annotate_session.unticked,
            self.document_state.active_path.as_deref(),
        );
        let ticked = rows.iter().filter(|r| r.ticked).count();
        let mut heading = row![
            section_heading(i18n, ids::DIALOG_ANNOTATE_SHEETS_HEADING, tokens),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center);
        if !rows.is_empty() {
            heading = heading.push(pill(
                t!(i18n, ids::DIALOG_ANNOTATE_SHEETS_TICKED, "ticked" => ticked, "total" => rows.len()),
                PillTone::Neutral,
                tokens,
            ));
        }
        let empty = t!(
            i18n,
            if preview.loading {
                ids::DIALOG_ANNOTATE_LOADING
            } else {
                ids::DIALOG_ANNOTATE_SHEETS_EMPTY
            }
        );
        // Ticks wait while a run holds the sheets.
        let editable = !self.ui_state.annotate_runs.is_running();
        column![
            heading,
            ga::sheet_table(tokens, i18n, &rows, editable, empty),
            text(t!(i18n, ids::DIALOG_ANNOTATE_SHEET_OPTIONS_UNAVAILABLE))
                .size(TS.caption)
                .color(faint)
                .width(Length::Fill),
        ]
        .spacing(SP.s)
        .boxed()
    }

    /// The four options that cannot change yet, folded into one group with a
    /// one-line digest; opening it lists them.
    fn annotate_fixed_options_block(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let r = crate::styles::tokens::resolve(tokens);
        let ink = crate::styles::ti(tokens.text);
        let open = self.ui_state.annotate_session.fixed_options_open;
        let toggle = button(
            row![
                signex_widgets::chevron::chevron(open, TS.body, r.ink3),
                text(t!(i18n, ids::DIALOG_ANNOTATE_FIXED_OPTIONS))
                    .size(TS.body)
                    .color(ink),
                pill(
                    t!(i18n, ids::DIALOG_ANNOTATE_FIXED_READ_ONLY),
                    PillTone::Neutral,
                    tokens,
                ),
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center),
        )
        .padding(0.0)
        .style(ga::bare_button_style)
        .on_press(Message::Annotate(AnnotateMsg::FixedOptionsToggled));
        let mut block: Column<Element<'_, Message>> = column![toggle.boxed()].spacing(SP.s);
        if !open {
            block = block.push(
                text(t!(i18n, ids::DIALOG_ANNOTATE_FIXED_DIGEST))
                    .size(TS.caption)
                    .color(r.ink4)
                    .width(Length::Fill)
                    .boxed(),
            );
            return block.boxed();
        }
        let kv = |key: signex_i18n::Localized, value: Element<'static, Message>| {
            row![
                text(key)
                    .size(TS.body)
                    .color(r.ink3)
                    .width(Length::Fixed(KV_KEY_W)),
                value,
            ]
            .spacing(SP.m)
            .align_y(Alignment::Start)
        };
        let faint = |s: signex_i18n::Localized| -> Element<'static, Message> {
            text(s)
                .size(TS.body)
                .color(r.ink4)
                .width(Length::Fill)
                .boxed()
        };
        let list = column![
            kv(
                t!(i18n, ids::DIALOG_ANNOTATE_PROCESS_LOCATION),
                text(t!(i18n, ids::DIALOG_ANNOTATE_PROCESS_LOCATION_DESIGNATOR))
                    .size(TS.body)
                    .color(ink)
                    .boxed(),
            ),
            kv(
                t!(i18n, ids::DIALOG_ANNOTATE_REPLACE_SUB_PARTS),
                faint(t!(i18n, ids::DIALOG_ANNOTATE_REPLACE_SUB_PARTS_OFF)),
            ),
            kv(
                t!(i18n, ids::DIALOG_ANNOTATE_COMPLETE_PACKAGES),
                faint(t!(i18n, ids::DIALOG_ANNOTATE_COMPLETE_PACKAGES_NONE)),
            ),
            kv(
                t!(i18n, ids::DIALOG_ANNOTATE_MATCHING_HEADING),
                text(UserText::new(self.annotate_matching_names()))
                    .size(TS.body)
                    .color(r.ink4)
                    .width(Length::Fill)
                    .boxed(),
            ),
        ]
        .spacing(SP.s);
        block
            .push(container(list).padding([SP.s, SP.xs]).boxed())
            .push(
                text(t!(i18n, ids::DIALOG_ANNOTATE_MATCHING_UNAVAILABLE))
                    .size(TS.caption)
                    .color(r.ink4)
                    .width(Length::Fill)
                    .boxed(),
            )
            .boxed()
    }

    /// The parameter names on the active sheet's symbols, the built-in three
    /// first, for the Matching Options line.
    fn annotate_matching_names(&self) -> String {
        let mut names: Vec<String> = ["Reference", "Value", "Footprint"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let mut extra: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        if let Some(snapshot) = self.active_render_snapshot() {
            for symbol in &snapshot.symbols {
                extra.extend(symbol.fields.keys().cloned());
            }
        }
        for name in extra {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names.join(", ")
    }
}
