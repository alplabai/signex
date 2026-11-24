//! State of the one-screen Export PDF dialog (#2430) that is not a
//! `PdfOptions` field: which settings sections are folded, and the
//! Watermark and PDF Layers switches with the settings they keep while off
//! (#1298). Rules: docs/MENU_BAR.md, "File ▸ Export ▸ PDF".

use std::collections::BTreeSet;

use signex_output::{PdfLayer, PdfLayerVisibility, PdfOptions, Watermark, WatermarkText};

use super::PreviewState;

/// A section of the dialog's settings column, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PdfSection {
    Sheets,
    Pages,
    PageSetup,
    Output,
    Variant,
    Structure,
    Watermark,
    PdfLayers,
    Bookmarks,
    BookmarksAndNets,
    SchematicsInclude,
}

impl PdfSection {
    /// Rarely changed, so folded when the dialog opens; their header shows a
    /// summary instead.
    pub fn starts_collapsed(self) -> bool {
        matches!(
            self,
            Self::Structure | Self::BookmarksAndNets | Self::SchematicsInclude
        )
    }
}

/// Which settings sections are folded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfSections {
    collapsed: BTreeSet<PdfSection>,
}

impl Default for PdfSections {
    fn default() -> Self {
        use PdfSection::*;
        let all = [
            Sheets,
            Pages,
            PageSetup,
            Output,
            Variant,
            Structure,
            Watermark,
            PdfLayers,
            Bookmarks,
            BookmarksAndNets,
            SchematicsInclude,
        ];
        Self {
            collapsed: all.into_iter().filter(|s| s.starts_collapsed()).collect(),
        }
    }
}

impl PdfSections {
    pub fn is_open(&self, section: PdfSection) -> bool {
        !self.collapsed.contains(&section)
    }

    /// Fold an open section, unfold a folded one.
    pub fn toggle(&mut self, section: PdfSection) {
        if !self.collapsed.remove(&section) {
            self.collapsed.insert(section);
        }
    }
}

/// The watermark preset switch: the two printed presets or the user's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatermarkPreset {
    Draft,
    Confidential,
    Custom,
}

/// The Watermark section. Off by default (an export had no watermark
/// before #1298). The settings stay while the switch is off, so turning it
/// back on restores them.
#[derive(Debug, Clone, PartialEq)]
pub struct WatermarkDraft {
    pub is_on: bool,
    /// What prints while on; `revision_line` stays `None` here and is filled
    /// from [`Self::stamp`] when the options are handed out.
    pub settings: Watermark,
    /// "Add date and revision under the text".
    pub with_revision: bool,
    /// The last Custom text, kept while a preset is picked.
    pub custom_text: String,
    /// The Opacity field as typed.
    pub opacity_input: String,
    /// The Angle field as typed.
    pub angle_input: String,
    /// The project revision, read by the handler from the export metadata.
    pub revision: String,
    /// The export date (`YYYY-MM-DD`), taken by the handler, never by a view.
    pub date: String,
}

impl Default for WatermarkDraft {
    fn default() -> Self {
        let settings = Watermark::default();
        Self {
            is_on: false,
            opacity_input: settings.opacity_percent.to_string(),
            angle_input: angle_text(settings.angle_deg),
            settings,
            with_revision: false,
            custom_text: String::new(),
            revision: String::new(),
            date: String::new(),
        }
    }
}

impl WatermarkDraft {
    pub fn preset(&self) -> WatermarkPreset {
        match self.settings.text {
            WatermarkText::Draft => WatermarkPreset::Draft,
            WatermarkText::Confidential => WatermarkPreset::Confidential,
            WatermarkText::Custom(_) => WatermarkPreset::Custom,
        }
    }

