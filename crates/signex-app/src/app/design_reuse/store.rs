//! The files of snippets and templates (#1283). Every function here blocks:
//! run it off the update thread.

use std::io::Write;
use std::path::Path;

use signex_types::object_id::UuidV7Ids;
use signex_types::project::ProjectData;
use signex_types::reuse::template::{apply_project_template, apply_sheet_template};
use signex_types::schematic::SchematicSheet;

use crate::app::handlers::dock::project_navigation::add_io::create_noclobber;
use crate::app::project_sheets::load_sheet_file;

/// Write `text` as the snippet file `file`, its folder made if missing. An
/// existing snippet of that name is replaced only when `overwrite`; else the
/// write is refused and the file left as it is.
pub(crate) fn write_snippet(file: &Path, text: &str, overwrite: bool) -> Result<(), String> {
    if overwrite {
        return signex_types::atomic_io::atomic_write(file, text.as_bytes())
            .map_err(|error| error.to_string());
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    }
    create_noclobber(file, |out| out.write_all(text.as_bytes())).map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            "a snippet of that name exists; give `overwrite=true` to replace it".to_string()
        } else {
            error.to_string()
        }
    })
}

/// `sheet` with the frame of the sheet template at `template` on it.
pub(crate) fn sheet_from_template(
    mut sheet: SchematicSheet,
    template: &Path,
) -> Result<SchematicSheet, String> {
    let frame = load_sheet_file(template)?
        .ok_or_else(|| format!("there is no sheet template at {}", template.display()))?;
    apply_sheet_template(&frame, &mut sheet, &mut UuidV7Ids);
    Ok(sheet)
}

/// What a project template gives a new project, read before the project is
/// made: its settings, and its first sheet when it has one.
#[derive(Debug, Clone)]
pub(crate) struct ProjectTemplate {
    data: ProjectData,
    sheet: Option<SchematicSheet>,
}

/// Read the project template in `folder`: its one `.snxprj` and the sheet
/// it starts at (its root, else its first listed sheet).
pub(crate) fn read_project_template(folder: &Path) -> Result<ProjectTemplate, String> {
    if !folder.is_dir() {
        return Err(format!(
            "there is no project template at {}",
            folder.display()
        ));
    }
    let file = signex_types::reuse::project_template_file(folder)?;
    let data = signex_types::project::parse_project(&file)
        .map_err(|error| format!("{}: {error}", file.display()))?;
    let first = data
        .schematic_root
        .clone()
        .or_else(|| data.sheets.first().map(|sheet| sheet.filename.clone()));
    let sheet = match first {
        Some(name) => load_sheet_file(&folder.join(name))?,
        None => None,
    };
    Ok(ProjectTemplate { data, sheet })
}

/// Put `template` on the project just made at `project_file` and on its
/// first sheet `sheet_file`.
pub(crate) fn apply_project_template_to(
    template: &ProjectTemplate,
    project_file: &Path,
    sheet_file: &Path,
) -> Result<(), String> {
    let mut data = signex_types::project::parse_project(project_file)
        .map_err(|error| format!("{}: {error}", project_file.display()))?;
    apply_project_template(&template.data, &mut data);
    signex_types::project::write_project(project_file, &data)
        .map_err(|error| format!("{}: {error}", project_file.display()))?;
    let Some(frame) = &template.sheet else {
        return Ok(());
    };
    let mut sheet = load_sheet_file(sheet_file)?
        .ok_or_else(|| format!("{} was not written", sheet_file.display()))?;
    apply_sheet_template(frame, &mut sheet, &mut UuidV7Ids);
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .map_err(|error| format!("{}: {error}", sheet_file.display()))?;
    signex_types::atomic_io::atomic_write(sheet_file, text.as_bytes())
        .map_err(|error| format!("{}: {error}", sheet_file.display()))
}
