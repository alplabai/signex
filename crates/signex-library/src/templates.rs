//! Parameter templates — class-typed schemas that constrain a component's
//! `parameters` map.
//!
//! Per `v0.9-refactor-2-plan.md` §4, every `Component` has a class
//! (e.g. "resistor", "opamp"); the matching template lists which parameters
//! are required vs optional and what kind of value (text, number, measurement)
//! each carries.
//!
//! Resolution order (§4.3):
//! 1. per-library override (inside `*.snxlib/templates/<class>.toml`),
//! 2. global override (`<config_dir>/signex/templates/<class>.toml`) — *not
//!    loaded by this crate; `TemplateRegistry::load_global_dir` is called by
//!    the app shell during start-up*,
//! 3. the bundled built-in.
//!
//! `TemplateRegistry::new_with_builtins` ships with five starter classes:
//! resistor, capacitor, inductor, opamp, generic. Anything else falls through
//! to "no template" (the `validate` step then trivially passes).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::param::{ParamMap, ParamValue};

/// Kind of value a parameter slot accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    Text,
    Number,
    Bool,
    Measurement,
}

/// One parameter slot in a [`ParameterTemplate`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamSlot {
    pub name: String,
    pub kind: ParamKind,
    /// Required for `kind = measurement`, optional otherwise.
    #[serde(default)]
    pub unit: Option<String>,
}

/// Class-typed parameter schema.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ParameterTemplate {
    pub class: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub required_params: Vec<ParamSlot>,
    #[serde(default)]
    pub optional_params: Vec<ParamSlot>,
}

impl ParameterTemplate {
    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}

/// Why a parameter map fails to validate against its template.
#[derive(Clone, Debug, PartialEq)]
pub enum TemplateViolation {
    /// A required parameter is missing.
    MissingRequired { name: String },
    /// A parameter is present but of the wrong kind for its slot.
    WrongKind {
        name: String,
        expected: ParamKind,
        found: ParamKind,
    },
    /// A measurement parameter has the wrong unit.
    WrongUnit {
        name: String,
        expected: String,
        found: String,
    },
}

impl std::fmt::Display for TemplateViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateViolation::MissingRequired { name } => {
                write!(f, "missing required parameter: {name}")
            }
            TemplateViolation::WrongKind {
                name,
                expected,
                found,
            } => write!(
                f,
                "parameter {name} has wrong kind: expected {expected:?}, found {found:?}"
            ),
            TemplateViolation::WrongUnit {
                name,
                expected,
                found,
            } => write!(
                f,
                "parameter {name} has wrong unit: expected {expected}, found {found}"
            ),
        }
    }
}

/// Per-library + global template registry.
///
/// `global` keeps one entry per class — populated by `new_with_builtins` and
/// optionally augmented by `load_global_dir`.
/// `per_lib` overrides per `(library_id, class)` pair — populated by
/// `LibrarySet` when a library exposes its own `templates/<class>.toml`.
#[derive(Clone, Debug, Default)]
pub struct TemplateRegistry {
    global: HashMap<String, ParameterTemplate>,
    per_lib: HashMap<(Uuid, String), ParameterTemplate>,
}

/// Bundled built-in templates. Slurped at compile time via `include_str!` so
/// we never depend on filesystem state for default behaviour.
const BUILTIN_RESISTOR: &str = include_str!("../templates-builtin/resistor.toml");
const BUILTIN_CAPACITOR: &str = include_str!("../templates-builtin/capacitor.toml");
const BUILTIN_INDUCTOR: &str = include_str!("../templates-builtin/inductor.toml");
const BUILTIN_OPAMP: &str = include_str!("../templates-builtin/opamp.toml");
const BUILTIN_GENERIC: &str = include_str!("../templates-builtin/generic.toml");

impl TemplateRegistry {
    /// Empty registry — no templates resolved. Mostly useful for tests.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registry seeded with the five bundled built-ins.
    pub fn new_with_builtins() -> Self {
        let mut r = Self::default();
        for text in [
            BUILTIN_RESISTOR,
            BUILTIN_CAPACITOR,
            BUILTIN_INDUCTOR,
            BUILTIN_OPAMP,
            BUILTIN_GENERIC,
        ] {
            // The bundled TOML is part of the crate source — a parse failure
            // here is a build-time bug, not runtime input.
            let t: ParameterTemplate = toml::from_str(text).expect("bundled template parses");
            r.global.insert(t.class.clone(), t);
        }
        r
    }

    /// Add (or replace) a global override for `class`.
    pub fn insert_global(&mut self, t: ParameterTemplate) {
        self.global.insert(t.class.clone(), t);
    }

    /// Add (or replace) a per-library override.
    pub fn insert_for_library(&mut self, library_id: Uuid, t: ParameterTemplate) {
        self.per_lib.insert((library_id, t.class.clone()), t);
    }

    /// Lookup order (per plan §4.3):
    /// 1. `per_lib[(library_id, class)]`,
    /// 2. `global[class]`,
    /// 3. `None` (no template; validation trivially passes).
    pub fn resolve(&self, library_id: Uuid, class: &str) -> Option<&ParameterTemplate> {
        if let Some(t) = self.per_lib.get(&(library_id, class.to_string())) {
            return Some(t);
        }
        self.global.get(class)
    }

