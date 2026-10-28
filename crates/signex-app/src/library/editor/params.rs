//! Parameters tab — template-validated parameter form, retargeted at
//! `state.row.parameters` (DBLib model).
//!
//! Renders three groups against the active class template:
//! - **Required:** every `ParameterTemplate.required_params` slot, with
//!   a "✗ missing" amber tag when no value is currently bound.
//! - **Optional:** every `ParameterTemplate.optional_params` slot.
//! - **Custom:** parameters on the row that aren't in the template.
//!   Custom rows carry an inline `[×]` remove button.
//!
//! When no template resolves the view falls back to a "no template —
//! populate with custom parameters" surface that only renders the
//! custom rows + the add-new control.
//!
//! Numeric / measurement edits go through a per-row `String` buffer on
//! the editor's `params_edit_buf`, following the
//! `reference_erasable_numeric_input` pattern.

use std::collections::BTreeSet;

use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Border, Element, Length, Theme};
use signex_library::{ParamKind, ParamSlot, ParamValue, ParameterTemplate, TemplateViolation};
use signex_types::quantity::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage, ParamKindMsg};
use super::super::prefix_picker::prefix_picker;
use super::super::state::{ComponentPreviewState, EditorAddress, LibraryState};
use super::super::unit_picker::{unit_of_token, unit_picker};

/// Width of the status column: the missing badge or a slot problem.
const STATUS_W: f32 = 170.0;

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

fn library_id_of(
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
/// the slot's status column. A missing required slot has its own badge.
fn violation_text(v: &TemplateViolation) -> Option<(&str, String)> {
    match v {
        TemplateViolation::MissingRequired { .. } => None,
        TemplateViolation::WrongKind { name, expected, .. } => {
            Some((name, format!("expects {expected:?}")))
        }
        TemplateViolation::WrongUnit {
            name,
            expected,
            found,
        } => Some((name, format!("unit {found}, expected {expected}"))),
        TemplateViolation::ZeroNotAllowed { name } => Some((name, "zero not allowed".into())),
        TemplateViolation::NegativeNotAllowed { name } => {
            Some((name, "negative not allowed".into()))
        }
    }
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
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let template = resolve_template(state, library_state);
    let problems = slot_problems(state, library_state);

    let header_label = match template {
        Some(t) => format!("Parameters (class: {})", t.class),
        None => match state.row.class.as_str() {
            "" => "Parameters (no class)".to_string(),
            class => format!("Parameters (class: {class}, no template)"),
        },
    };

    let mut body = column![
        text(header_label).size(13).color(text_c),
        Space::new().height(10),
    ]
    .spacing(0)
    .width(Length::Fill);

    let template_keys: BTreeSet<String> = template
        .map(|t| {
            t.required_params
                .iter()
                .chain(t.optional_params.iter())
                .map(|s| s.name.clone())
                .collect()
        })
        .unwrap_or_default();

    if let Some(t) = template {
        // ── Required ────────────────────────────────────────────────
        if !t.required_params.is_empty() {
            body = body.push(text("Required").size(11).color(muted));
            body = body.push(Space::new().height(6));
            for slot in &t.required_params {
                body = body.push(template_row(
                    state,
                    slot,
                    true,
                    problems.get(&slot.name),
                    tokens,
                    &address,
                ));
                body = body.push(Space::new().height(4));
            }
            body = body.push(Space::new().height(8));
        }
        // ── Optional ────────────────────────────────────────────────
        if !t.optional_params.is_empty() {
            body = body.push(text("Optional").size(11).color(muted));
            body = body.push(Space::new().height(6));
            for slot in &t.optional_params {
                body = body.push(template_row(
                    state,
                    slot,
                    false,
                    problems.get(&slot.name),
                    tokens,
                    &address,
                ));
                body = body.push(Space::new().height(4));
            }
            body = body.push(Space::new().height(8));
        }
    } else {
        body = body.push(
            text(format!(
                "No template for class `{}` — populate with custom parameters.",
                state.row.class.as_str()
            ))
            .size(11)
            .color(muted),
        );
        body = body.push(Space::new().height(10));
    }

    // ── Custom parameters ────────────────────────────────────────────
    let custom_keys: Vec<&String> = state
        .row
        .parameters
        .keys()
        .filter(|k| !template_keys.contains(k.as_str()))
        .collect();

    body = body.push(text("Custom").size(11).color(muted));
    body = body.push(Space::new().height(6));

    if custom_keys.is_empty() {
        body = body.push(text("No custom parameters yet.").size(11).color(muted));
        body = body.push(Space::new().height(6));
    } else {
        for key in custom_keys {
            let val = &state.row.parameters[key];
            body = body.push(custom_row(state, key, val, tokens, &address));
            body = body.push(Space::new().height(4));
        }
        body = body.push(Space::new().height(6));
    }

    // [+ Add custom parameter] row
    body = body.push(add_custom_row(state, tokens, &address));

    container(scrollable(body).width(Length::Fill).height(Length::Fill))
        .style(crate::styles::modal_card(tokens))
        .padding(14)
        .into()
}

/// Layout one slot row from the template — value editor + unit suffix +
/// optional "✗ missing" badge for required-but-empty required slots.
fn template_row<'a>(
    state: &'a ComponentPreviewState,
    slot: &'a ParamSlot,
    required: bool,
    problem: Option<&String>,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);

    let label_color = if required { text_c } else { muted };
    let present = state.row.parameters.contains_key(&slot.name);

    let label = container(text(slot.name.clone()).size(11).color(label_color))
        .width(Length::FillPortion(3));

    let editor_widget: Element<'a, LibraryMessage> =
        slot_input(state, &slot.name, slot.kind, slot.unit.clone(), address);

    let unit_label: Element<'a, LibraryMessage> = match (&slot.unit, slot.kind) {
        (Some(u), ParamKind::Measurement) => container(text(u.clone()).size(11).color(muted))
            .padding([0, 8])
            .width(Length::Fixed(60.0))
            .into(),
        _ => Space::new().width(Length::Fixed(60.0)).into(),
    };

    let status: Element<'a, LibraryMessage> = if required && !present {
        let amber = iced::Color::from_rgb(0.95, 0.70, 0.18);
        container(text("\u{2717} missing").size(11).color(amber))
            .width(Length::Fixed(STATUS_W))
            .into()
    } else if let Some(why) = problem {
        container(
            text(format!("\u{2717} {why}"))
                .size(11)
                .color(theme_ext::error_color(tokens)),
        )
        .width(Length::Fixed(STATUS_W))
        .into()
    } else {
        Space::new().width(Length::Fixed(STATUS_W)).into()
    };

    row![
        label,
        container(editor_widget).width(Length::FillPortion(7)),
        unit_label,
        status,
    ]
    .align_y(iced::Alignment::Center)
    .spacing(8)
    .width(Length::Fill)
    .into()
}

