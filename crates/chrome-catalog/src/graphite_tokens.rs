//! Graphite token sheet: every colour token as a swatch, the power and wire
//! colours, the thin scrollbar and the disabled / read-only looks, so the
//! tokens can be checked against the mockup `:root` blocks.

use iced::Widget;
use iced::widget::{Space, column, container, row, scrollable};
use iced::{Background, Border, Color, Element, Theme};
use signex_types::theme::{Color as SxColor, ThemeId, ThemeTokens, canvas_colors, theme_tokens};
use signex_widgets::theme_ext::{
    self, ChromeMetrics, HAIRLINE, Radii, Spacing, TypeScale, to_color,
};
use signex_widgets::{Label, button, raw, text};

use crate::catalog::Message;

/// Sample text for this page: fixture content, not catalog copy. The one
/// `raw` site of the page.
fn sample_text(text: impl Into<String>) -> Label<'static> {
    raw(text.into())
}

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
const METRICS: ChromeMetrics = ChromeMetrics::DEFAULT;
/// Swatches per grid row.
const PER_ROW: usize = 7;
/// Rows in the scrollbar sample; more than its height shows.
const SAMPLE_ROWS: usize = 24;
/// Visible rows of the scrollbar sample.
const VISIBLE_ROWS: f32 = 5.0;

type Swatch = (&'static str, Color);

/// The catalog section holding the token sheet for `theme`.
pub(crate) fn section(theme: ThemeId, tokens: &ThemeTokens) -> Element<'static, Message> {
    crate::section::view("Graphite tokens", tokens, view(theme))
}

/// The token sheet for `theme`.
fn view(theme: ThemeId) -> Element<'static, Message> {
    let tokens = theme_tokens(theme);
    let canvas = canvas_colors(theme);
    let ink = to_color(&tokens.text);
    let canvas_swatches = vec![
        ("power", theme_ext::power_port(&canvas)),
        ("wire", to_color(&canvas.wire)),
        ("grid", to_color(&canvas.grid)),
    ];
    column![
        grid(ink, base_swatches(&tokens)),
        grid(ink, extended_swatches(&tokens)),
        grid(ink, canvas_swatches),
        row![scroll_sample(&tokens), states_sample(&tokens)].spacing(SP.xl),
    ]
    .spacing(SP.l)
    .boxed()
}

fn base_swatches(t: &ThemeTokens) -> Vec<Swatch> {
    [
        ("bg", t.bg),
        ("paper", t.paper),
        ("text", t.text),
        ("text_secondary", t.text_secondary),
        ("accent", t.accent),
        ("border", t.border),
        ("panel_bg", t.panel_bg),
        ("toolbar_bg", t.toolbar_bg),
        ("statusbar_bg", t.statusbar_bg),
        ("selection", t.selection),
        ("hover", t.hover),
        ("error", t.error),
        ("warning", t.warning),
        ("success", t.success),
    ]
    .into_iter()
    .map(|(name, c)| (name, to_color(&c)))
    .collect()
}

/// The extended tokens the theme sets; Graphite sets them all.
fn extended_swatches(t: &ThemeTokens) -> Vec<Swatch> {
    let e = &t.extended;
    let all: [(&'static str, Option<SxColor>); 19] = [
        ("raised", e.raised),
        ("field", e.field),
        ("sunken", e.sunken),
        ("line_soft", e.line_soft),
        ("line_strong", e.line_strong),
        ("ink3", e.ink3),
        ("ink4", e.ink4),
        ("accent_text", e.accent_text),
        ("accent_soft", e.accent_soft),
        ("accent_soft_ink", e.accent_soft_ink),
        ("primary", e.primary),
        ("primary_ink", e.primary_ink),
        ("info", e.info),
        ("ok_soft", e.ok_soft),
        ("warn_soft", e.warn_soft),
        ("err_soft", e.err_soft),
        ("info_soft", e.info_soft),
        ("shadow", e.shadow),
        ("scrim", e.scrim),
    ];
    all.into_iter()
        .filter_map(|(name, c)| c.map(|c| (name, to_color(&c))))
        .collect()
}

fn grid(ink: Color, swatches: Vec<Swatch>) -> Element<'static, Message> {
    let mut rows = column![].spacing(SP.m);
    for chunk in swatches.chunks(PER_ROW) {
        let mut line = row![].spacing(SP.m);
        for &(name, fill) in chunk {
            line = line.push(swatch(ink, name, fill));
        }
        rows = rows.push(line);
    }
    rows.boxed()
}

