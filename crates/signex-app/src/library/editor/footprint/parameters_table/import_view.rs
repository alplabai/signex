//! Import from another footprint (mockup row 189): the picked footprint's
//! parameters with a tick each. A name already in use stays unticked, so an
//! import never overwrites a value.

use iced::widget::{Space, column, row, scrollable};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, checkbox, text};

use super::SketchParametersMsg;
use super::state::{ImportRow, ParametersImport};
use super::view::{Look, Sender, caption, mono, popup};
use crate::library::messages::LibraryMessage;
use crate::styles::{DialogButtonKind, dialog_button};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Tallest the list grows before it scrolls, in body text heights.
const LIST_MAX_EM: f32 = 12.0;

fn import_row<'a>(
    line: &ImportRow,
    send: &Sender,
    look: Look,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let name = line.name.clone();
    let send = send.clone();
    let mut tick = checkbox(line.is_picked)
        .labelled(UserText::new(line.name.as_str()))
        .size(TS.label)
        .text_size(TS.caption)
        .spacing(SP.s)
        .style(crate::panels::form_look::checkbox_style);
    if !line.is_refused {
        tick = tick.on_toggle(move |_| {
            send.msg(SketchParametersMsg::ImportRowToggled { name: name.clone() })
        });
    }
    let mut cells = row![
        tick.width(Length::FillPortion(2)),
        text(UserText::new(line.expr.as_str()))
            .size(TS.caption)
            .font(mono())
            .color(look.muted)
            .width(Length::FillPortion(3)),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    if line.is_refused {
        cells = cells.push(caption(
            t!(i18n, ids::SKETCH_PARAMETERS_IMPORT_TAKEN),
            look.error,
        ));
    }
    cells.boxed()
}

/// The import list, raised over the table's footer.
pub(super) fn panel<'a>(
    import: &ParametersImport,
    send: &Sender,
    look: Look,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let mut list = column![].spacing(SP.xs);
    if import.rows.is_empty() {
        list = list.push(caption(
            t!(i18n, ids::SKETCH_PARAMETERS_IMPORT_NONE),
            look.muted,
        ));
    }
    for line in &import.rows {
        list = list.push(import_row(line, send, look, i18n));
    }
    let count = import.picked().count();
    let apply = (count > 0).then(|| send.msg(SketchParametersMsg::ImportConfirmed));
    let body = column![
        text(t!(
            i18n,
            ids::SKETCH_PARAMETERS_IMPORT_FROM,
            "footprint" => import.source.as_str()
        ))
        .size(TS.body)
        .color(look.ink),
        scrollable(list).height(Length::Fit.max(TS.body * LIST_MAX_EM)),
        row![
            Space::new().width(Length::Fill),
            dialog_button(
                t!(i18n, ids::SKETCH_PARAMETERS_CANCEL),
                DialogButtonKind::Cancel,
                Some(send.msg(SketchParametersMsg::ImportCancelled)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::SKETCH_PARAMETERS_IMPORT_APPLY, "count" => count),
                DialogButtonKind::Primary,
                apply,
                tokens,
            ),
        ]
        .spacing(SP.s),
    ]
    .spacing(SP.s);
    popup(body.boxed(), tokens)
}
