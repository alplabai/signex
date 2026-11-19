//! Chips: small pills for a choice among states ([`choice_chips`]), toggled
//! filters that carry a count ([`filter_chips`]), read-only kind tags
//! ([`kind_chip`]) and a bar of tabs with count badges ([`tab_chips`]).
//!
//! Stateless: the caller owns what is on and maps each press to a message.
//! Sizes derive from the body text size ([`ChipMetrics`]); colours come from
//! the caller's tokens.

use std::collections::BTreeSet;

use iced::alignment::{Horizontal, Vertical};
use iced::font::Weight;
use iced::widget::{Row, container};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Padding, Theme, Widget};

use crate::{Label, button, raw, text};

// Graphite chip sizes at a 12 px body, kept as ratios of the body size.
const CHIP_HEIGHT_EM: f32 = 22.0 / 12.0;
const KIND_HEIGHT_EM: f32 = 20.0 / 12.0;
const CHIP_PAD_EM: f32 = 8.0 / 12.0;
const CHIP_GAP_EM: f32 = 5.0 / 12.0;
const ROW_GAP_EM: f32 = 4.0 / 11.0;
const TAB_HEIGHT_EM: f32 = 28.0 / 12.0;
const TAB_PAD_EM: f32 = 10.0 / 12.0;
const TAB_GAP_EM: f32 = 6.0 / 12.0;
const TAB_SPACING_EM: f32 = 2.0 / 12.0;
const TAB_RADIUS_EM: f32 = 8.0 / 12.0;
const BADGE_HEIGHT_EM: f32 = 16.0 / 12.0;
const BADGE_PAD_EM: f32 = 4.0 / 12.0;
const BADGE_TEXT_EM: f32 = 10.0 / 12.0;
/// Caption step of the type scale (`TypeScale::caption`).
const CAPTION_RATIO: f32 = 10.0 / 11.0;

/// Opacity of a chip count next to its label.
const COUNT_ALPHA: f32 = 0.85;
/// Opacity of a dimmed tab (a domain marked for later).
const DIMMED_ALPHA: f32 = 0.55;
/// Opacity of a chip without a press message.
const DISABLED_ALPHA: f32 = 0.5;
/// Largest count shown in full; above it the badge reads "999+".
pub const COUNT_CAP: u32 = 999;

/// Sizes for every chip kind, derived from the body text size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChipMetrics {
    pub text_size: f32,
    pub caption_size: f32,
    pub height: f32,
    pub kind_height: f32,
    pub pad_h: f32,
    pub gap: f32,
    pub row_gap: f32,
    pub tab_height: f32,
    pub tab_pad_h: f32,
    pub tab_gap: f32,
    pub tab_spacing: f32,
    pub tab_radius: f32,
    pub badge_height: f32,
    pub badge_pad_h: f32,
    pub badge_text: f32,
}

impl ChipMetrics {
    /// Every size from `body`, the body text size of the type scale.
    pub fn from_text_size(body: f32) -> Self {
        Self {
            text_size: body,
            caption_size: body * CAPTION_RATIO,
            height: body * CHIP_HEIGHT_EM,
            kind_height: body * KIND_HEIGHT_EM,
            pad_h: body * CHIP_PAD_EM,
            gap: body * CHIP_GAP_EM,
            row_gap: body * ROW_GAP_EM,
            tab_height: body * TAB_HEIGHT_EM,
            tab_pad_h: body * TAB_PAD_EM,
            tab_gap: body * TAB_GAP_EM,
            tab_spacing: body * TAB_SPACING_EM,
            tab_radius: body * TAB_RADIUS_EM,
            badge_height: body * BADGE_HEIGHT_EM,
            badge_pad_h: body * BADGE_PAD_EM,
            badge_text: body * BADGE_TEXT_EM,
        }
    }

    /// Corner radius that rounds the ends of a pill `height` tall.
    pub fn pill_radius(height: f32) -> f32 {
        height / 2.0
    }
}

