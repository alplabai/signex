//! The pictures of the placed images the canvas draws (#1116), by file and
//! content hash. A file is read and checked in a `Task`; `view` and `update`
//! only look entries up. The renderer decodes a handle once and keeps it, so
//! an entry is the decoded image for every sheet that shows it.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use iced::advanced::{image, svg};
use signex_i18n::{ids, t};
use signex_types::image_asset::{ImageFormat, ImageReadError};
use signex_types::schematic::{SchDrawing, SchImage};

use super::ImagesMsg;
use crate::app::{Message, Signex};

/// Which picture an image shows: its file and the content it placed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageKey {
    pub file: String,
    pub hash: String,
}

impl ImageKey {
    pub(crate) fn of(image: &SchImage) -> Self {
        Self {
            file: image.file.clone(),
            hash: image.hash.clone(),
        }
    }
}

/// What the canvas has for one picture.
#[derive(Debug, Clone)]
pub enum ImageEntry {
    /// Being read; nothing is drawn but the image's border.
    Loading,
    Raster(image::Handle),
    Vector(svg::Handle),
    /// Missing, refused or changed: the placeholder.
    Missing,
}

/// The pictures read so far.
#[derive(Debug, Default)]
pub struct ImageCache {
    entries: HashMap<ImageKey, ImageEntry>,
    /// Sheets whose missing images Messages already named.
    warned: HashSet<PathBuf>,
}

impl ImageCache {
    pub(crate) fn entry(&self, key: &ImageKey) -> Option<&ImageEntry> {
        self.entries.get(key)
    }

    /// The handle for `bytes` of `format`; the renderer decodes it.
    fn put(&mut self, key: ImageKey, format: ImageFormat, bytes: &Arc<[u8]>) {
        let entry = if format.is_vector() {
            ImageEntry::Vector(svg::Handle::from_memory(bytes.to_vec()))
        } else {
            ImageEntry::Raster(image::Handle::from_bytes(bytes.to_vec()))
        };
        self.entries.insert(key, entry);
    }

    /// Whether Messages already named the missing images of `sheet`.
    #[cfg(test)]
    pub(crate) fn has_warned(&self, sheet: &Path) -> bool {
        self.warned.contains(sheet)
    }

    /// Forget the missing pictures, so the next look reads their files again
    /// (a copy into the project landed, or a file was put back).
    pub(crate) fn retry_missing(&mut self) {
        self.entries
            .retain(|_, entry| !matches!(entry, ImageEntry::Missing));
        self.warned.clear();
    }
}

impl Signex {
    /// The folder a sheet's image paths are relative to: its project's, or
    /// for a sheet in no project, its own.
    pub(crate) fn image_root(&self, sheet: &Path) -> PathBuf {
        match self.document_state.document_project(sheet) {
            Some(project) => project.dir().to_path_buf(),
            None => sheet.parent().map(Path::to_path_buf).unwrap_or_default(),
        }
    }

    /// Start reading the pictures of the active sheet's images the cache does
    /// not have. A picture still waiting to be copied into the project is
    /// taken from the pick, with no read.
    pub(crate) fn request_image_loads(&mut self) -> Task<Message> {
        let Some(sheet) = self.document_state.active_path.clone() else {
            return Task::none();
        };
        let Some(engine) = self.document_state.engines.get(&sheet) else {
            return Task::none();
        };
        let wanted: Vec<ImageKey> = engine
            .document()
            .drawings
            .iter()
            .filter_map(|d| match d {
                SchDrawing::Image(image) => Some(ImageKey::of(image)),
                _ => None,
            })
            .filter(|key| self.ui_state.image_cache.entry(key).is_none())
            .collect();
        if wanted.is_empty() {
            return Task::none();
        }
        let root = self.image_root(&sheet);
        let mut tasks = Vec::new();
        for key in wanted {
            let pending = self
                .document_state
                .image_copies
                .iter()
                .find(|copy| copy.file == key.file && copy.hash == key.hash)
                .map(|copy| Arc::clone(&copy.bytes));
            if let Some(bytes) = pending {
                let format = signex_types::image_asset::sniff_format(&bytes);
                match format {
                    Some(format) => self.ui_state.image_cache.put(key, format, &bytes),
                    None => {
                        self.ui_state
                            .image_cache
                            .entries
                            .insert(key, ImageEntry::Missing);
                    }
                }
                continue;
            }
            self.ui_state
                .image_cache
                .entries
                .insert(key.clone(), ImageEntry::Loading);
            let (root, sheet) = (root.clone(), sheet.clone());
            tasks.push(Task::perform(
                async move {
                    let (file, hash) = (key.file.clone(), key.hash.clone());
                    let read = tokio::task::spawn_blocking(move || {
                        super::read::read_placed_image(&root, &file, &hash)
                    })
                    .await
                    .unwrap_or_else(|error| Err(ImageReadError::Unreadable(error.to_string())));
                    (key, sheet, read)
                },
                |(key, sheet, read)| ImagesMsg::PictureRead { key, sheet, read }.into(),
            ));
        }
        Task::batch(tasks)
    }

    /// A picture was read: keep it, or keep the placeholder and say once per
    /// sheet that its file is missing. A read for an entry that is no longer
    /// waiting is dropped.
    pub(super) fn picture_read(
        &mut self,
        key: ImageKey,
        sheet: &Path,
        read: Result<(ImageFormat, Arc<[u8]>), ImageReadError>,
    ) {
        let cache = &mut self.ui_state.image_cache;
        if !matches!(cache.entries.get(&key), Some(ImageEntry::Loading)) {
            return;
        }
        match read {
            Ok((format, bytes)) => cache.put(key, format, &bytes),
            Err(_) => {
                let first = cache.warned.insert(sheet.to_path_buf());
                cache.entries.insert(key.clone(), ImageEntry::Missing);
                if first {
                    let i18n = &self.ui_state.i18n;
                    let name = sheet.file_name().map_or_else(
                        || sheet.display().to_string(),
                        |name| name.to_string_lossy().into_owned(),
                    );
                    crate::diagnostics::log_warning(
                        t!(i18n, ids::IMAGES_MISSING, "sheet" => name, "file" => key.file)
                            .into_string(),
                    );
                }
            }
        }
    }
}
