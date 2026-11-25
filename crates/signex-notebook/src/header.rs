//! The TOML header of a `.snxnbk` file: its format token and the stored id
//! of each reference. Signex owns the header and keeps it in the file
//! itself, between two `+++` lines; there is no sidecar file. The editor
//! shows it as one folded line.

use signex_types::format_version::{FormatVersion, FormatVersionError, MissingToken, NewerMinor};
use uuid::Uuid;

use crate::reference::Reference;

/// The version policy of `.snxnbk` (`docs/FORMAT_VERSIONS.md`). A file with
/// no header, or a header with no `format` key, reads as `snxnbk/1`; a
/// newer minor opens read-only.
pub const NOTEBOOK_FORMAT: FormatVersion = FormatVersion {
    current: "snxnbk/1",
    readable: &["snxnbk/1"],
    newer_minor: NewerMinor::ReadOnly,
    missing: MissingToken::ReadAs("snxnbk/1"),
};

/// The header key that carries the format token.
const FORMAT_KEY: &str = "format";
/// The header key of the reference list.
const REFERENCES_KEY: &str = "references";

/// One reference the notebook makes, with the id of what it names.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredReference {
    /// The reference as the text reads it.
    pub reference: Reference,
    /// The UUID of the part (for a component or a pin) or net it named
    /// when it was last resolved. A renamed part is still found by it.
    pub id: Option<Uuid>,
    /// Keys this build does not know, kept and written back.
    pub extra: toml::Table,
}

/// The header of one notebook file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Header {
    /// The references and their stored ids, in the order they were stored.
    pub references: Vec<StoredReference>,
    /// Top-level keys this build does not know, kept and written back.
    pub extra: toml::Table,
}

/// Why a header could not be read. The caller adds the file name.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum HeaderError {
    /// The text between the fences is not TOML.
    #[error("the notebook header is not valid TOML: {0}")]
    Toml(String),
    /// The format token is refused by [`NOTEBOOK_FORMAT`].
    #[error(transparent)]
    Version(#[from] FormatVersionError),
    /// A key that must hold a string holds something else.
    #[error("the notebook header key {key:?} is not a string")]
    NotText { key: String },
    /// `references` is not a list of tables.
    #[error("the notebook header key \"references\" is not a list of tables")]
    NotAList,
    /// Reference `index` (0-based) does not say what it names.
    #[error("reference {index} of the notebook header {reason}")]
    BadEntry { index: usize, reason: &'static str },
    /// Reference `index` carries an id that is not a UUID.
    #[error("reference {index} of the notebook header has the id {id:?}, which is not a UUID")]
    BadId { index: usize, id: String },
}

impl Header {
    /// True when the header stores nothing but its token.
    pub fn is_empty(&self) -> bool {
        self.references.is_empty() && self.extra.is_empty()
    }

    /// The lowest token the header's content needs. Every `.snxnbk` this
    /// build writes is `snxnbk/1`; a later field that an older build would
    /// misread takes the next minor here.
    pub fn written_token(&self) -> &'static str {
        NOTEBOOK_FORMAT.current
    }

    /// The stored id of `reference`, if the header has one.
    pub fn id_of(&self, reference: &Reference) -> Option<Uuid> {
        self.references
            .iter()
            .find(|stored| &stored.reference == reference)
            .and_then(|stored| stored.id)
    }

    /// Store `id` for `reference`, adding the reference when it is new.
    pub fn set_id(&mut self, reference: &Reference, id: Option<Uuid>) {
        match self
            .references
            .iter_mut()
            .find(|stored| &stored.reference == reference)
        {
            Some(stored) => stored.id = id,
            None => self.references.push(StoredReference {
                reference: reference.clone(),
                id,
                extra: toml::Table::new(),
            }),
        }
    }

    /// Drop the stored references the text no longer makes.
    pub fn retain_used(&mut self, used: &[Reference]) {
        self.references
            .retain(|stored| used.contains(&stored.reference));
    }

    /// Read the TOML between the fences. Returns the header and the token
    /// it is read as (see [`NOTEBOOK_FORMAT`]).
    pub fn read(text: &str) -> Result<(Header, String), HeaderError> {
        let mut table: toml::Table = toml::from_str(text)
            .map_err(|e: toml::de::Error| HeaderError::Toml(e.message().to_string()))?;
        let token = match table.remove(FORMAT_KEY) {
            None => None,
            Some(toml::Value::String(token)) => Some(token),
            Some(_) => {
                return Err(HeaderError::NotText {
                    key: FORMAT_KEY.to_string(),
                });
            }
        };
        let token = NOTEBOOK_FORMAT.read(token.as_deref())?;
        let references = match table.remove(REFERENCES_KEY) {
            None => Vec::new(),
            Some(toml::Value::Array(entries)) => entries
                .into_iter()
                .enumerate()
                .map(|(index, entry)| match entry {
                    toml::Value::Table(entry) => read_entry(index, entry),
                    _ => Err(HeaderError::NotAList),
                })
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(HeaderError::NotAList),
        };
        Ok((
            Header {
                references,
                extra: table,
            },
            token,
        ))
    }

    /// The header as TOML, without the fences: the token, the unknown keys
    /// (sorted), then one `[[references]]` table per reference. Same header,
    /// same bytes.
    pub fn to_toml(&self) -> String {
        let mut out = format!("{FORMAT_KEY} = {}\n", toml_string(self.written_token()));
        write_keys(&mut out, &self.extra);
        for stored in &self.references {
            out.push_str("\n[[references]]\n");
            let (kind, fields) = reference_fields(&stored.reference);
            out.push_str(&format!("kind = {}\n", toml_string(kind)));
            for (key, value) in fields {
                out.push_str(&format!("{key} = {}\n", toml_string(value)));
            }
            if let Some(id) = stored.id {
                out.push_str(&format!("id = {}\n", toml_string(&id.to_string())));
            }
            write_keys(&mut out, &stored.extra);
        }
        out
    }
}

