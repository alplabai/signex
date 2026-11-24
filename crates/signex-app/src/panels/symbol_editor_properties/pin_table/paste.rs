//! The Paste rows box of the Pin table: the pasted text, what each column
//! is read as, the type words, the mode, a count of what Apply paste does,
//! and Cancel / Apply paste.

use iced::widget::{Column, column, container, row, text_editor};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_widgets::{button, text};

use super::super::super::{PanelMsg, SymbolEditorPanelContext};
use super::super::rows::{Choice, Form, SP, TS, mono};
use super::table;
use crate::library::editor::symbol::{
    PasteDraft, PasteField, PasteMode, PinTableMsg, paste_preview,
};

/// Lines the paste box shows before it scrolls.
const EDITOR_LINES: f32 = 8.0;
/// The width of a column's field dropdown, in body sizes.
const MAP_EM: f32 = 150.0 / 12.0;

fn field_label(form: &Form<'_>, field: PasteField) -> Localized {
    let id = match field {
        PasteField::Number => ids::EDITORS_PIN_TABLE_FIELD_NUMBER,
        PasteField::Name => ids::EDITORS_PIN_TABLE_FIELD_NAME,
        PasteField::Electrical => ids::EDITORS_PIN_TABLE_FIELD_ELECTRICAL,
        PasteField::Part => ids::EDITORS_PIN_TABLE_FIELD_PART,
        PasteField::Length => ids::EDITORS_PIN_TABLE_FIELD_LENGTH,
        PasteField::Description => ids::EDITORS_PIN_TABLE_FIELD_DESCRIPTION,
        PasteField::Ignore => ids::EDITORS_PIN_TABLE_FIELD_IGNORE,
    };
    t!(form.i18n, id)
}

fn mode_label(form: &Form<'_>, mode: PasteMode) -> Localized {
    let id = match mode {
        PasteMode::UpdateByNumber => ids::EDITORS_PIN_TABLE_MODE_UPDATE,
        PasteMode::Append => ids::EDITORS_PIN_TABLE_MODE_APPEND,
        PasteMode::ReplaceAll => ids::EDITORS_PIN_TABLE_MODE_REPLACE,
    };
    t!(form.i18n, id)
}

pub(super) fn view<'a>(
    sym: &SymbolEditorPanelContext,
    paste: &'a PasteDraft,
    form: &Form<'_>,
) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let line = TS.body * signex_widgets::theme_ext::LINE_HEIGHT;
    let editor = text_editor(&paste.content)
        .size(TS.body)
        .font(mono())
        .padding(SP.s)
        .height(Length::Fixed(line * EDITOR_LINES + 2.0 * SP.s))
        .on_action(|action| table(PinTableMsg::PasteEdited(action)));
    let left = column![
        text(t!(i18n, ids::EDITORS_PIN_TABLE_PASTE_HINT))
            .size(TS.caption)
            .color(form.look.ink3)
            .boxed(),
        editor.boxed(),
    ]
    .spacing(SP.xs)
    .width(Length::FillPortion(3));

    let rows = paste.rows();
    let numbers: Vec<&str> = sym.pins.iter().map(|p| p.number.as_str()).collect();
    let preview = paste_preview(&rows, paste.mode, &numbers);
    let counted = if preview.rows == 0 {
        t!(i18n, ids::EDITORS_PIN_TABLE_PREVIEW_EMPTY)
    } else {
        match paste.mode {
            PasteMode::UpdateByNumber => t!(
                i18n,
                ids::EDITORS_PIN_TABLE_PREVIEW_UPDATE,
                "rows" => preview.rows,
                "updated" => preview.updated,
                "added" => preview.added
            ),
            PasteMode::Append => {
                t!(i18n, ids::EDITORS_PIN_TABLE_PREVIEW_APPEND, "rows" => preview.rows)
            }
            PasteMode::ReplaceAll => t!(
                i18n,
                ids::EDITORS_PIN_TABLE_PREVIEW_REPLACE,
                "rows" => preview.rows,
                "pins" => preview.replaced
            ),
        }
    };
    let modes = form.segmented(
        PasteMode::ALL
            .into_iter()
            .map(|m| {
                (
                    mode_label(form, m),
                    paste.mode == m,
                    table(PinTableMsg::PasteModePicked(m)),
                )
            })
            .collect(),
    );
    let cancel = form.ghost(
        t!(i18n, ids::EDITORS_PIN_TABLE_PASTE_CANCEL),
        Some(table(PinTableMsg::PasteCancelled)),
        None,
    );
    let apply = button(text(t!(i18n, ids::EDITORS_PIN_TABLE_PASTE_APPLY)).size(TS.body))
        .padding([SP.xs, SP.m])
        .on_press_maybe((preview.rows > 0).then_some(table(PinTableMsg::PasteApplied)))
        .style(crate::styles::primary_button(form.tokens));
    let right: Column<Element<'a, PanelMsg>> = Column::new()
        .spacing(SP.xs)
        .width(Length::FillPortion(2))
        .push(form.mini_heading(t!(i18n, ids::EDITORS_PIN_TABLE_PASTE_COLUMNS)))
        .push(column_map(paste, form))
        .push(form.mini_heading(t!(i18n, ids::EDITORS_PIN_TABLE_PASTE_TYPE_WORDS)))
        .push(caption(
            form,
            t!(i18n, ids::EDITORS_PIN_TABLE_PASTE_TYPE_WORDS_HINT),
        ))
        .push(form.mini_heading(t!(i18n, ids::EDITORS_PIN_TABLE_PASTE_MODE)))
        .push(modes)
        .push(caption(form, counted))
        .push(
            row![cancel, apply.boxed()]
                .spacing(SP.s)
                .align_y(Alignment::Center)
                .boxed(),
        );
    container(
        row![left.boxed(), right.boxed()]
            .spacing(SP.l)
            .align_y(Alignment::Start),
    )
    .padding([SP.s, SP.l])
    .width(Length::Fill)
    .boxed()
}

/// One row per column: "Column 1" and what it is read as.
fn column_map<'a>(paste: &PasteDraft, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let mut map: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.xs);
    for index in 0..paste.column_count() {
        let choices = PasteField::ALL
            .into_iter()
            .map(|f| Choice::new(f, field_label(form, f)))
            .collect();
        map = map.push(
            row![
                text(t!(i18n, ids::EDITORS_PIN_TABLE_PASTE_COLUMN, "number" => index + 1))
                    .size(TS.caption)
                    .color(form.look.ink3)
                    .width(Length::Fill)
                    .boxed(),
                container(
                    form.dropdown(choices, Some(paste.field_of(index)), move |f| {
                        table(PinTableMsg::PasteColumnMapped(index, f))
                    })
                )
                .width(TS.body * MAP_EM)
                .boxed(),
            ]
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .boxed(),
        );
    }
    map.boxed()
}

fn caption<'a>(form: &Form<'_>, line: Localized) -> Element<'a, PanelMsg> {
    text(line).size(TS.caption).color(form.look.ink4).boxed()
}
