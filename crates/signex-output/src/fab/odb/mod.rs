//! The ODB++ job (#2485): the board as an ODB++ 7.0 job tree in mm, one
//! step `pcb` holding the profile, every layer's features, the drills and
//! slots with their tools, the placed components and the nets. Each
//! matrix layer's type comes from `fab::roles`, the table IPC-2581
//! shares. Written as a folder or as one tar archive. Format and choices:
//! `docs/FABRICATION_OUTPUTS.md`, "ODB++".

mod eda;
mod features;
mod matrix;
mod tar;
mod text;

use pcb_geom::Side;
use signex_types::pcb::{PcbBoard, Point};

use super::FabExtras;
use super::collect;
use super::documents::{board_marks, collect_documents, used_layers};
use super::drill::{self, DrillFile, Hole};
use super::frame::FabFrame;
use super::layers::{CopperStack, FabLayer, Face};
use super::marks::Mark;
use super::notes::Tally;
use super::parts::Parts;
use super::roles::board_stack;
use eda::{CopperFeatures, Eda};
use features::{Features, round_symbol};
use matrix::{MatrixLayer, MatrixPlan, Source, component_layer, write_matrix};
use text::{entity_name, um};

pub(crate) use tar::archive;
#[cfg(test)]
pub(crate) use tar::tests::read_archive;

/// The one step's name.
const STEP: &str = "pcb";
/// The info file's dates: a real date would make two generations of one
/// board differ, so none is recorded.
const NOT_RECORDED: &str = "19700101.000000";

/// One file of the job: its path under the job folder and its text.
pub(crate) type JobFile = (String, String);

/// The job's folder name inside an archive: `name` as an entity name.
pub(crate) fn job_name(name: &str) -> String {
    entity_name(name)
}

