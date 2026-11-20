//! The primitive picker's list (#131): every group and row, flattened for the
//! virtualised view. Data only, no widgets, so it is cheap enough to build on
//! every view; the view formats the labels of the rows in sight.

use signex_library::{PrimitiveKind, PrimitiveRef, PrimitiveSummary};

use super::{PICKER_H, ROW_H};
use crate::library::browser::{OVERSCAN_ROWS, visible_range};
use crate::library::loose_symbols::LooseSymbol;
use crate::library::state::{LibraryState, PrimitivePickerState, PrimitivePickerTarget};

/// What a group of rows holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Group {
    /// The footprints the component's own library parts offer (#2366).
    Suggested,
    /// An open library, by its display name.
    Library(String),
    /// Symbols of a library that is not open, by the library's name.
    NotOpen(String),
    /// Symbols of files that belong to no library.
    NoLibrary,
}

/// What choosing a row does.
#[derive(Debug, Clone)]
pub(super) enum Choice {
    /// Bind a primitive of an open library.
    Pick(PrimitiveRef),
    /// Ask to copy (or, for a library that is not open, link) the symbol.
    Loose(LooseSymbol),
}

/// One entry of the flattened list.
#[derive(Debug, Clone)]
pub(super) enum ListItem {
    Header {
        group: Group,
        count: usize,
    },
    /// A primitive: `file` alone when `name` is `None`, else "file · name".
    Row {
        file: String,
        name: Option<String>,
        choice: Choice,
    },
}

/// Flatten the filtered per-library cache into header + row entries.
pub(super) fn list_items(
    i18n: &signex_i18n::I18n,
    state: &LibraryState,
    picker: &PrimitivePickerState,
    needle: &str,
) -> Vec<ListItem> {
    let mut items = Vec::new();
    push_suggested(i18n, &mut items, state, picker, needle);
    for lib in &state.open_libraries {
        // Read the per-library cache instead of calling `adapter.list_*`
        // every view tick; it is refreshed on open, post-save and after row
        // mutations, so the picker does no per-frame disk IO.
        let summaries: &[PrimitiveSummary] = match picker.kind {
            PrimitiveKind::Symbol => &lib.cached_symbols,
            PrimitiveKind::Footprint => &lib.cached_footprints,
            PrimitiveKind::Sim => &lib.cached_sims,
            _ => &[],
        };
        let rows: Vec<ListItem> = summaries
            .iter()
            .filter(|s| matches(i18n, s, needle))
            .map(|s| library_row(s, picker.kind, PrimitiveRef::new(lib.library_id, s.uuid)))
            .collect();
        push_group(&mut items, Group::Library(lib.display_name.clone()), rows);
    }
    push_loose(i18n, &mut items, state, &picker.loose, needle);
    items
}

fn push_group(items: &mut Vec<ListItem>, group: Group, rows: Vec<ListItem>) {
    if rows.is_empty() {
        return;
    }
    items.push(ListItem::Header {
        group,
        count: rows.len(),
    });
    items.extend(rows);
}

/// "file · name" from the summary's file; "name.ext" when the backend keeps
/// no file.
fn library_row(s: &PrimitiveSummary, kind: PrimitiveKind, pick: PrimitiveRef) -> ListItem {
    let file = s
        .file
        .as_deref()
        .and_then(std::path::Path::file_name)
        .map(|n| n.to_string_lossy().into_owned());
    let (file, name) = match file {
        Some(file) => (file, Some(s.name.clone())),
        None => (format!("{}.{}", s.name, extension(kind)), None),
    };
    ListItem::Row {
        file,
        name,
        choice: Choice::Pick(pick),
    }
}

fn extension(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Symbol => "snxsym",
        PrimitiveKind::Footprint => "snxfpt",
        PrimitiveKind::Sim => "snxsim",
        _ => "snx",
    }
}

/// Whether the filter `needle` takes the primitive: its name or its uuid.
/// The uuid is not shown, but it still finds the primitive.
fn matches(i18n: &signex_i18n::I18n, s: &PrimitiveSummary, needle: &str) -> bool {
    needle.is_empty()
        || contains_ci_in(i18n, &s.name, needle)
        || s.uuid
            .simple()
            .encode_lower(&mut uuid::Uuid::encode_buffer())
            .contains(needle)
}

