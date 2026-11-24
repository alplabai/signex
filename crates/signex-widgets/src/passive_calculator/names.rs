//! Catalog names of the calculator's choices: the component kinds, the
//! production-date cycles and months, and the target errors. The enums keep
//! their English `Display` for codes and tests; the views read these.

use signex_i18n::{I18n, Localized, MessageId, ids, t};

use super::control::TargetError;
use super::domain::ComponentKind;
use super::network::BoundaryCondition;
use super::production_date_code::{ProductionDateCycle, ProductionMonth};

pub(super) fn component_name(i18n: &I18n, kind: ComponentKind) -> Localized {
    t!(
        i18n,
        match kind {
            ComponentKind::Resistor => ids::PASSIVE_CALCULATOR_RESISTOR,
            ComponentKind::Capacitor => ids::PASSIVE_CALCULATOR_CAPACITOR,
            ComponentKind::Inductor => ids::PASSIVE_CALCULATOR_INDUCTOR,
        }
    )
}

pub(super) fn cycle_name(i18n: &I18n, cycle: ProductionDateCycle) -> Localized {
    t!(
        i18n,
        match cycle {
            ProductionDateCycle::TwentyYear => ids::PASSIVE_CALCULATOR_CYCLE_TWENTY,
            ProductionDateCycle::TenYear => ids::PASSIVE_CALCULATOR_CYCLE_TEN,
            ProductionDateCycle::FourYear => ids::PASSIVE_CALCULATOR_CYCLE_FOUR,
        }
    )
}

pub(super) fn month_name(i18n: &I18n, month: ProductionMonth) -> Localized {
    t!(i18n, month_id(month))
}

const fn month_id(month: ProductionMonth) -> MessageId {
    match month {
        ProductionMonth::January => ids::PASSIVE_CALCULATOR_MONTH_JANUARY,
        ProductionMonth::February => ids::PASSIVE_CALCULATOR_MONTH_FEBRUARY,
        ProductionMonth::March => ids::PASSIVE_CALCULATOR_MONTH_MARCH,
        ProductionMonth::April => ids::PASSIVE_CALCULATOR_MONTH_APRIL,
        ProductionMonth::May => ids::PASSIVE_CALCULATOR_MONTH_MAY,
        ProductionMonth::June => ids::PASSIVE_CALCULATOR_MONTH_JUNE,
        ProductionMonth::July => ids::PASSIVE_CALCULATOR_MONTH_JULY,
        ProductionMonth::August => ids::PASSIVE_CALCULATOR_MONTH_AUGUST,
        ProductionMonth::September => ids::PASSIVE_CALCULATOR_MONTH_SEPTEMBER,
        ProductionMonth::October => ids::PASSIVE_CALCULATOR_MONTH_OCTOBER,
        ProductionMonth::November => ids::PASSIVE_CALCULATOR_MONTH_NOVEMBER,
        ProductionMonth::December => ids::PASSIVE_CALCULATOR_MONTH_DECEMBER,
    }
}

pub(super) fn target_error(i18n: &I18n, error: TargetError) -> Localized {
    t!(
        i18n,
        match error {
            TargetError::NotANumber => ids::PASSIVE_CALCULATOR_ERROR_NOT_A_NUMBER,
            TargetError::Negative => ids::PASSIVE_CALCULATOR_ERROR_NEGATIVE,
            TargetError::NoResult => ids::PASSIVE_CALCULATOR_ERROR_NO_RESULT,
        }
    )
}

pub(super) fn boundary_name(i18n: &I18n, condition: BoundaryCondition) -> Localized {
    t!(
        i18n,
        match condition {
            BoundaryCondition::WireBridge => ids::PASSIVE_CALCULATOR_WIRE_BRIDGE,
            BoundaryCondition::OpenCircuit => ids::PASSIVE_CALCULATOR_OPEN_CIRCUIT,
        }
    )
}
