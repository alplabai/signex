//! Document-file open / save / git / history handler impls for `Signex`.

use std::path::PathBuf;

use super::super::*;

mod git;
mod history;
mod open;
mod recent;
mod save;

/// Save-As dialog Task for a symbol / footprint, used by the first save
/// of a new primitive and by File > Save As (#671). The dialog defaults
/// to the suggested path's parent + filename so the common case is a
/// single Enter key; the user can pick a global library directory
/// outside the project. Cancel = no save (editor stays dirty). The
/// result carries the source path (`from_path`) so the (non-modal)
/// dialog cannot redirect the write to whichever tab is active when it
/// returns; the picked path is passed through unresolved and
/// `handle_save_primitive_as` applies the extension and overwrite check.
pub(crate) fn spawn_save_as_for_primitive(suggested: PathBuf) -> iced::Task<Message> {
    spawn_primitive_save_dialog(suggested)
}

fn primitive_save_as_message(from_path: PathBuf, to_path: PathBuf) -> Message {
    Message::File(FileMsg::SavePrimitiveAs { from_path, to_path })
}

/// Extension of the primitive kind the source path belongs to.
pub(crate) fn primitive_extension(from_path: &std::path::Path) -> &'static str {
    match from_path.extension().and_then(|s| s.to_str()) {
        Some("snxfpt") => "snxfpt",
        _ => "snxsym",
    }
}

/// Append the kind's extension to a picked path that has none (an
/// extensionless "foo" would be written but rejected by reopen). A path
/// that already carries an extension is left alone.
pub(crate) fn with_primitive_extension(to_path: PathBuf, ext: &str) -> PathBuf {
    if to_path.extension().is_some() {
        return to_path;
    }
    let mut name = to_path.into_os_string();
    name.push(".");
    name.push(ext);
    PathBuf::from(name)
}

fn spawn_primitive_save_dialog(suggested: PathBuf) -> iced::Task<Message> {
    let filter_ext = primitive_extension(&suggested);
    let (filter_label, title) = match filter_ext {
        "snxfpt" => ("Signex Footprint", "Save Footprint As"),
        _ => ("Signex Symbol", "Save Symbol As"),
    };
    let default_dir = suggested
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let default_name = suggested
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("New.{filter_ext}"));
    let from = suggested;

    iced::Task::perform(
        async move {
            rfd::AsyncFileDialog::new()
                .set_title(title)
                .add_filter(filter_label, &[filter_ext])
                .set_directory(&default_dir)
                .set_file_name(&default_name)
                .save_file()
                .await
                .map(|file| file.path().to_path_buf())
        },
        move |picked| match picked {
            Some(to_path) => primitive_save_as_message(from.clone(), to_path),
            None => Message::Noop,
        },
    )
}

/// Build the bare-minimum [`SchematicSheet`] used as the starting state
/// for File ▸ New Project. Only the fields that don't have a serde
/// default need explicit values; everything else falls through to the
/// per-field defaults the writer/parser already round-trip.
pub(crate) fn blank_schematic_sheet_for_new_doc() -> signex_types::schematic::SchematicSheet {
    blank_schematic_sheet()
}

fn blank_schematic_sheet() -> signex_types::schematic::SchematicSheet {
    signex_types::schematic::SchematicSheet {
        unknown: Default::default(),
        uuid: uuid::Uuid::new_v4(),
        version: 1,
        generator: "signex".into(),
        generator_version: env!("CARGO_PKG_VERSION").into(),
        paper_size: "A4".into(),
        root_sheet_page: "1".into(),
        symbols: Vec::new(),
        wires: Vec::new(),
        junctions: Vec::new(),
        labels: Vec::new(),
        child_sheets: Vec::new(),
        no_connects: Vec::new(),
        text_notes: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        drawings: Vec::new(),
        no_erc_directives: Vec::new(),
        title_block: Default::default(),
        lib_symbols: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialog_result_carries_the_source_path() {
        let msg = primitive_save_as_message("a.snxsym".into(), "b.snxsym".into());
        assert!(matches!(
            msg,
            Message::File(FileMsg::SavePrimitiveAs { from_path, to_path })
                if from_path.as_path() == std::path::Path::new("a.snxsym")
                    && to_path.as_path() == std::path::Path::new("b.snxsym")
        ));
    }

    #[test]
    fn extensionless_pick_gets_the_kind_extension() {
        assert_eq!(
            with_primitive_extension(PathBuf::from("/x/foo"), "snxsym"),
            PathBuf::from("/x/foo.snxsym")
        );
    }

    #[test]
    fn picked_extension_is_left_alone() {
        assert_eq!(
            with_primitive_extension(PathBuf::from("/x/foo.snxfpt"), "snxfpt"),
            PathBuf::from("/x/foo.snxfpt")
        );
    }
}
