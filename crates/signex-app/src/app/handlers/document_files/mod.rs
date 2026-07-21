//! Document-file open / save / git / history handler impls for `Signex`.

use std::path::PathBuf;

use super::super::*;

mod git;
mod history;
mod open;
mod save;

/// Build the AsyncFileDialog Task for a primitive's first save.
/// The dialog defaults to the suggested path's parent + filename so
/// the common case is a single Enter key; the user can navigate to
/// a global library directory outside the project if they want a
/// shared symbol. Cancel = no save (editor stays dirty).
pub(crate) fn spawn_save_as_for_new_primitive(suggested: PathBuf) -> iced::Task<Message> {
    let ext = suggested
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("snxsym")
        .to_string();
    let (filter_label, filter_ext) = match ext.as_str() {
        "snxfpt" => ("Signex Footprint", "snxfpt"),
        _ => ("Signex Symbol", "snxsym"),
    };
    let title = match ext.as_str() {
        "snxfpt" => "Save Footprint As",
        _ => "Save Symbol As",
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
            Some(to_path) => Message::File(FileMsg::SavePrimitiveAs {
                from_path: from.clone(),
                to_path,
            }),
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
