//! Manifest schema for `.snxlib` libraries. Mirrors v0.9-library-plan.md §13
//! and `v0.9-refactor-2-plan.md` §3 (Altium DBLib model).
//!
//! Adds the `[[tables]]` config section per `v0.9-refactor-2-plan.md` §6
//! step 1.5: a class with no override gets its own table (`<class>s`).
//! Explicit overrides let multiple classes share a table or rename it. A table
//! is a `[tables.<name>]` block inside the `.snxlib` file.

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

/// Manifest `auth` prefix that names an OS-keychain item instead of a token.
pub(crate) const KEYCHAIN_SIGIL: &str = "@signex-keychain:";

/// A manifest `auth` of the form `${env:NAME}` names an environment
/// variable that holds the token, so the token itself never reaches the
/// manifest. Read when the library connects.
///
/// Only variables whose name starts with `SIGNEX_` (upper-case letters,
/// digits and `_`, for example `SIGNEX_DB_TOKEN`) can be named. A `.snxlib`
/// comes from a shared repository and also sets the server `url`, so letting
/// it name any variable would let a tampered manifest send `GITHUB_TOKEN` or
/// a cloud secret to its own server as the bearer token.
const ENV_REFERENCE_PREFIX: &str = "${env:";

/// Variable names a manifest may reference.
const ENV_NAME_PREFIX: &str = "SIGNEX_";

/// The variable name in an `auth` of the form `${env:NAME}`, whatever the
/// name (it may still be refused by [`resolve_auth`]).
pub(crate) fn env_reference(auth: &str) -> Option<&str> {
    auth.strip_prefix(ENV_REFERENCE_PREFIX)?.strip_suffix('}')
}

fn env_name_is_allowed(name: &str) -> bool {
    name.strip_prefix(ENV_NAME_PREFIX).is_some_and(|rest| {
        !rest.is_empty()
            && rest
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
    })
}

/// True when `auth` is a hand-written token: not empty, not a keychain
/// reference and not an `${env:NAME}` reference. Saving one stores it in
/// plain text.
pub fn auth_is_plain_token(auth: &str) -> bool {
    !auth.is_empty() && !auth.starts_with(KEYCHAIN_SIGIL) && env_reference(auth).is_none()
}

/// Turn a manifest `auth` into the bearer token to send. An `${env:NAME}`
/// reference is read through `lookup` (the process environment in
/// production) only when its name starts with `SIGNEX_`; any other name is
/// refused without calling `lookup`. Anything else is the token itself.
/// Empty means no auth.
pub(crate) fn resolve_auth(
    auth: &str,
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<String, String> {
    match env_reference(auth) {
        None => Ok(auth.to_string()),
        Some(name) if !env_name_is_allowed(name) => Err(format!(
            "library auth names environment variable {name:?}; only SIGNEX_ followed by upper-case \
             letters, digits or _ is allowed, so a shared library file cannot read other secrets"
        )),
        Some(name) => lookup(name)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                format!("library auth names environment variable {name}, which is not set")
            }),
    }
}

#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LibraryMode {
    #[default]
    LocalGit,
    Database {
        url: String,
        /// Empty (no auth), an `${env:SIGNEX_NAME}` reference to an
        /// environment variable holding the token (the name must start with
        /// `SIGNEX_`), a `@signex-keychain:` reference, or
        /// a raw token. The database adapter cannot resolve keychain
        /// references yet (#976). A raw token is written back as is so the
        /// manifest stays saveable; the library adapter warns once per
        /// session that it is stored in plain text. `Debug` never prints a
        /// raw token.
        #[serde(serialize_with = "serialize_auth_reference")]
        auth: String,
    },
}

/// Write the `auth` value as is. A raw token is written back because the
/// keychain alternative does not work yet (#976); refusing would leave a
/// working manifest unsaveable. Serialising is not the moment to warn (it
/// runs for every encode); the adapter that saves the library does, once.
fn serialize_auth_reference<S: serde::Serializer>(
    auth: &str,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(auth)
}

/// Replace the userinfo of `scheme://user:pass@host/...` with
/// `<redacted>`, so a credential embedded in a URL never reaches a log.
/// A URL without userinfo is returned unchanged.
fn redact_url_userinfo(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return url.to_string();
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    match rest[..authority_end].rfind('@') {
        Some(at) => format!("{scheme}://<redacted>@{}", &rest[at + 1..]),
        None => url.to_string(),
    }
}