/// Layout a custom parameter row — same value editor as `template_row`
/// but with a per-row `[×]` remove button instead of the missing badge.
fn custom_row<'a>(
    state: &'a ComponentPreviewState,
    name: &'a str,
    val: &'a ParamValue,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);

    let kind = match val {
        ParamValue::Text(_) => ParamKind::Text,
        ParamValue::Number(_) => ParamKind::Number,
        ParamValue::Bool(_) => ParamKind::Bool,
        ParamValue::Measurement { .. } => ParamKind::Measurement,
    };
    let unit = match val {
        ParamValue::Measurement { unit, .. } => Some(unit.clone()),
        _ => None,
    };

    let label =
        container(text(name.to_string()).size(11).color(text_c)).width(Length::FillPortion(3));

    let editor_widget = slot_input(state, name, kind, unit.clone(), address);

    // No slot fixes the unit of a custom row, so it is picked, never
    // typed. A unit string the build does not know stays a read-only label.
    let unit_label: Element<'a, LibraryMessage> = match (unit, kind) {
        (Some(u), ParamKind::Measurement) => match unit_of_token(&u) {
            Ok(selected) => {
                let lib_path = address.library_path.clone();
                let table = address.table.clone();
                let row_id = address.row_id;
                let name_owned = name.to_string();
                let allow_none = selected.is_none();
                container(unit_picker(selected, allow_none, 11, move |unit| {
                    LibraryMessage::EditorEvent {
                        library_path: lib_path.clone(),
                        table: table.clone(),
                        row_id,
                        msg: EditorMsg::ParamUnitPicked {
                            name: name_owned.clone(),
                            unit,
                        },
                    }
                }))
                .width(Length::Fixed(90.0))
                .into()
            }
            Err(raw) => container(text(raw.to_string()).size(11).color(muted))
                .padding([0, 8])
                .width(Length::Fixed(90.0))
                .into(),
        },
        _ => Space::new().width(Length::Fixed(90.0)).into(),
    };

    let lib_path = address.library_path.clone();
    let table = address.table.clone();
    let row_id = address.row_id;
    let name_owned = name.to_string();
    let remove_btn = button(container(text("\u{00D7}").size(13).color(muted)).padding([0, 6]))
        .on_press(LibraryMessage::EditorEvent {
            library_path: lib_path,
            table,
            row_id,
            msg: EditorMsg::ParamRemove { name: name_owned },
        })
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => {
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                }
                _ => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.03),
            };
            iced::widget::button::Style {
                background: Some(iced::Background::Color(bg)),
                text_color: muted,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: theme_ext::border_color(tokens),
                },
                ..iced::widget::button::Style::default()
            }
        });

    let status_block: Element<'a, LibraryMessage> =
        container(remove_btn).width(Length::Fixed(110.0)).into();

    row![
        label,
        container(editor_widget).width(Length::FillPortion(7)),
        unit_label,
        status_block,
    ]
    .align_y(iced::Alignment::Center)
    .spacing(8)
    .width(Length::Fill)
    .into()
}

