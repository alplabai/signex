//! Text-field state for the PDF export modal's page setup controls.
//!
//! The authoritative values live on `PreviewState.pdf_options`; these strings
//! are what the user has typed. They differ only while the text is invalid —
//! `pdf_options` then keeps the last good value and the notice says why. Each
//! field tracks its own rejection, so a valid edit to one field never hides
//! the complaint about another that still holds rejected text.

use signex_output::{
    MarginEdge, Orientation, PageSetupError, PageSize, PdfOptions, PdfScale, PdfScaleMode,
    parse_custom_edge_mm, parse_percent,
};

/// Millimetres as the shortest faithful text (`10`, `215.9`, not `10.000000`).
pub(crate) fn format_mm(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A text field of the page setup controls that can hold rejected input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageField {
    CustomWidth,
    CustomHeight,
    Margin(MarginEdge),
    Percent,
}

#[derive(Debug, Clone)]
pub struct PageSetupInputs {
    pub custom_width: String,
    pub custom_height: String,
    /// Indexed by [`MarginEdge::ALL`] order: top, right, bottom, left.
    pub margins: [String; 4],
    pub percent: String,
    /// One entry per field whose current text was rejected, oldest first.
    rejections: Vec<(PageField, String)>,
    /// Note about an automatic adjustment (a margin limited or refitted to the
    /// page); cleared by the next edit.
    info: Option<String>,
}

impl PageSetupInputs {
    /// Text that mirrors `options` exactly.
    pub fn from_options(options: &PdfOptions) -> Self {
        let mut inputs = Self {
            custom_width: String::new(),
            custom_height: String::new(),
            margins: Default::default(),
            percent: "100".to_string(),
            rejections: Vec::new(),
            info: None,
        };
        inputs.sync_size(options);
        for (slot, edge) in inputs.margins.iter_mut().zip(MarginEdge::ALL) {
            *slot = format_mm(options.margins.edge_mm(edge));
        }
        if let PdfScale::Percent(p) = options.scale {
            inputs.percent = format_mm(p);
        }
        inputs
    }

    /// Re-read the custom width/height fields from the page size.
    pub fn sync_size(&mut self, options: &PdfOptions) {
        if let PageSize::Custom {
            width_mm,
            height_mm,
        } = options.page_size
        {
            self.custom_width = format_mm(width_mm);
            self.custom_height = format_mm(height_mm);
        }
    }

    pub fn margin_text(&self, edge: MarginEdge) -> &str {
        &self.margins[Self::slot(edge)]
    }

    pub fn set_margin_text(&mut self, edge: MarginEdge, text: String) {
        self.margins[Self::slot(edge)] = text;
    }

    /// What to tell the user: the oldest rejection that still stands, else
    /// the adjustment note.
    pub fn notice(&self) -> Option<&str> {
        self.rejections
            .first()
            .map(|(_, message)| message.as_str())
            .or(self.info.as_deref())
    }

    /// Whether `field` currently holds rejected text.
    pub fn is_rejected(&self, field: PageField) -> bool {
        self.rejections.iter().any(|(f, _)| *f == field)
    }

    /// Record an automatic adjustment: shown in the modal and written to the
    /// Messages panel, so a value the user did not type never changes unseen.
    fn note(&mut self, message: String) {
        crate::diagnostics::log_info(format!("PDF page setup: {message}"));
        self.info = Some(message);
    }

    fn reject(&mut self, field: PageField, message: String) {
        crate::diagnostics::log_warning(format!("PDF page setup: {message}"));
        self.clear_rejection(field);
        self.rejections.push((field, message));
        self.info = None;
    }

    fn clear_rejection(&mut self, field: PageField) {
        self.rejections.retain(|(f, _)| *f != field);
    }

    fn slot(edge: MarginEdge) -> usize {
        match edge {
            MarginEdge::Top => 0,
            MarginEdge::Right => 1,
            MarginEdge::Bottom => 2,
            MarginEdge::Left => 3,
        }
    }
}

/// Edits that validate typed text and mutate `pdf_options`. Each returns
/// `true` when the options changed, i.e. when the preview must be re-rasterised.
/// A rejected edit leaves `pdf_options` untouched and records a rejection for
/// that field only.
impl super::PreviewState {
    fn reject(
        &mut self,
        field: PageField,
        label: &str,
        err: &PageSetupError,
        kept: String,
    ) -> bool {
        self.page_setup
            .reject(field, format!("{label}: {err}. Keeping {kept}."));
        false
    }

    /// A Custom page has no orientation of its own (the exporter never
    /// rotates it), so the orientation follows the page's shape.
    fn derive_orientation_for_custom(&mut self) {
        if let PageSize::Custom {
            width_mm,
            height_mm,
        } = self.pdf_options.page_size
        {
            self.pdf_options.orientation = if width_mm >= height_mm {
                Orientation::Landscape
            } else {
                Orientation::Portrait
            };
        }
    }

    /// A page change (size, orientation or custom edge) can leave margins
    /// larger than the page; pull them back, refresh the text of every margin
    /// that moved and say so.
    fn refit_margins_to_page(&mut self) {
        let (w, h) = self
            .pdf_options
            .page_size
            .dimensions_mm(self.pdf_options.orientation);
        let before = self.pdf_options.margins;
        self.pdf_options.margins = before.fitted_to_page(w, h);
        let mut refitted = false;
        let mut changes = Vec::new();
        for edge in MarginEdge::ALL {
            let mm = self.pdf_options.margins.edge_mm(edge);
            if (mm - before.edge_mm(edge)).abs() > 1e-9 {
                refitted = true;
                changes.push(format!(
                    "{edge} {} -> {} mm",
                    format_mm(before.edge_mm(edge)),
                    format_mm(mm)
                ));
                self.page_setup.set_margin_text(edge, format_mm(mm));
                self.page_setup.clear_rejection(PageField::Margin(edge));
            }
        }
        if refitted {
            self.page_setup.note(format!(
                "Margins were reduced to fit the new page size ({}).",
                changes.join(", ")
            ));
        }
    }

    pub fn apply_page_size(&mut self, size: PageSize) -> bool {
        self.pdf_options.page_size = size;
        self.page_setup.info = None;
        self.page_setup.clear_rejection(PageField::CustomWidth);
        self.page_setup.clear_rejection(PageField::CustomHeight);
        self.derive_orientation_for_custom();
        self.page_setup.sync_size(&self.pdf_options);
        self.refit_margins_to_page();
        true
    }

    pub fn apply_orientation(&mut self, orientation: Orientation) -> bool {
        // A Custom page's orientation is derived from its width and height.
        if matches!(self.pdf_options.page_size, PageSize::Custom { .. })
            || self.pdf_options.orientation == orientation
        {
            return false;
        }
        self.pdf_options.orientation = orientation;
        self.page_setup.info = None;
        self.refit_margins_to_page();
        true
    }

    pub fn apply_custom_dimension(&mut self, is_width: bool, text: String) -> bool {
        let PageSize::Custom {
            width_mm,
            height_mm,
        } = self.pdf_options.page_size
        else {
            return false;
        };
        let (field, label, kept) = if is_width {
            (PageField::CustomWidth, "Width", width_mm)
        } else {
            (PageField::CustomHeight, "Height", height_mm)
        };
        let parsed = parse_custom_edge_mm(&text);
        if is_width {
            self.page_setup.custom_width = text;
        } else {
            self.page_setup.custom_height = text;
        }
        match parsed {
            Err(err) => self.reject(field, label, &err, format!("{} mm", format_mm(kept))),
            Ok(value) => {
                self.pdf_options.page_size = if is_width {
                    PageSize::Custom {
                        width_mm: value,
                        height_mm,
                    }
                } else {
                    PageSize::Custom {
                        width_mm,
                        height_mm: value,
                    }
                };
                self.page_setup.clear_rejection(field);
                self.page_setup.info = None;
                self.derive_orientation_for_custom();
                self.refit_margins_to_page();
                true
            }
        }
    }

    pub fn apply_margin(&mut self, edge: MarginEdge, text: String) -> bool {
        let field = PageField::Margin(edge);
        let (w, h) = self
            .pdf_options
            .page_size
            .dimensions_mm(self.pdf_options.orientation);
        let result = self.pdf_options.margins.with_edge_text(edge, &text, w, h);
        self.page_setup.set_margin_text(edge, text);
        match result {
            Err(err) => {
                let kept = format!("{} mm", format_mm(self.pdf_options.margins.edge_mm(edge)));
                self.reject(field, &format!("{edge} margin"), &err, kept)
            }
            Ok(update) => {
                self.pdf_options.margins = update.margins;
                self.page_setup.clear_rejection(field);
                self.page_setup.info = None;
                if update.clamped {
                    let mm = format_mm(update.margins.edge_mm(edge));
                    self.page_setup.set_margin_text(edge, mm.clone());
                    self.page_setup
                        .note(format!("{edge} margin limited to {mm} mm to fit the page."));
                }
                true
            }
        }
    }

    pub fn apply_scale_mode(&mut self, mode: PdfScaleMode) -> bool {
        let mut fallback_note = None;
        self.pdf_options.scale = match mode {
            PdfScaleMode::FitToPage => PdfScale::FitToPage,
            PdfScaleMode::OneToOne => PdfScale::OneToOne,
            PdfScaleMode::Percent => match parse_percent(&self.page_setup.percent) {
                Ok(percent) => PdfScale::Percent(percent),
                Err(_) => {
                    fallback_note = Some(format!(
                        "Scale text \"{}\" is not a valid percentage; reset to 100%.",
                        self.page_setup.percent
                    ));
                    self.page_setup.percent = "100".to_string();
                    PdfScale::Percent(100.0)
                }
            },
        };
        // The percent field is either reset to a good value or hidden.
        self.page_setup.clear_rejection(PageField::Percent);
        self.page_setup.info = None;
        if let Some(message) = fallback_note {
            self.page_setup.note(message);
        }
        true
    }

    pub fn apply_scale_percent(&mut self, text: String) -> bool {
        let parsed = parse_percent(&text);
        self.page_setup.percent = text;
        match parsed {
            Err(err) => {
                let kept = match self.pdf_options.scale {
                    PdfScale::Percent(p) => format!("{}%", format_mm(p)),
                    _ => "the current scale".to_string(),
                };
                self.reject(PageField::Percent, "Scale", &err, kept)
            }
            Ok(percent) => {
                self.page_setup.clear_rejection(PageField::Percent);
                self.page_setup.info = None;
                if matches!(self.pdf_options.scale, PdfScale::Percent(_)) {
                    self.pdf_options.scale = PdfScale::Percent(percent);
                    true
                } else {
                    false
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mm_text_drops_trailing_zeros() {
        assert_eq!(format_mm(10.0), "10");
        assert_eq!(format_mm(215.9), "215.9");
        assert_eq!(format_mm(0.5), "0.5");
        assert_eq!(format_mm(199.000000001), "199");
    }
}
