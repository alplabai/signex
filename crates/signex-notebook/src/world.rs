//! The Typst world a notebook compiles in: the notebook's files from
//! [`NotebookSources`], the bundled `@signex/notebook` package and the
//! fonts that ship with Typst. Nothing is read from the disk, the network
//! or the system fonts, so the same sources give the same pages on every
//! machine.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use typst::diag::{FileError, FileResult, PackageError};
use typst::foundations::{Bytes, Datetime, Dict, Duration, Str, Value};
use typst::syntax::package::{PackageSpec, PackageVersion};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

use crate::date::IsoDate;
use crate::sources::NotebookSources;
use crate::syntax::typst_source;

/// The namespace of the bundled package: `@signex/notebook:0.1.0`.
pub const PACKAGE_NAMESPACE: &str = "signex";
/// The name of the bundled package.
pub const PACKAGE_NAME: &str = "notebook";
/// The version of the bundled package, `(major, minor, patch)`.
pub const PACKAGE_VERSION: (u32, u32, u32) = (0, 1, 0);
/// The import line a new notebook starts with.
pub const PACKAGE_IMPORT: &str = "#import \"@signex/notebook:0.1.0\": *";

/// The package's files, served by the world as written in the crate.
const PACKAGE_FILES: [(&str, &str); 4] = [
    ("typst.toml", include_str!("../typst/notebook/typst.toml")),
    ("lib.typ", include_str!("../typst/notebook/lib.typ")),
    ("blocks.typ", include_str!("../typst/notebook/blocks.typ")),
    ("measure.typ", include_str!("../typst/notebook/measure.typ")),
];

/// The package file that holds the Python cell results of one compile,
/// written by `cell::PageCells::to_json`.
const CELLS_FILE: &str = "cells.json";

/// The extensions read as Typst source.
const SOURCE_EXTENSIONS: [&str; 2] = [crate::NOTEBOOK_EXTENSION, "typ"];

/// The bundled fonts, parsed once. Immutable, so sharing them between
/// compiles is safe.
struct FontSet {
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

static FONTS: LazyLock<FontSet> = LazyLock::new(|| {
    let fonts: Vec<Font> = typst_assets::fonts()
        .flat_map(|data| Font::iter(Bytes::new(data)))
        .collect();
    FontSet {
        book: LazyHash::new(FontBook::from_fonts(&fonts)),
        fonts,
    }
});

/// The standard library, built once and never changed.
static LIBRARY: LazyLock<LazyHash<Library>> =
    LazyLock::new(|| LazyHash::new(<Library as LibraryExt>::default()));

/// One compile's view of the notebook's files.
pub(crate) struct NotebookWorld {
    main: FileId,
    sources: HashMap<FileId, Source>,
    files: HashMap<FileId, Bytes>,
    today: Option<Datetime>,
    /// The library with this compile's `sys.inputs`; `None` uses the shared
    /// one, which has none.
    library: Option<LazyHash<Library>>,
}

impl NotebookWorld {
    /// The world of `notebook`; `today` answers Typst's
    /// `datetime.today()` (none: it is an error, so no page depends on the
    /// clock). `cells` is the package's `cells.json`, the Python cell
    /// results the page prints, and `inputs` are `sys.inputs`. `None` when
    /// the notebook's path cannot name a Typst file.
    pub(crate) fn new(
        notebook: &NotebookSources,
        today: Option<IsoDate>,
        cells: Vec<u8>,
        inputs: &[(String, String)],
    ) -> Option<Self> {
        let library = (!inputs.is_empty()).then(|| {
            let inputs: Dict = inputs
                .iter()
                .map(|(key, text)| {
                    (
                        Str::from(key.as_str()),
                        Value::Str(Str::from(text.as_str())),
                    )
                })
                .collect();
            LazyHash::new(
                <Library as LibraryExt>::builder()
                    .with_inputs(inputs)
                    .build(),
            )
        });
        let mut world = Self {
            main: project_id(notebook.main())?,
            sources: HashMap::new(),
            files: HashMap::new(),
            today: today.and_then(|d| Datetime::from_ymd(d.year(), d.month(), d.day())),
            library,
        };
        for path in notebook.paths() {
            let (Some(id), Some(bytes)) = (project_id(path), notebook.get(path)) else {
                continue;
            };
            world.add(id, path, bytes.to_vec());
        }
        for (path, text) in PACKAGE_FILES {
            if let Some(id) = package_id(path) {
                world.add(id, path, text.as_bytes().to_vec());
            }
        }
        // What the package's #attachment prints (`crate::derived`).
        for (path, bytes) in crate::derived::package_files(notebook) {
            if let Some(id) = package_id(&path) {
                world.add(id, &path, bytes);
            }
        }
        if let Some(id) = package_id(CELLS_FILE) {
            world.add(id, CELLS_FILE, cells);
        }
        Some(world)
    }

