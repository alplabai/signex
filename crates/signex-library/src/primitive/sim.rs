//! `SimModel` primitive — SPICE / Verilog-A model body, reusable across MPNs.
//!
//! Per `v0.9-refactor-2-plan.md` §2.3, a `SimModel` carries the model
//! body and a default symbol-pin → SPICE-node mapping. A binding `Component`
//! references it via `Revision::sim_ref` and may override the node map for
//! a specific MPN.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const SIM_FILE_FORMAT_TOKEN: &str = "snxsim/v1";

/// Sentinel string substituted for each model's `body` field before
/// TOML serialise; replaced post-emit with the literal multi-line
/// `'''…'''` block so SPICE source is git-diffable.
const BODY_PLACEHOLDER_PREFIX: &str = "__SIGNEX_SIM_BODY_a1b2c3d4_";

/// SPICE / behavioural model dialect.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SimKind {
    Spice3,
    Ngspice,
    LtSpice,
    VerilogA,
}

/// Reusable simulation model. Bound by `Component::sim_ref`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimModel {
    pub uuid: Uuid,
    pub name: String,
    pub kind: SimKind,
    /// SPICE / Verilog-A model body.
    pub body: String,
    /// Default symbol-pin number → SPICE-node mapping. Component revisions can
    /// override on a per-MPN basis.
    #[serde(default)]
    pub default_node_map: BTreeMap<String, String>,
    /// Semver-style revision string. Stage 14 of
    /// `v0.9-snxlib-as-file-plan.md`: sim models version independently
    /// of the bound symbols and component rows. Defaults to `"0.0.1"`.
    #[serde(default = "default_sim_version")]
    pub version: String,
    /// Released-flag: locks edit-in-place under Team mode.
    #[serde(default)]
    pub released: bool,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

fn default_sim_version() -> String {
    "0.0.1".to_string()
}

impl SimModel {
    pub fn empty(name: impl Into<String>, kind: SimKind) -> Self {
        let now = Utc::now();
        Self {
            uuid: Uuid::now_v7(),
            name: name.into(),
            kind,
            body: String::new(),
            default_node_map: BTreeMap::new(),
            version: default_sim_version(),
            released: false,
            created: now,
            updated: now,
        }
    }
}

/// `.snxsim` container — Altium parity for SimModel storage. The
/// envelope mirrors [`crate::primitive::SymbolFile`] /
/// [`crate::primitive::FootprintFile`] (file-level uuid + display
/// name + array-of-tables payload). Today the convention is one
/// SimModel per file; the Vec leaves room for multi-model SPICE
/// libraries without a wire-format break.
///
/// Wire format (v0.18.5): TOML manifest header + one `[[models]]`
/// entry per `SimModel`. Each entry's `body` field is emitted as a
/// `body = '''…'''` literal multi-line string so SPICE / Verilog-A
/// source is line-diffable in git output. Everything else (kind
/// enum, default_node_map, scalars) stays as inline TOML.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimFile {
    /// Schema sentinel — current emitters write `"snxsim/v1"`.
    #[serde(default = "default_sim_format")]
    pub format: String,
    /// File-level UUID — distinct from any contained model's uuid.
    pub file_uuid: Uuid,
    /// Display name shown in the library tree.
    #[serde(default)]
    pub display_name: String,
    /// All sim models living in this file. Today's writers always
    /// emit a single-element Vec; reader accepts arbitrary length.
    #[serde(default)]
    pub models: Vec<SimModel>,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

fn default_sim_format() -> String {
    SIM_FILE_FORMAT_TOKEN.to_string()
}

#[derive(Serialize, Deserialize)]
struct SimFileWire {
    format: String,
    file_uuid: Uuid,
    #[serde(default)]
    display_name: String,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
    #[serde(default)]
    models: Vec<SimModelWire>,
}

#[derive(Serialize, Deserialize)]
struct SimModelWire {
    uuid: Uuid,
    name: String,
    kind: SimKind,
    /// Sentinel placeholder; replaced post-emit with the literal
    /// multi-line `body = '''…'''` block.
    body: String,
    #[serde(default)]
    default_node_map: BTreeMap<String, String>,
    #[serde(default = "default_sim_version")]
    version: String,
    #[serde(default)]
    released: bool,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
}

impl SimFile {
    /// Wrap a single `SimModel` into a one-element file envelope.
    pub fn from_model(model: SimModel) -> Self {
        let now = Utc::now();
        Self {
            format: default_sim_format(),
            file_uuid: Uuid::now_v7(),
            display_name: model.name.clone(),
            created: model.created,
            updated: now,
            models: vec![model],
        }
    }

