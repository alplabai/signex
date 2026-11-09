//! Parameters tab — template-validated parameter grid, retargeted at
//! `state.row.parameters` (DBLib model).
//!
//! One grid with Scope / Name / Value / Unit / Status columns, built from
//! the shared dialog kit:
//! - **Required:** every `ParameterTemplate.required_params` slot, with a
//!   "Missing value" note in the status column when nothing is bound.
//! - **Optional:** every `ParameterTemplate.optional_params` slot.
//! - **Custom:** parameters on the row that aren't in the template, each
//!   with a Remove button.
//!
//! Below the grid, an add control (name + one button per value kind)
//! creates custom parameters. Without a template the grid holds only
//! custom rows.
//!
//! Numeric / measurement edits go through a per-row `String` buffer on
//! the editor's `params_edit_buf`, following the
//! `reference_erasable_numeric_input` pattern.

use std::collections::BTreeSet;

use iced::widget::{Column, Widget, column, row, scrollable};
use iced::{Element, Length};
use signex_library::{
    ParamKind, ParamMap, ParamSlot, ParamValue, ParameterTemplate, TemplateViolation,
};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;
use signex_widgets::{raw, text, text_input};

use super::super::messages::{EditorMsg, LibraryMessage, ParamKindMsg};
use super::super::state::{ComponentPreviewState, EditorAddress, LibraryState};
use super::super::unit_picker::{unit_of_token, unit_picker};
use super::kit::{self, SP, TS};

const COLUMNS: [(&str, Length); 6] = [
    ("Scope", Length::FillPortion(2)),
    ("Name", Length::FillPortion(3)),
    ("Value", Length::FillPortion(6)),
    ("Unit", Length::FillPortion(2)),
    ("Status", Length::FillPortion(3)),
    ("", Length::FillPortion(2)),
];

/// Where a grid row comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Required,
    Optional,
    Custom,
}

impl Scope {
    fn label(self) -> &'static str {
        match self {
            Scope::Required => "Required",
            Scope::Optional => "Optional",
            Scope::Custom => "Custom",
        }
    }
}

/// One grid row: a template slot or a custom parameter.
struct RowSpec<'a> {
    name: &'a str,
    scope: Scope,
    slot: Option<&'a ParamSlot>,
}

/// Grid rows in display order: required slots, optional slots, then the
/// row's parameters that no slot names.
fn row_specs<'a>(
    template: Option<&'a ParameterTemplate>,
    params: &'a ParamMap,
) -> Vec<RowSpec<'a>> {
    let mut specs = Vec::new();
    let mut known = BTreeSet::new();
    if let Some(t) = template {
        for (slots, scope) in [
            (&t.required_params, Scope::Required),
            (&t.optional_params, Scope::Optional),
        ] {
            for slot in slots {
                known.insert(slot.name.as_str());
                specs.push(RowSpec {
                    name: slot.name.as_str(),
                    scope,
                    slot: Some(slot),
                });
            }
        }
    }
    for name in params.keys().filter(|k| !known.contains(k.as_str())) {
        specs.push(RowSpec {
            name: name.as_str(),
            scope: Scope::Custom,
            slot: None,
        });
    }
    specs
}

/// What the status column says about a row.
#[derive(Debug, Clone, PartialEq, Eq)]
enum StatusNote {
    Missing,
    Problem(String),
}

fn status_note(scope: Scope, present: bool, problem: Option<&String>) -> Option<StatusNote> {
    if scope == Scope::Required && !present {
        return Some(StatusNote::Missing);
    }
    problem.map(|why| StatusNote::Problem(why.clone()))
}

/// Class summary shown beside the section title.
fn class_label(class: &str, has_template: bool) -> String {
    match (class, has_template) {
        ("", _) => "no class".to_string(),
        (class, true) => format!("class: {class}"),
        (class, false) => format!("class: {class}, no template"),
    }
}

/// Empty-grid message.
fn empty_text(has_template: bool) -> &'static str {
    if has_template {
        "No parameters yet. Add a custom parameter below."
    } else {
        "No template for this class. Add custom parameters below."
    }
}