fn swatch(ink: Color, name: &'static str, fill: Color) -> Element<'static, Message> {
    let chip = container(Space::new().width(METRICS.row * 3.0).height(METRICS.row)).style(
        move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                width: HAIRLINE,
                color: Color { a: 0.3, ..ink },
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        },
    );
    column![chip, text(sample_text(name)).size(TS.caption).color(ink)]
        .spacing(SP.xs)
        .boxed()
}

fn line_strong_and_ink4(t: &ThemeTokens) -> (Color, Color) {
    let pick = |set: Option<SxColor>, base: SxColor| to_color(&set.unwrap_or(base));
    (
        pick(t.extended.line_strong, t.border),
        pick(t.extended.ink4, t.text_secondary),
    )
}

/// A list taller than its viewport, scrolled with the thin scrollbar.
fn scroll_sample(t: &ThemeTokens) -> Element<'static, Message> {
    let ink = to_color(&t.text);
    let (thumb, thumb_hot) = line_strong_and_ink4(t);
    let mut list = column![];
    for i in 1..=SAMPLE_ROWS {
        list = list.push(
            container(
                text(sample_text(format!("Row {i}")))
                    .size(TS.body)
                    .color(ink),
            )
            .height(METRICS.row)
            .padding([0.0, SP.m]),
        );
    }
    scrollable(list)
        .direction(scrollable::Direction::Vertical(theme_ext::thin_scrollbar(
            &METRICS,
        )))
        .style(theme_ext::thin_scrollbar_style(thumb, thumb_hot))
        .width(METRICS.row * 8.0)
        .height(METRICS.row * VISIBLE_ROWS)
        .boxed()
}

/// An enabled and a disabled button, and a read-only field.
fn states_sample(t: &ThemeTokens) -> Element<'static, Message> {
    let ink = to_color(&t.text);
    let line = to_color(&t.border);
    let fill = to_color(&t.extended.raised.unwrap_or(t.panel_bg));
    let (_, ink4) = line_strong_and_ink4(t);
    let sunken = to_color(&t.extended.sunken.unwrap_or(t.hover));
    let ink3 = to_color(&t.extended.ink3.unwrap_or(t.text_secondary));
    let base = button::Style {
        background: Some(Background::Color(fill)),
        text_color: ink,
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.button.into(),
            ..Border::default()
        },
        ..button::Style::default()
    };
    // Static samples: neither takes a press; `enabled` picks the look.
    let sample = |label: &'static str, enabled: bool| -> Element<'static, Message> {
        let style = if enabled {
            base
        } else {
            theme_ext::dimmed_button(base)
        };
        button(text(sample_text(label)).size(TS.body))
            .height(METRICS.button)
            .padding([SP.s, SP.l])
            .style(move |_: &Theme, _: button::Status| style)
            .boxed()
    };
    let read_only = container(
        text(sample_text("Read-only 10 mm"))
            .size(TS.body)
            .color(ink3),
    )
    .height(METRICS.field)
    .padding([SP.s, SP.m])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(sunken)),
        border: Border {
            width: HAIRLINE,
            color: line,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    column![
        row![sample("Enabled", true), sample("Disabled", false)].spacing(SP.m),
        read_only,
        text(sample_text("Disabled hint text"))
            .size(TS.caption)
            .color(ink4),
    ]
    .spacing(SP.m)
    .boxed()
}
