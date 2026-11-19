//! The page in the middle of the Export PDF dialog: a toolbar for page
//! stepping and zoom over the page, the page with the watermark drawn over
//! it, and a caption in paper units ("ISO A4 landscape · 297 × 210 mm ·
//! Fit to Page"). Drag pans a zoomed page; the wheel zooms.

use iced::Widget as _;
use iced::widget::{Space, column, container, image, row, stack, svg};
use signex_i18n::{I18n, ids, t};
use signex_output::{PdfOptions, PdfScale};
use signex_types::schematic::SheetOrientation;
use signex_widgets::{raw, text};

use super::super::*;
use super::form::{Look, SP, TS, rule};
use super::rail::{RailCaption, rail_entries};
use super::watermark_overlay::{self, WatermarkOverlay};
use crate::app::state::PreviewState;

/// Height of the page toolbar.
const TOOLBAR_H: f32 = 36.0;
/// Side of a page-step button and of its icon.
const STEP_BUTTON: f32 = 24.0;
const STEP_ICON: f32 = 14.0;
/// Space around the page.
const PAGE_MARGIN: f32 = 20.0;

/// The paper's name and orientation, as "ISO A4 landscape".
pub(super) fn paper_name(options: &PdfOptions, i18n: &I18n) -> String {
    let id = match options.orientation {
        SheetOrientation::Portrait => ids::PDF_PAPER_PORTRAIT,
        SheetOrientation::Landscape => ids::PDF_PAPER_LANDSCAPE,
    };
    let size = options.page_size.kind().to_string();
    i18n.message_plain(id, &[("size", size.into())])
}

/// A length in mm as the caption prints it: whole mm, or one decimal.
pub(super) fn mm_text(mm: f64) -> String {
    if (mm - mm.round()).abs() < 0.05 {
        format!("{mm:.0}")
    } else {
        format!("{mm:.1}")
    }
}

/// The page caption in paper units: paper, size in mm and scale.
pub(super) fn paper_caption(options: &PdfOptions, i18n: &I18n) -> signex_i18n::Localized {
    let (w_mm, h_mm) = options.page_size.dimensions_mm(options.orientation);
    let scale = match options.scale {
        PdfScale::FitToPage => i18n.message_plain(ids::PDF_SCALE_FIT, &[]),
        PdfScale::OneToOne => i18n.message_plain(ids::PDF_SCALE_ONE_TO_ONE, &[]),
        PdfScale::Percent(percent) => i18n.message_plain(
            ids::PDF_SCALE_PERCENT,
            &[("percent", mm_text(percent).into())],
        ),
    };
    i18n.message(
        ids::PDF_PAGE_CAPTION,
        &[
            ("paper", paper_name(options, i18n).into()),
            ("width", mm_text(w_mm).into()),
            ("height", mm_text(h_mm).into()),
            ("scale", scale.into()),
        ],
    )
}

impl Signex {
    /// The middle column: toolbar, page and caption, or the empty state
    /// when there is no sheet to export.
    pub(super) fn view_pdf_viewer<'a>(&'a self, preview: &'a PreviewState) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = Look::of(&self.document_state.panel_ctx.tokens);
        let sunken = move |_: &iced::Theme| container::Style {
            background: Some(look.sunken.into()),
            ..container::Style::default()
        };
        if preview.sheet_files.is_empty() {
            return container(
                text(t!(i18n, ids::PDF_NO_SHEETS))
                    .size(TS.label)
                    .color(look.ink3),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(sunken)
            .boxed();
        }
        let caption: Element<'a, Message> = if preview.pages.is_empty() {
            Space::new().height(0).boxed()
        } else {
            container(
                text(paper_caption(&preview.pdf_options, i18n))
                    .size(TS.caption)
                    .color(look.ink4),
            )
            .center_x(Length::Fill)
            .padding(iced::Padding {
                bottom: SP.m,
                ..iced::Padding::ZERO
            })
            .boxed()
        };
        container(
            column![
                self.view_pdf_page_toolbar(preview, &look),
                rule(look.line_soft, false),
                self.view_pdf_page_surface(preview, &look),
                caption,
            ]
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(sunken)
        .boxed()
    }