/// A custom parameter needs a name before it can be added.
fn can_add(new_name: &str) -> bool {
    !new_name.trim().is_empty()
}

/// Resolve the parameter template for a preview state by walking
/// `LibraryState.open_libraries` for the matching `library_path`, then
/// asking the registry for the entry under `(library_id, class)`.
fn resolve_template<'a>(
    state: &ComponentPreviewState,
    library_state: &'a LibraryState,
) -> Option<&'a ParameterTemplate> {
    let class = state.row.class.as_str();
    if class.trim().is_empty() {
        return None;
    }
    let library_id = library_id_of(state, library_state)?;
    library_state.template_registry.resolve(library_id, class)
}

pub(super) fn library_id_of(
    state: &ComponentPreviewState,
    library_state: &LibraryState,
) -> Option<uuid::Uuid> {
    library_state
        .open_libraries
        .iter()
        .find(|lib| lib.root == state.library_path)
        .map(|lib| lib.library_id)
}

/// The short reason a slot's current value breaks its template, shown in
/// the slot's status column. A missing required slot has its own note.
fn violation_text(v: &TemplateViolation) -> Option<(&str, String)> {
    let name = match v {
        TemplateViolation::MissingRequired { name }
        | TemplateViolation::WrongKind { name, .. }
        | TemplateViolation::WrongUnit { name, .. }
        | TemplateViolation::ZeroNotAllowed { name }
        | TemplateViolation::NegativeNotAllowed { name } => name.as_str(),
    };
    signex_library::violation_words(v).map(|why| (name, why))
}

/// Per slot name, what is wrong with the row's current value for it.
fn slot_problems(
    state: &ComponentPreviewState,
    library_state: &LibraryState,
) -> std::collections::BTreeMap<String, String> {
    let Some(library_id) = library_id_of(state, library_state) else {
        return Default::default();
    };
    library_state
        .template_registry
        .validate_params(library_id, state.row.class.as_str(), &state.row.parameters)
        .iter()
        .filter_map(violation_text)
        .map(|(name, why)| (name.to_string(), why))
        .collect()
}

pub fn view<'a>(
    state: &'a ComponentPreviewState,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let template = resolve_template(state, library_state);
    let problems = slot_problems(state, library_state);

    let class_note = text(raw(class_label(
        state.row.class.as_str(),
        template.is_some(),
    )))
    .size(TS.body)
    .color(theme_ext::text_secondary(tokens));

    let specs = row_specs(template, &state.row.parameters);
    let mut grid: Column<Element<'a, LibraryMessage>> =
        column![kit::table_header(&COLUMNS, tokens)];
    if specs.is_empty() {
        grid = grid.push(kit::empty_state(
            empty_text(template.is_some()),
            None,
            tokens,
        ));
    }
    for (index, spec) in specs.iter().enumerate() {
        grid = grid.push(param_row(
            state,
            spec,
            index,
            problems.get(spec.name),
            tokens,
            &address,
        ));
    }

    let body = column![
        kit::section_header("Parameters", Some(class_note.boxed()), tokens),
        kit::card(grid.boxed(), tokens),
        add_custom_card(state, tokens, &address),
    ]
    .spacing(SP.l)
    .width(Length::Fill)
    .boxed();

    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

