//! The "Font" row of the Properties panel: the installed families plus
//! "Default" (inherit the sheet default). Shared by every text-bearing
//! selection and by the sheet-wide default in Page Options (#510).

use super::super::*;
use signex_widgets::dropdown_style::GraphiteDropdown as _;

/// The pick-list entry meaning "no family of its own".
pub(in crate::panels) const DEFAULT_OPTION: &str = "Default";

/// "Default" and every installed family, built once per catalog. This runs
/// in `view`, so it never scans: until the background scan lands only the
/// bundled families are offered, and the scan's completion message triggers
/// the redraw that shows the rest.
fn base_options() -> Vec<String> {
    use crate::fonts::catalog;
    static SCANNED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    match catalog::installed() {
        Some(scanned) => SCANNED.get_or_init(|| options_of(scanned)).clone(),
        None => options_of(catalog::installed_or_bundled()),
    }
}

fn options_of(catalog: &crate::fonts::catalog::FontCatalog) -> Vec<String> {
    std::iter::once(DEFAULT_OPTION.to_string())
        .chain(catalog.names().iter().cloned())
        .collect()
}

/// The entries to offer: "Default", every installed family, and the
/// stored family itself when this machine does not have it, so the row
/// shows what the document names rather than hiding it.
pub(in crate::panels) fn font_family_options(current: Option<&str>) -> Vec<String> {
    with_current(base_options(), current)
}

fn with_current(mut options: Vec<String>, current: Option<&str>) -> Vec<String> {
    if let Some(current) = current
        && !options.iter().any(|name| name == current)
    {
        options.push(current.to_string());
    }
    options
}

/// `Default` is `None`; any other entry is the family name.
pub(in crate::panels) fn family_from_option(option: String) -> Option<String> {
    (option != DEFAULT_OPTION).then_some(option)
}

/// The pick-list choice of a family entry: "Default" is named from the
/// catalog, a family by its own name.
fn family_choice(
    i18n: &signex_i18n::I18n,
    option: String,
) -> crate::choice::Choice<Option<String>> {
    let value = family_from_option(option);
    let label = value
        .clone()
        .unwrap_or_else(|| i18n.message_plain(signex_i18n::ids::PROPERTIES_FONT_DEFAULT, &[]));
    crate::choice::Choice { value, label }
}

pub(in crate::panels) fn font_family_row<'a>(
    label: signex_i18n::Localized,
    current: Option<&str>,
    muted: Color,
    on_pick: impl Fn(Option<String>) -> PanelMsg + 'a,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    // Show the installed spelling when the document spells it differently.
    let shown = current.map(|name| {
        crate::fonts::catalog::installed_or_bundled()
            .canonical(name)
            .unwrap_or(name)
    });
    let choices = font_family_options(shown)
        .into_iter()
        .map(|option| family_choice(i18n, option))
        .collect();
    form_choice_row(
        label,
        choices,
        family_choice(i18n, shown.unwrap_or(DEFAULT_OPTION).to_string()),
        on_pick,
        muted,
    )
}

/// The sizes the Font Size list offers, in points.
pub(in crate::panels) const FONT_SIZES_PT: [u32; 13] =
    [6, 8, 10, 12, 14, 16, 18, 20, 24, 28, 36, 48, 72];

/// A Font Size choice: the size and how it reads (`10 pt`).
fn size_choice(i18n: &signex_i18n::I18n, size_pt: u32) -> crate::choice::Choice<u32> {
    crate::choice::Choice {
        value: size_pt,
        label: i18n.message_plain(
            signex_i18n::ids::PROPERTIES_FONT_SIZE_PT,
            &[("size", size_pt.to_string().into())],
        ),
    }
}

