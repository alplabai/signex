//! The Annotate dialog's change list (#75): the summary banner, the list
//! header (Changes / All, the filter, Re-prefix to standard), the table
//! with its Lock, Current, Proposed, Sheet and Location columns, and the
//! Re-prefix note under it.

use super::super::*;
use iced::widget::{Column, Row, column, container, row, scrollable};
use iced::{Alignment, Background, Element, Length, Theme, Widget};
use signex_i18n::{UserText, ids, t};
use signex_widgets::{button, segmented, text, text_input};

use super::super::graphite as gr;
use super::super::graphite_annotate as ga;
use super::controls::{
    all_listed_locked, banner_style, bordered_style, line_glyph, lock_all_header, reprefix_note,
    scroll_style, section_heading,
};
use super::{SP, TS};
use crate::app::handlers::erc::{
    AnnotateListView, AnnotatePreviewEntry, change_count, changing_sheets, is_changing, listed_rows,
};
use crate::styles::{DialogButtonKind, DialogIcon, PillTone, dialog_button};

/// Column widths: the mock's 56 / 110 / 130 / 130 px at the 12 px body.
const LOCK_COL_W: f32 = TS.body * 56.0 / 12.0;
const CURRENT_COL_W: f32 = TS.body * 110.0 / 12.0;
const PROPOSED_COL_W: f32 = TS.body * 130.0 / 12.0;
const LOCATION_COL_W: f32 = TS.body * 130.0 / 12.0;
/// Widest the filter field grows: 170 px.
const FILTER_MAX_W: f32 = TS.body * 170.0 / 12.0;
/// Height of one list row.
const ROW_H: f32 = TS.body + 2.0 * SP.m;

/// The semibold UI face of the banner's count.
fn semibold() -> iced::Font {
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// A row's cells laid out on the table's columns.
fn table_row<'a>(
    lock: Element<'a, Message>,
    current: Element<'a, Message>,
    proposed: Element<'a, Message>,
    sheet: Element<'a, Message>,
    location: Element<'a, Message>,
) -> Row<Element<'a, Message>> {
    row![
        container(lock).width(Length::Fixed(LOCK_COL_W)),
        container(current).width(Length::Fixed(CURRENT_COL_W)),
        container(proposed).width(Length::Fixed(PROPOSED_COL_W)),
        container(sheet).width(Length::Fill),
        container(location).width(Length::Fixed(LOCATION_COL_W)),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
}

