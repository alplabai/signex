//! The report's facts that come straight from the project file and the
//! hand-written notebook: the sheets, the revision, the variants and the
//! blocks of every notebook file.

use signex_types::project::{BASE_VARIANT, ProjectData, is_base_variant};

use super::facts::SheetFact;
use crate::annotation::Annotation;
use crate::scan::scan;
use crate::sources::NotebookSources;

/// The project's sheets, the root sheet first at depth 0, the others in list
/// order at depth 1. The parts are the symbols the project file counts.
pub fn sheets_of(data: &ProjectData) -> Vec<SheetFact> {
    let root = data.schematic_root.as_deref().map(str::trim);
    let is_root = |file: &str| root.is_some_and(|root| root == file.trim());
    let fact = |sheet: &signex_types::project::SheetEntry, depth| SheetFact {
        name: sheet.name.clone(),
        file: sheet.filename.clone(),
        depth,
        parts: sheet.symbols_count,
    };
    let mut sheets: Vec<SheetFact> = data
        .sheets
        .iter()
        .filter(|sheet| is_root(&sheet.filename))
        .map(|sheet| fact(sheet, 0))
        .collect();
    sheets.extend(
        data.sheets
            .iter()
            .filter(|sheet| !is_root(&sheet.filename))
            .map(|sheet| fact(sheet, 1)),
    );
    sheets
}

/// The project's revision: its `Revision` (or `Rev`) parameter, any case,
/// when it is not blank.
pub fn revision_of(data: &ProjectData) -> Option<String> {
    data.parameters
        .iter()
        .find(|(key, _)| {
            let key = key.trim();
            key.eq_ignore_ascii_case("revision") || key.eq_ignore_ascii_case("rev")
        })
        .map(|(_, value)| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// The variants a report can be built for: `Base`, then the project's own.
pub fn variants_of(data: &ProjectData) -> Vec<String> {
    let mut variants = vec![BASE_VARIANT.to_string()];
    variants.extend(
        data.variant_definitions
            .iter()
            .filter(|name| !is_base_variant(name))
            .cloned(),
    );
    variants
}

/// The variant named `picked` as the project spells it, or the project's
/// active one when none is picked. Refused when the project has no variant
/// of that name.
pub fn variant_of(data: &ProjectData, picked: Option<&str>) -> Result<String, String> {
    let wanted = picked
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .or(data.active_variant.as_deref())
        .unwrap_or(BASE_VARIANT);
    variants_of(data)
        .into_iter()
        .find(|name| name.eq_ignore_ascii_case(wanted.trim()))
        .ok_or_else(|| format!("the project has no variant named `{wanted}`"))
}

/// The blocks of every file of the notebook, the notebook file's first, then
/// its chapters by path.
pub fn notebook_blocks(sources: &NotebookSources) -> Vec<Annotation> {
    let main = sources.main().to_string();
    let mut paths: Vec<&str> = sources.paths().filter(|path| *path != main).collect();
    paths.insert(0, main.as_str());
    paths
        .into_iter()
        .filter_map(|path| sources.get(path))
        .flat_map(|bytes| scan(&String::from_utf8_lossy(bytes)).0)
        .collect()
}
