//! The name of a stored entry: a snippet, a template or a board reuse block
//! (#2410). One rule, since each name is a file name on every platform.

/// The longest entry name, in characters.
pub const MAX_NAME_CHARS: usize = 64;

/// Names a file cannot take on Windows, whatever its extension.
const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Why a name cannot name a stored entry. Each caller words it for what it
/// stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameRefusal {
    Empty,
    TooLong,
    /// A character other than letters, digits, spaces, `-`, `_` and `.`
    /// inside, or a leading or trailing dot.
    Characters,
    /// A name the system keeps for itself.
    Reserved,
}

/// `name` trimmed, or why it cannot name a file.
pub fn checked_name(name: &str) -> Result<String, NameRefusal> {
    let name = name.trim();
    if name.is_empty() {
        return Err(NameRefusal::Empty);
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(NameRefusal::TooLong);
    }
    let allowed = |c: char| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.');
    if !name.chars().all(allowed) || name.starts_with('.') || name.ends_with('.') {
        return Err(NameRefusal::Characters);
    }
    // A reserved name stays reserved whatever follows its first dot.
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem)) {
        return Err(NameRefusal::Reserved);
    }
    Ok(name.to_string())
}