impl std::fmt::Debug for LibraryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LocalGit => f.write_str("LocalGit"),
            Self::Database { url, auth } => {
                let shown = if auth_is_plain_token(auth) {
                    "<redacted>"
                } else {
                    auth.as_str()
                };
                f.debug_struct("Database")
                    .field("url", &redact_url_userinfo(url))
                    .field("auth", &shown)
                    .finish()
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkflowConfig {
    /// Per `v0.9-snxlib-as-file-plan.md` §3.6, library workflow features
    /// scale with this mode: `Personal` hides released-flag /
    /// bump-dialog / cascade-modal / Library-Updates-dialog (silent
    /// auto-cascade + auto-update); `Team` enables them all; `Enterprise`
    /// adds approval workflow + audit log + sign-off (preview only,
    /// v0.9 doesn't expose the picker).
    #[serde(default)]
    pub mode: WorkflowMode,
    #[serde(default)]
    pub review_required: bool,
    #[serde(default = "default_reviewers_required")]
    pub reviewers_required: u32,
    #[serde(default = "default_auto_promote")]
    pub auto_lifecycle_promote: String,
}

/// Per-library workflow mode — controls which versioning + cascade
/// gates are visible in the UI. Stored as a TOML string token.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowMode {
    /// Default for new libraries — auto-bump on save, silent cascade,
    /// auto-update on schematic open. The `released` flag, bump
    /// dialogs, and Library Updates dialog are hidden.
    #[default]
    Personal,
    /// Released flag + bump dialog + cascade modal + Library Updates
    /// dialog all visible. The plan's full §3.5 / §3.5.cascade
    /// behaviour is active.
    Team,
    /// Adds approval workflow, audit log, and sign-off on top of Team.
    /// Design preview only — v0.9 does not expose this in the UI.
    Enterprise,
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
            mode: WorkflowMode::default(),
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
    #[serde(default, flatten, deserialize_with = "users_skipping_plain_values")]
    pub by_email: BTreeMap<String, UserEntry>,
}

fn default_role() -> String {
    "Designer".into()
}

