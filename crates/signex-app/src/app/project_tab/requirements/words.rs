//! The catalog words of the Requirements section: kinds, fields and the
//! reason an edit was refused.

use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::project::{MAX_REQUIREMENT_ID_CHARS, RequirementField, RequirementProblem};

/// The name of a kind as the section shows it.
pub(super) fn kind_words(kind: &str) -> MessageId {
    match kind {
        "erc_clean" => ids::REQUIREMENTS_KIND_ERC_CLEAN,
        "drc_clean" => ids::REQUIREMENTS_KIND_DRC_CLEAN,
        "board_size" => ids::REQUIREMENTS_KIND_BOARD_SIZE,
        "parts_state" => ids::REQUIREMENTS_KIND_PARTS_STATE,
        "net_class" => ids::REQUIREMENTS_KIND_NET_CLASS,
        _ => ids::REQUIREMENTS_KIND_BOM_COST,
    }
}

/// The caption of a field.
pub(super) fn field_words(field: RequirementField) -> MessageId {
    match field {
        RequirementField::Id => ids::REQUIREMENTS_FIELD_ID,
        RequirementField::Severity => ids::REQUIREMENTS_FIELD_SEVERITY,
        RequirementField::AllowWarnings => ids::REQUIREMENTS_FIELD_ALLOW_WARNINGS,
        RequirementField::MaxWidthMm => ids::REQUIREMENTS_FIELD_MAX_WIDTH,
        RequirementField::MaxHeightMm => ids::REQUIREMENTS_FIELD_MAX_HEIGHT,
        RequirementField::States => ids::REQUIREMENTS_FIELD_STATES,
        RequirementField::Variant => ids::REQUIREMENTS_FIELD_VARIANT,
        RequirementField::FittedOnly => ids::REQUIREMENTS_FIELD_FITTED_ONLY,
        RequirementField::Nets => ids::REQUIREMENTS_FIELD_NETS,
        RequirementField::Class => ids::REQUIREMENTS_FIELD_CLASS,
        RequirementField::MinTraceWidthMm => ids::REQUIREMENTS_FIELD_MIN_TRACE,
        RequirementField::MinClearanceMm => ids::REQUIREMENTS_FIELD_MIN_CLEARANCE,
        RequirementField::Quantity => ids::REQUIREMENTS_FIELD_QUANTITY,
        RequirementField::Currency => ids::REQUIREMENTS_FIELD_CURRENCY,
        RequirementField::MaxUnitCost => ids::REQUIREMENTS_FIELD_MAX_UNIT_COST,
    }
}

/// The words of a refused edit.
pub(super) fn problem_words(problem: &RequirementProblem, i18n: &I18n) -> Localized {
    use RequirementProblem as P;
    match problem {
        P::ListUnreadable => t!(i18n, ids::REQUIREMENTS_PROBLEM_LIST_UNREADABLE),
        P::NoEntry(_) => t!(i18n, ids::REQUIREMENTS_PROBLEM_NO_ENTRY),
        P::UnknownId(id) => t!(i18n, ids::REQUIREMENTS_PROBLEM_UNKNOWN_ID, "id" => id.as_str()),
        P::AmbiguousId(id) => {
            t!(i18n, ids::REQUIREMENTS_PROBLEM_AMBIGUOUS_ID, "id" => id.as_str())
        }
        P::EntryUnreadable(_) => t!(i18n, ids::REQUIREMENTS_PROBLEM_ENTRY_UNREADABLE),
        P::UnknownKind(kind) => {
            t!(i18n, ids::REQUIREMENTS_PROBLEM_UNKNOWN_KIND, "kind" => kind.as_str())
        }
        P::NotAField { field, .. } => {
            t!(i18n, ids::REQUIREMENTS_PROBLEM_NOT_A_FIELD, "field" => field.as_str())
        }
        P::IdEmpty => t!(i18n, ids::REQUIREMENTS_PROBLEM_ID_EMPTY),
        P::IdTooLong => {
            t!(i18n, ids::REQUIREMENTS_PROBLEM_ID_TOO_LONG, "max" => MAX_REQUIREMENT_ID_CHARS)
        }
        P::IdInvalid => t!(i18n, ids::REQUIREMENTS_PROBLEM_ID_INVALID),
        P::IdTaken(id) => t!(i18n, ids::REQUIREMENTS_PROBLEM_ID_TAKEN, "id" => id.as_str()),
        P::UnknownSeverity(text) => {
            t!(i18n, ids::REQUIREMENTS_PROBLEM_UNKNOWN_SEVERITY, "text" => text.as_str())
        }
        P::NotYesNo(text) => {
            t!(i18n, ids::REQUIREMENTS_PROBLEM_NOT_YES_NO, "text" => text.as_str())
        }
        P::NotPositive(text) => {
            t!(i18n, ids::REQUIREMENTS_PROBLEM_NOT_POSITIVE, "text" => text.as_str())
        }
        P::NotACount(text) => {
            t!(i18n, ids::REQUIREMENTS_PROBLEM_NOT_A_COUNT, "text" => text.as_str())
        }
        P::Empty(_) => t!(i18n, ids::REQUIREMENTS_PROBLEM_EMPTY),
        P::UnknownState(state) => {
            t!(i18n, ids::REQUIREMENTS_PROBLEM_UNKNOWN_STATE, "state" => state.as_str())
        }
        P::UnknownVariant(name) => {
            t!(i18n, ids::REQUIREMENTS_PROBLEM_UNKNOWN_VARIANT, "name" => name.as_str())
        }
        P::NetClassAsksNothing => t!(i18n, ids::REQUIREMENTS_PROBLEM_NET_CLASS_ASKS_NOTHING),
    }
}