    fn add(&mut self, id: FileId, path: &str, bytes: Vec<u8>) {
        let extension = path.rsplit_once('.').map_or("", |(_, ext)| ext);
        if !SOURCE_EXTENSIONS.contains(&extension) {
            self.files.insert(id, Bytes::new(bytes));
            return;
        }
        // Not UTF-8: held as bytes, so `source` reports it as not Typst.
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => {
                self.files.insert(id, Bytes::new(error.into_bytes()));
                return;
            }
        };
        let text = if extension == crate::NOTEBOOK_EXTENSION {
            let typst = typst_source(&text);
            if id != self.main && typst.starts_with('\n') {
                // A chapter is evaluated in its own scope by `#include`:
                // it reads the package on its first header line, which is
                // blank, so every line keeps its number.
                format!("{PACKAGE_IMPORT}{typst}")
            } else {
                typst
            }
        } else {
            text
        };
        self.files.insert(id, Bytes::from_string(text.clone()));
        self.sources.insert(id, Source::new(id, text));
    }

    /// The path a diagnostic names for `id`: the path inside the notebook
    /// folder, or `@signex/notebook/lib.typ` for the package.
    pub(crate) fn path_of(id: FileId) -> String {
        let path = id.vpath().get_without_slash();
        match id.root() {
            VirtualRoot::Project => path.to_string(),
            VirtualRoot::Package(spec) => format!("@{}/{}/{path}", spec.namespace, spec.name),
        }
    }

    fn missing(&self, id: FileId) -> FileError {
        match id.root() {
            VirtualRoot::Package(spec) if !is_bundled(spec) => {
                FileError::Package(PackageError::NotFound(spec.clone()))
            }
            VirtualRoot::Package(_) | VirtualRoot::Project => {
                FileError::NotFound(PathBuf::from(id.vpath().get_without_slash()))
            }
        }
    }
}

impl World for NotebookWorld {
    fn library(&self) -> &LazyHash<Library> {
        self.library.as_ref().unwrap_or(&*LIBRARY)
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &FONTS.book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        match self.sources.get(&id) {
            Some(source) => Ok(source.clone()),
            None if self.files.contains_key(&id) => Err(FileError::NotSource),
            None => Err(self.missing(id)),
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.files.get(&id).cloned().ok_or_else(|| self.missing(id))
    }

    fn font(&self, index: usize) -> Option<Font> {
        FONTS.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        self.today
    }
}

/// The spec of the bundled package.
fn bundled_spec() -> PackageSpec {
    let (major, minor, patch) = PACKAGE_VERSION;
    PackageSpec {
        namespace: PACKAGE_NAMESPACE.into(),
        name: PACKAGE_NAME.into(),
        version: PackageVersion {
            major,
            minor,
            patch,
        },
    }
}

fn is_bundled(spec: &PackageSpec) -> bool {
    *spec == bundled_spec()
}

fn project_id(path: &str) -> Option<FileId> {
    let vpath = VirtualPath::new(format!("/{path}")).ok()?;
    Some(RootedPath::new(VirtualRoot::Project, vpath).intern())
}

fn package_id(path: &str) -> Option<FileId> {
    let vpath = VirtualPath::new(format!("/{path}")).ok()?;
    Some(RootedPath::new(VirtualRoot::Package(bundled_spec()), vpath).intern())
}