/// The ODB++ job of `board`, its files sorted by path. Items the geometry
/// refuses are counted in `tally`, as the other files count them.
pub(crate) fn write_odb(
    board: &PcbBoard,
    extras: &FabExtras,
    copper: &CopperStack,
    name: &str,
    tally: &mut Tally,
) -> Vec<JobFile> {
    let frame = FabFrame::of(board);
    let images = collect::collect(board, extras, copper, tally);
    let documents = collect_documents(board, frame);
    let holes = drill::collect_holes(board, copper, &mut tally.skipped);
    let drills = drill::drill_files(&holes, copper.count());
    let stack = board_stack(board);
    let parts = Parts::of(board, text::word);
    let plan = MatrixPlan {
        board,
        copper,
        stack: &stack,
        used: used_layers(board, &images, &documents),
        parts_on: [Side::Top, Side::Bottom].map(|side| {
            parts
                .placed
                .iter()
                .any(|(f, package)| package.is_some() && Side::of_layer(&f.layer) == side)
        }),
        v_score: images.contains_key(&FabLayer::VScore),
        drills: &drills,
    };
    let layers = plan.declare();
    let step = format!("steps/{STEP}");
    let mut files: Vec<JobFile> = vec![
        ("matrix/matrix".into(), write_matrix(&layers, STEP)),
        ("misc/info".into(), info(&job_name(name))),
        (format!("{step}/stephdr"), STEPHDR.into()),
        (
            format!("{step}/profile"),
            profile(board, frame, &mut tally.skipped),
        ),
    ];
    let mut copper_features = Vec::new();
    for layer in &layers {
        let dir = format!("{step}/layers/{}", layer.name);
        let marks: &[Mark] = match layer.source {
            Source::Board(board_layer) => board_marks(&images, &documents, board_layer),
            Source::VScore => images.get(&FabLayer::VScore).map_or(&[][..], Vec::as_slice),
            Source::Dielectric => &[],
            Source::Drill(file) => {
                files.push((format!("{dir}/features"), drill_features(file, &holes)));
                files.push((format!("{dir}/tools"), tools(file, &holes)));
                continue;
            }
            Source::Components(_) => continue,
        };
        let mut features = Features::default();
        let numbered: Vec<(&Mark, std::ops::Range<usize>)> =
            marks.iter().map(|m| (m, features.mark(m))).collect();
        if matches!(layer.source, Source::Board(l) if l.is_copper()) {
            copper_features.push(CopperFeatures {
                name: layer.name.clone(),
                marks: numbered,
            });
        }
        files.push((format!("{dir}/features"), features.finish()));
    }
    let eda = Eda::of(&parts, frame, &copper_features, &mut tally.skipped);
    files.push((format!("{step}/eda/data"), eda.data()));
    for face in component_faces(&layers) {
        files.push((
            format!("{step}/layers/{}/components", component_layer(face)),
            eda.components(face),
        ));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn component_faces(layers: &[MatrixLayer]) -> Vec<Face> {
    layers
        .iter()
        .filter_map(|l| match l.source {
            Source::Components(face) => Some(face),
            _ => None,
        })
        .collect()
}

/// The `misc/info` file.
fn info(job: &str) -> String {
    format!(
        "UNITS=MM\nJOB_NAME={job}\nODB_VERSION_MAJOR=7\nODB_VERSION_MINOR=0\n\
         ODB_SOURCE=Signex\nCREATION_DATE={NOT_RECORDED}\nSAVE_DATE={NOT_RECORDED}\n\
         SAVE_APP=Signex\nSAVE_USER=\n"
    )
}

/// The step header: datum and origin at the fabrication frame's origin.
const STEPHDR: &str = "UNITS=MM\nX_DATUM=0\nY_DATUM=0\nX_ORIGIN=0\nY_ORIGIN=0\n\
                       TOP_ACTIVE=0\nBOTTOM_ACTIVE=0\nRIGHT_ACTIVE=0\nLEFT_ACTIVE=0\n\
                       AFFECTING_BOM=\nAFFECTING_BOM_CHANGED=0\n";

/// The step profile: one surface, the board outline with every through
/// cutout of the board sketch as a hole. A ring the geometry refuses is
/// counted in `skipped`.
fn profile(board: &PcbBoard, frame: FabFrame, skipped: &mut usize) -> String {
    let ring = |points: &[Point]| {
        points
            .iter()
            .map(|p| frame.of_mm(*p, "profile"))
            .collect::<Result<Vec<_>, _>>()
    };
    let mut features = Features::default();
    let Ok(outline) = ring(&board.outline) else {
        *skipped += 1;
        return features.finish();
    };
    let mut cutouts = Vec::new();
    let through = board
        .sketch
        .iter()
        .flat_map(|s| s.cutouts.iter())
        .filter(|c| c.through);
    for cutout in through {
        match ring(&cutout.boundary) {
            Ok(points) => cutouts.push(points),
            Err(_) => *skipped += 1,
        }
    }
    features.surface(&outline, &cutouts);
    features.finish()
}

/// A drill layer's features: a pad per hole, a line per slot, each of the
/// round symbol of its tool.
fn drill_features(file: DrillFile, holes: &[Hole]) -> String {
    let mut features = Features::default();
    for hole in holes.iter().filter(|hole| file.holds(hole)) {
        match hole.slot_to {
            Some(to) => features.line(hole.at, to, hole.diameter),
            None => features.pad(hole.at, round_symbol(hole.diameter), 0, None),
        }
    }
    features.finish()
}

/// A drill layer's `tools` file: one tool per size and kind (hole or
/// slot), smallest first, sizes in micrometres.
fn tools(file: DrillFile, holes: &[Hole]) -> String {
    let mut sizes: Vec<(i64, bool)> = holes
        .iter()
        .filter(|hole| file.holds(hole))
        .map(|hole| (hole.diameter, hole.slot_to.is_some()))
        .collect();
    sizes.sort_unstable();
    sizes.dedup();
    let plating = if file.plated { "PLATED" } else { "NON_PLATED" };
    let mut out = String::from("UNITS=MM\nTHICKNESS=0\nUSER_PARAMS=\n");
    for (i, (diameter, slot)) in sizes.into_iter().enumerate() {
        let size = um(diameter);
        let shape = if slot { "SLOT" } else { "HOLE" };
        out.push_str(&format!(
            "TOOLS {{\n   NUM={}\n   TYPE={plating}\n   TYPE2=STANDARD\n   \
             FINISH_SIZE={size}\n   DRILL_SIZE={size}\n   MIN_TOL=0\n   MAX_TOL=0\n   \
             BIT=\n   SHAPE={shape}\n}}\n",
            i + 1
        ));
    }
    out
}