/// Layout one grid row: scope, name, value editor, unit, status, remove.
fn param_row<'a>(
    state: &'a ComponentPreviewState,
    spec: &RowSpec<'a>,
    index: usize,
    problem: Option<&String>,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let name = spec.name;
    let value = state.row.parameters.get(name);

    let (kind, unit) = match (spec.slot, value) {
        (Some(slot), _) => (slot.kind, slot.unit.clone()),
        (None, Some(v)) => custom_kind_and_unit(v),
        (None, None) => (ParamKind::Text, None),
    };

    let scope_color = if spec.scope == Scope::Required {
        text_c
    } else {
        muted
    };
    let scope_cell: Element<'a, LibraryMessage> = text(raw(spec.scope.label()))
        .size(TS.body)
        .color(scope_color)
        .boxed();
    let name_cell: Element<'a, LibraryMessage> = text(signex_i18n::UserText::new(name.to_string()))
        .size(TS.body)
        .color(text_c)
        .boxed();

    let editor = slot_input(state, name, kind, unit.clone(), spec.slot, tokens, address);
    let unit_cell = unit_cell(spec.scope, kind, unit, name, tokens, address);

    let status_cell: Element<'a, LibraryMessage> =
        match status_note(spec.scope, value.is_some(), problem) {
            Some(StatusNote::Missing) => text(raw("Missing value"))
                .size(TS.body)
                .color(theme_ext::warning_color(tokens))
                .boxed(),
            Some(StatusNote::Problem(why)) => text(raw(why))
                .size(TS.body)
                .color(theme_ext::error_color(tokens))
                .boxed(),
            None => text(raw("")).size(TS.body).boxed(),
        };

    let action_cell: Element<'a, LibraryMessage> = if spec.scope == Scope::Custom {
        kit::danger_button(
            "Remove",
            Some(editor_event(
                address,
                EditorMsg::ParamRemove {
                    name: name.to_string(),
                },
            )),
            tokens,
        )
    } else {
        text(raw("")).size(TS.body).boxed()
    };

    let [scope_w, name_w, value_w, unit_w, status_w, action_w] = COLUMNS.map(|(_, w)| w);
    let row = kit::table_row(
        vec![
            (scope_cell, scope_w),
            (name_cell, name_w),
            (editor, value_w),
            (unit_cell, unit_w),
            (status_cell, status_w),
            (action_cell, action_w),
        ],
        index,
        None,
        tokens,
    );
    // A row with a problem sits on an `err_soft` wash.
    if problem.is_none() {
        return row;
    }
    let wash = crate::styles::tokens::resolve(tokens).err_soft;
    iced::widget::container(row)
        .width(Length::Fill)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(wash)),
            ..iced::widget::container::Style::default()
        })
        .boxed()
}

fn custom_kind_and_unit(v: &ParamValue) -> (ParamKind, Option<String>) {
    match v {
        ParamValue::Text(_) => (ParamKind::Text, None),
        ParamValue::Number(_) => (ParamKind::Number, None),
        ParamValue::Bool(_) => (ParamKind::Bool, None),
        ParamValue::Measurement { unit, .. } => (ParamKind::Measurement, Some(unit.clone())),
    }
}

/// Unit column. A template slot fixes its unit, shown as text. No slot
/// fixes the unit of a custom row, so it is picked, never typed; a unit
/// string the build does not know stays a read-only label.
fn unit_cell<'a>(
    scope: Scope,
    kind: ParamKind,
    unit: Option<String>,
    name: &str,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let (Some(u), ParamKind::Measurement) = (unit, kind) else {
        return text(raw("")).size(TS.body).boxed();
    };
    if scope != Scope::Custom {
        return text(raw(u)).size(TS.body).color(muted).boxed();
    }
    match unit_of_token(&u) {
        Ok(selected) => {
            let address = address.clone();
            let name = name.to_string();
            let allow_none = selected.is_none();
            unit_picker(selected, allow_none, move |unit| {
                editor_event(
                    &address,
                    EditorMsg::ParamUnitPicked {
                        name: name.clone(),
                        unit,
                    },
                )
            })
        }
        Err(raw) => text(signex_widgets::raw(raw.to_string()))
            .size(TS.body)
            .color(muted)
            .boxed(),
    }
}

/// Build the kind-appropriate editor widget for one parameter cell.
/// `slot` is the class-template slot behind it, if any.
fn slot_input<'a>(
    state: &'a ComponentPreviewState,
    name: &str,
    kind: ParamKind,
    unit: Option<String>,
    slot: Option<&ParamSlot>,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    match kind {
        ParamKind::Text => text_cell(state, name, address),
        ParamKind::Number => number_cell(state, name, address),
        ParamKind::Bool => bool_cell(state, name, address),
        ParamKind::Measurement => {
            super::measurement_cell::view(state, name, unit, slot, tokens, address)
        }
    }
}

