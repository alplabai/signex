//! Graphite parts shared by the dialog bodies: the subtitled header, the
//! severity chips, the changed-row tint and the callout.

use iced::widget::{Row, container, svg};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, raw, text};

use crate::styles::{
    self, DialogButtonKind, DialogIcon, PillTone, Radii, Spacing, TypeScale, chrome_px, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The chrome mono face for designators and numbers.
pub(super) fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// Dialog header: title plus optional subtitle.
pub(super) fn titled_header<'a, M: Clone + 'a>(
    title: impl Into<String>,
    subtitle: Option<String>,
    on_close: M,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    super::header::graphite_header(
        title,
        subtitle,
        None,
        on_close,
        super::header::DIALOG_HEADER_H,
        theme_tokens,
    )
}

/// The button kind of a dialog's call to action: copper.
pub(super) fn call_to_action(_theme_tokens: &ThemeTokens) -> DialogButtonKind {
    DialogButtonKind::Accent
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
    i18n: &signex_i18n::I18n,
    theme_tokens: &ThemeTokens,
    count: usize,
    label: signex_i18n::MessageId,
    tone: Option<PillTone>,
) -> Element<'a, M> {
    let r = tokens::resolve(theme_tokens);
    let ink = match tone {
        Some(t) => styles::pill_colors(theme_tokens, t).1,
        None => r.ink3,
    };
    let mut line: Row<Element<'a, M>> = Row::new().spacing(SP.m).align_y(Alignment::Center);
    // raw: the count is already formatted for the locale by `I18n::count`.
    line = line.push(
        text(raw(i18n.count(count as i64)))
            .size(TS.display)
            .color(ink)
            .font(mono())
            .boxed(),
    );
    line = line.push(
        text(signex_i18n::t!(i18n, label))
            .size(TS.label)
            .color(r.ink3)
            .boxed(),
    );
    container(line)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(styles::card(theme_tokens))
        .boxed()
}

/// Error / Warning / Info / Off counts over the rule set.
pub(super) fn severity_chips<'a, M: 'a>(
    i18n: &signex_i18n::I18n,
    theme_tokens: &ThemeTokens,
    counts: [usize; 4],
) -> Element<'a, M> {
    let [errors, warnings, infos, off] = counts;
    let mut chips: Row<Element<'a, M>> = Row::new().spacing(SP.m);
    use signex_i18n::ids;
    chips = chips.push(severity_chip(
        i18n,
        theme_tokens,
        errors,
        ids::DIALOG_ERC_CHIP_ERRORS,
        Some(PillTone::Err),
    ));
    chips = chips.push(severity_chip(
        i18n,
        theme_tokens,
        warnings,
        ids::DIALOG_ERC_CHIP_WARNINGS,
        Some(PillTone::Warn),
    ));
    chips = chips.push(severity_chip(
        i18n,
        theme_tokens,
        infos,
        ids::DIALOG_ERC_CHIP_INFO,
        Some(PillTone::Info),
    ));
    chips = chips.push(severity_chip(
        i18n,
        theme_tokens,
        off,
        ids::DIALOG_ERC_CHIP_OFF,
        None,
    ));
    chips.boxed()
}

/// Horizontal padding of a [`callout`] (10 px at the 12 px base).
const CALLOUT_PAD_X: f32 = chrome_px(10.0 * 11.0 / 12.0);

/// A note in a dialog body: `icon`'s glyph in its tone's ink, then
/// `message` in the body ink, on the tone's soft fill. `DialogIcon::Info`
/// explains, `DialogIcon::Warn` warns.
pub(super) fn callout<'a, M: 'a>(
    icon: DialogIcon,
    message: impl Into<Label<'a>>,
    theme_tokens: &ThemeTokens,
) -> Element<'a, M> {
    let (fill, ink) = styles::pill_colors(theme_tokens, icon.tone());
    // One body line tall, so the glyph sits on the first line.
    let glyph = TS.body * styles::LINE_HEIGHT;
    let mut line: Row<Element<'a, M>> = Row::new().spacing(SP.m).align_y(Alignment::Start);
    line = line.push(
        svg(icon.glyph_handle())
            .width(Length::Fixed(glyph))
            .height(Length::Fixed(glyph))
            .style(move |_: &Theme, _| svg::Style {
                color: Some(ink),
                ..Default::default()
            })
            .boxed(),
    );
    line = line.push(
        text(message)
            .size(TS.body)
            .color(styles::ti(theme_tokens.text))
            .width(Length::Fill)
            .boxed(),
    );
    container(line)
        .padding([SP.m, CALLOUT_PAD_X])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}
