//! `library.toml` schema for `*.snxlib/` directories. Mirrors LIBRARY_PLAN §13
//! and `v0.9-refactor-2-plan.md` §3 (Altium DBLib model).
//!
//! Adds the `[[tables]]` config section per `v0.9-refactor-2-plan.md` §6
//! step 1.5: a class with no override gets its own table (`<class>s.tsv`).
//! Explicit overrides let multiple classes share a table or rename the
//! filename.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub library: LibraryMeta,
    #[serde(default)]
    pub mode: LibraryMode,
    #[serde(default)]
    pub workflow: WorkflowConfig,
    #[serde(default)]
    pub users: UsersConfig,
    /// Optional explicit table overrides — see [`TableConfig`].
    #[serde(default, rename = "tables")]
    pub tables: Vec<TableConfig>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryMeta {
    pub name: String,
    pub library_id: uuid::Uuid,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LibraryMode {
    #[default]
    LocalGit,
    Database {
        url: String,
        auth: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowConfig {
    #[serde(default)]
    pub review_required: bool,
    #[serde(default = "default_reviewers_required")]
    pub reviewers_required: u32,
    #[serde(default = "default_auto_promote")]
    pub auto_lifecycle_promote: String,
}

fn default_reviewers_required() -> u32 {
    1
}
fn default_auto_promote() -> String {
    "Released".to_string()
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            review_required: false,
            reviewers_required: 1,
            auto_lifecycle_promote: "Released".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct UsersConfig {
    #[serde(default = "default_role")]
    pub default_role: String,
    #[serde(default)]
    pub allow_self_signup: bool,
    /// Per-user role table — key is email.
    #[serde(default, flatten)]
    pub by_email: BTreeMap<String, UserEntry>,
}

fn default_role() -> String {
    "Designer".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserEntry {
    pub display_name: String,
    pub roles: Vec<String>,
}

/// Optional `[[tables]]` override block — Altium DBLib parity.
///
/// Per `v0.9-refactor-2-plan.md` §3:
/// - The default is class → `<class>s.tsv` (mechanical pluralisation).
/// - An explicit override lets multiple classes share one table
///   (e.g. resistors + capacitors both rolled into `Discrete_Passives.tsv`),
///   or rename the file for non-English / domain-specific layouts.
///
/// `name` is the filename stem (no extension); the file lives at
/// `tables/<name>.tsv`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableConfig {
    /// Filename stem (no extension). Becomes `tables/<name>.tsv`.
    pub name: String,
    /// Class names that route into this table.
    #[serde(default)]
    pub classes: Vec<String>,
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn write(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Configured `[[tables]]` overrides, in declaration order.
    pub fn tables(&self) -> &[TableConfig] {
        &self.tables
    }

    /// Resolve a class name to its table filename stem.
    ///
    /// 1. If any `[[tables]]` override lists `class` in its `classes` array,
    ///    return that override's `name`.
    /// 2. Otherwise default-pluralise: `"resistor"` → `"resistors"`.
    ///    The plural is mechanical (`s` suffix); irregulars need an explicit
    ///    override.
    pub fn table_for_class(&self, class: &str) -> String {
        for cfg in &self.tables {
            if cfg.classes.iter().any(|c| c == class) {
                return cfg.name.clone();
            }
        }
        format!("{class}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_git_manifest() {
        let text = r#"
[library]
name = "MyComponents"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[workflow]
review_required = false
"#;
        let m = Manifest::parse(text).unwrap();
        assert_eq!(m.library.name, "MyComponents");
        assert!(matches!(m.mode, LibraryMode::LocalGit));
        assert!(!m.workflow.review_required);
        assert!(m.tables().is_empty());
    }

    #[test]
    fn parses_database_manifest() {
        let text = r#"
[library]
name = "OrgLib"
library_id = "0192a8c0-0000-7000-8000-000000000001"

[mode]
kind = "database"
url = "https://lib.alplab.example/api"
auth = "@signex-keychain:alplab-libserver"
"#;
        let m = Manifest::parse(text).unwrap();
        match m.mode {
            LibraryMode::Database { url, auth } => {
                assert_eq!(url, "https://lib.alplab.example/api");
                assert_eq!(auth, "@signex-keychain:alplab-libserver");
            }
            _ => panic!("expected database mode"),
        }
    }

    #[test]
    fn round_trip_preserves_workflow_defaults() {
        let m = Manifest {
            library: LibraryMeta {
                name: "X".into(),
                library_id: uuid::Uuid::now_v7(),
                description: None,
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            tables: Vec::new(),
        };
        let text = m.write().unwrap();
        let back = Manifest::parse(&text).unwrap();
        assert_eq!(m, back);
    }

    /// Step 1.5 from the plan: tables overrides round-trip through TOML and
    /// `table_for_class` honours the explicit `classes` list.
    #[test]
    fn tables_overrides_round_trip_and_resolve() {
        let text = r#"
[library]
name = "MixedLib"
library_id = "0192a8c0-0000-7000-8000-000000000002"

[[tables]]
name = "Discrete_Passives"
classes = ["resistor", "capacitor", "inductor"]

[[tables]]
name = "Connectors"
classes = ["connector"]
"#;
        let m = Manifest::parse(text).unwrap();
        assert_eq!(m.tables().len(), 2);
        assert_eq!(m.table_for_class("resistor"), "Discrete_Passives");
        assert_eq!(m.table_for_class("capacitor"), "Discrete_Passives");
        assert_eq!(m.table_for_class("connector"), "Connectors");
        // Unknown class falls back to the default plural.
        assert_eq!(m.table_for_class("opamp"), "opamps");

        // Round-trip preserves the override list.
        let written = m.write().unwrap();
        let back = Manifest::parse(&written).unwrap();
        assert_eq!(m, back);
    }

    /// `table_for_class` defaults are pure suffix pluralisation — irregulars
    /// must use the explicit override.
    #[test]
    fn table_for_class_defaults_are_mechanical_plural() {
        let m = Manifest {
            library: LibraryMeta {
                name: "X".into(),
                library_id: uuid::Uuid::nil(),
                description: None,
            },
            mode: LibraryMode::default(),
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            tables: Vec::new(),
        };
        assert_eq!(m.table_for_class("resistor"), "resistors");
        // Irregulars are NOT special-cased — that's the documented contract.
        assert_eq!(m.table_for_class("ic"), "ics");
    }
}
