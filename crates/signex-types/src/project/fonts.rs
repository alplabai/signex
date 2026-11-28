//! Fonts carried with the project (#1178, mockup row 205): font files copied
//! into the project's `assets/fonts/` folder, so every machine that opens the
//! project has the faces its texts name.
//!
//! The `.snxprj` keeps the top-level `fonts` list: one entry per family, with
//! the asset files that hold its styles, each file's embedding permission and
//! the license the person recorded. Texts still store only a family name.
//!
//! The list is read lazily: it stays in its JSON form in the project's
//! `unknown` bag and is read and written through the methods here, so a
//! section this build cannot read is kept byte for byte and refused for edits,
//! and adding the list changed no `ProjectData` literal. Rationale:
//! `docs/PROJECT_TAB.md`, "Fonts".

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ProjectData, UnknownJson};

/// The `.snxprj` key of the font list.
pub const PROJECT_FONTS_KEY: &str = "fonts";

/// The project folder the font files are copied into.
pub const PROJECT_FONTS_DIR: &str = "assets/fonts";

/// The font file extensions a project takes, lower case. WOFF2 is compressed
/// and the font reader cannot open it, so it is not taken.
pub const PROJECT_FONT_EXTENSIONS: [&str; 3] = ["ttf", "otf", "ttc"];

/// Largest font file a project takes, in bytes: past any text face, CJK
/// collections included; a larger file is refused before it is read.
pub const MAX_PROJECT_FONT_BYTES: u64 = 32 * 1024 * 1024;

/// Longest license a family records, in characters.
pub const MAX_FONT_LICENSE_CHARS: usize = 200;

/// What a font's `OS/2.fsType` lets a document do with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontEmbedding {
    /// No restriction: may be copied and installed.
    Installable,
    /// May be embedded and edited with the document.
    Editable,
    /// May be embedded to view and print, not to edit.
    PreviewAndPrint,
    /// Must not be embedded: never copied into a project.
    Restricted,
}

impl FontEmbedding {
    /// The token the file and the command answers write.
    pub fn token(self) -> &'static str {
        match self {
            Self::Installable => "installable",
            Self::Editable => "editable",
            Self::PreviewAndPrint => "preview_and_print",
            Self::Restricted => "restricted",
        }
    }

    /// Whether the font may be copied into a project.
    pub fn allows_project_copy(self) -> bool {
        self != Self::Restricted
    }

    /// The stricter of two permissions: a collection is as free as its most
    /// restricted face.
    #[must_use]
    pub fn strictest(self, other: Self) -> Self {
        if other.rank() < self.rank() {
            other
        } else {
            self
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Restricted => 0,
            Self::PreviewAndPrint => 1,
            Self::Editable => 2,
            Self::Installable => 3,
        }
    }
}

/// One face a font file holds: the family it belongs to and its style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFace {
    pub family: String,
    /// The style name the font gives (`Regular`, `Bold Italic`); may be empty.
    pub style: String,
}

/// What a font file says about itself, read before it is copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFileInfo {
    /// Every face, in the file's order; a collection has several.
    pub faces: Vec<FontFace>,
    /// The strictest permission of the faces.
    pub embedding: FontEmbedding,
    /// The license URL or license text the font names, shortened; shown so
    /// the person knows what they commit. Never stored by itself.
    pub license_hint: Option<String>,
}

/// One font file of a family, as the project stores it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFontFile {
    /// Relative to the project folder: `assets/fonts/<sha256>.<ext>`.
    pub path: String,
    /// The style the file holds for this family; empty when it names none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style: String,
    pub embedding: FontEmbedding,
    /// Keys written by a newer build; kept on save.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

/// A family carried with the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectFontFamily {
    /// The family name the font gives and texts name.
    pub family: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<ProjectFontFile>,
    /// An SPDX id or the name of a license file beside the fonts, as the
    /// person typed it; `None` while not set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Keys written by a newer build; kept on save.
    #[serde(default, flatten, skip_serializing_if = "UnknownJson::is_empty")]
    pub unknown: UnknownJson,
}

/// Why a font edit was refused. Nothing changed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectFontProblem {
    #[error("the project's font list could not be read; it is kept as it is and cannot be changed")]
    Unreadable,
    #[error("{family} does not allow embedding, so it is not copied into the project")]
    Restricted { family: String },
    #[error("the project has no font {family}")]
    NoSuchFamily { family: String },
    #[error("{path} is not a file of the project's {PROJECT_FONTS_DIR} folder")]
    NotAnAssetPath { path: String },
    #[error("a license is at most {MAX_FONT_LICENSE_CHARS} characters")]
    LicenseTooLong,
    #[error("the font file names no family")]
    NoFamily,
}

/// The project-relative path a font file with this content is stored at.
/// `extension` is lower-cased; the caller has checked it is one of
/// [`PROJECT_FONT_EXTENSIONS`].
pub fn project_font_asset_path(sha256_hex: &str, extension: &str) -> String {
    format!(
        "{PROJECT_FONTS_DIR}/{sha256_hex}.{}",
        extension.to_ascii_lowercase()
    )
}

/// The extension of `file_name` when a project takes it, lower case.
pub fn project_font_extension(file_name: &str) -> Option<&'static str> {
    let (_, extension) = file_name.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    PROJECT_FONT_EXTENSIONS
        .into_iter()
        .find(|known| *known == extension)
}