/// Colours of a choice or filter chip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChipStyle {
    /// Fill when off (the field token).
    pub fill: Color,
    /// Outline when off.
    pub edge: Color,
    /// Fill when off and hovered.
    pub hover: Color,
    /// Label colour when off.
    pub ink: Color,
    /// Fill when on (accent-soft).
    pub on_fill: Color,
    /// Label colour when on (accent-soft ink).
    pub on_ink: Color,
}

/// Fill and ink of a read-only kind chip ("panel", "modal", "global").
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChipTone {
    pub fill: Color,
    pub ink: Color,
}

/// Colours of a [`tab_chips`] bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabChipStyle {
    /// Fill of a hovered, unselected tab.
    pub hover: Color,
    /// Label colour of an unselected tab.
    pub ink: Color,
    /// Fill of the selected tab.
    pub on_fill: Color,
    /// Label colour of the selected tab.
    pub on_ink: Color,
    /// Badge for a non-zero count (the error tone).
    pub badge: ChipTone,
    /// Colour of a zero count.
    pub zero_ink: Color,
}

/// How a tab shows its count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountBadge {
    /// No count given: nothing is drawn.
    Hidden,
    /// A zero count: a quiet mono "0".
    Zero,
    /// A non-zero count: a filled pill.
    Alert,
}

impl CountBadge {
    pub fn for_count(count: Option<u32>) -> Self {
        match count {
            None => Self::Hidden,
            Some(0) => Self::Zero,
            Some(_) => Self::Alert,
        }
    }
}

/// The text of a count, capped at [`COUNT_CAP`].
pub fn count_text(count: u32) -> String {
    if count > COUNT_CAP {
        format!("{COUNT_CAP}+")
    } else {
        count.to_string()
    }
}

/// The set of filters that are on. An empty set filters nothing out.
/// It lives in the caller's model; [`filter_chips`] only reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterSet<K: Ord> {
    on: BTreeSet<K>,
}

impl<K: Ord> Default for FilterSet<K> {
    fn default() -> Self {
        Self {
            on: BTreeSet::new(),
        }
    }
}

impl<K: Ord + Copy> FilterSet<K> {
    pub fn new(on: impl IntoIterator<Item = K>) -> Self {
        Self {
            on: on.into_iter().collect(),
        }
    }

    pub fn is_on(&self, key: K) -> bool {
        self.on.contains(&key)
    }

    /// Flip `key`; returns whether it is now on.
    pub fn toggle(&mut self, key: K) -> bool {
        if self.on.remove(&key) {
            false
        } else {
            self.on.insert(key);
            true
        }
    }

    pub fn clear(&mut self) {
        self.on.clear();
    }

    /// Nothing is on, so every item shows.
    pub fn is_empty(&self) -> bool {
        self.on.is_empty()
    }

    /// Whether an item of kind `key` passes the filter.
    pub fn matches(&self, key: K) -> bool {
        self.on.is_empty() || self.on.contains(&key)
    }

    /// The keys that are on, in order.
    pub fn iter(&self) -> impl Iterator<Item = K> + '_ {
        self.on.iter().copied()
    }
}

fn faded(c: Color, alpha: f32) -> Color {
    Color {
        a: c.a * alpha,
        ..c
    }
}

fn medium() -> Font {
    Font {
        weight: Weight::Medium,
        ..Font::DEFAULT
    }
}

fn label_text<'a, M: 'a>(label: Label<'a>, size: f32, ink: Color, is_on: bool) -> Element<'a, M> {
    let mut t = text(label)
        .size(size)
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None);
    if is_on {
        t = t.font(medium());
    }
    t.boxed()
}

