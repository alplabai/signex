//! A project's image assets as the Projects tree, the status bar, ERC and
//! `list_image_assets` see them (#1116, mockup row 204): every file under
//! `assets/images/` and every file an image on a sheet names, with how many
//! images use it and whether a use is missing. Pure: the app reads the disk.
//! See docs/SCHEMATIC_IMAGES.md, "Missing files".

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;

use crate::image_asset::ImageFacts;

/// A file the scan looked at, with what its checks said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedAsset {
    /// Project-relative, `/` separated.
    pub file: String,
    /// `None` when the file is not there or fails the checks.
    pub facts: Option<ImageFacts>,
}

/// One placed image: the sheet it is on and the file and content it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageUse {
    pub sheet: PathBuf,
    pub image: uuid::Uuid,
    pub file: String,
    pub hash: String,
}

/// One file of the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetEntry {
    pub file: String,
    /// `png`, `jpeg`, `gif` or `svg`; `None` when the file is not readable.
    pub format: Option<String>,
    pub bytes: Option<u64>,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
    /// How many placed images name the file.
    pub uses: usize,
    /// An image names it and it is gone, refused, or holds other content.
    pub missing: bool,
    /// A pick not saved yet: the next save copies it into the project.
    pub pending: bool,
    /// The sheets whose images name it, each once.
    pub sheets: Vec<PathBuf>,
}

/// Every asset of one project, by file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssetReport {
    pub entries: Vec<AssetEntry>,
}

impl AssetReport {
    /// How many files an image names and cannot show.
    pub fn missing_count(&self) -> usize {
        self.entries.iter().filter(|e| e.missing).count()
    }

    /// The entry of `file`.
    pub fn entry(&self, file: &str) -> Option<&AssetEntry> {
        self.entries.iter().find(|e| e.file == file)
    }

    /// Whether an image naming `file` cannot show it.
    pub fn is_missing(&self, file: &str) -> bool {
        self.entry(file).is_some_and(|e| e.missing)
    }
}

/// The report of `scanned` files used by `uses`; `pending` are the
/// `(file, hash)` picks a save will copy, which count as there.
pub fn asset_report(
    scanned: &[ScannedAsset],
    uses: &[ImageUse],
    pending: &[(String, String)],
) -> AssetReport {
    let blank = |file: &str| AssetEntry {
        file: file.to_string(),
        format: None,
        bytes: None,
        width_px: None,
        height_px: None,
        uses: 0,
        missing: false,
        pending: false,
        sheets: Vec::new(),
    };
    let mut by_file: BTreeMap<String, (AssetEntry, Option<&ImageFacts>)> = BTreeMap::new();
    for asset in scanned {
        let mut entry = blank(&asset.file);
        if let Some(facts) = &asset.facts {
            entry.format = Some(facts.format.as_str().to_string());
            entry.bytes = Some(facts.bytes);
            entry.width_px = Some(facts.width_px);
            entry.height_px = Some(facts.height_px);
        }
        by_file.insert(asset.file.clone(), (entry, asset.facts.as_ref()));
    }
    for used in uses {
        let (entry, facts) = by_file
            .entry(used.file.clone())
            .or_insert_with(|| (blank(&used.file), None));
        entry.uses += 1;
        if !entry.sheets.contains(&used.sheet) {
            entry.sheets.push(used.sheet.clone());
        }
        let waiting = pending
            .iter()
            .any(|(file, hash)| *file == used.file && *hash == used.hash);
        if waiting {
            entry.pending = true;
            continue;
        }
        if !(*facts).is_some_and(|f| f.hash == used.hash) {
            entry.missing = true;
        }
    }
    AssetReport {
        entries: by_file.into_values().map(|(entry, _)| entry).collect(),
    }
}

#[cfg(test)]
#[path = "image_asset_report_tests.rs"]
mod tests;
