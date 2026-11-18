//! The Synchronize Sheet Entries and Ports dialog (#1276): which side wins,
//! then the entries or ports that would be added, removed and renamed.
//! State and rules: `app/sheet_sync/`.

use iced::widget::{Column, Row, Space, column, container};
use iced::{Element, Length, Widget as _};
use signex_engine::{SheetSyncDirection, SheetSyncPreview};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::text;

use super::sizes;
use crate::app::sheet_sync::{SheetSyncMsg, SheetSyncProblem, SheetSyncState};
use crate::app::{Message, Signex};
use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button, ti, tokens};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

fn ss(msg: SheetSyncMsg) -> Message {
    Message::from(msg)
}

impl Signex {
    pub(in crate::app::view) fn view_sheet_sync_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_sheet_sync_card(), sizes::SHEET_SYNC)
    }

    fn view_sheet_sync_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.sheet_sync.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let cancel = Some(ss(SheetSyncMsg::CancelRequested));
        let subtitle = t!(
            i18n,
            ids::SHEET_SYNC_SUBTITLE,
            "symbol" => state.symbol_name.as_str(),
            "file" => state.child_filename.as_str()
        )
        .into_string();
        let header = super::header::graphite_header_closing(
            t!(i18n, ids::SHEET_SYNC_TITLE).into_string(),
            Some(subtitle),
            None,
            cancel.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let body = container(body(state, i18n, tokens)).padding([SP.xl, SP.xl + SP.m]);
        let footer = crate::styles::dialog_footer_split(
            Vec::new(),
            vec![
                dialog_button(
                    t!(i18n, ids::SHEET_SYNC_CANCEL),
                    DialogButtonKind::Cancel,
                    cancel,
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::SHEET_SYNC_APPLY),
                    super::graphite::call_to_action(tokens),
                    state.can_apply().then(|| ss(SheetSyncMsg::ApplyRequested)),
                    tokens,
                ),
            ],
        );
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::SHEET_SYNC.0),
        )))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// The intro, the direction choice, then the changes (or why there are
/// none) and the reason Apply refused, if it did.
fn body<'a>(state: &'a SheetSyncState, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.l);
    col = col.push(
        text(t!(i18n, ids::SHEET_SYNC_INTRO))
            .size(TS.body)
            .color(muted)
            .boxed(),
    );
    col = col.push(direction_choice(state.direction, i18n, tokens));
    let status = match (state.preview(), &state.problem) {
        (None, Some(problem)) => Some(problem_text(problem, i18n)),
        (None, None) => Some(t!(i18n, ids::SHEET_SYNC_READING)),
        (Some(preview), _) if preview.is_empty() => Some(t!(i18n, ids::SHEET_SYNC_NONE)),
        (Some(_), _) => None,
    };
    if let Some(line) = status {
        col = col.push(text(line).size(TS.body).color(ti(tokens.text)).boxed());
    }
    if let Some(preview) = state.preview().filter(|preview| !preview.is_empty()) {
        col = col.push(changes(preview, i18n, tokens));
    }
    if state.preview().is_some()
        && let Some(problem) = state.problem.as_ref()
    {
        col = col.push(
            text(problem_text(problem, i18n))
                .size(TS.body)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    col.boxed()
}

/// The two directions as a pair of buttons; the chosen one is filled.
fn direction_choice<'a>(
    chosen: SheetSyncDirection,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let option = |direction: SheetSyncDirection, id: MessageId| -> Element<'a, Message> {
        let button = signex_widgets::button(text(t!(i18n, id)).size(TS.label))
            .padding([SP.m, SP.l])
            .on_press(ss(SheetSyncMsg::DirectionPicked(direction)));
        if direction == chosen {
            button.style(crate::styles::accent_button(tokens)).boxed()
        } else {
            button.style(crate::styles::outline_button(tokens)).boxed()
        }
    };
    Row::<Element<'a, Message>>::new()
        .push(option(
            SheetSyncDirection::SheetToSymbol,
            ids::SHEET_SYNC_TO_SYMBOL,
        ))
        .push(option(
            SheetSyncDirection::SymbolToSheet,
            ids::SHEET_SYNC_TO_SHEET,
        ))
        .spacing(SP.m)
        .boxed()
}

/// One list per kind of change that has any: its heading with the count,
/// then a name per row (old → new for a rename).
fn changes<'a>(
    preview: &'a SheetSyncPreview,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let ink = ti(tokens.text);
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.m);
    let mut section = |heading: Localized, rows: Vec<String>| {
        if rows.is_empty() {
            return;
        }
        let mut list: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
        list = list.push(text(heading).size(TS.caption).color(r.ink4).boxed());
        for row in rows {
            list = list.push(
                text(UserText::new(row))
                    .size(TS.body)
                    .color(ink)
                    .font(super::graphite::mono())
                    .boxed(),
            );
        }
        col = col.push(list.boxed());
    };
    section(
        t!(i18n, ids::SHEET_SYNC_ADDED, "count" => preview.added.len()),
        preview.added.clone(),
    );
    section(
        t!(i18n, ids::SHEET_SYNC_REMOVED, "count" => preview.removed.len()),
        preview.removed.clone(),
    );
    section(
        t!(i18n, ids::SHEET_SYNC_RENAMED, "count" => preview.renamed.len()),
        preview
            .renamed
            .iter()
            .map(|rename| {
                t!(
                    i18n,
                    ids::SHEET_SYNC_RENAME_ROW,
                    "from" => rename.from.as_str(),
                    "to" => rename.to.as_str()
                )
                .into_string()
            })
            .collect(),
    );
    container(col)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(crate::styles::card(tokens))
        .boxed()
}

/// Why there is no preview or Apply refused, in the dialog's language.
fn problem_text(problem: &SheetSyncProblem, i18n: &I18n) -> Localized {
    let path = |path: &std::path::Path| path.display().to_string();
    match problem {
        SheetSyncProblem::ReadFailed(why) => {
            t!(i18n, ids::SHEET_SYNC_ERR_READ, "why" => why.as_str())
        }
        SheetSyncProblem::Stale => t!(i18n, ids::SHEET_SYNC_ERR_STALE),
        SheetSyncProblem::Changed => t!(i18n, ids::SHEET_SYNC_ERR_CHANGED),
        SheetSyncProblem::Busy(sheet) => t!(i18n, ids::SHEET_SYNC_ERR_BUSY, "path" => path(sheet)),
        SheetSyncProblem::ReadOnly(sheet) => {
            t!(i18n, ids::SHEET_SYNC_ERR_READ_ONLY, "path" => path(sheet))
        }
        SheetSyncProblem::NotLoaded(sheet) => {
            t!(i18n, ids::SHEET_SYNC_ERR_NOT_LOADED, "path" => path(sheet))
        }
        SheetSyncProblem::NotChanged(sheet) => {
            t!(i18n, ids::SHEET_SYNC_ERR_NOT_CHANGED, "path" => path(sheet))
        }
        SheetSyncProblem::DiskChanged(sheet) => {
            t!(i18n, ids::SHEET_SYNC_ERR_DISK_CHANGED, "path" => path(sheet))
        }
    }
}
