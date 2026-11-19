//! BOM dialog toolbar: the line filter, the one Variant picker, the
//! Grouped / Ungrouped / Flat switch and the issue chips that open the
//! Rules tab.

use super::super::*;
use iced::widget::{Row, Space, container, row, svg};
use iced::{Background, Border, Color, Element, Length, Widget};
use signex_i18n::{I18n, ids, t};
use signex_output::{BomGrouping, BomValidationReport};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{Label, segmented, text, text_input};

use crate::app::state::BomSidebarTab;
use crate::choice::Choice;
use crate::styles::tokens::resolve;
use crate::styles::{ChromeMetrics, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Widths of the toolbar controls, as drawn.
const FILTER_W: f32 = 220.0;
const VARIANT_W: f32 = 150.0;
const GROUPING_W: f32 = 236.0;
/// Side of the search glyph in the filter field.
const SEARCH_ICON: f32 = 13.0;

type VariantChoice = Choice<Option<String>>;

impl Signex {
    pub(super) fn bom_toolbar(&self) -> Element<'_, Message> {
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let r = resolve(tokens);

        let (variants, chosen) = variant_choices(
            &preview.variants,
            preview.options.active_variant.as_deref(),
            i18n,
        );
        let variant_picker =
            iced::widget::pick_list(Some(chosen), variants, |c: &VariantChoice| c.label.clone())
                .graphite()
                .on_select(|c: VariantChoice| {
                    Message::BomPreview(BomPreviewMsg::SetVariant(c.value))
                })
                .width(VARIANT_W);

        let groupings: [(BomGrouping, Label<'static>); 3] = [
            (
                BomGrouping::Grouped,
                t!(i18n, ids::BOM_GROUPING_GROUPED).into(),
            ),
            (
                BomGrouping::Ungrouped,
                t!(i18n, ids::BOM_GROUPING_UNGROUPED).into(),
            ),
            (BomGrouping::Flat, t!(i18n, ids::BOM_GROUPING_FLAT).into()),
        ];
        let grouping = container(segmented(
            groupings,
            preview.options.grouping,
            super::sidebar::segmented_style(tokens),
            |g| Message::BomPreview(BomPreviewMsg::SetGrouping(g)),
        ))
        .width(GROUPING_W);

        let mut bar: Row<Element<'_, Message>> = row![
            self.bom_filter_field(&preview.filter, tokens),
            text(t!(i18n, ids::BOM_VARIANT))
                .size(TS.caption)
                .color(r.ink4),
            variant_picker,
            grouping,
            Space::new().width(Length::Fill),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Center);
        for issue_chip in issue_chips(&preview.validation_report, tokens, i18n) {
            bar = bar.push(issue_chip);
        }
        container(bar)
            .padding([SP.m, SP.l])
            .width(Length::Fill)
            .boxed()
    }

    /// The filter field: search glyph and input on the field fill.
    fn bom_filter_field<'a>(
        &'a self,
        value: &'a str,
        tokens: &ThemeTokens,
    ) -> Element<'a, Message> {
        let r = resolve(tokens);
        let (ink, ink4) = (ti(tokens.text), r.ink4);
        let icon = svg(crate::icons::icon_chrome_search(
            self.ui_state.effective_theme_id(),
        ))
        .width(SEARCH_ICON)
        .height(SEARCH_ICON)
        .style(move |_: &iced::Theme, _| svg::Style {
            color: Some(ink4),
            ..Default::default()
        });
        let input = text_input(t!(self.ui_state.i18n, ids::BOM_FILTER_PLACEHOLDER), value)
            .on_input(|text| Message::BomPreview(BomPreviewMsg::FilterChanged(text)))
            .padding(iced::Padding::ZERO)
            .size(TS.body)
            .width(Length::Fill)
            .style(move |_: &iced::Theme, _| iced::widget::text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                placeholder: ink4,
                value: ink,
                selection: Color { a: 0.4, ..ink },
            });
        container(
            row![icon, input]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center),
        )
        .padding([0.0, SP.m])
        .width(FILTER_W)
        .height(ChromeMetrics::DEFAULT.field)
        .align_y(iced::alignment::Vertical::Center)
        .style(crate::styles::field(tokens, false))
        .boxed()
    }
}

/// Base, then each variant the project defines (a variant named "Base" is
/// the base view), and the one in effect.
fn variant_choices(
    variants: &[String],
    current: Option<&str>,
    i18n: &I18n,
) -> (Vec<VariantChoice>, VariantChoice) {
    let base = Choice {
        value: None,
        label: i18n.message_plain(ids::BOM_VARIANT_BASE, &[]),
    };
    let is_base = |name: &str| name.eq_ignore_ascii_case("Base");
    let named = |name: &str| Choice {
        value: Some(name.to_owned()),
        label: name.to_owned(),
    };
    let chosen = match current {
        Some(name) if !is_base(name) => named(name),
        _ => base.clone(),
    };
    let mut options = vec![base];
    options.extend(
        variants
            .iter()
            .filter(|name| !is_base(name))
            .map(|name| named(name)),
    );
    options.dedup();
    (options, chosen)
}

/// One chip per severity that has issues, errors first; each opens the
/// Rules tab. Notes get no chip: they flag no line.
fn issue_chips<'a>(
    report: &BomValidationReport,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, Message>> {
    let r = resolve(tokens);
    let tones = [
        (
            report.error_count(),
            ids::BOM_CHIP_ERRORS,
            r.err_soft,
            ti(tokens.error),
        ),
        (
            report.warning_count(),
            ids::BOM_CHIP_WARNINGS,
            r.warn_soft,
            ti(tokens.warning),
        ),
    ];
    tones
        .into_iter()
        .filter(|(count, ..)| *count > 0)
        .map(|(count, id, fill, ink)| {
            let style = ChipStyle {
                fill,
                edge: Color::TRANSPARENT,
                hover: fill,
                ink,
                on_fill: fill,
                on_ink: ink,
            };
            chip(
                t!(i18n, id, "count" => count),
                None,
                true,
                style,
                ChipMetrics::from_text_size(TS.body),
                i18n,
                Some(Message::BomPreview(BomPreviewMsg::SetSidebarTab(
                    BomSidebarTab::Rules,
                ))),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #112: the variant is picked in one place, the toolbar: Base first,
    /// then the project's variants, a variant named Base folded into it.
    #[test]
    fn the_variant_picker_lists_base_then_the_project_variants() {
        let i18n = I18n::en_us();
        let variants = ["Lite".to_owned(), "base".to_owned(), "Pro".to_owned()];
        let (options, chosen) = variant_choices(&variants, Some("Pro"), &i18n);
        let labels: Vec<&str> = options.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["Base", "Lite", "Pro"]);
        assert_eq!(chosen.value.as_deref(), Some("Pro"));
        let (_, base) = variant_choices(&variants, None, &i18n);
        assert_eq!(base.value, None);
        assert_eq!(base.label, "Base");
    }
}
