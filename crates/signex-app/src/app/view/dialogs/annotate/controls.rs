//! Small form controls used by the Annotate modal: the inline checkbox pip,
//! the uppercase section label, the order-of-processing dropdown, the Lock
//! column header checkbox (lock or unlock every row) and the shared
//! bordered-container style. Graphite looks come from `graphite_annotate`.

use super::super::*;
use iced::widget::{container, pick_list, scrollable};
use iced::{Background, Border, Element, Theme};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, raw, text};

use super::super::graphite_annotate as ga;
use crate::app::state::AnnotateOrder;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;

/// Scroll-rail style of the modal's lists: a quiet pill thumb (the
/// form-look rail).
pub(super) fn scroll_style(
    theme_tokens: &ThemeTokens,
) -> impl Fn(&Theme, scrollable::Status) -> scrollable::Style + 'static {
    let r = crate::styles::tokens::resolve(theme_tokens);
    let (thumb, thumb_hot) = (r.line_strong, r.ink4);
    move |theme: &Theme, status: scrollable::Status| {
        let mut style = scrollable::default(theme, status);
        let idle = matches!(status, scrollable::Status::Active { .. });
        for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
            rail.background = None;
            rail.border = Border::default();
            rail.scroller.background = Background::Color(if idle { thumb } else { thumb_hot });
            rail.scroller.border = Border {
                radius: 999.0.into(),
                ..Border::default()
            };
        }
        style
    }
}

/// Greyed checkbox pip for the parameter-matching list, which the
/// annotation engine does not read yet.
pub(super) fn disabled_check_pip(
    on: bool,
    theme_tokens: &ThemeTokens,
) -> Element<'static, Message> {
    ga::disabled_checkbox_pip(theme_tokens, on)
}

pub(super) fn bordered_style(
    theme_tokens: &ThemeTokens,
) -> Box<dyn Fn(&Theme) -> container::Style + 'static> {
    Box::new(ga::field_box(theme_tokens))
}

/// An uppercase section label (ORDER OF PROCESSING): the section size and
/// the muted section ink, in capitals for the message language.
pub(super) fn section_heading<'a>(
    i18n: &I18n,
    label: MessageId,
    theme_tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let words = t!(i18n, label).into_string();
    text(raw(i18n.upper_for_display(&words)))
        .size(TS.section)
        .style(crate::styles::section_label(theme_tokens))
        .boxed()
}

/// One entry of the order-of-processing dropdown.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct OrderChoice {
    pub(super) order: AnnotateOrder,
    pub(super) label: String,
}

/// The four orders, in the dropdown's order, with their words.
pub(super) fn order_choices(i18n: &I18n) -> Vec<OrderChoice> {
    [
        (
            AnnotateOrder::UpThenAcross,
            ids::DIALOG_ANNOTATE_ORDER_UP_THEN_ACROSS,
        ),
        (
            AnnotateOrder::AcrossThenDown,
            ids::DIALOG_ANNOTATE_ORDER_ACROSS_THEN_DOWN,
        ),
        (
            AnnotateOrder::DownThenAcross,
            ids::DIALOG_ANNOTATE_ORDER_DOWN_THEN_ACROSS,
        ),
        (
            AnnotateOrder::AcrossThenUp,
            ids::DIALOG_ANNOTATE_ORDER_ACROSS_THEN_UP,
        ),
    ]
    .into_iter()
    .map(|(order, id)| OrderChoice {
        order,
        label: t!(i18n, id).into_string(),
    })
    .collect()
}

/// The order-of-processing dropdown (Graphite pick list).
pub(super) fn order_dropdown<'a>(i18n: &I18n, current: AnnotateOrder) -> Element<'a, Message> {
    let options = order_choices(i18n);
    let selected = options.iter().find(|c| c.order == current).cloned();
    pick_list(selected, options, |c: &OrderChoice| c.label.clone())
        .graphite()
        .on_select(|c: OrderChoice| Message::Annotate(AnnotateMsg::OrderChanged(c.order)))
        .width(iced::Length::Fill)
        .boxed()
}

/// Whether every listed symbol is locked: the Lock column header shows a
/// tick then. An empty list is never "all locked".
pub(super) fn all_listed_locked(
    listed: impl IntoIterator<Item = uuid::Uuid>,
    locked: &std::collections::HashSet<uuid::Uuid>,
) -> bool {
    let mut any = false;
    for uuid in listed {
        if !locked.contains(&uuid) {
            return false;
        }
        any = true;
    }
    any
}

/// The message of the Lock column header checkbox (the former All On / All
/// Off buttons): unlock every row when all are locked, else lock them all.
pub(super) fn lock_all_message(all_locked: bool) -> Message {
    Message::Annotate(AnnotateMsg::SetAllLocked(!all_locked))
}

/// The Lock column header: a checkbox that locks or unlocks every row.
pub(super) fn lock_all_header<'a>(
    all_locked: bool,
    enabled: bool,
    theme_tokens: &ThemeTokens,
) -> Element<'a, Message> {
    // Disabled (no rows, loading, or a run busy): drawn dimmed, like the
    // other read-only boxes in the dialog.
    if !enabled {
        return disabled_check_pip(all_locked, theme_tokens);
    }
    button(ga::checkbox_pip(theme_tokens, all_locked))
        .on_press(lock_all_message(all_locked))
        .padding(0.0)
        .style(ga::bare_button_style)
        .boxed()
}

/// The line under the Annotation Summary that says what "Re-prefix to
/// standard" will do, or why it is unavailable (#1315).
pub(super) fn reprefix_note(
    i18n: &I18n,
    standard: Option<signex_types::designator::DesignatorStandard>,
) -> Localized {
    match standard {
        None => t!(i18n, ids::DIALOG_ANNOTATE_REPREFIX_UNAVAILABLE),
        Some(standard) => t!(
            i18n,
            ids::DIALOG_ANNOTATE_REPREFIX_NOTE,
            "standard" => standard.to_string()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn order_dropdown_offers_all_four_orders_once() {
        let choices = order_choices(&I18n::en_us());
        let orders: HashSet<AnnotateOrder> = choices.iter().map(|c| c.order).collect();
        assert_eq!(choices.len(), 4);
        assert_eq!(orders.len(), 4);
        assert!(choices.iter().all(|c| !c.label.is_empty()));
    }

    #[test]
    fn lock_header_is_ticked_only_when_every_listed_row_is_locked() {
        let (a, b) = (uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
        let mut locked = HashSet::new();
        assert!(!all_listed_locked([a, b], &locked));
        locked.insert(a);
        assert!(!all_listed_locked([a, b], &locked));
        locked.insert(b);
        assert!(all_listed_locked([a, b], &locked));
        assert!(
            !all_listed_locked(std::iter::empty(), &locked),
            "an empty list is not all locked"
        );
    }

    #[test]
    fn lock_header_locks_all_unless_all_are_locked_then_unlocks() {
        assert!(matches!(
            lock_all_message(false),
            Message::Annotate(AnnotateMsg::SetAllLocked(true))
        ));
        assert!(matches!(
            lock_all_message(true),
            Message::Annotate(AnnotateMsg::SetAllLocked(false))
        ));
    }
}