    /// Pick a preset. Custom brings back the last custom text, or starts from
    /// the text shown so the watermark never goes blank.
    pub fn pick_preset(&mut self, preset: WatermarkPreset) {
        if let WatermarkText::Custom(text) = &self.settings.text {
            self.custom_text = text.clone();
        }
        self.settings.text = match preset {
            WatermarkPreset::Draft => WatermarkText::Draft,
            WatermarkPreset::Confidential => WatermarkText::Confidential,
            WatermarkPreset::Custom => {
                if self.custom_text.is_empty() {
                    self.custom_text = self.settings.text.printed().to_owned();
                }
                WatermarkText::Custom(self.custom_text.clone())
            }
        };
    }

    /// The Text field: typed text is Custom text.
    pub fn set_text(&mut self, text: String) {
        self.custom_text = text.clone();
        self.settings.text = WatermarkText::Custom(text);
    }

    /// The Opacity field or slider. Text that is not a whole percent from 0
    /// to 100 is kept as typed and changes nothing.
    pub fn set_opacity(&mut self, text: String) {
        if let Some(percent) = parse_percent(&text) {
            self.settings.opacity_percent = percent;
        }
        self.opacity_input = text;
    }

    /// The Angle field or the 0 / 45 / 90 switch. Text that is not a finite
    /// angle from -360 to 360 degrees is kept as typed and changes nothing.
    pub fn set_angle(&mut self, text: String) {
        if let Some(angle) = parse_angle(&text) {
            self.settings.angle_deg = angle;
        }
        self.angle_input = text;
    }

    /// The preset angle the switch highlights, `None` for a typed angle such
    /// as 30.
    pub fn angle_preset(&self) -> Option<i32> {
        ANGLE_PRESETS
            .into_iter()
            .find(|&preset| f64::from(preset) == self.settings.angle_deg)
    }

    /// The line printed under the text, such as "Rev B · 2026-11-19"; `None`
    /// while the box is clear or there is neither a revision nor a date.
    /// Fixed English, never the UI language: it is PDF text.
    pub fn stamp(&self) -> Option<String> {
        if !self.with_revision {
            return None;
        }
        Watermark::revision_line_for(&self.revision, &self.date)
    }

    /// The watermark the PDF prints: `None` while the switch is off.
    pub fn output(&self) -> Option<Watermark> {
        self.is_on.then(|| Watermark {
            revision_line: self.stamp(),
            ..self.settings.clone()
        })
    }
}

/// The angles of the 0 / 45 / 90 switch.
pub const ANGLE_PRESETS: [i32; 3] = [0, 45, 90];

/// Largest angle the Angle field takes, either way round.
const MAX_ANGLE_DEG: f64 = 360.0;

fn parse_percent(text: &str) -> Option<u8> {
    let digits = text.trim().trim_end_matches('%').trim();
    digits.parse::<u8>().ok().filter(|p| *p <= 100)
}

fn parse_angle(text: &str) -> Option<f64> {
    let digits = text.trim().trim_end_matches('°').trim();
    digits
        .parse::<f64>()
        .ok()
        .filter(|a| a.is_finite() && a.abs() <= MAX_ANGLE_DEG)
}

/// An angle as the Angle field shows it: whole degrees without a fraction.
pub fn angle_text(angle_deg: f64) -> String {
    if angle_deg.fract() == 0.0 {
        format!("{angle_deg:.0}")
    } else {
        angle_deg.to_string()
    }
}

/// The PDF Layers section. Off by default, so an export stays a flat PDF as
/// before #1298 until the user ticks it (Caner, 2026-11-20). Every group
/// visible when the PDF opens.
#[derive(Debug, Clone, PartialEq)]
pub struct PdfLayersDraft {
    pub is_on: bool,
    pub visibility: PdfLayerVisibility,
}

impl Default for PdfLayersDraft {
    fn default() -> Self {
        Self {
            is_on: false,
            visibility: PdfLayerVisibility {
                hidden_on_open: BTreeSet::new(),
            },
        }
    }
}

impl PdfLayersDraft {
    pub fn is_visible(&self, layer: PdfLayer) -> bool {
        !self.visibility.hidden_on_open.contains(&layer)
    }

    pub fn set_visible(&mut self, layer: PdfLayer, visible: bool) {
        if visible {
            self.visibility.hidden_on_open.remove(&layer);
        } else {
            self.visibility.hidden_on_open.insert(layer);
        }
    }

