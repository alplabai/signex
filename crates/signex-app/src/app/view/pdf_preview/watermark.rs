//! The Watermark section of the Export PDF dialog (#1298, mockup row 168):
//! the "Print a watermark" switch heads it; off, its fields grey out and
//! the preview drops the watermark, and the settings stay for when it is on
//! again.

use iced::Widget as _;
use iced::widget::{Space, column, container, row, slider};
use signex_i18n::{ids, t};
use signex_output::WatermarkPages;
use signex_widgets::{Label, raw, segmented, text, text_input};

use super::super::*;
use super::form::{
    Look, NUM_W, SP, TS, check_with_help, form_row, num_field, switch_section, unit,
};
use crate::app::state::{ANGLE_PRESETS, PreviewState, WatermarkPreset};

/// Side of the colour swatch.
const SWATCH: f32 = 18.0;
/// Corner radius of the colour swatch.
const SWATCH_RADIUS: f32 = 4.0;

/// The colour as the dialog prints it: `#RRGGBB`.
pub(super) fn colour_hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

/// The Opacity field was typed in.
fn opacity_typed(text: String) -> PrintPreviewMsg {
    PdfOptionMsg::WatermarkOpacityChanged(text).into()
}

/// The Angle field was typed in.
fn angle_typed(text: String) -> PrintPreviewMsg {
    PdfOptionMsg::WatermarkAngleChanged(text).into()
}

impl Signex {
    /// The Watermark section body.
    pub(super) fn view_pdf_watermark<'a>(
        &'a self,
        preview: &'a PreviewState,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_view.panel_ctx.tokens;
        let look = Look::of(tokens);
        let draft = &preview.dialog.watermark;
        let settings = &draft.settings;

        let presets = segmented(
            [
                (WatermarkPreset::Draft, ids::PDF_WATERMARK_DRAFT),
                (
                    WatermarkPreset::Confidential,
                    ids::PDF_WATERMARK_CONFIDENTIAL,
                ),
                (WatermarkPreset::Custom, ids::PDF_WATERMARK_CUSTOM),
            ]
            .map(|(preset, label)| (preset, Label::from(t!(i18n, label)))),
            draft.preset(),
            look.segmented(),
            |preset| Message::from(PdfOptionMsg::WatermarkPresetPicked(preset)),
        );

        // The preset's printed text shows read-only; Custom text is typed.
        let text_field = text_input(
            signex_i18n::UserText::new(String::new()),
            settings.text.printed().to_owned(),
        )
        .padding([SP.s, SP.m])
        .size(TS.body)
        .width(Length::Fill)
        .style(crate::panels::form_look::text_input_style);
        let text_field = if draft.preset() == WatermarkPreset::Custom {
            text_field.on_input(|v| Message::from(PdfOptionMsg::WatermarkTextChanged(v)))
        } else {
            text_field
        };

        let opacity = row![
            slider(
                0.0..=100.0,
                f32::from(settings.opacity_percent),
                |v: f32| {
                    Message::from(PdfOptionMsg::WatermarkOpacityChanged(format!("{v:.0}")))
                }
            )
            .step(1.0)
            .width(Length::Fill),
            num_field(
                &draft.opacity_input,
                Length::Fixed(NUM_W),
                Some(opacity_typed)
            ),
            unit(t!(i18n, ids::PDF_UNIT_PERCENT), &look),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);

        // A typed angle such as 30 selects no preset.
        let angle_switch = segmented(
            ANGLE_PRESETS.map(|angle| {
                // raw: an angle in degrees, digits only.
                (Some(angle), raw(angle.to_string()))
            }),
            draft.angle_preset(),
            look.segmented(),
            |angle: Option<i32>| {
                Message::from(PdfOptionMsg::WatermarkAngleChanged(
                    angle.unwrap_or_default().to_string(),
                ))
            },
        );
        let angle = row![
            container(angle_switch).width(Length::Fill),
            num_field(&draft.angle_input, Length::Fixed(NUM_W), Some(angle_typed)),
            unit(t!(i18n, ids::PDF_UNIT_DEGREE), &look),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);

        let swatch_colour = {
            let [r, g, b] = settings.colour_rgb;
            iced::Color::from_rgb8(r, g, b)
        };
        let colour = row![
            container(Space::new())
                .width(SWATCH)
                .height(SWATCH)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(swatch_colour.into()),
                    border: iced::Border {
                        radius: SWATCH_RADIUS.into(),
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                }),
            // raw: a colour code, the same in every language.
            text(raw(colour_hex(settings.colour_rgb)))
                .size(TS.caption)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .color(look.ink4),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);

        let pages = segmented(
            [
                (WatermarkPages::All, ids::PDF_WATERMARK_PAGES_ALL),
                (WatermarkPages::FirstOnly, ids::PDF_WATERMARK_PAGES_FIRST),
            ]
            .map(|(pages, label)| (pages, Label::from(t!(i18n, label)))),
            settings.pages,
            look.segmented(),
            |pages| Message::from(PdfOptionMsg::WatermarkPagesPicked(pages)),
        );

        let fields = column![
            presets,
            form_row(t!(i18n, ids::PDF_WATERMARK_TEXT), &look, text_field.boxed()),
            form_row(t!(i18n, ids::PDF_WATERMARK_OPACITY), &look, opacity.boxed()),
            form_row(t!(i18n, ids::PDF_WATERMARK_ANGLE), &look, angle.boxed()),
            form_row(t!(i18n, ids::PDF_WATERMARK_COLOUR), &look, colour.boxed()),
            form_row(t!(i18n, ids::PDF_WATERMARK_PAGES), &look, pages),
            check_with_help(
                t!(i18n, ids::PDF_WATERMARK_REVISION),
                t!(i18n, ids::PDF_WATERMARK_REVISION_HELP),
                draft.with_revision,
                |on| PdfOptionMsg::WatermarkRevisionToggled(on).into(),
                &look,
            ),
        ]
        .spacing(SP.s + SP.xs)
        .width(Length::Fill);

        switch_section(
            t!(i18n, ids::PDF_WATERMARK_SWITCH),
            draft.is_on,
            Message::from(PdfOptionMsg::WatermarkToggled(!draft.is_on)),
            fields.boxed(),
            &look,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_colour_reads_as_an_upper_case_hex_code() {
        assert_eq!(colour_hex([0xC0, 0x39, 0x2B]), "#C0392B");
        assert_eq!(colour_hex([0, 0, 0]), "#000000");
    }
}
