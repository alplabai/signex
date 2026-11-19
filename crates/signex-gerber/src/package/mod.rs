//! A package: the files read together (a folder, a zip or a pick of files),
//! each image named, paired with its far-side partner and put in the
//! default drawing order. Rules: `docs/GERBER_VIEWER.md`, "Package".

mod naming;
mod order;

use crate::{FileFunction, Image, JobFile, ReadFile, ReadProblem};

pub use naming::label_from_file_name;

/// Where a layer's name comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameSource {
    /// The file's own X2 file function.
    X2,
    /// The job file's entry for the file.
    JobFile,
    /// The file name: the file carries no file function and no job file
    /// lists it.
    FileName,
}

/// One image of a package.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageLayer {
    pub image: Image,
    /// What the layer is; `None` draws it as a drawing, not compared.
    pub function: Option<FileFunction>,
    pub name_source: NameSource,
    /// The name made from the file name ("fab-notes.gbr" is "Fab notes")
    /// when `name_source` is [`NameSource::FileName`]. It is the user's
    /// own file name, not interface text.
    pub file_label: Option<String>,
    /// Index of the far-side partner (Top and Bottom legend, mask and
    /// paste; copper Lk and L(n+1-k)); `None` for none.
    pub pair: Option<usize>,
}

/// The files read together.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Package {
    /// The images in the default drawing order: index 0 draws on top.
    pub layers: Vec<PackageLayer>,
    /// The job file; with several, the first one given.
    pub job: Option<JobFile>,
    /// The files that could not be read at all, in the order given.
    pub not_read: Vec<ReadProblem>,
}

/// The package of `files`, in the order the caller read them. Nothing is
/// dropped: an unreadable file lands in [`Package::not_read`], an image
/// with no known function is kept as a drawing.
pub fn assemble(files: Vec<ReadFile>) -> Package {
    let mut images = Vec::new();
    let mut job = None;
    let mut not_read = Vec::new();
    for file in files {
        match file {
            ReadFile::Image(image) => images.push(image),
            ReadFile::Job(read) => {
                job.get_or_insert(read);
            }
            ReadFile::NotRead { problem, .. } => not_read.push(problem),
        }
    }
    let named = naming::name_images(images, job.as_ref());
    let layers = order::in_default_order(named, job.as_ref());
    Package {
        layers: order::with_pairs(layers, job.as_ref()),
        job,
        not_read,
    }
}

impl Package {
    /// Every read problem: the files not read in the order given, then each
    /// layer's, by file name and line.
    pub fn problems(&self) -> Vec<&ReadProblem> {
        let mut of_layers: Vec<&ReadProblem> = self
            .layers
            .iter()
            .flat_map(|layer| layer.image.problems.iter())
            .collect();
        of_layers.sort_by(|a, b| (&a.at.file, a.at.line).cmp(&(&b.at.file, b.at.line)));
        self.not_read.iter().chain(of_layers).collect()
    }

    /// The default drawing order, index 0 on top: the order
    /// [`Package::layers`] is kept in, which Reset order goes back to.
    pub fn default_order(&self) -> Vec<usize> {
        (0..self.layers.len()).collect()
    }

    /// Whether any object carries an X2 net, so Highlight net can work.
    pub fn has_net_attributes(&self) -> bool {
        self.layers
            .iter()
            .flat_map(|layer| layer.image.objects.iter())
            .any(|object| object.attributes.net.is_some())
    }
}