/// Build the kind-appropriate editor widget for one parameter cell.
fn slot_input<'a>(
    state: &'a ComponentPreviewState,
    name: &str,
    kind: ParamKind,
    unit: Option<String>,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let lib_path = address.library_path.clone();
    let table = address.table.clone();
    let row_id = address.row_id;
    let name_owned = name.to_string();

    match kind {
        ParamKind::Text => {
            let value = match state.row.parameters.get(name) {
                Some(ParamValue::Text(s)) => s.clone(),
                Some(ParamValue::Number(n)) => n.to_string(),
                Some(ParamValue::Bool(b)) => b.to_string(),
                Some(ParamValue::Measurement { value, .. }) => value.to_string(),
                None => String::new(),
            };
            let name_for_input = name_owned;
            text_input("", &value)
                .on_input(move |s| LibraryMessage::EditorEvent {
                    library_path: lib_path.clone(),
                    table: table.clone(),
                    row_id,
                    msg: EditorMsg::ParamSetText {
                        name: name_for_input.clone(),
                        value: s,
                    },
                })
                .padding([4, 8])
                .size(11)
                .into()
        }
        ParamKind::Number => {
            // Per-row String buffer wins over reading f64.to_string()
            // on every keystroke — the buffer is what the user is
            // typing.
            let buf = state.params_edit_buf.get(name).cloned().unwrap_or_else(|| {
                match state.row.parameters.get(name) {
                    Some(ParamValue::Number(n)) => n.to_string(),
                    Some(other) => display_param(other),
                    None => String::new(),
                }
            });
            let lib_path_input = lib_path.clone();
            let lib_path_submit = lib_path;
            let table_input = table.clone();
            let table_submit = table;
            let name_for_input = name_owned.clone();
            let name_for_submit = name_owned;
            text_input("", &buf)
                .on_input(move |s| LibraryMessage::EditorEvent {
                    library_path: lib_path_input.clone(),
                    table: table_input.clone(),
                    row_id,
                    msg: EditorMsg::ParamSetNumberBuf {
                        name: name_for_input.clone(),
                        buf: s,
                    },
                })
                .on_submit(LibraryMessage::EditorEvent {
                    library_path: lib_path_submit,
                    table: table_submit,
                    row_id,
                    msg: EditorMsg::ParamCommitNumber {
                        name: name_for_submit,
                    },
                })
                .padding([4, 8])
                .size(11)
                .into()
        }
        ParamKind::Bool => {
            let checked = matches!(state.row.parameters.get(name), Some(ParamValue::Bool(true)));
            let name_for_toggle = name_owned;
            iced::widget::checkbox(checked)
                .on_toggle(move |v| LibraryMessage::EditorEvent {
                    library_path: lib_path.clone(),
                    table: table.clone(),
                    row_id,
                    msg: EditorMsg::ParamSetBool {
                        name: name_for_toggle.clone(),
                        value: v,
                    },
                })
                .size(14)
                .spacing(4)
                .into()
        }
        ParamKind::Measurement => {
            let row_unit = match (unit, state.row.parameters.get(name)) {
                (Some(u), _) => u,
                (None, Some(ParamValue::Measurement { unit, .. })) => unit.clone(),
                _ => String::new(),
            };
            // A known unit: a number-only field plus the SI-prefix picker.
            // An unknown unit string keeps the raw text editor.
            let known = Unit::from_alias(row_unit.trim());
            let buf = state.params_edit_buf.get(name).cloned().unwrap_or_else(|| {
                let stored = state.row.parameters.get(name);
                match (stored, &known) {
                    (Some(v), Some(_)) if v.measurement_parts().is_some() => v
                        .measurement_parts()
                        .map(|p| p.mantissa)
                        .unwrap_or_default(),
                    (Some(ParamValue::Measurement { value, .. }), _) => value.to_string(),
                    (Some(other), _) => display_param(other),
                    (None, _) => String::new(),
                }
            });
            let lib_path_input = lib_path.clone();
            let lib_path_submit = lib_path.clone();
            let table_input = table.clone();
            let table_submit = table.clone();
            let name_for_input = name_owned.clone();
            let name_for_submit = name_owned.clone();
            let unit_for_submit = row_unit.clone();
            let field = text_input("", &buf)
                .on_input(move |s| LibraryMessage::EditorEvent {
                    library_path: lib_path_input.clone(),
                    table: table_input.clone(),
                    row_id,
                    msg: EditorMsg::ParamSetMeasurementBuf {
                        name: name_for_input.clone(),
                        buf: s,
                    },
                })
                .on_submit(LibraryMessage::EditorEvent {
                    library_path: lib_path_submit,
                    table: table_submit,
                    row_id,
                    msg: EditorMsg::ParamCommitMeasurement {
                        name: name_for_submit,
                        unit: unit_for_submit,
                    },
                })
                .padding([4, 8])
                .size(11);
            let picker = known.and_then(|u| {
                let unit_token = row_unit.clone();
                let name_for_pick = name_owned.clone();
                prefix_picker(&u, state.measurement_prefix(name), 11, move |prefix| {
                    LibraryMessage::EditorEvent {
                        library_path: lib_path.clone(),
                        table: table.clone(),
                        row_id,
                        msg: EditorMsg::ParamPrefixPicked {
                            name: name_for_pick.clone(),
                            unit: unit_token.clone(),
                            prefix,
                        },
                    }
                })
            });
            match picker {
                Some(pick) => row![field, pick]
                    .spacing(4)
                    .align_y(iced::Alignment::Center)
                    .into(),
                None => field.into(),
            }
        }
    }
}