    /// Validate a parameter map against its class template (looked up via
    /// `library_id` + `class`). Empty result = pass.
    pub fn validate_params(
        &self,
        library_id: Uuid,
        class: &str,
        params: &ParamMap,
    ) -> Vec<TemplateViolation> {
        let Some(t) = self.resolve(library_id, class) else {
            // No template registered → can't violate anything.
            return Vec::new();
        };
        let mut out = Vec::new();
        for slot in &t.required_params {
            match params.get(&slot.name) {
                None => out.push(TemplateViolation::MissingRequired {
                    name: slot.name.clone(),
                }),
                Some(v) => check_slot(slot, v, &mut out),
            }
        }
        for slot in &t.optional_params {
            if let Some(v) = params.get(&slot.name) {
                check_slot(slot, v, &mut out);
            }
        }
        out
    }
}

fn kind_of(v: &ParamValue) -> ParamKind {
    match v {
        ParamValue::Text(_) => ParamKind::Text,
        ParamValue::Number(_) => ParamKind::Number,
        ParamValue::Bool(_) => ParamKind::Bool,
        ParamValue::Measurement { .. } => ParamKind::Measurement,
    }
}

fn check_slot(slot: &ParamSlot, v: &ParamValue, out: &mut Vec<TemplateViolation>) {
    let found = kind_of(v);
    if found != slot.kind {
        out.push(TemplateViolation::WrongKind {
            name: slot.name.clone(),
            expected: slot.kind,
            found,
        });
        return;
    }
    if let (ParamValue::Measurement { unit, .. }, Some(expected_unit)) = (v, &slot.unit)
        && unit != expected_unit
    {
        out.push(TemplateViolation::WrongUnit {
            name: slot.name.clone(),
            expected: expected_unit.clone(),
            found: unit.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_resolves_resistor() {
        let r = TemplateRegistry::new_with_builtins();
        let t = r
            .resolve(Uuid::nil(), "resistor")
            .expect("resistor template");
        assert_eq!(t.class, "resistor");
        assert!(t.required_params.iter().any(|s| s.name == "value"));
        assert!(t.required_params.iter().any(|s| s.name == "tolerance"));
    }

    #[test]
    fn builtin_registry_resolves_capacitor_inductor_opamp_generic() {
        let r = TemplateRegistry::new_with_builtins();
        for class in ["capacitor", "inductor", "opamp", "generic"] {
            let t = r.resolve(Uuid::nil(), class).expect(class);
            assert_eq!(t.class, class);
        }
    }

    #[test]
    fn unknown_class_resolves_to_none() {
        let r = TemplateRegistry::new_with_builtins();
        assert!(r.resolve(Uuid::nil(), "unicorn").is_none());
    }

    #[test]
    fn per_lib_override_takes_precedence() {
        let mut r = TemplateRegistry::new_with_builtins();
        let lib = Uuid::now_v7();
        let custom = ParameterTemplate {
            class: "resistor".into(),
            description: "lib-specific".into(),
            required_params: vec![ParamSlot {
                name: "custom_value".into(),
                kind: ParamKind::Text,
                unit: None,
            }],
            optional_params: Vec::new(),
        };
        r.insert_for_library(lib, custom);
        let resolved = r.resolve(lib, "resistor").unwrap();
        assert_eq!(resolved.description, "lib-specific");
        // Different library still sees the bundled built-in.
        let other = r.resolve(Uuid::now_v7(), "resistor").unwrap();
        assert_ne!(other.description, "lib-specific");
    }

    #[test]
    fn validate_passes_when_no_template_registered() {
        let r = TemplateRegistry::new();
        let params = ParamMap::new();
        assert!(
            r.validate_params(Uuid::nil(), "resistor", &params)
                .is_empty()
        );
    }

    #[test]
    fn validate_flags_missing_required() {
        let r = TemplateRegistry::new_with_builtins();
        let params = ParamMap::new();
        let v = r.validate_params(Uuid::nil(), "resistor", &params);
        // resistor template requires value/tolerance/power.
        let names: Vec<_> = v
            .iter()
            .filter_map(|x| match x {
                TemplateViolation::MissingRequired { name } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        assert!(names.contains(&"value"));
        assert!(names.contains(&"tolerance"));
        assert!(names.contains(&"power"));
    }

    #[test]
    fn validate_flags_wrong_kind() {
        let r = TemplateRegistry::new_with_builtins();
        let mut params = ParamMap::new();
        params.insert("value".into(), ParamValue::Text("not a measurement".into()));
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
        let v = r.validate_params(Uuid::nil(), "resistor", &params);
        assert!(v.iter().any(|x| matches!(
            x,
            TemplateViolation::WrongKind { name, .. } if name == "value"
        )));
    }

    #[test]
    fn validate_flags_wrong_unit() {
        let r = TemplateRegistry::new_with_builtins();
        let mut params = ParamMap::new();
        params.insert(
            "value".into(),
            ParamValue::Measurement {
                value: 10.0,
                unit: "F".into(), // wrong — resistor expects ohm.
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
        let v = r.validate_params(Uuid::nil(), "resistor", &params);
        assert!(v.iter().any(|x| matches!(
            x,
            TemplateViolation::WrongUnit { name, .. } if name == "value"
        )));
    }

    #[test]
    fn template_round_trip_through_toml() {
        let t = ParameterTemplate {
            class: "test".into(),
            description: "desc".into(),
            required_params: vec![ParamSlot {
                name: "a".into(),
                kind: ParamKind::Measurement,
                unit: Some("V".into()),
            }],
            optional_params: vec![ParamSlot {
                name: "b".into(),
                kind: ParamKind::Text,
                unit: None,
            }],
        };
        let text = toml::to_string_pretty(&t).unwrap();
        let back = ParameterTemplate::parse(&text).unwrap();
        assert_eq!(t, back);
    }
}
