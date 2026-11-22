//! The file name a primitive shows under. Stock primitives are stored as
//! `<uuid>.<ext>`, an id nobody can read, so those show as `<name>.<ext>`
//! (mockup rows 118, 130 and 131).

use std::path::Path;

use signex_library::PrimitiveKind;

/// The file extension of a primitive kind.
pub(crate) fn extension(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => "snxsym",
        PrimitiveKind::Footprint => "snxfpt",
        PrimitiveKind::Sim => "snxsim",
        _ => "snx",
    }
}

/// Whether `file` is named by a bare uuid.
pub(crate) fn is_named_by_id(file: &Path) -> bool {
    file.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| uuid::Uuid::parse_str(stem).is_ok())
}

/// The name to show for `file`: its own file name, or `<name>.<ext>` when
/// the file is named by a uuid and the primitive has a name.
pub(crate) fn shown_file_name(file: &Path, name: Option<&str>, ext: &str) -> Option<String> {
    let own = file.file_name()?.to_string_lossy().into_owned();
    match name.filter(|name| !name.is_empty() && is_named_by_id(file)) {
        Some(name) => Some(format!("{name}.{ext}")),
        None => Some(own),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID_FILE: &str = "footprints/019df45d-af63-7823-b5c3-0123456789ab.snxfpt";

    #[test]
    fn a_file_named_by_a_uuid_shows_the_primitive_name() {
        assert_eq!(
            shown_file_name(Path::new(ID_FILE), Some("R_0402_1005"), "snxfpt").as_deref(),
            Some("R_0402_1005.snxfpt")
        );
    }

    #[test]
    fn a_file_with_a_real_name_keeps_it() {
        assert_eq!(
            shown_file_name(Path::new("chip-passives.snxfpt"), Some("R"), "snxfpt").as_deref(),
            Some("chip-passives.snxfpt")
        );
    }

    #[test]
    fn a_uuid_file_without_a_name_keeps_its_file_name() {
        let shown = shown_file_name(Path::new(ID_FILE), None, "snxfpt");
        assert_eq!(
            shown.as_deref(),
            Some("019df45d-af63-7823-b5c3-0123456789ab.snxfpt")
        );
    }
}
