//! `manifest.toml` schema for `*.snxlib/` directories. Mirrors LIBRARY_PLAN §13.

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

impl Manifest {
    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }

    pub fn write(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
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
        };
        let text = m.write().unwrap();
        let back = Manifest::parse(&text).unwrap();
        assert_eq!(m, back);
    }
}
