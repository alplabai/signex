use serde::{Deserialize, Serialize};
use std::fmt;

/// The prefix of every command id an extension adds (`ext.<name>.<command>`,
/// docs/EXTENSION_API.md, "Ids"). No built-in id starts with it.
pub const EXTENSION_ID_PREFIX: &str = "ext.";

/// The stable id of a catalog command: snake_case ASCII for a built-in
/// command (`place_wire`), `ext.<name>.<command>` for one an extension adds
/// (`ext.sample.say_hello`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AppCommandId(String);

impl AppCommandId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.is_empty() {
            return Err("command id must not be empty".to_string());
        }
        if value.starts_with(EXTENSION_ID_PREFIX) {
            if !is_extension_command_id(&value) {
                return Err(format!(
                    "extension command id `{value}` must be `ext.<name>.<command>`, with an \
                     extension name and a snake_case command"
                ));
            }
            return Ok(Self(value));
        }
        if !is_snake_case(&value) {
            return Err(format!(
                "command id `{value}` must use snake_case ASCII letters, digits, or underscores"
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Is this an extension's command (`ext.<name>.<command>`)?
    pub fn is_extension(&self) -> bool {
        self.0.starts_with(EXTENSION_ID_PREFIX)
    }

    /// The `<command>` part of an extension's id; `None` for a built-in id.
    pub fn extension_local(&self) -> Option<&str> {
        self.0
            .strip_prefix(EXTENSION_ID_PREFIX)
            .and_then(|rest| rest.split_once('.'))
            .map(|(_, local)| local)
    }
}

/// Can `name` name an extension? A lowercase ASCII letter, then lowercase
/// ASCII letters, digits and `_`. The one rule for extension names: the
/// extension module and [`AppCommandId::new`] both read it.
pub fn is_extension_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn is_snake_case(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `ext.<name>.<command>`: an extension name, then one snake_case segment.
fn is_extension_command_id(value: &str) -> bool {
    value
        .strip_prefix(EXTENSION_ID_PREFIX)
        .and_then(|rest| rest.split_once('.'))
        .is_some_and(|(name, local)| is_extension_name(name) && is_snake_case(local))
}

impl fmt::Display for AppCommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for AppCommandId {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<AppCommandId> for String {
    fn from(value: AppCommandId) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_built_in_id_is_snake_case() {
        for id in ["place_wire", "undo", "fast_grid_1"] {
            let id = AppCommandId::new(id).expect("valid");
            assert!(!id.is_extension());
            assert_eq!(id.extension_local(), None);
        }
        for id in ["", "Place", "place-wire", "place wire", "place.wire", "ext"] {
            assert!(AppCommandId::new(id).is_err(), "{id:?} was accepted");
        }
    }

    #[test]
    fn an_extension_id_is_ext_name_command() {
        let id = AppCommandId::new("ext.sample.say_hello").expect("valid");
        assert!(id.is_extension());
        assert_eq!(id.extension_local(), Some("say_hello"));
        for id in [
            "ext.",
            "ext.sample",
            "ext.sample.",
            "ext..say_hello",
            "ext.Sample.say_hello",
            "ext.2nd.say_hello",
            "ext.sample.Say",
            "ext.sample.say.hello",
            "ext.sample.say-hello",
        ] {
            assert!(AppCommandId::new(id).is_err(), "{id:?} was accepted");
        }
    }

    /// Built-in ids and extension ids never meet (`docs/DECISIONS.md`,
    /// 2026-11-25 extension ids row).
    #[test]
    fn no_built_in_command_is_an_extension_id() {
        for id in crate::catalog::all_command_ids() {
            assert!(!id.starts_with(EXTENSION_ID_PREFIX), "{id}");
            assert!(!AppCommandId::new(id).expect(id).is_extension(), "{id}");
        }
    }

    #[test]
    fn an_extension_name_is_a_lowercase_identifier() {
        for name in ["sample", "s", "sample_2", "a1"] {
            assert!(is_extension_name(name), "{name:?}");
        }
        for name in ["", "Sample", "2nd", "_sample", "sam-ple", "sam.ple", "ü"] {
            assert!(!is_extension_name(name), "{name:?}");
        }
    }
}
