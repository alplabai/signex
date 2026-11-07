//! Graphite parts shared by the dialog bodies: the subtitled header, the
//! severity chips, the changed-row tint and the recovery icon tile. Callers
//! use them only when `is_graphite(tokens)`; legacy themes keep their
//! existing look.

use iced::widget::{Row, container, text};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;

use crate::styles::{
    self, DialogButtonKind, PillTone, Radii, Spacing, TypeScale, is_graphite, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The chrome mono face for designators and numbers.
pub(super) fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// Dialog header: on Graphite, title plus subtitle; on other themes the
/// plain title strip (the subtitle is dropped there).
pub(super) fn titled_header<'a, M: Clone + 'a>(
    title: &'static str,
    subtitle: Option<String>,
    on_close: M,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    if !is_graphite(theme_tokens) {
        return super::widgets::modal_title_header(title, on_close, theme_tokens);
    }
    super::header::graphite_header(
        title,
        subtitle,
        None,
        on_close,
        super::header::DIALOG_HEADER_H,
        theme_tokens,
    )
}

/// The button kind of a dialog's call to action: copper on Graphite, the
/// existing primary elsewhere.
pub(super) fn call_to_action(theme_tokens: &ThemeTokens) -> DialogButtonKind {
    if is_graphite(theme_tokens) {
        DialogButtonKind::Accent
    } else {
        DialogButtonKind::Primary
    }
}

/// Row tint of an Annotate row whose designator changes: the soft accent
/// fill. Unchanged rows stay unfilled.
pub(super) fn changed_row(
    theme_tokens: &ThemeTokens,
    changing: bool,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let fill = changing.then(|| tokens::resolve(theme_tokens).accent_soft);
    move |_| container::Style {
        background: fill.map(Background::Color),
        ..container::Style::default()
    }
}

/// Ink of a proposed designator that changes.
pub(super) fn changed_ink(theme_tokens: &ThemeTokens) -> iced::Color {
    tokens::resolve(theme_tokens).accent_text
}

/// One severity chip: a large toned count beside its label, in a card.
fn severity_chip<'a, M: 'a>(
    theme_tokens: &ThemeTokens,
    count: usize,
    label: &'static str,
    tone: Option<PillTone>,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let ink = match tone {
        Some(t) => styles::pill_colors(theme_tokens, t).1,
        None => r.ink3,
    };
    let mut line: Row<Element<'a, M>> = Row::new().spacing(SP.m).align_y(Alignment::Center);
    line = line.push(
        text(count.to_string())
            .size(TS.display)
            .color(ink)
            .font(mono())
            .boxed(),
    );
    line = line.push(text(label).size(TS.label).color(r.ink3).boxed());
    container(line)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(styles::card(theme_tokens))
        .boxed()
}

/// Error / Warning / Info / Off counts over the rule set.
pub(super) fn severity_chips<'a, M: 'a>(
    theme_tokens: &ThemeTokens,
    counts: [usize; 4],
) -> Element<'a, M> {
    let [errors, warnings, infos, off] = counts;
    let mut chips: Row<Element<'a, M>> = Row::new().spacing(SP.m);
    chips = chips.push(severity_chip(
        theme_tokens,
        errors,
        "Errors",
        Some(PillTone::Err),
    ));
    chips = chips.push(severity_chip(
        theme_tokens,
        warnings,
        "Warnings",
        Some(PillTone::Warn),
    ));
    chips = chips.push(severity_chip(
        theme_tokens,
        infos,
        "Info",
        Some(PillTone::Info),
    ));
    chips = chips.push(severity_chip(theme_tokens, off, "Off", None));
    chips.boxed()
}

/// Soft-warning icon tile of the recovery prompt.
pub(super) fn recovery_icon_tile<'a, M: 'a>(theme_tokens: &ThemeTokens) -> Element<'a, M> {
    let (fill, ink) = styles::pill_colors(theme_tokens, PillTone::Warn);
    let side = TS.glyph + 2.0 * SP.m;
    container(
        text("\u{21BA}")
            .size(TS.glyph)
            .color(ink)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .width(Length::Fixed(side))
    .height(Length::Fixed(side))
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}
