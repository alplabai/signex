//! The Project tab's Fonts edits (#1178, mockup row 205): list a font file
//! that was copied into `assets/fonts/`, drop a family, set a family's
//! license. Each is one step on the project's General history; the checks
//! that refuse one live in `signex-types`. Copying the file is the app's IO;
//! this only changes the list.

use signex_types::project::{FontEmbedding, FontFace, ProjectData, ProjectFontProblem};

/// One edit of the project's font list. A family is named without case.
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectFontCommand {
    /// List the file at `path` (already in the font folder) under each
    /// family of `faces`.
    AddFile {
        path: String,
        faces: Vec<FontFace>,
        embedding: FontEmbedding,
    },
    Remove {
        family: String,
    },
    /// The license as typed; blank clears it.
    SetLicense {
        family: String,
        license: String,
    },
}

impl ProjectFontCommand {
    /// Apply to `data`. A refused edit changes nothing.
    pub fn apply(&self, data: &mut ProjectData) -> Result<(), ProjectFontProblem> {
        match self {
            Self::AddFile {
                path,
                faces,
                embedding,
            } => data
                .add_project_font_file(path, faces, *embedding)
                .map(|_| ()),
            Self::Remove { family } => data.remove_project_font(family),
            Self::SetLicense { family, license } => data.set_project_font_license(family, license),
        }
    }
}
