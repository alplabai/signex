use serde::{Deserialize, Serialize};
use std::fmt;

/// The stable id of a catalog command (`place_wire`): snake_case ASCII.
///
/// A command an extension adds has an id in its owner's namespace instead,
/// `ext.<name>.<local>` (`ext.sample.say_hello`): `ext`, the extension's
/// name and one or more snake_case parts, joined by `.`. A built-in id never
/// holds a `.`, so the two never collide (`docs/EXTENSION_API.md`, "Ids").
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AppCommandId(String);

/// Is `part` non-empty snake_case ASCII?
fn is_snake_part(part: &str) -> bool {
    !part.is_empty()
        && part
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Is `value` an extension command id, `ext.<name>.<local>`? The name
/// starts with a lowercase ASCII letter, as an extension name does.
fn is_extension_id(value: &str) -> bool {
    let mut parts = value.split('.');
    let starts_ext = parts.next() == Some("ext");
    let name_well = parts.next().is_some_and(|name| {
        is_snake_part(name) && name.starts_with(|c: char| c.is_ascii_lowercase())
    });
    let mut locals = parts.peekable();
    let has_local = locals.peek().is_some();
    starts_ext && name_well && has_local && locals.all(is_snake_part)
}

impl AppCommandId {
    pub fn new(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.is_empty() {
            return Err("command id must not be empty".to_string());
        }
        if !is_snake_part(&value) && !is_extension_id(&value) {
            return Err(format!(
                "command id `{value}` must use snake_case ASCII letters, digits, or underscores"
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Is this a command an extension added (`ext.<name>.<local>`)?
    pub fn is_extension(&self) -> bool {
        is_extension_id(&self.0)
    }
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
    use super::AppCommandId;

    #[test]
    fn a_built_in_id_is_snake_case_with_no_dot() {
        for id in ["place_wire", "zoom_in", "a1"] {
            let command = AppCommandId::new(id).expect("a built-in id");
            assert!(!command.is_extension(), "{id}");
        }
        for id in ["", "Place_wire", "place-wire", "place wire", "place.wire"] {
            assert!(AppCommandId::new(id).is_err(), "{id:?} was accepted");
        }
    }

    /// #2495: an extension's command sits under `ext.<name>.`.
    #[test]
    fn an_extension_id_is_ext_then_its_name_then_snake_case_parts() {
        for id in ["ext.sample.say_hello", "ext.s2.a.b_c", "ext.sample.1st"] {
            let command = AppCommandId::new(id).expect("an extension id");
            assert!(command.is_extension(), "{id}");
            assert_eq!(command.as_str(), id);
        }
        for id in [
            "ext.",
            "ext.sample",
            "ext.sample.",
            "ext..say_hello",
            "ext.2nd.say_hello",
            "ext.Sample.say_hello",
            "ext.sample.say-hello",
            "ext.sample..say_hello",
            "other.sample.say_hello",
        ] {
            assert!(AppCommandId::new(id).is_err(), "{id:?} was accepted");
        }
    }
}
