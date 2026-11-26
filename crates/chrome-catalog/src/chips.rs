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
use signex_widgets::theme_ext::TypeScale;
use signex_widgets::{Label, SegmentedStyle, raw, segmented};

use crate::catalog::Message;
use crate::theme;

/// Sample text for this page: fixture content, not catalog copy. The one
/// `raw` site of the page.
fn sample_text(text: impl Into<String>) -> Label<'static> {
    raw(text.into())
}

/// Body size of the Graphite type scale.
const BODY: f32 = TypeScale::DEFAULT.body;

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
            (Sample::A, sample_text("Project open")),
            (Sample::B, sample_text("Filter matches nothing")),
            (Sample::C, sample_text("No project open")),
        ],
        Sample::A,
        chip_style(tokens),
        m,
        ignore,
    );
    let filters = filter_chips(
        [
            (Sample::A, sample_text("Errors"), Some(3)),
            (Sample::B, sample_text("Warnings"), Some(12)),
            (Sample::C, sample_text("Notes"), Some(0)),
            (Sample::D, sample_text("Used On"), None),
        ],
        &FilterSet::new([Sample::A, Sample::D]),
        chip_style(tokens),
        m,
        crate::locale::en_us(),
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
        .push(kind_chip(sample_text("panel"), info, m))
        .push(kind_chip(sample_text("global"), neutral, m));
    let domains = tab_chips(
        [
            TabChip::new(Sample::A, sample_text("Clearance")).count(4),
            TabChip::new(Sample::B, sample_text("Physical")).count(1200),
            TabChip::new(Sample::C, sample_text("Mask")).count(0),
            TabChip::new(Sample::D, sample_text("Placement")),
            TabChip::new(Sample::E, sample_text("Signal integrity")).dimmed(true),
        ],
        Sample::B,
        tab_style(tokens),
        m,
        crate::locale::en_us(),
        ignore,
    );
    let seg_style = SegmentedStyle::new(
        ext(x.sunken, tokens.selection),
        ext(x.selected_segment.or(x.raised), tokens.panel_bg),
        ext(x.selected_segment_shadow.or(x.shadow), tokens.border),
        ext(x.ink3, tokens.text_secondary),
        ext(x.selected_segment_ink, tokens.text),
        BODY,
    );
    let modes = segmented(
        [
            (Sample::A, sample_text("Nets")),
            (Sample::B, sample_text("Net Classes")),
            (Sample::C, sample_text("Differential Pairs")),
            (Sample::D, sample_text("xSignals")),
            (Sample::E, sample_text("Polygons")),
        ],
        Sample::B,
        seg_style.fill_width(true),
        ignore,
    );
    let pages = segmented(
        [
            (Sample::A, sample_text("General")),
            (Sample::B, sample_text("Parameters")),
        ],
        Sample::B,
        seg_style,
        ignore,
    );

    column![variants, filters, kinds, domains, modes, pages]
        .spacing(m.tab_height / 2.0)
        .boxed()
}
