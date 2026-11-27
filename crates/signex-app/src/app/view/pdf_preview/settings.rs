//! The settings column of the Export PDF dialog, in the order of mockup
//! mockup rows 111 and 168, and its Output, Variant and Structure sections. Each
//! section reads and writes `preview.pdf_options` (or the dialog state) so
//! the rasteriser and the exporter see what the column shows.

use iced::Widget as _;
use iced::widget::{column, container, pick_list, scrollable};
use signex_i18n::{I18n, ids, t};
use signex_output::ColourMode;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{Label, segmented};

use super::super::*;
use super::form::{
    Look, SP, TS, caption, check, check_with_help, checklist, form_row, gated, indented,
    net_bookmarks, schematics_include, ticked,
};
use super::sheets::sheets_summary;
use crate::app::state::{PdfQuality, PdfSection, PreviewState};
use crate::choice::Choice;

/// Width of the settings column.
pub(super) const SIDE_W: f32 = 320.0;

/// A variant pick-list option: `None` prints the base design.
type VariantChoice = Choice<Option<String>>;

/// The variant options, Base first, and the one `current` names. A saved
/// variant the project no longer lists still shows as chosen. A variant
/// named "Base" (any case) is Base itself, as it always was: no row of its
/// own, and choosing it prints the base design.
fn variant_choices(
    variants: &[String],
    current: Option<&str>,
    i18n: &I18n,
) -> (Vec<VariantChoice>, VariantChoice) {
    let base = Choice {
        value: None,
        label: i18n.message_plain(ids::PDF_VARIANT_BASE, &[]),
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

impl Signex {
    /// The settings column: every section, scrolling, 320 px wide.
    pub(super) fn view_pdf_settings_column<'a>(
        &'a self,
        preview: &'a PreviewState,
    ) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let tokens = &self.document_view.panel_ctx.tokens;
        let look = Look::of(tokens);
        let options = &preview.pdf_options;
        let (sheets_on, sheets_all) = sheets_summary(preview);
        let summary_of = |on: usize, all: usize| -> Option<Label<'a>> {
            Some(t!(i18n, ids::PDF_SUMMARY_OF, "on" => on, "all" => all).into())
        };
        let structure = if options.use_physical_structure {
            ids::PDF_STRUCTURE_PHYSICAL
        } else {
            ids::PDF_STRUCTURE_LOGICAL
        };
        let nets_on = ticked(&net_bookmarks(), options);
        let include = schematics_include();
        let sections = column![
            self.pdf_section(
                preview,
                PdfSection::Sheets,
                ids::PDF_SECTION_SHEETS,
                summary_of(sheets_on, sheets_all),
                || self.view_pdf_sheets(preview),
            ),
            self.pdf_section(
                preview,
                PdfSection::Pages,
                ids::PDF_SECTION_PAGES,
                None,
                || { self.view_pdf_pages(preview) }
            ),
            self.pdf_section(
                preview,
                PdfSection::PageSetup,
                ids::PDF_SECTION_PAGE_SETUP,
                None,
                || self.view_pdf_page_setup(preview),
            ),
            self.pdf_section(
                preview,
                PdfSection::Output,
                ids::PDF_SECTION_OUTPUT,
                None,
                || { self.view_pdf_output(preview) }
            ),
            self.pdf_section(
                preview,
                PdfSection::Variant,
                ids::PDF_SECTION_VARIANT,
                None,
                || self.view_pdf_variant(preview),
            ),
            self.pdf_section(
                preview,
                PdfSection::Structure,
                ids::PDF_SECTION_STRUCTURE,
                Some(t!(i18n, structure).into()),
                || self.view_pdf_structure(preview),
            ),
            self.pdf_section(
                preview,
                PdfSection::Watermark,
                ids::PDF_SECTION_WATERMARK,
                None,
                || self.view_pdf_watermark(preview),
            ),
            self.pdf_section(
                preview,
                PdfSection::PdfLayers,
                ids::PDF_SECTION_LAYERS,
                None,
                || self.view_pdf_layers(preview),
            ),
            self.pdf_section(
                preview,
                PdfSection::Bookmarks,
                ids::PDF_SECTION_BOOKMARKS,
                None,
                || self.view_pdf_bookmarks(preview),
            ),
            self.pdf_section(
                preview,
                PdfSection::BookmarksAndNets,
                ids::PDF_SECTION_BOOKMARKS_NETS,
                Some(t!(i18n, ids::PDF_SUMMARY_ON, "count" => nets_on).into()),
                || self.view_pdf_bookmarks_and_nets(preview),
            ),
            self.pdf_section(
                preview,
                PdfSection::SchematicsInclude,
                ids::PDF_SECTION_SCHEMATICS_INCLUDE,
                summary_of(ticked(&include, options), include.len()),
                || column(checklist(&include, options, i18n))
                    .spacing(SP.xs)
                    .boxed(),
            ),
        ]
        .width(Length::Fill)
        .padding(iced::Padding {
            top: SP.xs,
            right: SP.m,
            bottom: SP.l,
            left: SP.m,
        });
        container(
            scrollable(sections)
                .direction(crate::styles::thin_vertical())
                .style(crate::styles::scrollbar_style(tokens))
                .height(Length::Fill),
        )
        .width(SIDE_W)
        .height(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(look.panel.into()),
            ..container::Style::default()
        })
        .boxed()
    }

    /// Output: the schematic and PCB colour modes, the quality and the title
    /// block, in one place.
    fn view_pdf_output<'a>(&'a self, preview: &'a PreviewState) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = Look::of(&self.document_view.panel_ctx.tokens);
        let options = &preview.pdf_options;
        let modes = || {
            [
                (ColourMode::Colour, ids::PDF_COLOR_MODE_COLOR),
                (ColourMode::Grayscale, ids::PDF_COLOR_MODE_GREYSCALE),
                (ColourMode::BlackAndWhite, ids::PDF_COLOR_MODE_MONOCHROME),
            ]
            .map(|(mode, label)| (mode, Label::from(t!(i18n, label))))
        };
        let qualities = Choice::named(
            &[
                PdfQuality::Draft72,
                PdfQuality::Medium300,
                PdfQuality::High600,
            ],
            PdfQuality::label,
            i18n,
        );
        let quality_chosen = crate::choice::chosen(&qualities, &preview.quality);
        let quality = pick_list(quality_chosen, qualities, |q: &Choice<PdfQuality>| {
            q.label.clone()
        })
        .graphite()
        .text_size(TS.body)
        .width(Length::Fill)
        .on_select(|q: Choice<PdfQuality>| {
            Message::PrintPreview(PrintPreviewMsg::SetQuality(q.value))
        });
        column![
            form_row(
                t!(i18n, ids::PDF_SCHEMATICS),
                &look,
                segmented(modes(), options.colour_mode, look.segmented(), |m| {
                    Message::PrintPreview(PrintPreviewMsg::SetColourMode(m))
                }),
            ),
            form_row(
                t!(i18n, ids::PDF_PCB),
                &look,
                segmented(modes(), options.pcb_colour_mode, look.segmented(), |m| {
                    Message::PrintPreview(PrintPreviewMsg::SetPcbColourMode(m))
                }),
            ),
            form_row(t!(i18n, ids::PDF_QUALITY), &look, quality.boxed()),
            check(
                t!(i18n, ids::PDF_TITLE_BLOCK),
                options.include_title_block,
                PrintPreviewMsg::SetIncludeTitleBlock,
            ),
        ]
        .spacing(SP.s + SP.xs)
        .width(Length::Fill)
        .boxed()
    }

    /// Variant: one variant per PDF, a single select.
    fn view_pdf_variant<'a>(&'a self, preview: &'a PreviewState) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = Look::of(&self.document_view.panel_ctx.tokens);
        let (variant_options, variant_chosen) = variant_choices(
            &preview.variants,
            preview.pdf_options.variant.as_deref(),
            i18n,
        );
        let picker = pick_list(
            Some(variant_chosen),
            variant_options,
            |c: &VariantChoice| c.label.clone(),
        )
        .graphite()
        .text_size(TS.body)
        .width(Length::Fill)
        .on_select(|c: VariantChoice| Message::PrintPreview(PrintPreviewMsg::SetVariant(c.value)));
        column![
            form_row(t!(i18n, ids::PDF_VARIANT), &look, picker.boxed()),
            caption(t!(i18n, ids::PDF_VARIANT_HELP), &look),
        ]
        .spacing(SP.xs)
        .width(Length::Fill)
        .boxed()
    }

    /// Structure: Use Physical Structure, and the expanded names it gates.
    fn view_pdf_structure<'a>(&'a self, preview: &'a PreviewState) -> Element<'a, Message> {
        let i18n = self.ui_state.i18n.as_ref();
        let look = Look::of(&self.document_view.panel_ctx.tokens);
        let options = &preview.pdf_options;
        let names = column![
            caption(t!(i18n, ids::PDF_EXPANDED_NAMES), &look),
            check(
                t!(i18n, ids::PDF_PHYSICAL_DESIGNATORS),
                options.physical_designators,
                PrintPreviewMsg::SetPhysicalDesignators,
            ),
            check(
                t!(i18n, ids::PDF_PHYSICAL_NET_LABELS),
                options.physical_net_labels,
                PrintPreviewMsg::SetPhysicalNetLabels,
            ),
            check(
                t!(i18n, ids::PDF_PHYSICAL_PORTS),
                options.physical_ports,
                PrintPreviewMsg::SetPhysicalPorts,
            ),
            check(
                t!(i18n, ids::PDF_PHYSICAL_SHEET_NUMBER),
                options.physical_sheet_number,
                PrintPreviewMsg::SetPhysicalSheetNumber,
            ),
            check(
                t!(i18n, ids::PDF_PHYSICAL_DOCUMENT_NUMBER),
                options.physical_document_number,
                PrintPreviewMsg::SetPhysicalDocumentNumber,
            ),
        ]
        .spacing(SP.xs);
        column![
            check_with_help(
                t!(i18n, ids::PDF_USE_PHYSICAL),
                t!(i18n, ids::PDF_USE_PHYSICAL_HELP),
                options.use_physical_structure,
                PrintPreviewMsg::SetUsePhysicalStructure,
                &look,
            ),
            gated(
                indented(names.boxed()),
                preview.physical_options_enabled(),
                &look
            ),
        ]
        .spacing(SP.xs)
        .width(Length::Fill)
        .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Base is the first option and means no variant (#2059). A variant
    /// named "Base" in any case is Base itself, as on trunk: no row of its
    /// own, and choosing it prints the base design.
    #[test]
    fn the_variant_list_holds_base_as_no_variant() {
        let i18n = I18n::en_us();
        for base in ["Base", "base"] {
            let variants = [base.to_owned(), "Lite".to_owned()];
            let (options, chosen) = variant_choices(&variants, None, &i18n);
            let values: Vec<_> = options.iter().map(|c| c.value.as_deref()).collect();
            assert_eq!(values, [None, Some("Lite")], "{base}");
            assert_eq!(options[0].label, "Base");
            assert_eq!(chosen.value, None);
            let (_, chosen) = variant_choices(&variants, Some(base), &i18n);
            assert_eq!(chosen.value, None, "{base}");
            let (_, chosen) = variant_choices(&variants, Some("Lite"), &i18n);
            assert_eq!(chosen.value.as_deref(), Some("Lite"));
        }
    }

    /// The quality names read as they did when they came from `Display`.
    #[test]
    fn the_quality_names_come_from_the_catalog() {
        let i18n = I18n::en_us();
        let all = [
            PdfQuality::Draft72,
            PdfQuality::Medium300,
            PdfQuality::High600,
        ];
        let labels: Vec<String> = Choice::named(&all, PdfQuality::label, &i18n)
            .into_iter()
            .map(|c| c.label)
            .collect();
        assert_eq!(
            labels,
            ["Draft (72 dpi)", "Medium (300 dpi)", "High (600 dpi)"]
        );
    }
}