/// The keys a reference is written with, after its `kind`.
fn reference_fields(reference: &Reference) -> (&'static str, Vec<(&'static str, &str)>) {
    match reference {
        Reference::Component(name) => ("component", vec![("component", name)]),
        Reference::Net(name) => ("net", vec![("net", name)]),
        Reference::Pin { component, pin } => ("pin", vec![("component", component), ("pin", pin)]),
    }
}

fn read_entry(index: usize, mut entry: toml::Table) -> Result<StoredReference, HeaderError> {
    let mut text = |key: &str| -> Result<Option<String>, HeaderError> {
        match entry.remove(key) {
            None => Ok(None),
            Some(toml::Value::String(value)) if !value.trim().is_empty() => Ok(Some(value)),
            Some(_) => Err(HeaderError::BadEntry {
                index,
                reason: "has an empty or non-text name",
            }),
        }
    };
    let kind = text("kind")?;
    let reference = match kind.as_deref() {
        Some("component") => text("component")?.map(Reference::Component),
        Some("net") => text("net")?.map(Reference::Net),
        Some("pin") => match (text("component")?, text("pin")?) {
            (Some(component), Some(pin)) => Some(Reference::Pin { component, pin }),
            _ => None,
        },
        _ => {
            return Err(HeaderError::BadEntry {
                index,
                reason: "has no kind of component, pin or net",
            });
        }
    };
    let Some(reference) = reference else {
        return Err(HeaderError::BadEntry {
            index,
            reason: "does not name what it refers to",
        });
    };
    let id = match entry.remove("id") {
        None => None,
        Some(toml::Value::String(id)) => match Uuid::parse_str(&id) {
            Ok(uuid) => Some(uuid),
            Err(_) => return Err(HeaderError::BadId { index, id }),
        },
        Some(other) => {
            return Err(HeaderError::BadId {
                index,
                id: other.to_string(),
            });
        }
    };
    Ok(StoredReference {
        reference,
        id,
        extra: entry,
    })
}

/// `key = value` for each entry, values inline.
fn write_keys(out: &mut String, table: &toml::Table) {
    for (key, value) in table {
        out.push_str(&format!("{} = {value}\n", toml_key(key)));
    }
}

/// A basic TOML string, escaped by the TOML writer.
fn toml_string(text: &str) -> String {
    toml::Value::String(text.to_string()).to_string()
}

/// A bare key when it can be one, else a quoted key.
fn toml_key(key: &str) -> String {
    let bare = !key.is_empty()
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if bare {
        key.to_string()
    } else {
        toml_string(key)
    }
}
