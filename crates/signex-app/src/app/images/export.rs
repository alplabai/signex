//! Placed images in a PDF export (#1116, mockup row 204): which files the
//! exported sheets' images name, taken in `update`, and their checked bytes,
//! read on the export's blocking thread into `PdfOptions::images`. A pick
//! not saved yet is taken from memory; a file that is missing, refused or
//! changed is left out, and its image prints as an empty box.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_types::schematic::SchDrawing;

use crate::app::Signex;

/// The image files an export reads, and the ones it already has.
#[derive(Debug, Default)]
pub(crate) struct ExportImageReads {
    /// `(folder, file, hash)`: the file relative to the folder.
    reads: Vec<(PathBuf, String, String)>,
    /// `(file, hash, bytes)` of the picks a save will copy.
    ready: Vec<(String, String, Arc<[u8]>)>,
}

impl ExportImageReads {
    /// `options` with the bytes of every file that passes the checks.
    /// Blocking: it reads the files.
    pub(crate) fn into_options(
        self,
        mut options: signex_output::PdfOptions,
    ) -> signex_output::PdfOptions {
        for (file, hash, bytes) in self.ready {
            options.images.insert(file, hash, bytes);
        }
        for (root, file, hash) in self.reads {
            if let Some(bytes) = read_checked(&root, &file, &hash) {
                options.images.insert(file, hash, bytes);
            }
        }
        options
    }

    /// How many files it reads and has.
    #[cfg(test)]
    pub(crate) fn counts(&self) -> (usize, usize) {
        (self.reads.len(), self.ready.len())
    }
}

fn read_checked(root: &Path, file: &str, hash: &str) -> Option<Arc<[u8]>> {
    super::read::read_placed_image(root, file, hash)
        .ok()
        .map(|(_, bytes)| bytes)
}

impl Signex {
    /// The image files the sheets of `ctx` name, each with the folder it is
    /// relative to; a pick waiting for its save comes with its bytes.
    pub(crate) fn export_image_reads(
        &self,
        ctx: &signex_output::ExportContext,
    ) -> ExportImageReads {
        let mut out = ExportImageReads::default();
        for sheet in &ctx.sheets {
            let root = self.image_root(&sheet.path);
            for drawing in &sheet.schematic.drawings {
                let SchDrawing::Image(image) = drawing else {
                    continue;
                };
                let waiting = self
                    .document_state
                    .image_copies
                    .iter()
                    .find(|copy| copy.file == image.file && copy.hash == image.hash);
                if let Some(copy) = waiting {
                    let entry = (
                        copy.file.clone(),
                        copy.hash.clone(),
                        Arc::clone(&copy.bytes),
                    );
                    if !out
                        .ready
                        .iter()
                        .any(|(f, h, _)| *f == entry.0 && *h == entry.1)
                    {
                        out.ready.push(entry);
                    }
                    continue;
                }
                let read = (root.clone(), image.file.clone(), image.hash.clone());
                if !out.reads.contains(&read) {
                    out.reads.push(read);
                }
            }
        }
        out
    }
}