/// Whether `path` is a font file directly in the project's font folder, as
/// this build writes them. A project arrives from someone else: a stored
/// path that climbs out, is absolute or names another kind of file is never
/// read.
pub fn is_project_font_asset_path(path: &str) -> bool {
    let Some(name) = path
        .strip_prefix(PROJECT_FONTS_DIR)
        .and_then(|rest| rest.strip_prefix('/'))
    else {
        return false;
    };
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    !stem.is_empty()
        && stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && PROJECT_FONT_EXTENSIONS.contains(&extension)
}

impl ProjectData {
    /// The fonts the project carries, in the order they were added.
    /// [`ProjectFontProblem::Unreadable`] when the stored list has a shape
    /// this build cannot read.
    pub fn project_fonts(&self) -> Result<Vec<ProjectFontFamily>, ProjectFontProblem> {
        match self.unknown.get(PROJECT_FONTS_KEY) {
            None => Ok(Vec::new()),
            Some(raw) => {
                serde_json::from_value(raw.clone()).map_err(|_| ProjectFontProblem::Unreadable)
            }
        }
    }

    /// The family `name` names, compared without case.
    pub fn project_font_family(&self, name: &str) -> Option<ProjectFontFamily> {
        let name = name.trim();
        self.project_fonts()
            .ok()?
            .into_iter()
            .find(|entry| entry.family.eq_ignore_ascii_case(name))
    }

    /// The font list as stored, for an undo snapshot.
    pub fn project_fonts_raw(&self) -> Option<&Value> {
        self.unknown.get(PROJECT_FONTS_KEY)
    }

    /// Put back a list taken with [`Self::project_fonts_raw`].
    pub fn restore_project_fonts_raw(&mut self, raw: Option<Value>) {
        match raw {
            Some(raw) => self.unknown.insert(PROJECT_FONTS_KEY.to_string(), raw),
            None => self.unknown.remove(PROJECT_FONTS_KEY),
        }
    }

    /// Store `fonts`; an empty list drops the key, so a project that never
    /// had a font re-saves byte-identical.
    fn store_project_fonts(
        &mut self,
        fonts: &[ProjectFontFamily],
    ) -> Result<(), ProjectFontProblem> {
        if fonts.is_empty() {
            self.unknown.remove(PROJECT_FONTS_KEY);
            return Ok(());
        }
        let raw = serde_json::to_value(fonts).map_err(|_| ProjectFontProblem::Unreadable)?;
        self.unknown.insert(PROJECT_FONTS_KEY.to_string(), raw);
        Ok(())
    }

    /// List the file at `path` under each family of `faces`, a new family
    /// after the others. Returns the families it is listed under, as stored.
    /// A file already listed under a family is not listed twice.
    pub fn add_project_font_file(
        &mut self,
        path: &str,
        faces: &[FontFace],
        embedding: FontEmbedding,
    ) -> Result<Vec<String>, ProjectFontProblem> {
        if !is_project_font_asset_path(path) {
            return Err(ProjectFontProblem::NotAnAssetPath {
                path: path.to_string(),
            });
        }
        let first = faces
            .iter()
            .map(|face| face.family.trim())
            .find(|family| !family.is_empty())
            .ok_or(ProjectFontProblem::NoFamily)?;
        if !embedding.allows_project_copy() {
            return Err(ProjectFontProblem::Restricted {
                family: first.to_string(),
            });
        }
        let mut fonts = self.project_fonts()?;
        let mut touched: Vec<String> = Vec::new();
        for face in faces {
            let family = face.family.trim();
            if family.is_empty() {
                continue;
            }
            let at = match fonts
                .iter()
                .position(|entry| entry.family.eq_ignore_ascii_case(family))
            {
                Some(at) => at,
                None => {
                    fonts.push(ProjectFontFamily {
                        family: family.to_string(),
                        files: Vec::new(),
                        license: None,
                        unknown: UnknownJson::default(),
                    });
                    fonts.len() - 1
                }
            };
            let entry = &mut fonts[at];
            if !entry.files.iter().any(|file| file.path == path) {
                entry.files.push(ProjectFontFile {
                    path: path.to_string(),
                    style: face.style.trim().to_string(),
                    embedding,
                    unknown: UnknownJson::default(),
                });
            }
            if !touched.contains(&entry.family) {
                touched.push(entry.family.clone());
            }
        }
        self.store_project_fonts(&fonts)?;
        Ok(touched)
    }

    /// Drop the family `name` from the list. Its files stay in the folder:
    /// another family, or an undo, may still need them.
    pub fn remove_project_font(&mut self, name: &str) -> Result<(), ProjectFontProblem> {
        let mut fonts = self.project_fonts()?;
        let at = family_index(&fonts, name)?;
        fonts.remove(at);
        self.store_project_fonts(&fonts)
    }

    /// The license of the family `name` as typed; blank clears it.
    pub fn set_project_font_license(
        &mut self,
        name: &str,
        license: &str,
    ) -> Result<(), ProjectFontProblem> {
        let license = license.trim();
        if license.chars().count() > MAX_FONT_LICENSE_CHARS {
            return Err(ProjectFontProblem::LicenseTooLong);
        }
        let mut fonts = self.project_fonts()?;
        let at = family_index(&fonts, name)?;
        fonts[at].license = (!license.is_empty()).then(|| license.to_string());
        self.store_project_fonts(&fonts)
    }
}

/// The place of the family `name` (compared without case) in `fonts`.
fn family_index(fonts: &[ProjectFontFamily], name: &str) -> Result<usize, ProjectFontProblem> {
    let name = name.trim();
    fonts
        .iter()
        .position(|entry| entry.family.eq_ignore_ascii_case(name))
        .ok_or_else(|| ProjectFontProblem::NoSuchFamily {
            family: name.to_string(),
        })
}