    /// Decode bytes as UTF-8 and parse via [`SimFile::from_toml_str`].
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SimFileError> {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Err(SimFileError::Empty);
        }
        let text = std::str::from_utf8(bytes)?;
        Self::from_toml_str(text)
    }

    /// Parse the TOML wire format. Format-token mismatch surfaces
    /// [`SimFileError::UnsupportedFormat`].
    pub fn from_toml_str(text: &str) -> Result<Self, SimFileError> {
        let wire: SimFileWire = toml::from_str(text)?;
        if wire.format != SIM_FILE_FORMAT_TOKEN {
            return Err(SimFileError::UnsupportedFormat { got: wire.format });
        }
        let models = wire
            .models
            .into_iter()
            .map(|m| SimModel {
                uuid: m.uuid,
                name: m.name,
                kind: m.kind,
                body: m.body,
                default_node_map: m.default_node_map,
                version: m.version,
                released: m.released,
                created: m.created,
                updated: m.updated,
            })
            .collect();
        Ok(SimFile {
            format: wire.format,
            file_uuid: wire.file_uuid,
            display_name: wire.display_name,
            created: wire.created,
            updated: wire.updated,
            models,
        })
    }

    /// Serialise to canonical TOML. The per-model `body` field is
    /// emitted as a `body = '''…'''` literal multi-line string via a
    /// sentinel-replace pass post-`to_string_pretty`.
    pub fn to_toml_string(&self) -> Result<String, SimFileError> {
        for (idx, model) in self.models.iter().enumerate() {
            if model.body.contains("'''") {
                return Err(SimFileError::InvalidBody { model_index: idx });
            }
            // v0.18.12.1 — also reject the sentinel prefix in body
            // content. Without this, a SPICE source containing the
            // literal string `__SIGNEX_SIM_BODY_a1b2c3d4_` would
            // confuse the post-emit `str::replace` pass and corrupt
            // the output. Practically improbable but cheap to guard.
            if model.body.contains(BODY_PLACEHOLDER_PREFIX) {
                return Err(SimFileError::InvalidBody { model_index: idx });
            }
        }
        let mut wire_models: Vec<SimModelWire> = Vec::with_capacity(self.models.len());
        for (idx, model) in self.models.iter().enumerate() {
            wire_models.push(SimModelWire {
                uuid: model.uuid,
                name: model.name.clone(),
                kind: model.kind,
                body: format!("{BODY_PLACEHOLDER_PREFIX}{idx}__"),
                default_node_map: model.default_node_map.clone(),
                version: model.version.clone(),
                released: model.released,
                created: model.created,
                updated: model.updated,
            });
        }
        let wire = SimFileWire {
            format: self.format.clone(),
            file_uuid: self.file_uuid,
            display_name: self.display_name.clone(),
            created: self.created,
            updated: self.updated,
            models: wire_models,
        };
        let mut out = toml::to_string_pretty(&wire).map_err(SimFileError::TomlSerialize)?;
        for (idx, model) in self.models.iter().enumerate() {
            let needle = format!("\"{BODY_PLACEHOLDER_PREFIX}{idx}__\"");
            // Empty body emits `body = ''''''` (six ticks); non-empty
            // bodies get a leading newline so the opener sits on its
            // own line. Both parse back via toml's literal-string rule.
            let replacement = if model.body.is_empty() {
                "''''''".to_string()
            } else {
                format!("'''\n{}'''", model.body)
            };
            out = out.replace(&needle, &replacement);
        }
        Ok(out)
    }

    /// Locate a model by UUID within this file.
    pub fn get_model(&self, uuid: Uuid) -> Option<&SimModel> {
        self.models.iter().find(|m| m.uuid == uuid)
    }
}

