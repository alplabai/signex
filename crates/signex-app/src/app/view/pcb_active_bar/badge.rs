//! The Selection Filter's badge on the PCB Active Bar (m108): the number of
//! object kinds a click or box cannot pick, pinned to the top-right corner
//! of the filter button. No badge while every kind is on. The badge is a
//! layer stacked over the bar, so the bar's buttons stay as they are; the
//! layer is text and fills only and takes no input.

use iced::widget::{Space, column, container, row, stack};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use pcb_geom::{PickFilter, PickKind};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::active_bar::Metrics;
use signex_widgets::text;

use crate::app::Message;
use crate::styles::tokens::resolve;
use crate::styles::{Radii, TypeScale, chrome_px};

/// Height and smallest width of the badge (bar geometry, like
/// [`Metrics`]).
const BADGE_SIZE: f32 = chrome_px(12.0 * 11.0 / 12.0);
/// Space left and right of a count wider than the round badge.
const BADGE_PAD_X: f32 = chrome_px(2.0 * 11.0 / 12.0);
/// Gap between the badge and the button's top and right edges.
const BADGE_INSET: f32 = chrome_px(11.0 / 12.0);

/// How many kinds the filter has off, of those the board editor can pick.
pub(super) fn off_count(filter: &PickFilter) -> usize {
    PickKind::ALL
        .into_iter()
        .filter(|kind| kind.is_pickable() && !filter.is_on(*kind))
        .count()
}

/// `bar` with the badge of `off` kinds over the filter button, whose left
/// edge is `slot_x` from the bar's left edge; `bar` unchanged when nothing
/// is off or the bar has no filter button.
pub(super) fn with_filter_badge<'a>(
    bar: Element<'a, Message>,
    slot_x: Option<f32>,
    off: usize,
    tokens: &ThemeTokens,
    i18n: &I18n,
    ui_family: &str,
) -> Element<'a, Message> {
    let Some(slot_x) = slot_x.filter(|_| off > 0) else {
        return bar;
    };
    let m = Metrics::GRAPHITE;
    let count = t!(i18n, ids::PCB_BAR_FILTER_OFF_COUNT, "count" => off);
    let size = TypeScale::DEFAULT.badge;
    let family = signex_types::font::nonblank_family(ui_family);
    let shown = count.to_string();
    let width = (signex_types::font::estimate_text_width_px(&shown, size, family)
        + 2.0 * BADGE_PAD_X)
        .max(BADGE_SIZE);
    let resolved = resolve(tokens);
    let (fill, ink) = (resolved.primary, resolved.primary_ink);
    let font = iced::Font {
        weight: iced::font::Weight::Semibold,
        ..crate::fonts::iced_font_for_family(family.unwrap_or(crate::fonts::DEFAULT_UI_FONT))
    };
    let pill = container(text(count).size(size).color(ink).font(font))
        .width(Length::Fixed(width))
        .height(Length::Fixed(BADGE_SIZE))
        .center_x(Length::Fixed(width))
        .center_y(Length::Fixed(BADGE_SIZE))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let right = slot_x + m.btn - BADGE_INSET;
    let layer = column![
        Space::new().height(m.padding + BADGE_INSET),
        row![Space::new().width((right - width).max(0.0)), pill],
    ];
    stack![bar, layer].boxed()
}