fn mono_text<'a, M: 'a>(value: String, size: f32, ink: Color) -> Element<'a, M> {
    text(raw(value))
        .size(size)
        .font(Font::MONOSPACE)
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// One chip: a pill with a label and an optional count. A chip without a
/// press message is drawn faded and inert.
pub fn chip<'a, M: Clone + 'a>(
    label: impl Into<Label<'a>>,
    count: Option<u32>,
    is_on: bool,
    style: ChipStyle,
    metrics: ChipMetrics,
    on_press: Option<M>,
) -> Element<'a, M> {
    let alpha = if on_press.is_some() {
        1.0
    } else {
        DISABLED_ALPHA
    };
    let ink = faded(if is_on { style.on_ink } else { style.ink }, alpha);
    let mut content: Row<Element<'a, M>> = Row::new()
        .spacing(metrics.gap)
        .height(Length::Fill)
        .align_y(Alignment::Center);
    content = content.push(label_text(label.into(), metrics.text_size, ink, is_on));
    if let Some(n) = count {
        content = content.push(mono_text(
            count_text(n),
            metrics.caption_size,
            faded(ink, COUNT_ALPHA),
        ));
    }

    let radius = ChipMetrics::pill_radius(metrics.height);
    let mut b = button(content)
        .padding(Padding::from([0.0, metrics.pad_h]))
        .height(metrics.height)
        .style(move |_: &Theme, status: button::Status| {
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            let (fill, edge) = match (is_on, hovered) {
                (true, _) => (style.on_fill, Color::TRANSPARENT),
                (false, true) => (style.hover, style.edge),
                (false, false) => (style.fill, style.edge),
            };
            button::Style {
                background: Some(Background::Color(faded(fill, alpha))),
                text_color: ink,
                border: Border {
                    color: faded(edge, alpha),
                    width: 1.0,
                    radius: radius.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        });
    if let Some(m) = on_press {
        b = b.on_press(m);
    }
    b.boxed()
}

/// A wrapping row of single-select chips: variant and state switchers.
/// `on_select` maps the picked value to the caller's message.
pub fn choice_chips<'a, V, M>(
    options: impl IntoIterator<Item = (V, Label<'a>)>,
    selected: V,
    style: ChipStyle,
    metrics: ChipMetrics,
    on_select: impl Fn(V) -> M + 'a,
) -> Element<'a, M>
where
    V: Copy + PartialEq + 'a,
    M: Clone + 'a,
{
    let mut row: Row<Element<'a, M>> = Row::new().spacing(metrics.row_gap);
    for (value, label) in options {
        let on_press = Some(on_select(value));
        row = row.push(chip(
            label,
            None,
            value == selected,
            style,
            metrics,
            on_press,
        ));
    }
    row.wrap().vertical_spacing(metrics.row_gap).boxed()
}

/// A wrapping row of toggle chips, each with an optional count. A chip is
/// on when `active` holds its key; `on_toggle` maps the key to a message.
pub fn filter_chips<'a, K, M>(
    options: impl IntoIterator<Item = (K, Label<'a>, Option<u32>)>,
    active: &FilterSet<K>,
    style: ChipStyle,
    metrics: ChipMetrics,
    on_toggle: impl Fn(K) -> M + 'a,
) -> Element<'a, M>
where
    K: Ord + Copy + 'a,
    M: Clone + 'a,
{
    let mut row: Row<Element<'a, M>> = Row::new().spacing(metrics.row_gap);
    for (key, label, count) in options {
        let on_press = Some(on_toggle(key));
        row = row.push(chip(
            label,
            count,
            active.is_on(key),
            style,
            metrics,
            on_press,
        ));
    }
    row.wrap().vertical_spacing(metrics.row_gap).boxed()
}

/// A read-only kind tag: caption text on a tone fill, pill ends.
pub fn kind_chip<'a, M: 'a>(
    label: impl Into<Label<'a>>,
    tone: ChipTone,
    metrics: ChipMetrics,
) -> Element<'a, M> {
    let radius = ChipMetrics::pill_radius(metrics.kind_height);
    container(
        text(label.into())
            .size(metrics.caption_size)
            .color(tone.ink)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .height(metrics.kind_height)
    .padding(Padding::from([0.0, metrics.pad_h]))
    .align_y(Vertical::Center)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(tone.fill)),
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// One tab of a [`tab_chips`] bar.
#[derive(Debug, Clone)]
pub struct TabChip<'a, V> {
    pub value: V,
    pub label: Label<'a>,
    /// Shown as a badge; see [`CountBadge`].
    pub count: Option<u32>,
    /// Drawn faded but still selectable (a domain marked for later).
    pub is_dimmed: bool,
}