/// A parameter event addressed to `address`.
pub(super) fn editor_event(address: &EditorAddress, msg: EditorMsg) -> LibraryMessage {
    LibraryMessage::EditorEvent {
        library_path: address.library_path.clone(),
        table: address.table.clone(),
        row_id: address.row_id,
        msg,
    }
}

fn text_cell<'a>(
    state: &'a ComponentPreviewState,
    name: &str,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let value = match state.row.parameters.get(name) {
        Some(ParamValue::Text(s)) => s.clone(),
        Some(ParamValue::Number(n)) => n.to_string(),
        Some(ParamValue::Bool(b)) => b.to_string(),
        Some(ParamValue::Measurement { value, .. }) => value.to_string(),
        None => String::new(),
    };
    let address = address.clone();
    let name = name.to_string();
    text_input(raw(""), value)
        .on_input(move |s| {
            editor_event(
                &address,
                EditorMsg::ParamSetText {
                    name: name.clone(),
                    value: s,
                },
            )
        })
        .padding([SP.s, SP.m])
        .size(TS.body)
        .boxed()
}

fn number_cell<'a>(
    state: &'a ComponentPreviewState,
    name: &str,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    // The per-row String buffer wins over `f64::to_string()` on every
    // keystroke: the buffer is what the user is typing.
    let buf = state.params_edit_buf.get(name).cloned().unwrap_or_else(|| {
        match state.row.parameters.get(name) {
            Some(ParamValue::Number(n)) => n.to_string(),
            Some(other) => display_param(other),
            None => String::new(),
        }
    });
    let (input_address, input_name) = (address.clone(), name.to_string());
    let (submit_address, submit_name) = (address.clone(), name.to_string());
    text_input(raw(""), buf)
        .on_input(move |s| {
            editor_event(
                &input_address,
                EditorMsg::ParamSetNumberBuf {
                    name: input_name.clone(),
                    buf: s,
                },
            )
        })
        .on_submit(editor_event(
            &submit_address,
            EditorMsg::ParamCommitNumber { name: submit_name },
        ))
        .padding([SP.s, SP.m])
        .size(TS.body)
        .boxed()
}

fn bool_cell<'a>(
    state: &'a ComponentPreviewState,
    name: &str,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let checked = matches!(state.row.parameters.get(name), Some(ParamValue::Bool(true)));
    let address = address.clone();
    let name = name.to_string();
    signex_widgets::checkbox(checked)
        .on_toggle(move |v| {
            editor_event(
                &address,
                EditorMsg::ParamSetBool {
                    name: name.clone(),
                    value: v,
                },
            )
        })
        .size(TS.heading)
        .spacing(SP.s)
        .boxed()
}

/// Cheap text view of a `ParamValue` for fallback rendering when the
/// per-row buffer is empty. Replaces the previous `ParamValue::display`
/// method whose name collided with type inference.
fn display_param(v: &ParamValue) -> String {
    ParamValue::display(v)
}

/// Add control: a name input and one button per value kind. The buttons
/// add a parameter named by `ComponentPreviewState::new_param_name` and
/// stay disabled while the name is blank.
fn add_custom_card<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let name_input = {
        let address = address.clone();
        text_input(raw("Parameter name"), &state.new_param_name)
            .on_input(move |s| editor_event(&address, EditorMsg::ParamNewNameChanged(s)))
            .padding([SP.s, SP.m])
            .size(TS.body)
            .width(Length::FillPortion(3))
            .boxed()
    };
    let enabled = can_add(&state.new_param_name);
    let kind_button = |label: &'static str, kind: ParamKindMsg| -> Element<'a, LibraryMessage> {
        kit::secondary_button(
            label,
            enabled.then(|| editor_event(address, EditorMsg::ParamAddCustom { kind })),
            tokens,
        )
    };

    let controls = row![
        name_input,
        kind_button("Text", ParamKindMsg::Text),
        kind_button("Number", ParamKindMsg::Number),
        kind_button("Bool", ParamKindMsg::Bool),
        kind_button("Measurement", ParamKindMsg::Measurement(String::new())),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center)
    .boxed();

    column![
        kit::section_header("Add custom parameter", None, tokens),
        kit::card(controls, tokens),
    ]
    .spacing(SP.m)
    .boxed()
}