/// The per-user tables under `[users]`. A plain value there (`future_flag =
/// true`) is a key this build does not know, not a user: it is skipped here
/// and stays in the text the file keeps (#1435).
fn users_skipping_plain_values<'de, D>(d: D) -> Result<BTreeMap<String, UserEntry>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    #[allow(dead_code)] // the plain value is only matched, never read
    enum Entry {
        User(UserEntry),
        Plain(Plain),
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    #[allow(dead_code)] // read only to tell a plain value from a user table
    enum Plain {
        Bool(bool),
        Int(i64),
        Float(f64),
        Text(String),
        List(Vec<serde::de::IgnoredAny>),
    }
    let all = BTreeMap::<String, Entry>::deserialize(d)?;
    Ok(all
        .into_iter()
        .filter_map(|(k, e)| match e {
            Entry::User(u) => Some((k, u)),
            Entry::Plain(_) => None,
        })
        .collect())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserEntry {
    pub display_name: String,
    pub roles: Vec<String>,
}

/// Optional `[[tables]]` override block — Altium DBLib parity.
///
/// Per `v0.9-refactor-2-plan.md` §3:
/// - The default is class → `<class>s` (mechanical pluralisation).
/// - An explicit override lets multiple classes share one table
///   (e.g. resistors + capacitors both rolled into `Discrete_Passives`),
///   or rename it for non-English / domain-specific layouts.
///
/// `name` is the table name; the rows live in the `.snxlib` file's
/// `[tables.<name>]` block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableConfig {
    /// Table name. Becomes the `[tables.<name>]` block in the `.snxlib`.
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

    /// Resolve a class name to its table name.
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
        assert_eq!(m.workflow.mode, WorkflowMode::Personal);
        assert!(m.tables().is_empty());
    }

    /// Workflow mode round-trips through TOML — Stage 13 of
    /// `v0.9-snxlib-as-file-plan.md`. The picker defaults to
    /// `Personal` for new libraries and gates the §3.5 versioning UI
    /// (released flag, bump dialog, cascade modal) when set to
    /// `Team`.
    #[test]
    fn workflow_mode_round_trips() {
        for (token, mode) in [
            ("personal", WorkflowMode::Personal),
            ("team", WorkflowMode::Team),
            ("enterprise", WorkflowMode::Enterprise),
        ] {
            let text = format!(
                r#"
[library]
name = "X"
library_id = "0192a8c0-0000-7000-8000-000000000000"

[workflow]
mode = "{token}"
"#
            );
            let m = Manifest::parse(&text).unwrap();
            assert_eq!(m.workflow.mode, mode, "parse {token:?}");
            // Round-trip: write + parse back, mode survives.
            let back = Manifest::parse(&m.write().unwrap()).unwrap();
            assert_eq!(back.workflow.mode, mode, "round-trip {token:?}");
        }
    }

    /// Missing `[workflow]` block — `mode` defaults to `Personal`
    /// (no surprise UI gates appear).
    #[test]
    fn workflow_mode_defaults_to_personal_when_absent() {
        let text = r#"
[library]
name = "X"
library_id = "0192a8c0-0000-7000-8000-000000000000"
"#;
        let m = Manifest::parse(text).unwrap();
        assert_eq!(m.workflow.mode, WorkflowMode::Personal);
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
    fn raw_auth_token_is_written_back_but_never_printed() {
        let mode = LibraryMode::Database {
            url: "https://lib.example/api".into(),
            auth: "s3cr3t-token".into(),
        };
        assert!(!format!("{mode:?}").contains("s3cr3t-token"));
        assert!(!format!("{mode:#?}").contains("s3cr3t-token"));
        // Kept saveable until keychain references work (#976).
        assert!(toml::to_string(&mode).unwrap().contains("s3cr3t-token"));
        let keychain = LibraryMode::Database {
            url: "https://lib.example/api".into(),
            auth: "@signex-keychain:lab".into(),
        };
        assert!(
            toml::to_string(&keychain)
                .unwrap()
                .contains("@signex-keychain:lab")
        );
    }

    #[test]
    fn database_url_userinfo_is_redacted_in_debug() {
        let mode = LibraryMode::Database {
            url: "https://alice:hunter2@lib.example:8443/api?x=a@b".into(),
            auth: String::new(),
        };
        let shown = format!("{mode:?}");
        assert!(
            !shown.contains("alice") && !shown.contains("hunter2"),
            "{shown}"
        );
        assert!(
            shown.contains("https://<redacted>@lib.example:8443/api?x=a@b"),
            "{shown}"
        );
        let plain = LibraryMode::Database {
            url: "https://lib.example/api@v1".into(),
            auth: String::new(),
        };
        assert!(format!("{plain:?}").contains("https://lib.example/api@v1"));
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

    #[test]
    fn env_references_are_not_plain_tokens_and_resolve_through_the_lookup() {
        assert!(auth_is_plain_token("s3cr3t"));
        assert!(!auth_is_plain_token(""));
        assert!(!auth_is_plain_token("@signex-keychain:lab"));
        assert!(!auth_is_plain_token("${env:SIGNEX_DB_TOKEN}"));
        assert!(!auth_is_plain_token("${env:}"), "refused, not sent as text");

        let lookup = |name: &str| (name == "SIGNEX_DB_TOKEN").then(|| "tok".to_string());
        assert_eq!(
            resolve_auth("${env:SIGNEX_DB_TOKEN}", lookup).unwrap(),
            "tok"
        );
        assert_eq!(resolve_auth("raw", lookup).unwrap(), "raw");
        assert_eq!(resolve_auth("", lookup).unwrap(), "");
        let err = resolve_auth("${env:SIGNEX_OTHER}", lookup).unwrap_err();
        assert!(
            err.contains("SIGNEX_OTHER") && err.contains("not set"),
            "{err}"
        );
    }

    #[test]
    fn only_signex_prefixed_variables_can_be_read_and_the_lookup_is_never_called() {
        let lookup = |name: &str| -> Option<String> {
            panic!("the environment was read for {name}");
        };
        for auth in [
            "${env:HOME}",
            "${env:AWS_SECRET_ACCESS_KEY}",
            "${env:GITHUB_TOKEN}",
            "${env:signex_db_token}",
            "${env:SIGNEX_db}",
            "${env:SIGNEX_}",
            "${env:}",
            "${env: SIGNEX_DB_TOKEN}",
        ] {
            let err = resolve_auth(auth, lookup).unwrap_err();
            assert!(err.contains("SIGNEX_"), "{auth}: {err}");
        }
    }
}