/// Error variants raised by [`SimFile`] parsers + serialisers.
#[derive(Debug, thiserror::Error)]
pub enum SimFileError {
    #[error("empty .snxsim file")]
    Empty,
    #[error("invalid UTF-8 in TOML payload: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("TOML deserialise failed: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("TOML serialise failed: {0}")]
    TomlSerialize(toml::ser::Error),
    #[error("unsupported .snxsim format token {got:?}; this build supports \"snxsim/v1\"")]
    UnsupportedFormat { got: String },
    #[error(
        "model[{model_index}].body contains the literal triple-quote sequence '''; \
         the literal multi-line string envelope cannot escape it"
    )]
    InvalidBody { model_index: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sim_model_round_trip() {
        let mut node_map = BTreeMap::new();
        node_map.insert("1".into(), "in".into());
        node_map.insert("2".into(), "out".into());

        let s = SimModel {
            uuid: Uuid::now_v7(),
            name: "LM358".into(),
            kind: SimKind::Spice3,
            body: ".SUBCKT LM358 IN OUT VCC GND\n.ENDS".into(),
            default_node_map: node_map,
            version: "0.0.1".into(),
            released: false,
            created: Utc::now(),
            updated: Utc::now(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SimModel = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn sim_kind_round_trip_all_variants() {
        for k in [
            SimKind::Spice3,
            SimKind::Ngspice,
            SimKind::LtSpice,
            SimKind::VerilogA,
        ] {
            let json = serde_json::to_string(&k).unwrap();
            let back: SimKind = serde_json::from_str(&json).unwrap();
            assert_eq!(k, back);
        }
    }

    #[test]
    fn empty_sim_model_carries_no_body() {
        let s = SimModel::empty("test", SimKind::Spice3);
        assert!(s.body.is_empty());
        assert!(s.default_node_map.is_empty());
    }

    // ---- v0.18.5 — SimFile TOML envelope round-trips ----

    #[test]
    fn sim_file_from_model_wraps_into_one_element_vec() {
        let model = SimModel::empty("LM358", SimKind::Spice3);
        let file = SimFile::from_model(model.clone());
        assert_eq!(file.format, "snxsim/v1");
        assert_eq!(file.display_name, "LM358");
        assert_eq!(file.models.len(), 1);
        assert_eq!(file.models[0], model);
    }

    #[test]
    fn sim_file_toml_round_trip_empty_body() {
        let model = SimModel::empty("LM358", SimKind::Spice3);
        let file = SimFile::from_model(model);
        let text = file.to_toml_string().expect("serialise");
        let back = SimFile::from_toml_str(&text).expect("parse");
        assert_eq!(back.models.len(), 1);
        assert_eq!(back.models[0].name, "LM358");
        assert!(back.models[0].body.is_empty());
        assert_eq!(back.format, "snxsim/v1");
        assert_eq!(back.file_uuid, file.file_uuid);
    }

    #[test]
    fn sim_file_toml_round_trip_with_full_body() {
        let mut node_map = BTreeMap::new();
        node_map.insert("1".into(), "in".into());
        node_map.insert("2".into(), "out".into());
        node_map.insert("3".into(), "vcc".into());
        node_map.insert("4".into(), "gnd".into());
        let model = SimModel {
            uuid: Uuid::now_v7(),
            name: "LM358".into(),
            kind: SimKind::Ngspice,
            body: ".SUBCKT LM358 IN+ IN- OUT VCC GND\n* dual op-amp\nR1 IN+ N1 1k\n.ENDS LM358"
                .into(),
            default_node_map: node_map,
            version: "1.2.3".into(),
            released: true,
            created: Utc::now(),
            updated: Utc::now(),
        };
        let file = SimFile::from_model(model.clone());
        let text = file.to_toml_string().expect("serialise");
        let back = SimFile::from_toml_str(&text).expect("parse");
        assert_eq!(back.models.len(), 1);
        assert_eq!(back.models[0], model);
    }

    #[test]
    fn sim_file_to_toml_emits_body_as_literal_multiline() {
        let mut model = SimModel::empty("LM358", SimKind::Spice3);
        model.body = ".SUBCKT FOO\n.ENDS".into();
        let file = SimFile::from_model(model);
        let text = file.to_toml_string().expect("serialise");
        assert!(
            text.contains("body = '''\n.SUBCKT FOO"),
            "expected literal multi-line opener; got:\n{text}"
        );
        assert!(
            !text.contains(BODY_PLACEHOLDER_PREFIX),
            "placeholder should be fully replaced; got:\n{text}"
        );
    }

    #[test]
    fn sim_file_from_bytes_decodes_toml_envelope() {
        let model = SimModel::empty("foo", SimKind::LtSpice);
        let file = SimFile::from_model(model);
        let bytes = file.to_toml_string().unwrap().into_bytes();
        let back = SimFile::from_bytes(&bytes).expect("parse");
        assert_eq!(back.models.len(), 1);
    }

    #[test]
    fn sim_file_from_bytes_rejects_empty_payload() {
        match SimFile::from_bytes(b"   \n  \t\n") {
            Err(SimFileError::Empty) => {}
            other => panic!("expected Empty, got {other:?}"),
        }
    }

    #[test]
    fn sim_file_unsupported_format_token_is_rejected() {
        let bad = r#"
            format = "snxsim/v99"
            file_uuid = "00000000-0000-0000-0000-000000000000"
            created = "2026-05-04T00:00:00Z"
            updated = "2026-05-04T00:00:00Z"
        "#;
        match SimFile::from_toml_str(bad) {
            Err(SimFileError::UnsupportedFormat { got }) => {
                assert_eq!(got, "snxsim/v99");
            }
            other => panic!("expected UnsupportedFormat, got {other:?}"),
        }
    }

    #[test]
    fn sim_file_to_toml_rejects_triple_quote_in_body() {
        // SPICE source with a literal ''' would smuggle out of the
        // multi-line literal envelope.
        let mut model = SimModel::empty("evil", SimKind::Spice3);
        model.body = "* '''\n.SUBCKT".into();
        let file = SimFile::from_model(model);
        match file.to_toml_string() {
            Err(SimFileError::InvalidBody { model_index }) => {
                assert_eq!(model_index, 0);
            }
            other => panic!("expected InvalidBody, got {other:?}"),
        }
    }
}