impl<'a, V> TabChip<'a, V> {
    pub fn new(value: V, label: impl Into<Label<'a>>) -> Self {
        Self {
            value,
            label: label.into(),
            count: None,
            is_dimmed: false,
        }
    }

    pub fn count(mut self, count: u32) -> Self {
        self.count = Some(count);
        self
    }

    pub fn dimmed(mut self, is_dimmed: bool) -> Self {
        self.is_dimmed = is_dimmed;
        self
    }
}

fn count_badge<'a, M: 'a>(
    count: Option<u32>,
    style: TabChipStyle,
    metrics: ChipMetrics,
    alpha: f32,
) -> Option<Element<'a, M>> {
    let n = count?;
    match CountBadge::for_count(count) {
        CountBadge::Hidden => None,
        CountBadge::Zero => Some(mono_text(
            count_text(n),
            metrics.caption_size,
            faded(style.zero_ink, alpha),
        )),
        CountBadge::Alert => {
            let fill = faded(style.badge.fill, alpha);
            let radius = ChipMetrics::pill_radius(metrics.badge_height);
            Some(
                container(mono_text(
                    count_text(n),
                    metrics.badge_text,
                    faded(style.badge.ink, alpha),
                ))
                .height(metrics.badge_height)
                .padding(Padding::from([0.0, metrics.badge_pad_h]))
                .align_x(Horizontal::Center)
                .align_y(Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(fill)),
                    border: Border {
                        radius: radius.into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                })
                .boxed(),
            )
        }
    }
}

