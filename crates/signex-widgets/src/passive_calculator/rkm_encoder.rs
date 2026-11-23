//! The RKM encoder tab: a value, its tolerance and the optional resistor
//! fields turned into an RKM code, plus an optional production date code.
//! Plain data with a pure `update`; the page is drawn in
//! `rkm_encoder_view.rs`.

use chrono::{Datelike, Local, NaiveDate};

use super::domain::{
    ComponentKind, ESeries, PreferredComponent, PreferredNumber, SiPrefix, Tolerance,
};
use super::production_date_code::{ProductionDateCode, ProductionDateCycle, ProductionMonth};
use super::rkm_code::{RatedPower, RkmCode, TemperatureCoefficient};

#[derive(Debug, Clone)]
pub enum RkmEncoderMessage {
    KindChanged(ComponentKind),
    ValueChanged(PreferredNumber),
    PrefixChanged(SiPrefix),
    ToleranceChanged(Tolerance),
    TemperatureCoefficientChanged(TemperatureCoefficient),
    RatedPowerChanged(RatedPower),
    ProductionDateCycleChanged(ProductionDateCycle),
    ProductionYearChanged(u16),
    ProductionMonthChanged(ProductionMonth),
    ClearResistorOptionalFields,
    /// The "Optional" section header was clicked: fold or unfold it.
    OptionalSectionToggled,
    /// The production date subsection's enable box was ticked or cleared.
    ProductionDateToggled(bool),
}

#[derive(Debug, Clone)]
pub struct RkmEncoder {
    pub kind: ComponentKind,
    pub value: PreferredNumber,
    pub prefix: SiPrefix,
    pub tolerance: Tolerance,
    pub temperature_coefficient: Option<TemperatureCoefficient>,
    pub rated_power: Option<RatedPower>,
    /// Whether the production date code is generated. While off, the picks
    /// below are kept but give no code; turning it on restores them.
    pub is_production_date_enabled: bool,
    pub production_date_cycle: Option<ProductionDateCycle>,
    pub production_year: Option<u16>,
    pub production_month: Option<ProductionMonth>,
    /// Whether the "Optional" section (temperature coefficient, rated
    /// power) is unfolded.
    pub is_optional_expanded: bool,
    value_options: Vec<PreferredNumber>,
    production_year_options: Vec<u16>,
}

impl Default for RkmEncoder {
    fn default() -> Self {
        let current_year =
            u16::try_from(Local::now().year()).expect("the current calendar year fits into a u16");
        let value_options = ESeries::E192.preferred_numbers().collect::<Vec<_>>();
        let value = value_options
            .iter()
            .copied()
            .find(|number| number.significand == 470)
            .expect("E192 contains 4.70");
        Self {
            kind: ComponentKind::Resistor,
            value,
            prefix: SiPrefix::Kilo,
            tolerance: Tolerance::Percent1,
            temperature_coefficient: None,
            rated_power: None,
            is_production_date_enabled: false,
            production_date_cycle: None,
            production_year: None,
            production_month: None,
            is_optional_expanded: true,
            value_options,
            production_year_options: production_year_options(current_year),
        }
    }
}

impl RkmEncoder {
    pub fn update(&mut self, message: RkmEncoderMessage) {
        match message {
            RkmEncoderMessage::KindChanged(kind) => {
                self.kind = kind;
                self.prefix = match kind {
                    ComponentKind::Resistor => SiPrefix::Kilo,
                    ComponentKind::Capacitor => SiPrefix::Micro,
                    ComponentKind::Inductor => SiPrefix::Micro,
                };
                if kind != ComponentKind::Resistor {
                    self.temperature_coefficient = None;
                    self.rated_power = None;
                }
            }
            RkmEncoderMessage::ValueChanged(value) => self.value = value,
            RkmEncoderMessage::PrefixChanged(prefix) => self.prefix = prefix,
            RkmEncoderMessage::ToleranceChanged(tolerance) => self.tolerance = tolerance,
            RkmEncoderMessage::TemperatureCoefficientChanged(temperature_coefficient) => {
                self.temperature_coefficient = Some(temperature_coefficient);
            }
            RkmEncoderMessage::RatedPowerChanged(rated_power) => {
                self.rated_power = Some(rated_power);
            }
            RkmEncoderMessage::ProductionDateCycleChanged(cycle) => {
                self.production_date_cycle = Some(cycle);
            }
            RkmEncoderMessage::ProductionYearChanged(year) => {
                self.production_year = Some(year);
            }
            RkmEncoderMessage::ProductionMonthChanged(month) => {
                self.production_month = Some(month);
            }
            RkmEncoderMessage::ClearResistorOptionalFields => {
                self.temperature_coefficient = None;
                self.rated_power = None;
            }
            RkmEncoderMessage::OptionalSectionToggled => {
                self.is_optional_expanded = !self.is_optional_expanded;
            }
            RkmEncoderMessage::ProductionDateToggled(is_enabled) => {
                self.set_production_date_enabled(is_enabled, Local::now().date_naive());
            }
        }
    }

    /// Switches the production date code on or off. The first time it is
    /// switched on with nothing picked, the picks start at `today` (the
    /// 20-year cycle, this year and month); picks made before are kept.
    pub(super) fn set_production_date_enabled(&mut self, is_enabled: bool, today: NaiveDate) {
        self.is_production_date_enabled = is_enabled;
        let is_unpicked = self.production_date_cycle.is_none()
            && self.production_year.is_none()
            && self.production_month.is_none();
        if !is_enabled || !is_unpicked {
            return;
        }
        self.production_date_cycle = Some(ProductionDateCycle::TwentyYear);
        self.production_year = u16::try_from(today.year()).ok();
        self.production_month = usize::try_from(today.month0())
            .ok()
            .and_then(|index| ProductionMonth::ALL.get(index).copied());
    }

    pub fn code(&self) -> RkmCode {
        let component = self.component();
        let mut code = RkmCode::for_component(self.kind, component, self.tolerance);
        if let Some(temperature_coefficient) = self.temperature_coefficient {
            code = code.with_temperature_coefficient(temperature_coefficient);
        }
        if let Some(rated_power) = self.rated_power {
            code = code.with_rated_power(rated_power);
        }
        code
    }

    /// The production date code, or `None` while the subsection is off or
    /// a pick is missing.
    pub fn production_date_code(&self) -> Option<ProductionDateCode> {
        if !self.is_production_date_enabled {
            return None;
        }
        Some(ProductionDateCode::new(
            self.production_date_cycle?,
            self.production_year?,
            self.production_month?,
        ))
    }

    pub fn production_year_options(&self) -> &[u16] {
        &self.production_year_options
    }

    pub fn value_options(&self) -> &[PreferredNumber] {
        &self.value_options
    }

    pub(super) fn component(&self) -> PreferredComponent {
        PreferredComponent {
            number: self.value,
            decade: self.prefix.exponent(),
        }
    }
}

fn production_year_options(current_year: u16) -> Vec<u16> {
    const FIRST_DOCUMENTED_YEAR: u16 = 1970;
    if current_year < FIRST_DOCUMENTED_YEAR {
        return vec![current_year];
    }
    (FIRST_DOCUMENTED_YEAR..=current_year).rev().collect()
}
