//! Chips and the segmented mode switch, in the states the Graphite mockups
//! show: variant chips, filter chips with counts, kind chips, a domain bar
//! with count badges, and a five-mode panel switch.

use iced::widget::{Row, column};
use iced::{Color, Element, Widget};
use signex_types::theme::{Color as ThemeColor, ThemeTokens};
use signex_widgets::chips::{
    ChipMetrics, ChipStyle, ChipTone, FilterSet, TabChip, TabChipStyle, choice_chips, filter_chips,
    kind_chip, tab_chips,
};
use signex_widgets::{SegmentedStyle, raw, segmented};

use crate::catalog::Message;
use crate::theme;

/// Body size of the Graphite type scale.
const BODY: f32 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Sample {
    A,
    B,
    C,
    D,
    E,
}

fn ext(token: Option<ThemeColor>, fallback: ThemeColor) -> Color {
    theme::color(token.unwrap_or(fallback))
}

fn chip_style(t: &ThemeTokens) -> ChipStyle {
    let x = &t.extended;
    ChipStyle {
        fill: ext(x.field, t.paper),
        edge: theme::color(t.border),
        hover: theme::color(t.hover),
        ink: ext(x.ink3, t.text_secondary),
        on_fill: ext(x.accent_soft, t.selection),
        on_ink: ext(x.accent_soft_ink, t.accent),
    }
}

fn tab_style(t: &ThemeTokens) -> TabChipStyle {
    let x = &t.extended;
    TabChipStyle {
        hover: theme::color(t.hover),
        ink: ext(x.ink3, t.text_secondary),
        on_fill: ext(x.accent_soft, t.selection),
        on_ink: ext(x.accent_soft_ink, t.accent),
        badge: ChipTone {
            fill: theme::color(t.error),
            ink: theme::color(t.paper),
        },
        zero_ink: ext(x.ink4, t.text_secondary),
    }
}

/// Static samples: presses are ignored, like the project-tree samples.
pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let m = ChipMetrics::from_text_size(BODY);
    let ignore = |_: Sample| Message::TreeInteracted;
    let x = &tokens.extended;

    let variants = choice_chips(
        [
            (Sample::A, raw("Project open")),
            (Sample::B, raw("Filter matches nothing")),
            (Sample::C, raw("No project open")),
        ],
        Sample::A,
        chip_style(tokens),
        m,
        ignore,
    );
    let filters = filter_chips(
        [
            (Sample::A, raw("Errors"), Some(3)),
            (Sample::B, raw("Warnings"), Some(12)),
            (Sample::C, raw("Notes"), Some(0)),
            (Sample::D, raw("Used On"), None),
        ],
        &FilterSet::new([Sample::A, Sample::D]),
        chip_style(tokens),
        m,
        ignore,
    );
    let neutral = ChipTone {
        fill: ext(x.sunken, tokens.selection),
        ink: ext(x.ink3, tokens.text_secondary),
    };
    let info = ChipTone {
        fill: ext(x.info_soft, tokens.selection),
        ink: ext(x.info, tokens.accent),
    };
    let kinds: Row<Element<'a, Message>> = Row::new()
        .spacing(m.row_gap)
        .push(kind_chip(raw("panel"), info, m))
        .push(kind_chip(raw("global"), neutral, m));
    let domains = tab_chips(
        [
            TabChip::new(Sample::A, raw("Clearance")).count(4),
            TabChip::new(Sample::B, raw("Physical")).count(1200),
            TabChip::new(Sample::C, raw("Mask")).count(0),
            TabChip::new(Sample::D, raw("Placement")),
            TabChip::new(Sample::E, raw("Signal integrity")).dimmed(true),
        ],
        Sample::B,
        tab_style(tokens),
        m,
        ignore,
    );
    let seg_style = SegmentedStyle::new(
        ext(x.sunken, tokens.selection),
        ext(x.raised, tokens.panel_bg),
        ext(x.shadow, tokens.border),
        ext(x.ink3, tokens.text_secondary),
        theme::color(tokens.text),
        BODY,
    );
    let modes = segmented(
        [
            (Sample::A, raw("Nets")),
            (Sample::B, raw("Net Classes")),
            (Sample::C, raw("Differential Pairs")),
            (Sample::D, raw("xSignals")),
            (Sample::E, raw("Polygons")),
        ],
        Sample::B,
        seg_style.fill_width(true),
        ignore,
    );
    let pages = segmented(
        [(Sample::A, raw("General")), (Sample::B, raw("Parameters"))],
        Sample::B,
        seg_style,
        ignore,
    );

    column![variants, filters, kinds, domains, modes, pages]
        .spacing(m.tab_height / 2.0)
        .boxed()
}