    /// Previous / next page, "Page n of N", the sheet's file, and zoom.
    fn view_pdf_page_toolbar<'a>(
        &'a self,
        preview: &'a PreviewState,
        look: &Look,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_state.panel_ctx.tokens;
        let theme = self.ui_state.theme_id;
        let look = *look;
        let count = preview.pages.len();
        let selected = preview.selected.min(count.saturating_sub(1));
        let step = |icon: svg::Handle, target: Option<usize>| -> Element<'a, Message> {
            let ink = if target.is_some() {
                look.ink3
            } else {
                look.ink4
            };
            signex_widgets::button(
                container(svg(icon).width(STEP_ICON).height(STEP_ICON).style(
                    move |_: &iced::Theme, _| svg::Style {
                        color: Some(ink),
                        ..svg::Style::default()
                    },
                ))
                .center_x(STEP_BUTTON)
                .center_y(STEP_BUTTON),
            )
            .width(STEP_BUTTON)
            .height(STEP_BUTTON)
            .padding(0)
            .on_press_maybe(
                target.map(|page| Message::PrintPreview(PrintPreviewMsg::SelectPage(page))),
            )
            .style(crate::panels::form_look::ghost_button)
            .boxed()
        };
        let mut bar: iced::widget::Row<Element<'a, Message>> = row![]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .height(TOOLBAR_H);
        bar = bar.push(step(
            crate::icons::icon_collapse_left(theme),
            (count > 0 && selected > 0).then(|| selected - 1),
        ));
        if count > 0 {
            bar = bar.push(
                text(t!(
                    i18n,
                    ids::PDF_PAGE_OF,
                    "page" => selected + 1,
                    "pages" => count
                ))
                .size(TS.body)
                .color(look.ink)
                .boxed(),
            );
        }
        bar = bar.push(step(
            crate::icons::icon_collapse_right(theme),
            (selected + 1 < count).then_some(selected + 1),
        ));
        // The shown page's sheet file, when the pages map to the sheets.
        let file = rail_entries(preview)
            .into_iter()
            .filter(|entry| count > 0 && entry.page == Some(selected))
            .find_map(|entry| match entry.caption {
                RailCaption::Sheet { key, .. } => {
                    key.0.file_name().map(|name| name.to_string_lossy())
                }
                RailCaption::Page { .. } => None,
            });
        if let Some(file) = file {
            bar = bar.push(
                container(rule(look.line_soft, true))
                    .height(TOOLBAR_H - 2.0 * SP.m)
                    .boxed(),
            );
            bar = bar.push(
                text(signex_i18n::UserText::new(file.into_owned()))
                    .size(TS.body)
                    .font(crate::fonts::iced_font_for_family(
                        crate::fonts::CHROME_MONO_FONT,
                    ))
                    .color(look.ink3)
                    .wrapping(iced::widget::text::Wrapping::None)
                    .ellipsis(iced::widget::text::Ellipsis::End)
                    .boxed(),
            );
        }
        bar = bar.push(Space::new().width(Length::Fill).boxed());
        bar = bar.push(crate::canvas::zoom_pill::view(
            tokens,
            f64::from(preview.zoom) * 100.0,
            Message::PrintPreview(PrintPreviewMsg::Zoom(-1.0)),
            Message::PrintPreview(PrintPreviewMsg::Zoom(1.0)),
            Message::PrintPreview(PrintPreviewMsg::ZoomFitPressed),
        ));
        container(bar)
            .padding([0.0, SP.l])
            .width(Length::Fill)
            .style(move |_: &iced::Theme| container::Style {
                background: Some(look.panel.into()),
                ..container::Style::default()
            })
            .boxed()
    }

    /// The selected page, fitted below 1x zoom and panned above it, with the
    /// watermark over it. With no page, why there is none.
    fn view_pdf_page_surface<'a>(
        &'a self,
        preview: &'a PreviewState,
        look: &Look,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = *look;
        let Some(page) = preview.pages.get(preview.selected) else {
            let note = match preview.range_note.clone() {
                // raw: the range note is set by the preview handler (#2061: handler-worded state text).
                Some(note) => text(raw(note)),
                None => text(t!(i18n, ids::PDF_NO_FILES_SELECTED)),
            };
            return container(note.size(TS.label).color(look.ink3))
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .boxed();
        };
        let zoom = preview.zoom;
        let fits = zoom <= 1.0;
        let (width, height) = if fits {
            (Length::Fill, Length::Fill)
        } else {
            (
                Length::Fixed((page.width_px as f32 * zoom).max(64.0)),
                Length::Fixed((page.height_px as f32 * zoom).max(64.0)),
            )
        };
        let fit = if fits {
            iced::ContentFit::Contain
        } else {
            iced::ContentFit::Fill
        };
        // Until the page is drawn it is a blank sheet.
        let layered: Element<'a, Message> = match page.handle() {
            None => container(Space::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_: &iced::Theme| container::Style {
                    background: Some(iced::Color::WHITE.into()),
                    ..container::Style::default()
                })
                .boxed(),
            Some(handle) => {
                let picture = image(handle.clone())
                    .content_fit(fit)
                    .width(width)
                    .height(height);
                match self.pdf_watermark_overlay(preview) {
                    Some(overlay) => stack![picture, watermark_overlay::view(overlay)]
                        .width(width)
                        .height(height)
                        .boxed(),
                    None => picture.boxed(),
                }
            }
        };
        // Below 1x the pan is held at (0, 0) by the zoom handler.
        let positioned: Element<'a, Message> = if fits {
            layered
        } else {
            signex_widgets::Translate::new(layered, preview.pan).boxed()
        };
        let surface = container(positioned)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(PAGE_MARGIN)
            .clip(true);
        let interaction = match (fits, preview.panning.is_some()) {
            (true, _) => iced::mouse::Interaction::default(),
            (false, true) => iced::mouse::Interaction::Grabbing,
            (false, false) => iced::mouse::Interaction::Grab,
        };
        iced::widget::mouse_area(surface)
            .on_press(Message::PrintPreview(PrintPreviewMsg::PanStart))
            .on_release(Message::PrintPreview(PrintPreviewMsg::PanFinished))
            .on_scroll(|delta| {
                use iced::mouse::ScrollDelta;
                let dy = match delta {
                    ScrollDelta::Lines { y, .. } => y,
                    ScrollDelta::Pixels { y, .. } => y,
                };
                Message::PrintPreview(PrintPreviewMsg::Zoom(dy))
            })
            .interaction(interaction)
            .boxed()
    }

    /// The watermark over the shown page, while it prints on that page.
    fn pdf_watermark_overlay(&self, preview: &PreviewState) -> Option<WatermarkOverlay> {
        let draft = &preview.dialog.watermark;
        if !draft.is_on || !draft.settings.applies_to_page(preview.selected) {
            return None;
        }
        let options = &preview.pdf_options;
        Some(WatermarkOverlay {
            watermark: draft.settings.clone(),
            stamp: draft.stamp(&self.ui_state.i18n),
            page_mm: options.page_size.dimensions_mm(options.orientation),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The caption as shown, without the isolation marks around placeables.
    fn caption(options: &PdfOptions, i18n: &I18n) -> String {
        paper_caption(options, i18n)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    }

    /// m111: the caption names the paper in mm, not the raster in pixels.
    #[test]
    fn the_caption_is_in_paper_units() {
        let i18n = I18n::en_us();
        assert_eq!(
            caption(&PdfOptions::default(), &i18n),
            "ISO A4 landscape · 297 × 210 mm · Fit to Page"
        );
        let options = PdfOptions {
            orientation: SheetOrientation::Portrait,
            scale: PdfScale::Percent(50.0),
            ..PdfOptions::default()
        };
        assert_eq!(
            caption(&options, &i18n),
            "ISO A4 portrait · 210 × 297 mm · 50 %"
        );
    }

    #[test]
    fn lengths_print_whole_or_with_one_decimal() {
        assert_eq!(mm_text(297.0), "297");
        assert_eq!(mm_text(215.9), "215.9");
        assert_eq!(mm_text(279.4), "279.4");
    }
}
