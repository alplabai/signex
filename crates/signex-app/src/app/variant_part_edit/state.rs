//! What the Alternate part picker (mockup row 208) and the Edit value
//! popover (mockup row 209) hold while open: the part they edit, where the
//! edit applies, and what the user has picked or typed. Boxed on `UiState`.

use signex_types::project::{
    FOOTPRINT_PARAMETER, MPN_PARAMETER, PartRef, VALUE_PARAMETER, VariantSettings,
};
use signex_types::schematic::{SchematicSheet, Symbol};

use super::candidates::{Candidate, Candidates};
use crate::app::state::ProjectId;

/// Which In variant row opened the edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantEditKind {
    /// Alternate… (mockup row 208).
    Alternate,
    /// Edit value for this variant… (mockup row 209).
    Value,
}

/// The channels an edit applies to on a repeated sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditScope {
    /// The copy on the channel tab shown.
    ThisChannel,
    /// Every channel's copy (the default, DECISIONS 2026-11-26).
    AllChannels,
}

/// The placed part as Base has it: what the picker's top card, the compare
/// strip and the parameter list show.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartFacts {
    pub mpn: String,
    pub manufacturer: String,
    pub value: String,
    pub footprint: String,
    /// `(number, name)` of every pin of its drawing, sorted, each once.
    pub pins: Vec<(String, String)>,
    /// Its parameters with their Base values: Value first, then its fields
    /// (not Footprint: an alternate part changes that, DECISIONS
    /// 2026-11-26).
    pub parameters: Vec<(String, String)>,
}

