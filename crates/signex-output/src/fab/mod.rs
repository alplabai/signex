//! Fabrication outputs (#2388): Gerber RS-274X images with X2 attributes,
//! Excellon drill files and their table, the pick-and-place CSV, the
//! IPC-D-356 test netlist, the IPC-2581 file (#2484), the ODB++ job
//! (#2485) and a job file listing them.
//!
//! [`generate`] is pure: it reads the board and returns the files' bytes,
//! and the app writes them only when the user presses Generate. One board
//! gives the same bytes every time. Frame, units and formats:
//! `docs/FABRICATION_OUTPUTS.md`.
//!
//! Inputs the board does not hold as items come through [`FabExtras`]: the
//! zones' copper ([`FabExtras::of_board`] computes the pour fills, #2216)
//! and extra strokes. Without the fills the files leave zones out and
//! [`FabSet::notes`] says so. Text is written in the board stroke font
//! (#2368).

mod centroid;
mod collect;
mod documents;
mod draw;
mod drill;
mod extras;
mod frame;
mod gerber;
mod ipc2581;
mod ipc356;
mod job;
mod layers;
mod naming;
mod marks;
mod notes;
mod odb;
mod parts;
mod roles;
mod text;

#[cfg(test)]
mod tests;

use pcb_geom::PointNm;
use signex_types::pcb::{PCB_DEFAULT_THICKNESS_MM, PcbBoard, Point};
use signex_types::pcb_layer::PcbLayer;
use signex_types::zone_keepout::KEEPOUT_FILL;
use thiserror::Error;

pub use notes::FabNote;

use layers::{CopperStack, FabLayer, file_function, file_polarity, file_suffix};
use notes::Tally;

/// Which files to generate, and the name every file starts with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabOptions {
    /// The file-name stem: `<name>-copper-l1-top.gbr`, `<name>-pth.drl`, ...
    pub name: String,
    pub gerber: bool,
    pub drill: bool,
    pub centroid: bool,
    pub test_netlist: bool,
    /// The IPC-2581 file: the whole job in one file (#2484).
    pub ipc2581: bool,
    /// The ODB++ job (#2485), and whether it is a folder or one archive;
    /// `None` writes none.
    pub odb: Option<OdbForm>,
}

/// How the ODB++ job is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OdbForm {
    /// A folder `<name>-odb/` holding the archive's tree, the job folder
    /// named by the job name; each file's name is its path under the
    /// output folder, `/` separated.
    Directory,
    /// One uncompressed tar archive, `<name>-odb.tar`.
    Archive,
}

impl FabOptions {
    /// Every output, named `name`; the ODB++ job as one archive.
    pub fn all(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            gerber: true,
            drill: true,
            centroid: true,
            test_netlist: true,
            ipc2581: true,
            odb: Some(OdbForm::Archive),
        }
    }

    fn any(&self) -> bool {
        self.gerber
            || self.drill
            || self.centroid
            || self.test_netlist
            || self.ipc2581
            || self.odb.is_some()
    }
}

/// A filled copper area, from zone filling (#2216). Board mm, model frame.
#[derive(Debug, Clone, PartialEq)]
pub struct FilledRegion {
    pub layer: PcbLayer,
    /// The net's name, empty for none.
    pub net_name: String,
    pub outline: Vec<Point>,
    pub holes: Vec<Vec<Point>>,
}

/// An open polyline drawn with a round pen, such as a stroked text glyph
/// from board text drawing (#2387). Board mm, model frame.
#[derive(Debug, Clone, PartialEq)]
pub struct FabStroke {
    pub layer: PcbLayer,
    pub width_mm: f64,
    pub points: Vec<Point>,
}

/// What the board model does not hold yet but the images draw.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FabExtras {
    pub zone_fills: Vec<FilledRegion>,
    pub strokes: Vec<FabStroke>,
}

/// What a file holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FabFileKind {
    /// A Gerber image, with its X2 file function and polarity.
    Gerber {
        function: String,
        polarity: &'static str,
    },
    /// An Excellon drill file, with its X2 file function and polarity.
    Drill {
        function: String,
        polarity: &'static str,
    },
    DrillTable,
    Centroid,
    TestNetlist,
    /// The IPC-2581 file (#2484).
    Ipc2581,
    /// The ODB++ archive, or one file of the ODB++ folder (#2485).
    Odb,
    Job,
}