/// The B / I / U / T buttons beside the size, in one segmented frame. Placed
/// text does not store a text style yet, so they are drawn disabled in the
/// faint ink and send nothing (mockup row 41); the tooltip still names each.
fn text_style_buttons<'a>(i18n: &signex_i18n::I18n) -> Element<'a, PanelMsg> {
    use signex_i18n::{ids, t};
    let ts = crate::styles::TypeScale::DEFAULT;
    let sp = crate::styles::Spacing::DEFAULT;
    let style_button =
        |glyph: signex_i18n::MessageId, tip: signex_i18n::MessageId, font: iced::Font| {
            let face = button(
                text(t!(i18n, glyph))
                    .size(ts.caption)
                    .font(font)
                    .align_x(iced::alignment::Horizontal::Center)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .padding([sp.xs, sp.xs])
            .style(|theme: &Theme, _| disabled_style_cell(theme));
            signex_widgets::tooltip(
                face,
                container(text(t!(i18n, tip)).size(ts.caption))
                    .padding([sp.xs, sp.s])
                    .style(crate::panels::form_look::value_tip),
                iced::widget::tooltip::Position::Bottom,
            )
            .boxed()
        };
    let bold = iced::Font {
        weight: iced::font::Weight::Bold,
        ..signex_widgets::ui_font::regular()
    };
    let italic = iced::Font {
        style: iced::font::Style::Italic,
        ..signex_widgets::ui_font::regular()
    };
    container(
        iced::widget::row![
            style_button(
                ids::PROPERTIES_STYLE_BOLD_GLYPH,
                ids::PROPERTIES_STYLE_BOLD,
                bold
            ),
            style_button(
                ids::PROPERTIES_STYLE_ITALIC_GLYPH,
                ids::PROPERTIES_STYLE_ITALIC,
                italic
            ),
            style_button(
                ids::PROPERTIES_STYLE_UNDERLINE_GLYPH,
                ids::PROPERTIES_STYLE_UNDERLINE,
                signex_widgets::ui_font::regular()
            ),
            style_button(
                ids::PROPERTIES_STYLE_STRIKE_GLYPH,
                ids::PROPERTIES_STYLE_STRIKE,
                signex_widgets::ui_font::regular()
            ),
        ]
        .spacing(sp.xs)
        .width(Length::Fill),
    )
    .padding(sp.xs)
    .width(Length::Fill)
    .style(crate::panels::form_look::segmented_frame)
    .boxed()
}

/// A flat segment cell in the faint ink: how a disabled style button reads.
fn disabled_style_cell(theme: &Theme) -> iced::widget::button::Style {
    let cell = crate::panels::form_look::segment_cell(theme, false, false);
    iced::widget::button::Style {
        text_color: crate::panels::form_look::FormLook::of(theme).ink4,
        ..cell
    }
}

/// The "Font Size  [10 pt ▾] [B I U T]" row: the size list and the text
/// style buttons on one row. `on_pick` builds the message for a size.
#[allow(
    dead_code,
    reason = "row kit of epic #2523; the label, power port and text note units adopt it"
)]
pub(in crate::panels) fn font_size_row<'a>(
    i18n: &signex_i18n::I18n,
    size_pt: u32,
    on_pick: impl Fn(u32) -> PanelMsg + 'a,
) -> Element<'a, PanelMsg> {
    let choices: Vec<_> = FONT_SIZES_PT
        .into_iter()
        .map(|size| size_choice(i18n, size))
        .collect();
    let sp = crate::styles::Spacing::DEFAULT;
    let size = iced::widget::pick_list(
        Some(size_choice(i18n, size_pt)),
        choices,
        |c: &crate::choice::Choice<u32>| c.label.clone(),
    )
    .graphite()
    .on_select(move |c: crate::choice::Choice<u32>| on_pick(c.value))
    .padding([sp.xs, sp.s + sp.xs])
    .width(Length::Fill);
    super::rows::pair_row(
        signex_i18n::t!(i18n, signex_i18n::ids::PROPERTIES_FONT_SIZE),
        size.boxed(),
        text_style_buttons(i18n),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A size reads with its unit, and the list hands back the points.
    #[test]
    fn a_font_size_reads_in_points_and_picks_its_value() {
        let en = signex_i18n::I18n::en_us();
        let choice = size_choice(&en, 12);
        assert_eq!(choice.label, "12 pt");
        assert_eq!(choice.value, 12);
        assert!(FONT_SIZES_PT.contains(&10), "the 10 pt default is offered");
    }

    /// The row is labelled Font Size and shows the size and the four style
    /// buttons; they are disabled until text styles are stored (mockup row
    /// 41), so a press sends nothing.
    #[test]
    fn the_font_size_row_shows_the_size_and_the_style_buttons() {
        let en = signex_i18n::I18n::en_us();
        let size = iced::Size::new(320.0, 60.0);
        let shown: Vec<String> = crate::layout_probe::lay_out_texts(
            font_size_row(&en, 10, PanelMsg::SetPrePlacementFontSize),
            size,
        )
        .into_iter()
        .map(|t| t.text)
        .collect();
        for label in ["Font Size", "B", "I", "U", "T"] {
            assert!(shown.iter().any(|t| t == label), "{label:?} in {shown:?}");
        }
        for glyph in ["B", "I", "U", "T"] {
            let pressed = crate::layout_probe::messages_on_click(
                font_size_row(&en, 10, PanelMsg::SetPrePlacementFontSize),
                size,
                glyph,
            );
            assert!(pressed.is_empty(), "{glyph}: {pressed:?}");
        }
    }

    /// A disabled style button reads in the faint ink, not the live one.
    #[test]
    fn a_disabled_style_button_uses_the_faint_ink() {
        let theme = Theme::Dark;
        let look = crate::panels::form_look::FormLook::of(&theme);
        let style = disabled_style_cell(&theme);
        assert_eq!(style.text_color, look.ink4);
    }

    #[test]
    fn default_option_means_inherit_and_any_other_entry_is_the_family() {
        assert_eq!(family_from_option("Default".into()), None);
        assert_eq!(
            family_from_option("Fira Mono".into()),
            Some("Fira Mono".into())
        );
    }

    #[test]
    fn a_stored_family_this_machine_lacks_is_still_offered() {
        let catalog = crate::fonts::catalog::FontCatalog::from_families(["Fira Mono"]);
        let options = with_current(options_of(&catalog), Some("Surely Not Installed 510"));
        assert_eq!(options[0], DEFAULT_OPTION);
        assert!(options.contains(&"Surely Not Installed 510".to_string()));
        assert!(options.contains(&"Fira Mono".to_string()));
        assert!(
            options.contains(&"Iosevka".to_string()),
            "the bundled family is always offered"
        );
    }

    /// `view` calls this on every frame; it must never trigger the blocking
    /// system-font scan.
    #[test]
    fn building_the_options_does_not_scan_the_system_fonts() {
        let scanned_before = crate::fonts::catalog::installed().is_some();
        let options = font_family_options(None);
        assert_eq!(
            crate::fonts::catalog::installed().is_some(),
            scanned_before,
            "font_family_options started the scan"
        );
        assert!(options.contains(&"Iosevka".to_string()));
    }
}