/// A bar of tabs that carry counts, such as a constraint-domain bar.
/// `on_select` maps the picked tab's value to the caller's message.
pub fn tab_chips<'a, V, M>(
    tabs: impl IntoIterator<Item = TabChip<'a, V>>,
    selected: V,
    style: TabChipStyle,
    metrics: ChipMetrics,
    on_select: impl Fn(V) -> M + 'a,
) -> Element<'a, M>
where
    V: Copy + PartialEq + 'a,
    M: Clone + 'a,
{
    let mut bar: Row<Element<'a, M>> = Row::new().spacing(metrics.tab_spacing);
    for tab in tabs {
        let is_on = tab.value == selected;
        let alpha = if tab.is_dimmed { DIMMED_ALPHA } else { 1.0 };
        let ink = faded(if is_on { style.on_ink } else { style.ink }, alpha);
        let mut content: Row<Element<'a, M>> = Row::new()
            .spacing(metrics.tab_gap)
            .height(Length::Fill)
            .align_y(Alignment::Center);
        content = content.push(label_text(tab.label, metrics.text_size, ink, is_on));
        if let Some(badge) = count_badge(tab.count, style, metrics, alpha) {
            content = content.push(badge);
        }
        let radius = metrics.tab_radius;
        bar = bar.push(
            button(content)
                .padding(Padding::from([0.0, metrics.tab_pad_h]))
                .height(metrics.tab_height)
                .on_press(on_select(tab.value))
                .style(move |_: &Theme, status: button::Status| {
                    let hovered =
                        matches!(status, button::Status::Hovered | button::Status::Pressed);
                    let fill = match (is_on, hovered) {
                        (true, _) => Some(style.on_fill),
                        (false, true) => Some(style.hover),
                        (false, false) => None,
                    };
                    button::Style {
                        background: fill.map(Background::Color),
                        text_color: ink,
                        border: Border {
                            radius: radius.into(),
                            ..Border::default()
                        },
                        ..button::Style::default()
                    }
                })
                .boxed(),
        );
    }
    bar.boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_at_the_12_px_body_match_the_mockup() {
        let m = ChipMetrics::from_text_size(12.0);
        assert!((m.height - 22.0).abs() < 1e-4);
        assert!((m.kind_height - 20.0).abs() < 1e-4);
        assert!((m.tab_height - 28.0).abs() < 1e-4);
        assert!((m.pad_h - 8.0).abs() < 1e-4);
        assert!((m.badge_height - 16.0).abs() < 1e-4);
        assert!((m.caption_size - 12.0 * 10.0 / 11.0).abs() < 1e-4);
    }

    #[test]
    fn metrics_scale_with_the_body_size() {
        let a = ChipMetrics::from_text_size(12.0);
        let b = ChipMetrics::from_text_size(24.0);
        assert!((b.height - 2.0 * a.height).abs() < 1e-4);
        assert!((b.tab_radius - 2.0 * a.tab_radius).abs() < 1e-4);
        assert_eq!(ChipMetrics::pill_radius(b.height), b.height / 2.0);
    }

    #[test]
    fn count_badge_kind_follows_the_count() {
        assert_eq!(CountBadge::for_count(None), CountBadge::Hidden);
        assert_eq!(CountBadge::for_count(Some(0)), CountBadge::Zero);
        assert_eq!(CountBadge::for_count(Some(7)), CountBadge::Alert);
    }

    #[test]
    fn count_text_caps_large_counts() {
        assert_eq!(count_text(0), "0");
        assert_eq!(count_text(COUNT_CAP), "999");
        assert_eq!(count_text(COUNT_CAP + 1), "999+");
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    enum Kind {
        Errors,
        Warnings,
        Notes,
    }

    #[test]
    fn empty_filter_set_matches_everything() {
        let set = FilterSet::<Kind>::default();
        assert!(set.is_empty());
        assert!(set.matches(Kind::Errors) && set.matches(Kind::Notes));
    }

    #[test]
    fn toggle_flips_and_reports_the_new_state() {
        let mut set = FilterSet::default();
        assert!(set.toggle(Kind::Warnings));
        assert!(set.is_on(Kind::Warnings));
        assert!(set.matches(Kind::Warnings));
        assert!(!set.matches(Kind::Errors));
        assert!(!set.toggle(Kind::Warnings));
        assert!(set.is_empty());
    }

    #[test]
    fn new_and_clear_and_iter_order() {
        let mut set = FilterSet::new([Kind::Notes, Kind::Errors]);
        assert_eq!(set.iter().collect::<Vec<_>>(), [Kind::Errors, Kind::Notes]);
        set.clear();
        assert!(set.is_empty());
    }

    fn style() -> ChipStyle {
        ChipStyle {
            fill: Color::WHITE,
            edge: Color::BLACK,
            hover: Color::WHITE,
            ink: Color::BLACK,
            on_fill: Color::BLACK,
            on_ink: Color::WHITE,
        }
    }

    fn tab_style() -> TabChipStyle {
        TabChipStyle {
            hover: Color::WHITE,
            ink: Color::BLACK,
            on_fill: Color::BLACK,
            on_ink: Color::WHITE,
            badge: ChipTone {
                fill: Color::BLACK,
                ink: Color::WHITE,
            },
            zero_ink: Color::BLACK,
        }
    }

    #[test]
    fn every_chip_kind_builds_an_element() {
        let m = ChipMetrics::from_text_size(12.0);
        let set = FilterSet::new([Kind::Errors]);
        let _: Element<'_, Kind> = choice_chips(
            [(Kind::Errors, raw("Errors")), (Kind::Notes, raw("Notes"))],
            Kind::Notes,
            style(),
            m,
            |k| k,
        );
        let _: Element<'_, Kind> = filter_chips(
            [
                (Kind::Errors, raw("Errors"), Some(3)),
                (Kind::Warnings, raw("Warnings"), None),
            ],
            &set,
            style(),
            m,
            |k| k,
        );
        let tone = ChipTone {
            fill: Color::WHITE,
            ink: Color::BLACK,
        };
        let _: Element<'_, Kind> = kind_chip(raw("panel"), tone, m);
        let _: Element<'_, Kind> = chip(raw("Off"), Some(2), false, style(), m, None);
        let _: Element<'_, Kind> = tab_chips(
            [
                TabChip::new(Kind::Errors, raw("Clearance")).count(4),
                TabChip::new(Kind::Warnings, raw("Physical")).count(0),
                TabChip::new(Kind::Notes, raw("Signal integrity")).dimmed(true),
            ],
            Kind::Errors,
            tab_style(),
            m,
            |k| k,
        );
    }
}
