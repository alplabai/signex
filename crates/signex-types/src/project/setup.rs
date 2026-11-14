//! The defaults a project is created with (#2232): PCB rules and sheet
//! defaults.
//!
//! They are the project's own: a PCB or sheet added later starts from them
//! with no dialog. Every part is optional and written only when set, so a
//! project that never chose any re-saves byte-identical. The designator
//! standard lives in [`super::AnnotationSettings`]. Format and ranges:
//! `docs/PROJECT_SETUP.md`.

use serde::{Deserialize, Serialize};

use crate::designator::DesignatorStandard;
use crate::pcb::{LayerDef, PcbBoard, PcbSetup};
use crate::pcb_layer::{CopperLayer, InnerCopper, PcbLayer, TechnicalLayer};
use crate::schematic::SchematicSheet;

/// The copper layer counts a board can have.
pub const COPPER_LAYERS: std::ops::RangeInclusive<u8> = 1..=32;

/// One PCB rule field, for validation messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PcbField {
    CopperLayers,
    Thickness,
    Clearance,
    TrackWidth,
    ViaSize,
    ViaDrill,
}

impl PcbField {
    /// The operand and rule-file key.
    pub const fn key(self) -> &'static str {
        match self {
            Self::CopperLayers => "copper_layers",
            Self::Thickness => "thickness_mm",
            Self::Clearance => "clearance_mm",
            Self::TrackWidth => "track_width_mm",
            Self::ViaSize => "via_diameter_mm",
            Self::ViaDrill => "via_drill_mm",
        }
    }

    /// Accepted range in mm (layers: a count).
    pub const fn range(self) -> (f64, f64) {
        match self {
            Self::CopperLayers => (1.0, 32.0),
            Self::Thickness => (0.1, 10.0),
            Self::Clearance => (0.05, 5.0),
            Self::TrackWidth => (0.05, 25.0),
            Self::ViaSize => (0.1, 10.0),
            Self::ViaDrill => (0.05, 9.9),
        }
    }
}

/// Why a PCB rule value is refused.
#[derive(Debug, Clone, PartialEq)]
pub enum PcbValueError {
    NotANumber,
    OutOfRange {
        min: f64,
        max: f64,
    },
    /// A drill as large as the via it goes through.
    DrillNotSmallerThanVia,
}

impl std::fmt::Display for PcbValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotANumber => f.write_str("not a number"),
            Self::OutOfRange { min, max } => write!(f, "must be {min} to {max}"),
            Self::DrillNotSmallerThanVia => f.write_str("must be smaller than the via size"),
        }
    }
}

impl PcbField {
    /// Check one value against the field's range.
    pub fn check(self, value: f64) -> Result<f64, PcbValueError> {
        let (min, max) = self.range();
        if !value.is_finite() {
            return Err(PcbValueError::NotANumber);
        }
        if value < min || value > max {
            return Err(PcbValueError::OutOfRange { min, max });
        }
        Ok(value)
    }

    /// Parse typed text (`.` decimal, optional trailing `mm`) and check it.
    pub fn parse(self, text: &str) -> Result<f64, PcbValueError> {
        let t = text.trim();
        let t = t.strip_suffix("mm").map_or(t, str::trim_end);
        let value: f64 = t.parse().map_err(|_| PcbValueError::NotANumber)?;
        self.check(value)
    }
}

/// The PCB rules a new board starts from (mm).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PcbDefaults {
    pub copper_layers: u8,
    pub thickness_mm: f64,
    pub clearance_mm: f64,
    pub track_width_mm: f64,
    pub via_diameter_mm: f64,
    pub via_drill_mm: f64,
}

impl PcbDefaults {
    /// Every field's problem, in field order; empty when the rules are valid.
    pub fn problems(&self) -> Vec<(PcbField, PcbValueError)> {
        let mut out = Vec::new();
        let mut push = |field: PcbField, value: f64| {
            if let Err(e) = field.check(value) {
                out.push((field, e));
            }
        };
        push(PcbField::CopperLayers, f64::from(self.copper_layers));
        push(PcbField::Thickness, self.thickness_mm);
        push(PcbField::Clearance, self.clearance_mm);
        push(PcbField::TrackWidth, self.track_width_mm);
        push(PcbField::ViaSize, self.via_diameter_mm);
        push(PcbField::ViaDrill, self.via_drill_mm);
        if self.via_drill_mm >= self.via_diameter_mm
            && !out.iter().any(|(f, _)| *f == PcbField::ViaDrill)
        {
            out.push((PcbField::ViaDrill, PcbValueError::DrillNotSmallerThanVia));
        }
        out
    }