/// Build the validation list for a draft — wraps
/// `TemplateRegistry::validate_params` so view-level tests can assert
/// against a stable shape regardless of the registry's internal layout.
#[cfg(test)]
fn missing_required_for_test(
    registry: &signex_library::TemplateRegistry,
    library_id: uuid::Uuid,
    class: &str,
    params: &signex_library::ParamMap,
) -> Vec<String> {
    use signex_library::TemplateViolation;
    registry
        .validate_params(library_id, class, params)
        .into_iter()
        .filter_map(|v| match v {
            TemplateViolation::MissingRequired { name } => Some(name),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_library::{ParamMap, ParamValue, TemplateRegistry};
    use uuid::Uuid;

    /// Empty state path: no template registered for the component class
    /// AND no parameters bound. The validation list must be empty (a
    /// missing template is "trivially passes" per the registry contract)
    /// and `resolve` returns `None`.
    #[test]
    fn empty_state_no_template_no_params() {
        let registry = TemplateRegistry::new();
        let params = ParamMap::new();
        // No template → validation passes trivially.
        assert!(
            registry
                .validate_params(Uuid::nil(), "unicorn", &params)
                .is_empty()
        );
        assert!(registry.resolve(Uuid::nil(), "unicorn").is_none());
    }

    /// Fully-populated edit case for a class that has both required and
    /// optional slots. With every required slot filled the validator
    /// must report no missing-required violations; optional slots may
    /// stay absent without raising any error.
    #[test]
    fn template_with_required_and_optional_fully_populated() {
        let registry = TemplateRegistry::new_with_builtins();
        let mut params = ParamMap::new();
        // resistor template requires value/tolerance/power.
        params.insert(
            "value".into(),
            ParamValue::Measurement {
                value: 10_000.0,
                unit: "ohm".into(),
            },
        );
        params.insert(
            "tolerance".into(),
            ParamValue::Measurement {
                value: 1.0,
                unit: "%".into(),
            },
        );
        params.insert(
            "power".into(),
            ParamValue::Measurement {
                value: 0.125,
                unit: "W".into(),
            },
        );
        let missing = missing_required_for_test(&registry, Uuid::nil(), "resistor", &params);
        assert!(missing.is_empty(), "missing = {missing:?}");
    }

    /// Validation flag visible in the computed match list when a
    /// required parameter is absent. This is what drives the "✗ missing"
    /// amber tag in the view.
    #[test]
    fn template_with_missing_required_flag_visible() {
        let registry = TemplateRegistry::new_with_builtins();
        let mut params = ParamMap::new();
        // Only fill one of the three required resistor slots.
        params.insert(
            "value".into(),
            ParamValue::Measurement {
                value: 10_000.0,
                unit: "ohm".into(),
            },
        );
        let missing = missing_required_for_test(&registry, Uuid::nil(), "resistor", &params);
        // tolerance + power should both still be flagged as missing.
        assert!(missing.contains(&"tolerance".to_string()));
        assert!(missing.contains(&"power".to_string()));
        assert!(!missing.contains(&"value".to_string()));
    }

    /// A zero capacitor, a negative resistor and a wrong-unit value each
    /// get a visible reason in the slot's status column.
    #[test]
    fn template_violations_become_slot_problems() {
        let registry = TemplateRegistry::new_with_builtins();
        let capacitor = registry
            .resolve(Uuid::nil(), "capacitor")
            .expect("capacitor template");
        let value_slot = capacitor
            .required_params
            .iter()
            .find(|s| s.kind == ParamKind::Measurement && s.unit.as_deref() == Some("F"))
            .expect("a farad slot")
            .name
            .clone();
        let mut params = ParamMap::new();
        params.insert(
            value_slot.clone(),
            ParamValue::Measurement {
                value: 0.0,
                unit: "F".into(),
            },
        );
        let zero = problem_of(&registry, "capacitor", &params, &value_slot);
        assert_eq!(zero.as_deref(), Some("zero not allowed"));

        params.insert(
            value_slot.clone(),
            ParamValue::Measurement {
                value: -1e-6,
                unit: "F".into(),
            },
        );
        let negative = problem_of(&registry, "capacitor", &params, &value_slot);
        assert_eq!(negative.as_deref(), Some("negative not allowed"));

        params.insert(
            value_slot.clone(),
            ParamValue::Measurement {
                value: 4.7,
                unit: "V".into(),
            },
        );
        let wrong = problem_of(&registry, "capacitor", &params, &value_slot);
        assert_eq!(wrong.as_deref(), Some("unit V, expected F"));

        params.insert(
            value_slot.clone(),
            ParamValue::Measurement {
                value: 4.7e-6,
                unit: "F".into(),
            },
        );
        assert_eq!(
            problem_of(&registry, "capacitor", &params, &value_slot),
            None
        );
    }

    #[test]
    fn grid_lists_required_then_optional_then_custom() {
        let registry = TemplateRegistry::new_with_builtins();
        let template = registry.resolve(Uuid::nil(), "resistor").expect("template");
        let mut params = ParamMap::new();
        params.insert("zz_extra".into(), ParamValue::Text("x".into()));
        let specs = row_specs(Some(template), &params);
        let expected = template.required_params.len() + template.optional_params.len() + 1;
        assert_eq!(specs.len(), expected);
        let scopes: Vec<Scope> = specs.iter().map(|s| s.scope).collect();
        let first_optional = scopes.iter().position(|s| *s == Scope::Optional);
        let last_required = scopes.iter().rposition(|s| *s == Scope::Required);
        if let (Some(o), Some(r)) = (first_optional, last_required) {
            assert!(r < o);
        }
        assert_eq!(scopes.last(), Some(&Scope::Custom));
        assert_eq!(specs.last().map(|s| s.name), Some("zz_extra"));
    }

    #[test]
    fn grid_without_template_holds_only_custom_rows() {
        let mut params = ParamMap::new();
        params.insert("a".into(), ParamValue::Bool(true));
        let specs = row_specs(None, &params);
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].scope, Scope::Custom);
        assert!(row_specs(None, &ParamMap::new()).is_empty());
    }

    #[test]
    fn template_slot_value_is_not_repeated_as_custom() {
        let registry = TemplateRegistry::new_with_builtins();
        let template = registry.resolve(Uuid::nil(), "resistor").expect("template");
        let slot = template.required_params[0].name.clone();
        let mut params = ParamMap::new();
        params.insert(slot, ParamValue::Text("1".into()));
        let specs = row_specs(Some(template), &params);
        assert!(specs.iter().all(|s| s.scope != Scope::Custom));
    }

    #[test]
    fn status_note_flags_missing_required_only() {
        let why = "zero not allowed".to_string();
        assert_eq!(
            status_note(Scope::Required, false, None),
            Some(StatusNote::Missing)
        );
        assert_eq!(status_note(Scope::Optional, false, None), None);
        assert_eq!(status_note(Scope::Custom, true, None), None);
        assert_eq!(
            status_note(Scope::Required, true, Some(&why)),
            Some(StatusNote::Problem(why))
        );
    }

    #[test]
    fn header_and_empty_texts() {
        assert_eq!(class_label("", false), "no class");
        assert_eq!(class_label("", true), "no class");
        assert_eq!(class_label("resistor", true), "class: resistor");
        assert_eq!(class_label("widget", false), "class: widget, no template");
        assert!(empty_text(false).contains("No template"));
        assert!(empty_text(true).starts_with("No parameters"));
    }

    #[test]
    fn add_needs_a_name() {
        assert!(!can_add(""));
        assert!(!can_add("   "));
        assert!(can_add("voltage"));
    }

    fn problem_of(
        registry: &signex_library::TemplateRegistry,
        class: &str,
        params: &ParamMap,
        slot: &str,
    ) -> Option<String> {
        registry
            .validate_params(Uuid::nil(), class, params)
            .iter()
            .filter_map(violation_text)
            .find(|(name, _)| *name == slot)
            .map(|(_, why)| why)
    }
}