/// A board footprint swap lists first the footprints the components' own
/// library parts offer (#2366); the full list follows.
fn push_suggested(
    i18n: &signex_i18n::I18n,
    items: &mut Vec<ListItem>,
    state: &LibraryState,
    picker: &PrimitivePickerState,
    needle: &str,
) {
    let PrimitivePickerTarget::BoardFootprint { suggested, .. } = &picker.target else {
        return;
    };
    let rows: Vec<ListItem> = suggested
        .iter()
        .filter_map(|reference| {
            let lib = state
                .open_libraries
                .iter()
                .find(|lib| lib.library_id == reference.library_id)?;
            let s = lib
                .cached_footprints
                .iter()
                .find(|s| s.uuid == reference.uuid)?;
            matches(i18n, s, needle).then(|| library_row(s, picker.kind, *reference))
        })
        .collect();
    push_group(items, Group::Suggested, rows);
}

/// The symbols of files in no open library (#1841): one group per library
/// that holds some but is not open, then one group of the files in no
/// library at all.
fn push_loose(
    i18n: &signex_i18n::I18n,
    items: &mut Vec<ListItem>,
    state: &LibraryState,
    loose: &[LooseSymbol],
    needle: &str,
) {
    // A library opened since the list was read lists its own symbols.
    let loose: Vec<&LooseSymbol> = loose
        .iter()
        .filter(|s| {
            s.library
                .as_deref()
                .is_none_or(|l| state.library_at(l).is_none())
        })
        .collect();
    let taken = |s: &LooseSymbol| needle.is_empty() || contains_ci_in(i18n, &s.name, needle);
    let mut owners: Vec<&std::path::Path> = Vec::new();
    for symbol in &loose {
        if let Some(lib) = symbol.library.as_deref()
            && !owners.contains(&lib)
        {
            owners.push(lib);
        }
    }
    for owner in owners {
        let rows = loose
            .iter()
            .copied()
            .filter(|s| s.library.as_deref() == Some(owner) && taken(s))
            .map(|s| ListItem::Row {
                file: file_name(&s.file),
                name: Some(s.name.clone()),
                choice: Choice::Loose(s.clone()),
            })
            .collect();
        push_group(items, Group::NotOpen(lib_name(owner)), rows);
    }
    // A file in no library shows its whole path; its symbol's name follows
    // only when the file holds several.
    let unowned: Vec<&LooseSymbol> = loose
        .iter()
        .copied()
        .filter(|s| s.library.is_none())
        .collect();
    let rows = unowned
        .iter()
        .copied()
        .filter(|s| taken(s))
        .map(|s| {
            let shared = unowned.iter().filter(|o| o.file == s.file).count() > 1;
            ListItem::Row {
                file: s.file.display().to_string(),
                name: shared.then(|| s.name.clone()),
                choice: Choice::Loose(s.clone()),
            }
        })
        .collect();
    push_group(items, Group::NoLibrary, rows);
}

fn file_name(file: &std::path::Path) -> String {
    file.file_name().map_or_else(
        || file.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// The name a `.snxlib` is known by: its file stem.
pub(super) fn lib_name(snxlib: &std::path::Path) -> String {
    snxlib.file_stem().map_or_else(
        || snxlib.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// Entries to build for `count` list entries at `scroll_y`. The viewport is
/// taken as the whole picker height, which only ever over-builds.
pub(super) fn list_window(scroll_y: f32, count: usize) -> std::ops::Range<usize> {
    visible_range(scroll_y, PICKER_H, ROW_H, count, OVERSCAN_ROWS)
}

/// Case-insensitive substring test against a `needle` already folded by
/// `i18n.fold_for_search`. An all-ASCII `hay` and `needle` skip the
/// allocation under a non-Turkic language (this runs per primitive per
/// frame); anything else folds `hay` the same way as the needle.
pub(super) fn contains_ci_in(i18n: &signex_i18n::I18n, hay: &str, needle: &str) -> bool {
    let plain_ascii = hay.is_ascii() && needle.is_ascii() && i18n.fold_char_for_search('I') == 'i';
    if plain_ascii {
        let n = needle.as_bytes();
        if n.is_empty() {
            return true;
        }
        return hay
            .as_bytes()
            .windows(n.len())
            .any(|w| w.eq_ignore_ascii_case(n));
    }
    i18n.fold_for_search(hay).contains(needle)
}
