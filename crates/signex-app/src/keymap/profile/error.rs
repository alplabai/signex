use std::{error::Error, fmt, io};

#[derive(Debug)]
pub enum ProfileLoadError {
    Toml(toml::de::Error),
    TomlSerialize(toml::ser::Error),
    Io(io::Error),
    KeyParse(crate::keymap::KeyParseError),
    NoConfigDir,
    InvalidHeader(String),
    UnsupportedHeaderVersion(u32),
    UnsupportedSchemaVersion(u32),
    InvalidProfileId(String),
    DuplicateProfileId,
    UnknownActiveProfile(String),
    BuiltInMutation(String),
    NoActionExport,
    UnknownBindingSlot,
}

impl fmt::Display for ProfileLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Toml(error) => write!(f, "{error}"),
            Self::TomlSerialize(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "{error}"),
            Self::KeyParse(error) => write!(f, "{error}"),
            Self::NoConfigDir => f.write_str("no config directory available"),
            Self::InvalidHeader(error) => write!(f, "invalid shortcut profile header: {error}"),
            Self::UnsupportedHeaderVersion(version) => {
                write!(f, "unsupported Signex settings header version {version}")
            }
            Self::UnsupportedSchemaVersion(version) => {
                write!(f, "unsupported keyboard shortcut schema version {version}")
            }
            Self::InvalidProfileId(id) => write!(f, "invalid shortcut profile id `{id}`"),
            Self::DuplicateProfileId => f.write_str("duplicate shortcut profile id"),
            Self::UnknownActiveProfile(id) => write!(f, "unknown shortcut profile `{id}`"),
            Self::BuiltInMutation(id) => write!(f, "built-in profile `{id}` cannot be modified"),
            Self::NoActionExport => f.write_str("no-action shortcut bindings cannot be exported"),
            Self::UnknownBindingSlot => {
                f.write_str("that shortcut no longer exists in the profile")
            }
        }
    }
}

impl Error for ProfileLoadError {}
