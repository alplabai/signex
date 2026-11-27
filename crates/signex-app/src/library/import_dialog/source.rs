//! Step 1, source and options: which converter reads the picked files,
//! what the file picker offers, and what keeps Next off. Pure: no disk, no
//! process; the handlers feed it what the pickers and workers answered.

use std::path::{Path, PathBuf};

use super::{ImportDialogState, SourceKind};
use crate::library_import::snapshot::default_library_dir_in;
use crate::library_import::{
    DiscoveredConverter, ImportJob, InputKind, InputKindClass, LibraryRunRequest, ModelsMode,
    PickerFilter, RunLimits, RunMode,
};

/// Why Next is off, the first that applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Blocker {
    /// The converter search is still running.
    Looking,
    /// No library converter is installed.
    NotInstalled,
    NoSource,
    /// No installed converter reads this file.
    Unreadable(PathBuf),
    /// Each file has a converter, but no one converter reads them all.
    NoCommonConverter,
    InvalidName,
    NoLocation,
    LocationNotAbsolute,
    /// The folder check found a problem; the state holds its words.
    TargetRefused,
    /// The folder check behind Next is running.
    Checking,
    /// A dry run left by Back or Close is still stopping.
    RunStopping,
}

/// Whether an input kind belongs on the `kind` side of the picker.
fn is_kind(input: &InputKind, kind: SourceKind) -> bool {
    match kind {
        SourceKind::Database => input.kind == InputKindClass::Database,
        SourceKind::Files => input.kind != InputKindClass::Database,
    }
}

/// A name a library folder can have: the rule `default_library_dir_in`
/// applies, on a root that is never written.
pub fn is_valid_name(name: &str) -> bool {
    default_library_dir_in(Path::new("/"), name).is_some()
}

impl ImportDialogState {
    fn converters(&self) -> &[DiscoveredConverter] {
        self.discovery
            .as_ref()
            .map_or(&[], |d| d.converters.as_slice())
    }

    /// Whether any installed converter reads `kind` of input; the picker
    /// shows the Files / Database choice only when both are offered.
    pub fn offers(&self, kind: SourceKind) -> bool {
        self.converters()
            .iter()
            .flat_map(|c| &c.description.inputs)
            .any(|input| is_kind(input, kind))
    }

    /// The picker's filters for the current kind, one per input kind
    /// across the converters, the first of each label kept.
    pub fn picker_filters(&self) -> Vec<PickerFilter> {
        let mut filters: Vec<PickerFilter> = Vec::new();
        let inputs = self
            .converters()
            .iter()
            .flat_map(|c| &c.description.inputs)
            .filter(|input| is_kind(input, self.source_kind) && !input.extensions.is_empty());
        for input in inputs {
            match filters.iter_mut().find(|f| f.label == input.label) {
                Some(filter) => {
                    for ext in &input.extensions {
                        if !filter.extensions.contains(ext) {
                            filter.extensions.push(ext.clone());
                        }
                    }
                }
                None => filters.push(PickerFilter {
                    label: input.label.clone(),
                    extensions: input.extensions.clone(),
                }),
            }
        }
        filters
    }

    /// The converters that read every source, in discovery order.
    pub fn readers(&self) -> Vec<&DiscoveredConverter> {
        if self.sources.is_empty() {
            return Vec::new();
        }
        self.converters()
            .iter()
            .filter(|c| {
                self.sources
                    .iter()
                    .all(|s| c.description.input_for(s).is_some())
            })
            .collect()
    }

    /// The converter that will run: the one picked, else the first reader.
    pub fn chosen_converter(&self) -> Option<&DiscoveredConverter> {
        let readers = self.readers();
        readers
            .iter()
            .find(|c| Some(&c.path) == self.converter.as_ref())
            .or_else(|| readers.first())
            .copied()
    }

    /// The first source no installed converter reads.
    pub fn unreadable_source(&self) -> Option<&Path> {
        self.sources
            .iter()
            .find(|s| {
                !self
                    .converters()
                    .iter()
                    .any(|c| c.description.input_for(s).is_some())
            })
            .map(PathBuf::as_path)
    }