impl PartFacts {
    /// `symbol` placed on `sheet`.
    pub fn of(sheet: &SchematicSheet, symbol: &Symbol) -> Self {
        let field = |name: &str| {
            symbol
                .fields
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.trim().to_string())
                .unwrap_or_default()
        };
        let mut pins: Vec<(String, String)> = sheet
            .body_of(symbol)
            .map(|body| {
                body.pins
                    .iter()
                    .map(|pin| (pin.pin.number.clone(), pin.pin.name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        pins.sort();
        pins.dedup();
        let mut fields: Vec<(String, String)> = symbol
            .fields
            .iter()
            .filter(|(key, _)| {
                !key.eq_ignore_ascii_case(VALUE_PARAMETER)
                    && !key.eq_ignore_ascii_case(FOOTPRINT_PARAMETER)
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        fields.sort();
        let mut parameters = vec![(VALUE_PARAMETER.to_string(), symbol.value.clone())];
        parameters.extend(fields);
        Self {
            mpn: field(MPN_PARAMETER),
            manufacturer: field("Manufacturer"),
            value: symbol.value.clone(),
            footprint: symbol.footprint.clone(),
            pins,
            parameters,
        }
    }

    /// The Base value of `parameter`, empty when the part has none.
    pub fn base_value(&self, parameter: &str) -> &str {
        self.parameters
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(parameter))
            .map_or("", |(_, value)| value.as_str())
    }
}

/// The part an edit is for and where it can apply.
#[derive(Debug, Clone, PartialEq)]
pub struct EditedPart {
    pub project: ProjectId,
    /// The active variant, as the project spells it.
    pub variant: String,
    /// The sheet's file name, for the picker's heading.
    pub sheet: String,
    /// The sheet's path: where the picker reads the part's wiring.
    pub path: std::path::PathBuf,
    /// Whether the part's sheets fit it in the variant, before the
    /// variant's own fit (`EditedPart::is_fitted`).
    pub sheet_fitted: bool,
    /// The copy the canvas shows (`CH1_C22` on a channel tab), or the part
    /// on a sheet that is not repeated.
    pub this_part: PartRef,
    /// The channel tab shown; `None` on Editor and on a plain sheet.
    pub channel: Option<String>,
    /// The first and last channel of a repeated sheet.
    pub channels: Option<(String, String)>,
    /// Every channel's copy; empty on a plain sheet.
    pub every_channel: Vec<PartRef>,
    /// The placed part as Base has it.
    pub base: PartFacts,
    /// The library row the part was placed from: `(library, row)`.
    pub row: Option<(uuid::Uuid, uuid::Uuid)>,
    /// Where the menu opened, in window coordinates: the popover's anchor.
    pub anchor: (f32, f32),
    /// The undocked window the menu opened in; `None` for the main one.
    pub window: Option<iced::window::Id>,
}

impl EditedPart {
    /// The parts an edit with `scope` sets: every channel's copy, or the
    /// one shown. Off a channel tab a repeated part is set in every channel.
    pub fn parts(&self, scope: EditScope) -> Vec<PartRef> {
        let every = scope == EditScope::AllChannels || self.channel.is_none();
        if every && !self.every_channel.is_empty() {
            self.every_channel.clone()
        } else {
            vec![self.this_part.clone()]
        }
    }

    /// Whether the scope can be picked: on a channel tab only.
    pub fn has_scope(&self) -> bool {
        self.channel.is_some() && !self.every_channel.is_empty()
    }

    /// Whether the shown part is fitted in the edited variant: the variant's
    /// own fit, else what its sheets say (the BOM's rule).
    pub fn is_fitted(&self, settings: &VariantSettings) -> bool {
        settings
            .part(Some(self.variant.as_str()), self.this_part.placed())
            .and_then(|own| own.fitted)
            .unwrap_or(self.sheet_fitted)
    }

    /// The value `settings` give `parameter` of the shown part in the
    /// edited variant, if any.
    pub fn own_value<'s>(&self, settings: &'s VariantSettings, parameter: &str) -> Option<&'s str> {
        settings
            .part(Some(self.variant.as_str()), self.this_part.placed())?
            .parameters
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(parameter))
            .map(|(_, value)| value.as_str())
    }
}

/// The candidates' read off the update thread.
#[derive(Debug, Clone, PartialEq)]
pub enum CandidateRead {
    Reading,
    Read(Box<Candidates>),
    Failed(String),
}

/// Alternate… (mockup row 208): the picker over the editor.
#[derive(Debug, Clone, PartialEq)]
pub struct AlternatePicker {
    pub part: EditedPart,
    /// The search over the project libraries.
    pub query: String,
    pub same_footprint_only: bool,
    pub scope: EditScope,
    pub read: CandidateRead,
    /// Which read is the current one; an older read that lands is dropped.
    pub generation: u64,
    pub picked: Option<Candidate>,
    /// Why the last Apply was refused.
    pub problem: Option<String>,
    /// The placed part's wired pins, from the picker's first read.
    pub wired: Option<Vec<String>>,
}

impl AlternatePicker {
    pub fn new(part: EditedPart) -> Self {
        let query = part.base.value.clone();
        Self {
            part,
            query,
            same_footprint_only: false,
            scope: EditScope::AllChannels,
            read: CandidateRead::Reading,
            generation: 0,
            picked: None,
            problem: None,
            wired: None,
        }
    }
}

/// Edit value for this variant… (mockup row 209): the popover on the part.
#[derive(Debug, Clone, PartialEq)]
pub struct ValuePopover {
    pub part: EditedPart,
    /// The parameter edited: Value, or one of the part's Base parameters.
    pub parameter: String,
    pub parameters_open: bool,
    /// The edited variant's value being typed.
    pub text: String,
    pub scope: EditScope,
    /// Why the last OK or Reset was refused.
    pub problem: Option<String>,
}

impl ValuePopover {
    /// The popover on Value, its field holding the value the variant gives
    /// the part now, else the Base value.
    pub fn new(part: EditedPart, settings: &VariantSettings) -> Self {
        let parameter = VALUE_PARAMETER.to_string();
        let text = part
            .own_value(settings, &parameter)
            .unwrap_or(part.base.value.as_str())
            .to_string();
        Self {
            part,
            parameter,
            parameters_open: false,
            text,
            scope: EditScope::AllChannels,
            problem: None,
        }
    }

    /// Whether OK may apply: an empty field is not a value (DECISIONS
    /// 2026-11-26; Reset to base goes back to Base).
    pub fn can_apply(&self) -> bool {
        !self.text.trim().is_empty()
    }
}

/// The open variant part edit.
#[derive(Debug, Clone, PartialEq)]
pub enum VariantPartEdit {
    Alternate(AlternatePicker),
    Value(ValuePopover),
}

impl VariantPartEdit {
    pub fn part(&self) -> &EditedPart {
        match self {
            Self::Alternate(picker) => &picker.part,
            Self::Value(popover) => &popover.part,
        }
    }
}
