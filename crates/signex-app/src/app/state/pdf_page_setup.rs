//! Text-field state for the PDF export modal's page setup controls.
//!
//! The authoritative values live on `PreviewState.pdf_options`; these strings
//! are what the user has typed. They differ only while the text is invalid —
//! `pdf_options` then keeps the last good value and `notice` says why.

use signex_output::{
    MarginEdge, Orientation, PageSetupError, PageSize, PdfOptions, PdfScale, PdfScaleMode,
    parse_mm, parse_percent,
};

/// Millimetres as the shortest faithful text (`10`, `215.9`, not `10.000000`).
pub(crate) fn format_mm(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[derive(Debug, Clone)]
pub struct PageSetupInputs {
    pub custom_width: String,
    pub custom_height: String,
    /// Indexed by [`MarginEdge::ALL`] order: top, right, bottom, left.
    pub margins: [String; 4],
    pub percent: String,
    /// Why the last edit was rejected or adjusted; `None` when it applied cleanly.
    pub notice: Option<String>,
}

impl PageSetupInputs {
    /// Text that mirrors `options` exactly.
    pub fn from_options(options: &PdfOptions) -> Self {
        let mut inputs = Self {
            custom_width: String::new(),
            custom_height: String::new(),
            margins: Default::default(),
            percent: "100".to_string(),
            notice: None,
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
/// A rejected edit leaves `pdf_options` untouched and sets `page_setup.notice`.
impl super::PreviewState {
    fn reject(&mut self, field: &str, err: &PageSetupError, kept: String) -> bool {
        self.page_setup.notice = Some(format!("{field}: {err}. Keeping {kept}."));
        false
    }

    /// A page change (size or orientation) can leave margins larger than the
    /// page; pull them back and refresh the mirrored text.
    fn refit_margins_to_page(&mut self) {
        let (w, h) = self
            .pdf_options
            .page_size
            .dimensions_mm(self.pdf_options.orientation);
        self.pdf_options.margins = self.pdf_options.margins.fitted_to_page(w, h);
        for edge in MarginEdge::ALL {
            let mm = self.pdf_options.margins.edge_mm(edge);
            self.page_setup.set_margin_text(edge, format_mm(mm));
        }
    }

    pub fn apply_page_size(&mut self, size: PageSize) -> bool {
        self.pdf_options.page_size = size;
        self.page_setup.notice = None;
        self.page_setup.sync_size(&self.pdf_options);
        self.refit_margins_to_page();
        true
    }

    pub fn apply_orientation(&mut self, orientation: Orientation) -> bool {
        if self.pdf_options.orientation == orientation {
            return false;
        }
        self.pdf_options.orientation = orientation;
        // Custom sizes are never rotated by the exporter, so the flip has to
        // swap the stored edges for the page to actually turn.
        if let PageSize::Custom {
            width_mm,
            height_mm,
        } = self.pdf_options.page_size
        {
            self.pdf_options.page_size = PageSize::Custom {
                width_mm: height_mm,
                height_mm: width_mm,
            };
        }
        self.page_setup.notice = None;
        self.page_setup.sync_size(&self.pdf_options);
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
        let (field, kept) = if is_width {
            ("Width", width_mm)
        } else {
            ("Height", height_mm)
        };
        let parsed = parse_mm(&text);
        if is_width {
            self.page_setup.custom_width = text;
        } else {
            self.page_setup.custom_height = text;
        }
        match parsed {
            Err(err) => self.reject(field, &err, format!("{} mm", format_mm(kept))),
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
                self.page_setup.notice = None;
                self.refit_margins_to_page();
                true
            }
        }
    }

    pub fn apply_margin(&mut self, edge: MarginEdge, text: String) -> bool {
        let (w, h) = self
            .pdf_options
            .page_size
            .dimensions_mm(self.pdf_options.orientation);
        let result = self.pdf_options.margins.with_edge_text(edge, &text, w, h);
        self.page_setup.set_margin_text(edge, text);
        match result {
            Err(err) => {
                let kept = format!("{} mm", format_mm(self.pdf_options.margins.edge_mm(edge)));
                self.reject(&format!("{edge} margin"), &err, kept)
            }
            Ok(update) => {
                self.pdf_options.margins = update.margins;
                self.page_setup.notice = None;
                if update.clamped {
                    let mm = format_mm(update.margins.edge_mm(edge));
                    self.page_setup.set_margin_text(edge, mm.clone());
                    self.page_setup.notice =
                        Some(format!("{edge} margin limited to {mm} mm to fit the page."));
                }
                true
            }
        }
    }

    pub fn apply_scale_mode(&mut self, mode: PdfScaleMode) -> bool {
        self.pdf_options.scale = match mode {
            PdfScaleMode::FitToPage => PdfScale::FitToPage,
            PdfScaleMode::OneToOne => PdfScale::OneToOne,
            PdfScaleMode::Percent => match parse_percent(&self.page_setup.percent) {
                Ok(percent) => PdfScale::Percent(percent),
                Err(_) => {
                    self.page_setup.percent = "100".to_string();
                    PdfScale::Percent(100.0)
                }
            },
        };
        self.page_setup.notice = None;
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
                self.reject("Scale", &err, kept)
            }
            Ok(percent) => {
                self.page_setup.notice = None;
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