    /// The kinds the chosen converter detected in the sources, each once,
    /// in source order: the Detected chips.
    pub fn detected_kinds(&self) -> Vec<&InputKind> {
        let Some(converter) = self.chosen_converter() else {
            return Vec::new();
        };
        let mut kinds: Vec<&InputKind> = Vec::new();
        for source in &self.sources {
            if let Some(kind) = converter.description.input_for(source)
                && !kinds.iter().any(|k| k.label == kind.label)
            {
                kinds.push(kind);
            }
        }
        kinds
    }

    /// A database input was detected: the connection override is shown.
    pub fn has_database_input(&self) -> bool {
        self.detected_kinds()
            .iter()
            .any(|k| k.kind == InputKindClass::Database)
    }

    /// Why Next is off, or `None` when it is on.
    pub fn blocker(&self) -> Option<Blocker> {
        let Some(discovery) = &self.discovery else {
            return Some(Blocker::Looking);
        };
        if discovery.converters.is_empty() {
            return Some(Blocker::NotInstalled);
        }
        if self.sources.is_empty() {
            return Some(Blocker::NoSource);
        }
        if let Some(path) = self.unreadable_source() {
            return Some(Blocker::Unreadable(path.to_path_buf()));
        }
        if self.chosen_converter().is_none() {
            return Some(Blocker::NoCommonConverter);
        }
        if !is_valid_name(&self.name) {
            return Some(Blocker::InvalidName);
        }
        let location = self.location.trim();
        if location.is_empty() {
            return Some(Blocker::NoLocation);
        }
        if !Path::new(location).is_absolute() {
            return Some(Blocker::LocationNotAbsolute);
        }
        if self.target_problem.is_some() {
            return Some(Blocker::TargetRefused);
        }
        if self.checking_target {
            return Some(Blocker::Checking);
        }
        if self.run_stopping {
            return Some(Blocker::RunStopping);
        }
        None
    }

    /// The picked sources; the name follows the first one's stem until it
    /// is typed.
    pub fn apply_sources(&mut self, paths: Vec<PathBuf>, root: Option<&Path>) {
        self.sources = paths;
        self.converter = None;
        self.target_problem = None;
        let stem = self
            .sources
            .first()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned());
        if let Some(stem) = stem
            && (self.name.trim().is_empty() || !self.name_edited)
        {
            self.set_name(stem, root);
        }
    }

    /// A new name; the folder follows it under `root` until it is changed
    /// by hand (`~/Signex/Libraries/<name>` by default).
    pub fn set_name(&mut self, name: String, root: Option<&Path>) {
        self.name = name;
        self.target_problem = None;
        if self.location_edited {
            return;
        }
        self.location = root
            .and_then(|root| default_library_dir_in(root, &self.name))
            .map(|dir| dir.display().to_string())
            .unwrap_or_default();
    }

    /// A folder typed or picked by hand; it stops following the name.
    pub fn set_location(&mut self, location: String) {
        self.location = location;
        self.location_edited = true;
        self.target_problem = None;
    }

    /// The `library` run for `mode` into `output`, once Next is on (the
    /// folder check aside).
    pub fn request(&self, mode: RunMode, output: PathBuf) -> Option<LibraryRunRequest> {
        self.chosen_converter()?;
        let database = self.database.trim();
        Some(LibraryRunRequest {
            inputs: self.sources.clone(),
            output,
            name: Some(self.name.trim().to_string()),
            db: (self.has_database_input() && !database.is_empty()).then(|| database.to_string()),
            search_paths: Vec::new(),
            models: if self.include_models {
                ModelsMode::Include
            } else {
                ModelsMode::Skip
            },
            mode,
        })
    }

    /// The job for `mode` into `output`.
    pub fn job(&self, mode: RunMode, output: PathBuf) -> Option<ImportJob> {
        let converter = self.chosen_converter()?;
        Some(ImportJob {
            command: converter.command.clone(),
            converter: converter.description.converter.clone(),
            converter_version: converter.description.version.clone(),
            request: self.request(mode, output)?,
            limits: RunLimits::default(),
            group: Default::default(),
        })
    }

    /// The folder the import writes to (step 1's Location).
    pub fn location_path(&self) -> PathBuf {
        PathBuf::from(self.location.trim())
    }
}