    /// The layers the PDF writes: `None` (a flat PDF) while the switch is off.
    pub fn output(&self) -> Option<PdfLayerVisibility> {
        self.is_on.then(|| self.visibility.clone())
    }
}

/// The dialog's own state beside `PreviewState::pdf_options`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PdfDialogState {
    pub sections: PdfSections,
    pub watermark: WatermarkDraft,
    pub layers: PdfLayersDraft,
}

impl PreviewState {
    /// The options an export writes with: `pdf_options` plus the watermark
    /// and the layers, each only while its switch is on. The preview
    /// rasterises `pdf_options` as is and draws the watermark over the page
    /// itself, so `pdf_options.watermark` and `.layers` stay `None`.
    pub fn output_options(&self) -> PdfOptions {
        PdfOptions {
            watermark: self.dialog.watermark.output(),
            layers: self.dialog.layers.output(),
            ..self.pdf_options.clone()
        }
    }

    /// Whether the dependents of Use Physical Structure take input.
    pub fn physical_options_enabled(&self) -> bool {
        self.pdf_options.use_physical_structure
    }

    /// Whether the Group switch of One bookmark per part takes input.
    pub fn part_order_enabled(&self) -> bool {
        self.pdf_options.bookmark_parts
    }

    /// Whether a PDF Layers row takes input: the Watermark row only while a
    /// watermark prints.
    pub fn layer_row_enabled(&self, layer: PdfLayer) -> bool {
        layer != PdfLayer::Watermark || self.dialog.watermark.is_on
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structure_and_the_two_checklists_start_folded() {
        let sections = PdfSections::default();
        for open in [
            PdfSection::Sheets,
            PdfSection::Pages,
            PdfSection::PageSetup,
            PdfSection::Output,
            PdfSection::Variant,
            PdfSection::Watermark,
            PdfSection::PdfLayers,
            PdfSection::Bookmarks,
        ] {
            assert!(sections.is_open(open), "{open:?}");
        }
        for folded in [
            PdfSection::Structure,
            PdfSection::BookmarksAndNets,
            PdfSection::SchematicsInclude,
        ] {
            assert!(!sections.is_open(folded), "{folded:?}");
        }
    }

    #[test]
    fn a_section_toggles_open_and_shut() {
        let mut sections = PdfSections::default();
        sections.toggle(PdfSection::Structure);
        assert!(sections.is_open(PdfSection::Structure));
        sections.toggle(PdfSection::Structure);
        assert!(!sections.is_open(PdfSection::Structure));
    }

    #[test]
    fn the_watermark_is_off_by_default_and_keeps_its_settings_while_off() {
        let mut draft = WatermarkDraft::default();
        assert!(!draft.is_on);
        assert_eq!(draft.output(), None);
        draft.pick_preset(WatermarkPreset::Confidential);
        draft.set_opacity("40".into());
        draft.is_on = true;
        let printed = draft.output().expect("on");
        assert_eq!(printed.text, WatermarkText::Confidential);
        assert_eq!(printed.opacity_percent, 40);
        draft.is_on = false;
        assert_eq!(draft.output(), None);
        draft.is_on = true;
        assert_eq!(draft.output(), Some(printed), "restored as it was");
    }

    #[test]
    fn custom_text_comes_back_after_a_preset() {
        let mut draft = WatermarkDraft::default();
        draft.set_text("FOR BENCH USE ONLY".into());
        assert_eq!(draft.preset(), WatermarkPreset::Custom);
        draft.pick_preset(WatermarkPreset::Draft);
        assert_eq!(draft.settings.text, WatermarkText::Draft);
        draft.pick_preset(WatermarkPreset::Custom);
        assert_eq!(
            draft.settings.text,
            WatermarkText::Custom("FOR BENCH USE ONLY".into())
        );
    }

    #[test]
    fn custom_without_text_starts_from_the_preset_shown() {
        let mut draft = WatermarkDraft::default();
        draft.pick_preset(WatermarkPreset::Custom);
        assert_eq!(
            draft.settings.text,
            WatermarkText::Custom(WatermarkText::Draft.printed().to_owned())
        );
    }

    #[test]
    fn a_typed_angle_deselects_the_presets_and_bad_text_changes_nothing() {
        let mut draft = WatermarkDraft::default();
        assert_eq!(draft.angle_preset(), Some(45));
        draft.set_angle("30".into());
        assert_eq!(draft.settings.angle_deg, 30.0);
        assert_eq!(draft.angle_preset(), None);
        draft.set_angle("90°".into());
        assert_eq!(draft.angle_preset(), Some(90));
        for bad in ["", "abc", "720", "NaN", "inf"] {
            draft.set_angle(bad.into());
            assert_eq!(draft.settings.angle_deg, 90.0, "{bad:?}");
            assert_eq!(draft.angle_input, bad);
        }
    }

    #[test]
    fn opacity_takes_whole_percents_up_to_100() {
        let mut draft = WatermarkDraft::default();
        draft.set_opacity("35 %".into());
        assert_eq!(draft.settings.opacity_percent, 35);
        for bad in ["101", "-1", "12.5", "x"] {
            draft.set_opacity(bad.into());
            assert_eq!(draft.settings.opacity_percent, 35, "{bad:?}");
        }
    }

    #[test]
    fn the_stamp_reads_rev_and_date_only_when_ticked() {
        let mut draft = WatermarkDraft {
            revision: "B".into(),
            date: "2026-11-19".into(),
            ..WatermarkDraft::default()
        };
        assert_eq!(draft.stamp(), None);
        draft.with_revision = true;
        assert_eq!(draft.stamp().as_deref(), Some("Rev B · 2026-11-19"));
        draft.revision.clear();
        assert_eq!(draft.stamp().as_deref(), Some("2026-11-19"));
        draft.date.clear();
        assert_eq!(draft.stamp(), None);
    }

    #[test]
    fn layers_are_flat_by_default_and_written_when_switched_on() {
        let mut layers = PdfLayersDraft::default();
        assert!(!layers.is_on);
        assert_eq!(layers.output(), None, "a flat PDF, as before #1298");
        layers.is_on = true;
        layers.set_visible(PdfLayer::Parts, false);
        let written = layers.output().expect("on");
        assert!(written.hidden_on_open.contains(&PdfLayer::Parts));
        layers.is_on = false;
        assert_eq!(layers.output(), None);
        layers.is_on = true;
        assert!(!layers.is_visible(PdfLayer::Parts), "kept while off");
    }

    #[test]
    fn the_exporter_gets_watermark_and_layers_only_while_switched_on() {
        let mut preview = PreviewState::blank_for_tests();
        let options = preview.output_options();
        assert_eq!(options.watermark, None, "watermark off by default");
        assert_eq!(options.layers, None, "layers off by default");

        preview.dialog.watermark.is_on = true;
        preview.dialog.layers.is_on = true;
        let options = preview.output_options();
        assert!(options.watermark.is_some());
        assert!(options.layers.is_some());
        preview.dialog.layers.is_on = false;
        assert_eq!(preview.output_options().layers, None);
        assert_eq!(
            preview.pdf_options.watermark, None,
            "the preview's own options never carry the watermark"
        );
    }

    #[test]
    fn dependents_follow_their_switches() {
        let mut preview = PreviewState::blank_for_tests();
        preview.pdf_options.use_physical_structure = false;
        preview.pdf_options.bookmark_parts = false;
        assert!(!preview.physical_options_enabled());
        assert!(!preview.part_order_enabled());
        assert!(!preview.layer_row_enabled(PdfLayer::Watermark));
        assert!(preview.layer_row_enabled(PdfLayer::Parts));
        preview.pdf_options.use_physical_structure = true;
        preview.pdf_options.bookmark_parts = true;
        preview.dialog.watermark.is_on = true;
        assert!(preview.physical_options_enabled());
        assert!(preview.part_order_enabled());
        assert!(preview.layer_row_enabled(PdfLayer::Watermark));
    }
}