impl FabFileKind {
    /// The job file's word for a file that is not an image or a drill file.
    fn content(&self) -> &'static str {
        match self {
            FabFileKind::DrillTable => "DrillTable",
            FabFileKind::Centroid => "PickAndPlace",
            FabFileKind::TestNetlist => "TestNetlist",
            FabFileKind::Ipc2581 => "IPC2581",
            FabFileKind::Odb => "ODB++",
            FabFileKind::Gerber { .. } | FabFileKind::Drill { .. } | FabFileKind::Job => "",
        }
    }
}

/// One generated file: its name in the output folder and its bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabFile {
    pub name: String,
    pub kind: FabFileKind,
    pub contents: String,
}

/// Everything one generation produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FabSet {
    /// Images top to bottom, then drill, table, centroid, test netlist,
    /// IPC-2581, ODB++, job.
    pub files: Vec<FabFile>,
    pub notes: Vec<FabNote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FabError {
    #[error("no output is selected")]
    NothingSelected,
    #[error("the file name is empty or holds a character a file name cannot")]
    BadName,
    #[error("the board has no outline")]
    NoOutline,
}

/// Characters a file-name stem may not hold on any platform.
const NAME_FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// The file-name stem `name` trimmed, when it can name files and `board`
/// has an outline: the checks every output of a generation shares.
pub fn check_job<'a>(board: &PcbBoard, name: &'a str) -> Result<&'a str, FabError> {
    let name = name.trim();
    if name.is_empty() || name.contains(NAME_FORBIDDEN) || name.chars().any(char::is_control) {
        return Err(FabError::BadName);
    }
    if board.outline.len() < 3 {
        return Err(FabError::NoOutline);
    }
    Ok(name)
}

/// The fabrication files of `board`.
pub fn generate(
    board: &PcbBoard,
    extras: &FabExtras,
    options: &FabOptions,
) -> Result<FabSet, FabError> {
    if !options.any() {
        return Err(FabError::NothingSelected);
    }
    let name = check_job(board, &options.name)?;
    let stack = CopperStack::of(board, collect::used_copper(board, extras).into_iter());
    let mut tally = Tally::default();
    let mut skipped = 0;
    let mut files = Vec::new();
    if options.gerber {
        gerber_files(board, extras, &stack, name, &mut tally, &mut files);
        skipped = skipped.max(tally.skipped);
    }
    if options.drill {
        let mut drill_skipped = 0;
        drill_files(board, &stack, name, &mut drill_skipped, &mut files);
        skipped = skipped.max(drill_skipped);
    }
    if options.centroid {
        let mut centroid_skipped = 0;
        files.push(FabFile {
            name: format!("{name}-centroid.csv"),
            kind: FabFileKind::Centroid,
            contents: centroid::write_centroid(
                board,
                &mut tally.unnamed_parts,
                &mut centroid_skipped,
            ),
        });
        skipped = skipped.max(centroid_skipped);
    }
    if options.test_netlist {
        let mut ipc_skipped = 0;
        files.push(FabFile {
            name: format!("{name}-netlist.ipc"),
            kind: FabFileKind::TestNetlist,
            contents: ipc356::write_ipc356(board, &stack, name, &mut ipc_skipped),
        });
        skipped = skipped.max(ipc_skipped);
    }
    if options.ipc2581 {
        let mut own = Tally::default();
        files.push(FabFile {
            name: format!("{name}-ipc2581.xml"),
            kind: FabFileKind::Ipc2581,
            contents: ipc2581::write_ipc2581(board, extras, &stack, name, &mut own),
        });
        skipped = skipped.max(own.skipped);
    }
    if let Some(form) = options.odb {
        let mut own = Tally::default();
        let job = odb::write_odb(board, extras, &stack, name, &mut own);
        odb_files(form, name, job, &mut files);
        skipped = skipped.max(own.skipped);
    }
    let facts = job::JobFacts {
        name: name.to_string(),
        extent: outline_extent(board),
        copper_layers: stack.count(),
        thickness_mm: if board.thickness > 0.0 {
            board.thickness
        } else {
            PCB_DEFAULT_THICKNESS_MM
        },
    };
    let job = job::write_job(&facts, &files);
    files.push(FabFile {
        name: format!("{name}-job.gbrjob"),
        kind: FabFileKind::Job,
        contents: job,
    });
    tally.skipped = skipped;
    // A keepout (#2373) is a zone on a copper layer with no copper to fill.
    let copper_zones = board
        .zones
        .iter()
        .filter(|z| z.fill_type != KEEPOUT_FILL)
        .filter(|z| z.pcb_layer().is_some_and(PcbLayer::is_copper))
        .count();
    let pours = options.gerber || options.ipc2581 || options.odb.is_some();
    let zones_not_filled = if pours && extras.zone_fills.is_empty() {
        copper_zones
    } else {
        0
    };
    Ok(FabSet {
        files,
        notes: tally.notes(zones_not_filled),
    })
}