    /// A blank board with these rules: the copper stack-up, the fixed
    /// technical layers and the rules in the board setup.
    pub fn new_board(&self) -> PcbBoard {
        let copper = copper_layers(self.copper_layers);
        let technical = [
            TechnicalLayer::SilkscreenTop,
            TechnicalLayer::SilkscreenBottom,
            TechnicalLayer::SolderMaskTop,
            TechnicalLayer::SolderMaskBottom,
            TechnicalLayer::EdgeCuts,
        ]
        .map(PcbLayer::Technical);
        let layers = copper
            .into_iter()
            .map(PcbLayer::Copper)
            .chain(technical)
            .enumerate()
            .map(|(i, layer)| LayerDef {
                id: u8::try_from(i).unwrap_or(u8::MAX),
                name: crate::format::pcb_layer_token(layer),
                layer_type: match layer {
                    PcbLayer::Copper(_) => "copper".into(),
                    _ => String::new(),
                },
                display_name: String::new(),
            })
            .collect();
        PcbBoard {
            sketch: None,
            uuid: uuid::Uuid::now_v7(),
            version: 1,
            generator: "signex".into(),
            thickness: self.thickness_mm,
            outline: Vec::new(),
            layers,
            setup: Some(PcbSetup {
                grid_size: NEW_BOARD_GRID_MM,
                trace_width: self.track_width_mm,
                via_diameter: self.via_diameter_mm,
                via_drill: self.via_drill_mm,
                clearance: self.clearance_mm,
                track_min_width: self.track_width_mm,
                via_min_diameter: self.via_diameter_mm,
                via_min_drill: self.via_drill_mm,
            }),
            nets: Vec::new(),
            footprints: Vec::new(),
            segments: Vec::new(),
            vias: Vec::new(),
            zones: Vec::new(),
            graphics: Vec::new(),
            texts: Vec::new(),
            unknown: Default::default(),
        }
    }
}

/// Snap grid of a new board, in mm.
pub const NEW_BOARD_GRID_MM: f64 = 0.1;

/// Copper layers top to bottom for a count; one layer is the top only.
pub fn copper_layers(count: u8) -> Vec<CopperLayer> {
    let count = count.clamp(1, 32);
    let mut out = vec![CopperLayer::Top];
    if count >= 2 {
        out.extend((1..=count - 2).filter_map(|n| InnerCopper::new(n).map(CopperLayer::Inner)));
        out.push(CopperLayer::Bottom);
    }
    out
}

/// What a new sheet starts from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetDefaults {
    /// The `paper_size` token, such as `A3` or `ANSI A portrait`.
    pub paper_size: String,
    /// Draw the title block on the frame.
    pub show_title_block: bool,
}

/// Sheet sizes the setup offers, as `paper_size` tokens.
pub const SHEET_SIZES: [&str; 12] = [
    "A5", "A4", "A3", "A2", "A1", "A0", "ANSI A", "ANSI B", "ANSI C", "ANSI D", "Letter", "Legal",
];

impl Default for SheetDefaults {
    fn default() -> Self {
        Self {
            paper_size: "A3".into(),
            show_title_block: true,
        }
    }
}

impl SheetDefaults {
    /// Put these on a new sheet: paper size and title block.
    pub fn apply(&self, sheet: &mut SchematicSheet) {
        sheet.paper_size = self.paper_size.clone();
        sheet.frame.show_title_block = self.show_title_block;
    }
}

/// The optional defaults of a project; each part is written only when set.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pcb: Option<PcbDefaults>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet: Option<SheetDefaults>,
    /// The id of the project's default profile: a built-in token or a user
    /// profile id. Only the id is stored, never the profile; absent = none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

impl ProjectDefaults {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The designator standard a new project follows unless the user picks one.
pub const DEFAULT_DESIGNATORS: DesignatorStandard = DesignatorStandard::Ieee315;

impl super::ProjectData {
    /// The blank board a PCB added to this project starts as: from its PCB
    /// defaults, else `None` and the caller keeps its own blank board.
    pub fn new_board(&self) -> Option<PcbBoard> {
        self.project_defaults
            .pcb
            .as_ref()
            .map(PcbDefaults::new_board)
    }

    /// Put the project's sheet defaults on a sheet added to it.
    pub fn apply_sheet_defaults(&self, sheet: &mut SchematicSheet) {
        if let Some(defaults) = &self.project_defaults.sheet {
            defaults.apply(sheet);
        }
    }
}

/// The `.snxprj` key of the project defaults.
pub(super) const PROJECT_DEFAULTS_KEY: &str = "project_defaults";

/// Interpret a `project_defaults` section, or keep it verbatim for re-save
/// when it cannot be used: a shape this build does not read, or PCB values
/// out of range. The project opens either way; the section is not applied
/// and a warning says so (as for `rules` and `annotation`, #1180).
pub(super) fn take_project_defaults(data: &mut super::ProjectData, raw: serde_json::Value) {
    let why = match serde_json::from_value::<ProjectDefaults>(raw.clone()) {
        Ok(defaults) => match defaults.pcb.as_ref().map(PcbDefaults::problems) {
            Some(problems) if !problems.is_empty() => problems
                .iter()
                .map(|(field, why)| format!("{} {why}", field.key()))
                .collect::<Vec<_>>()
                .join(", "),
            _ => {
                data.project_defaults = defaults;
                return;
            }
        },
        Err(err) => err.to_string(),
    };
    tracing::warn!(
        target: "signex::project",
        "The project's setup defaults are not usable ({why}); they are kept unchanged \
         and not applied."
    );
    data.unknown.insert(PROJECT_DEFAULTS_KEY.into(), raw);
}

impl super::ProjectData {
    /// The file has a `project_defaults` section this build could not use:
    /// kept verbatim for re-save, not applied.
    pub fn has_unreadable_project_defaults(&self) -> bool {
        self.project_defaults.is_empty() && self.unknown.get(PROJECT_DEFAULTS_KEY).is_some()
    }
}