impl Signex {
    /// The change list column: banner, list header, table and the
    /// Re-prefix note.
    pub(super) fn annotate_change_list_column(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let faint = crate::styles::tokens::resolve(tokens).ink4;
        let standard = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.designator_standard());
        column![
            self.annotate_banner(),
            self.annotate_list_header(standard.is_some()),
            self.annotate_table(),
            row![
                line_glyph(DialogIcon::Info, TS.caption, faint),
                text(reprefix_note(i18n, standard))
                    .size(TS.caption)
                    .color(faint)
                    .width(Length::Fill),
            ]
            .spacing(SP.s)
            .align_y(Alignment::Start),
        ]
        .spacing(SP.m + SP.xs)
        .height(Length::Fill)
        .boxed()
    }

    /// The summary on top: how many designators change, in bold, and where.
    fn annotate_banner(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let ink = crate::styles::ti(tokens.text);
        let preview = &self.ui_state.annotate_preview;
        let entries = &preview.entries;
        let line = |s: signex_i18n::Localized| text(s).size(TS.body).color(ink).boxed();
        let (content, tone): (Element<'_, Message>, PillTone) = if preview.load_failed {
            (
                line(t!(i18n, ids::DIALOG_ANNOTATE_LOAD_FAILED)),
                PillTone::Warn,
            )
        } else if preview.loading && entries.is_empty() {
            (line(t!(i18n, ids::DIALOG_ANNOTATE_LOADING)), PillTone::Info)
        } else {
            let changes = change_count(entries);
            if changes == 0 {
                let nothing =
                    t!(i18n, ids::DIALOG_ANNOTATE_SUMMARY_NOTHING, "total" => entries.len());
                (line(nothing), PillTone::Info)
            } else {
                (self.annotate_banner_changes(changes), PillTone::Info)
            }
        };
        let icon = match tone {
            PillTone::Warn => DialogIcon::Warn,
            _ => DialogIcon::Info,
        };
        let (_, glyph_ink) = crate::styles::pill_colors(tokens, tone);
        let line = row![line_glyph(icon, TS.body, glyph_ink), content]
            .spacing(SP.m)
            .align_y(Alignment::Start);
        container(line)
            .padding([SP.m, SP.m + SP.xs])
            .width(Length::Fill)
            .style(banner_style(tokens, tone))
            .boxed()
    }

    /// "7 of 42 designators will change" in bold, then the sheets the
    /// changes land on and how it undoes.
    fn annotate_banner_changes(&self, changes: usize) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let ink = crate::styles::ti(tokens.text);
        let preview = &self.ui_state.annotate_preview;
        let sheets = changing_sheets(&preview.entries);
        let undo = if self.annotate_undo_is_one_step() {
            "project"
        } else {
            "sheet"
        };
        let scope = t!(
            i18n,
            ids::DIALOG_ANNOTATE_BANNER_SCOPE,
            "sheets" => sheets.len(),
            "sheet" => sheets.first().cloned().unwrap_or_default(),
            "undo" => undo
        );
        row![
            text(t!(
                i18n,
                ids::DIALOG_ANNOTATE_CHANGE_COUNT,
                "changes" => changes,
                "total" => preview.entries.len()
            ))
            .size(TS.body)
            .color(ink)
            .font(semibold())
            .wrapping(iced::widget::text::Wrapping::None),
            text(scope).size(TS.body).color(ink).width(Length::Fill),
        ]
        .spacing(SP.s)
        .boxed()
    }

    /// The list header: its heading, the Changes / All switch, the filter
    /// and Re-prefix to standard.
    fn annotate_list_header(&self, has_standard: bool) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let r = crate::styles::tokens::resolve(tokens);
        let session = &self.ui_state.annotate_session;
        let entries = &self.ui_state.annotate_preview.entries;
        // Hugging its labels, the switch takes its whole width before the
        // filter shares out the rest, so its two segments never fold.
        let switch_style = r.segmented(TS.body);
        let switch = segmented(
            [
                (
                    AnnotateListView::Changes,
                    signex_widgets::Label::from(t!(
                        i18n,
                        ids::DIALOG_ANNOTATE_VIEW_CHANGES,
                        "count" => change_count(entries)
                    )),
                ),
                (
                    AnnotateListView::All,
                    signex_widgets::Label::from(
                        t!(i18n, ids::DIALOG_ANNOTATE_VIEW_ALL, "count" => entries.len()),
                    ),
                ),
            ],
            session.list_view,
            switch_style,
            |view| Message::Annotate(AnnotateMsg::ListViewPicked(view)),
        );
        let filter = text_input(
            t!(i18n, ids::DIALOG_ANNOTATE_FILTER_PLACEHOLDER),
            &session.filter,
        )
        .style(crate::panels::form_look::text_input_style)
        .on_input(|v| Message::Annotate(AnnotateMsg::ListFilterEdited(v)))
        .size(TS.body)
        .padding([SP.s, SP.m])
        .width(Length::Fill);
        let busy = self.ui_state.annotate_runs.is_running();
        let reprefix = (has_standard && !self.ui_state.annotate_preview.loading && !busy)
            .then_some(Message::Annotate(AnnotateMsg::ReprefixPressed));
        row![
            section_heading(i18n, ids::DIALOG_ANNOTATE_CHANGES_HEADING, tokens),
            switch,
            container(filter).width(Length::Fill.max(FILTER_MAX_W)),
            dialog_button(
                t!(i18n, ids::DIALOG_ANNOTATE_REPREFIX),
                DialogButtonKind::Secondary,
                reprefix,
                tokens,
            ),
        ]
        .spacing(SP.s + SP.xs)
        .align_y(Alignment::Center)
        .boxed()
    }

    /// The table: a sunken header with the lock-all box, then the listed
    /// rows (and the locked rows kept, under their subhead), scrolling.
    fn annotate_table(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let r = crate::styles::tokens::resolve(tokens);
        let preview = &self.ui_state.annotate_preview;
        let locked = &self.ui_state.annotate_locked;
        let session = &self.ui_state.annotate_session;
        let busy = self.ui_state.annotate_runs.is_running();

        let head_cell =
            |id: signex_i18n::MessageId| text(t!(i18n, id)).size(TS.caption).color(r.ink4).boxed();
        let all_locked = all_listed_locked(preview.entries.iter().map(|e| e.uuid), locked);
        let lock_head = row![
            lock_all_header(
                all_locked,
                !preview.entries.is_empty() && !preview.loading && !busy,
                tokens,
            ),
            head_cell(ids::DIALOG_ANNOTATE_COL_LOCK),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed();
        let sunken = r.sunken;
        let head = container(table_row(
            lock_head,
            head_cell(ids::DIALOG_ANNOTATE_COL_CURRENT),
            head_cell(ids::DIALOG_ANNOTATE_COL_PROPOSED),
            head_cell(ids::DIALOG_ANNOTATE_COL_SHEET),
            head_cell(ids::DIALOG_ANNOTATE_COL_LOCATION),
        ))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            ..container::Style::default()
        });

        let listed = listed_rows(&preview.entries, locked, session.list_view, &session.filter);
        let mut rows: Column<Element<'_, Message>> = column![].width(Length::Fill);
        let note = |id: signex_i18n::MessageId| {
            container(text(t!(i18n, id)).size(TS.body).color(r.ink3))
                .padding([SP.m, SP.l])
                .boxed()
        };
        if preview.entries.is_empty() {
            rows = rows.push(note(if preview.loading {
                ids::DIALOG_ANNOTATE_LOADING
            } else if preview.load_failed {
                ids::DIALOG_ANNOTATE_LOAD_FAILED
            } else {
                ids::DIALOG_ANNOTATE_NOTHING_TO_NUMBER
            }));
        } else if listed.is_empty() {
            rows = rows.push(note(if session.filter.trim().is_empty() {
                ids::DIALOG_ANNOTATE_NO_CHANGES
            } else {
                ids::DIALOG_ANNOTATE_NO_MATCH
            }));
        }
        for entry in &listed.rows {
            rows = rows.push(self.annotate_list_row(entry));
        }
        if !listed.kept_locked.is_empty() {
            rows = rows.push(
                container(section_heading(
                    i18n,
                    ids::DIALOG_ANNOTATE_KEPT_LOCKED,
                    tokens,
                ))
                .padding([SP.s, SP.m])
                .boxed(),
            );
            for entry in &listed.kept_locked {
                rows = rows.push(self.annotate_list_row(entry));
            }
        }
        let body = scrollable(rows)
            .height(Length::Fill)
            .style(scroll_style(tokens));
        container(column![head, body])
            .width(Length::Fill)
            .height(Length::Fill)
            .clip(true)
            .style(bordered_style(tokens))
            .boxed()
    }

    /// One list row. A changing row shows "→ new" in accent ink; the rest
    /// are muted. The lock box toggles the row's lock.
    fn annotate_list_row<'a>(&'a self, entry: &AnnotatePreviewEntry) -> Element<'a, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let r = crate::styles::tokens::resolve(tokens);
        let ink = crate::styles::ti(tokens.text);
        let changing = is_changing(entry);
        let is_locked = self.ui_state.annotate_locked.contains(&entry.uuid);
        let body_ink = if changing { ink } else { r.ink3 };
        let mono = |s: String, color: iced::Color| -> Element<'a, Message> {
            text(UserText::new(s))
                .size(TS.body)
                .color(color)
                .font(ga::mono())
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed()
        };
        let lock: Element<'a, Message> = button(ga::checkbox_pip(tokens, is_locked))
            .on_press(Message::Annotate(AnnotateMsg::ToggleLock(entry.uuid)))
            .padding(0.0)
            .style(ga::bare_button_style)
            .boxed();
        let proposed = if changing {
            mono(
                format!("\u{2192} {}", entry.proposed),
                gr::changed_ink(tokens),
            )
        } else {
            mono(entry.proposed.clone(), r.ink3)
        };
        let (x, y) = entry.location;
        let line = table_row(
            lock,
            mono(entry.current.clone(), body_ink),
            proposed,
            mono(entry.sheet.clone(), body_ink),
            mono(format!("{x:.2}, {y:.2}"), body_ink),
        );
        container(line)
            .padding([0.0, SP.m])
            .height(Length::Fixed(ROW_H))
            .width(Length::Fill)
            .align_y(iced::alignment::Vertical::Center)
            .boxed()
    }
}