fn gerber_files(
    board: &PcbBoard,
    extras: &FabExtras,
    stack: &CopperStack,
    name: &str,
    tally: &mut Tally,
    files: &mut Vec<FabFile>,
) {
    let mut images = collect::collect(board, extras, stack, tally);
    for copper in stack.layers() {
        images.entry(FabLayer::Copper(*copper)).or_default();
    }
    for (layer, marks) in &images {
        files.push(FabFile {
            name: format!("{name}-{}.gbr", file_suffix(*layer, stack)),
            kind: FabFileKind::Gerber {
                function: file_function(*layer, stack),
                polarity: file_polarity(*layer),
            },
            contents: gerber::write_image(*layer, marks, stack),
        });
    }
}

fn drill_files(
    board: &PcbBoard,
    stack: &CopperStack,
    name: &str,
    skipped: &mut usize,
    files: &mut Vec<FabFile>,
) {
    let holes = drill::collect_holes(board, stack, skipped);
    let layers = stack.count();
    let drill_files = drill::drill_files(&holes, layers);
    for drill_file in &drill_files {
        files.push(FabFile {
            name: format!("{name}-{}.drl", drill_file.suffix(layers)),
            kind: FabFileKind::Drill {
                function: drill_file.function(layers),
                polarity: "Positive",
            },
            contents: drill::write_drill(&holes, *drill_file, layers),
        });
    }
    if !holes.is_empty() {
        files.push(FabFile {
            name: format!("{name}-drill-table.csv"),
            kind: FabFileKind::DrillTable,
            contents: drill::write_table(&holes, &drill_files, layers),
        });
    }
}

/// The ODB++ job as one archive, or as one file per job file under
/// `<name>-odb/<job>/`: the archive's tree, whose job folder is the job
/// name a reader takes, an entity name.
fn odb_files(form: OdbForm, name: &str, job: Vec<odb::JobFile>, files: &mut Vec<FabFile>) {
    let job_name = odb::job_name(name);
    match form {
        OdbForm::Archive => files.push(FabFile {
            name: format!("{name}-odb.tar"),
            kind: FabFileKind::Odb,
            contents: odb::archive(&job_name, &job),
        }),
        OdbForm::Directory => files.extend(job.into_iter().map(|(path, contents)| FabFile {
            name: format!("{name}-odb/{job_name}/{path}"),
            kind: FabFileKind::Odb,
            contents,
        })),
    }
}

/// The outline's bounding box in the fabrication frame; `None` when a
/// vertex is not representable.
fn outline_extent(board: &PcbBoard) -> Option<(PointNm, PointNm)> {
    let fab_frame = frame::FabFrame::of(board);
    let points = board
        .outline
        .iter()
        .map(|p| fab_frame.of_mm(*p, "outline"))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let min = PointNm::new(
        points.iter().map(|p| p.x).min()?,
        points.iter().map(|p| p.y).min()?,
    );
    let max = PointNm::new(
        points.iter().map(|p| p.x).max()?,
        points.iter().map(|p| p.y).max()?,
    );
    Some((min, max))
}