/// Cheap text view of a `ParamValue` for fallback rendering when the
/// per-row buffer is empty. Replaces the previous `ParamValue::display`
/// method whose name collided with type inference.
fn display_param(v: &ParamValue) -> String {
    ParamValue::display(v)
}

/// Inline "+ Add custom parameter" row — text input + four kind buttons
/// (Text / Number / Bool / Measurement). The kind buttons live as
/// separate `+` buttons because adding a custom row is rare and a
/// pick-list would be heavier than the four-pill UI.
///
/// The kind pills add a parameter named by the input's text
/// (`ComponentPreviewState::new_param_name`); a blank name adds nothing.
fn add_custom_row<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);

    let lib_path = address.library_path.clone();
    let table = address.table.clone();
    let row_id = address.row_id;

    // Static placeholder row — the Add operation requires the user to
    // type the name into the input below and pick a kind. A single
    // "Add" pill seeds with `Text` for now (custom-row kind switching
    // happens via toggling in a later polish pass).
    let pill = |label: &'static str, kind: ParamKindMsg| -> Element<'a, LibraryMessage> {
        let lib_path = lib_path.clone();
        let table = table.clone();
        let kind_for_msg = kind;
        button(container(text(label).size(11).color(text_c)).padding([3, 10]))
            .on_press(LibraryMessage::EditorEvent {
                library_path: lib_path,
                table,
                row_id,
                msg: EditorMsg::ParamAddCustom {
                    kind: kind_for_msg.clone(),
                },
            })
            .style(move |_: &Theme, status: iced::widget::button::Status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => {
                        iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                    }
                    _ => iced::Color::from_rgba(1.0, 1.0, 1.0, 0.03),
                };
                iced::widget::button::Style {
                    background: Some(iced::Background::Color(bg)),
                    text_color: text_c,
                    border: Border {
                        width: 1.0,
                        radius: 3.0.into(),
                        color: border,
                    },
                    ..iced::widget::button::Style::default()
                }
            })
            .into()
    };

    let name_input = {
        let lib_path = address.library_path.clone();
        let table = address.table.clone();
        text_input("Parameter name", &state.new_param_name)
            .on_input(move |s| LibraryMessage::EditorEvent {
                library_path: lib_path.clone(),
                table: table.clone(),
                row_id,
                msg: EditorMsg::ParamNewNameChanged(s),
            })
            .padding([3, 6])
            .size(11)
            .width(Length::Fixed(160.0))
    };

    row![
        text("[+ Add custom parameter]").size(11).color(muted),
        Space::new().width(8),
        name_input,
        Space::new().width(8),
        pill("Text", ParamKindMsg::Text),
        Space::new().width(4),
        pill("Number", ParamKindMsg::Number),
        Space::new().width(4),
        pill("Bool", ParamKindMsg::Bool),
        Space::new().width(4),
        pill("Measurement", ParamKindMsg::Measurement(String::new())),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(0)
    .into()
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
