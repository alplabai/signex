//! The file work of placed images (#1116). Every function here blocks on
//! the disk: run it off the UI thread. The rules a file must meet are
//! `signex_types::image_asset`'s; this module only reads and writes.

use std::path::Path;
use std::sync::Arc;

use signex_types::image_asset::{
    self, AssetChoice, IMAGE_ASSET_DIR, ImageFormat, ImageReadError, ImageRefusal, MAX_IMAGE_BYTES,
    PendingImageCopy, PickedImage,
};

use crate::app::handlers::canvas::escapes_through_symlink;

/// The bytes of `path`, refused over [`MAX_IMAGE_BYTES`] before they are
/// read.
fn read_bounded(path: &Path) -> Result<Vec<u8>, ImageReadError> {
    let unreadable = |error: std::io::Error| ImageReadError::Unreadable(error.to_string());
    let size = std::fs::metadata(path).map_err(unreadable)?.len();
    if size > MAX_IMAGE_BYTES {
        return Err(ImageReadError::Refused(ImageRefusal::TooLarge {
            bytes: size,
            limit: MAX_IMAGE_BYTES,
        }));
    }
    std::fs::read(path).map_err(unreadable)
}

/// The files already in `root`'s images folder as `(name, hash)`. Only a
/// file of `size` bytes can hold the same content, so only those are
/// hashed; the others are listed with no hash, which keeps their names
/// taken.
fn assets_on_disk(root: &Path, size: u64) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(root.join(IMAGE_ASSET_DIR)) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let same_size = entry
                .metadata()
                .is_ok_and(|meta| meta.is_file() && meta.len() == size);
            let hash = if same_size {
                std::fs::read(entry.path())
                    .map(|bytes| image_asset::content_hash(&bytes))
                    .unwrap_or_default()
            } else {
                String::new()
            };
            Some((name, hash))
        })
        .collect()
}

/// The project-relative path of `path` when it lies in `root` (with links
/// resolved), else `None`.
fn inside_project(path: &Path, root: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    if escapes_through_symlink(root, path) {
        return None;
    }
    image_asset::checked_asset_path(relative.to_str()?).ok()
}

/// Read and check the picked file `path` for a sheet of the project at
/// `root`. A file in the project is placed where it is; any other gets a
/// name under `assets/images/` (`pending` are the names, with their hashes,
/// that picks not saved yet already took) and is copied there by the save.
pub(crate) fn read_picked_image(
    path: &Path,
    root: &Path,
    pending: &[(String, String)],
) -> Result<PickedImage, ImageReadError> {
    let bytes = read_bounded(path)?;
    let facts = image_asset::check_image(&bytes).map_err(ImageReadError::Refused)?;
    let picked_name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let bytes: Arc<[u8]> = bytes.into();
    if let Some(file) = inside_project(path, root) {
        return Ok(PickedImage {
            facts,
            bytes,
            file,
            root: root.to_path_buf(),
            needs_copy: false,
            picked_name,
        });
    }
    let on_disk = assets_on_disk(root, facts.bytes);
    let mut taken = on_disk.clone();
    taken.extend(pending.iter().cloned());
    let wanted = image_asset::asset_file_name(&picked_name, facts.format);
    let AssetChoice {
        file,
        reuses_existing,
    } = image_asset::pick_asset_name(&wanted, &facts.hash, &taken);
    let already_there = reuses_existing
        && on_disk
            .iter()
            .any(|(name, hash)| *hash == facts.hash && file == format!("{IMAGE_ASSET_DIR}/{name}"));
    Ok(PickedImage {
        facts,
        bytes,
        file,
        root: root.to_path_buf(),
        needs_copy: !already_there,
        picked_name,
    })
}

/// Read a placed image's file `file` of the project at `root` to draw it:
/// the path must stay in the project, the file must pass the checks and
/// hold the content `hash` names.
pub(crate) fn read_placed_image(
    root: &Path,
    file: &str,
    hash: &str,
) -> Result<(ImageFormat, Arc<[u8]>), ImageReadError> {
    let relative = image_asset::checked_asset_path(file).map_err(ImageReadError::Refused)?;
    let path = root.join(relative);
    if escapes_through_symlink(root, &path) {
        return Err(ImageReadError::Refused(ImageRefusal::LeavesProject(
            file.to_string(),
        )));
    }
    let bytes = read_bounded(&path)?;
    let facts = image_asset::check_image(&bytes).map_err(ImageReadError::Refused)?;
    if facts.hash != hash {
        return Err(ImageReadError::Changed);
    }
    Ok((facts.format, bytes.into()))
}

/// The files directly in `root`'s images folder, project-relative.
pub(crate) fn list_asset_folder(root: &Path) -> std::collections::BTreeSet<String> {
    let Ok(entries) = std::fs::read_dir(root.join(IMAGE_ASSET_DIR)) else {
        return Default::default();
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.'))
        .map(|name| format!("{IMAGE_ASSET_DIR}/{name}"))
        .collect()
}

/// What the file `file` of the project at `root` is, by the checks; `None`
/// when it is not there, leaves the project or fails a check.
pub(crate) fn read_asset_facts(root: &Path, file: &str) -> Option<image_asset::ImageFacts> {
    let relative = image_asset::checked_asset_path(file).ok()?;
    let path = root.join(relative);
    if escapes_through_symlink(root, &path) {
        return None;
    }
    let bytes = read_bounded(&path).ok()?;
    image_asset::check_image(&bytes).ok()
}

/// Why one copy into the project was not written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyFailure {
    /// Another file is already at the path: it is never replaced.
    Exists,
    Refused(ImageRefusal),
    Io(String),
}

/// Write `copy` into its project: a file already there with the same
/// content counts as written, one with other content is left alone.
pub(crate) fn write_image_copy(copy: &PendingImageCopy) -> Result<(), CopyFailure> {
    let relative = image_asset::checked_asset_path(&copy.file).map_err(CopyFailure::Refused)?;
    let target = copy.root.join(relative);
    if escapes_through_symlink(&copy.root, &target) {
        return Err(CopyFailure::Refused(ImageRefusal::LeavesProject(
            copy.file.clone(),
        )));
    }
    match std::fs::read(&target) {
        Ok(existing) if image_asset::content_hash(&existing) == copy.hash => Ok(()),
        Ok(_) => Err(CopyFailure::Exists),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            signex_types::atomic_io::atomic_write(&target, &copy.bytes)
                .map_err(|error| CopyFailure::Io(error.to_string()))
        }
        Err(error) => Err(CopyFailure::Io(error.to_string())),
    }
}
