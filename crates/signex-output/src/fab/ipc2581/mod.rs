//! The IPC-2581 revision C file (#2484): the whole job in one XML file,
//! with the stackup, every layer's features, the drills and slots, the
//! packages and placed parts, the BOM and the nets. Each layer's function
//! comes from `fab::roles`, the table the ODB++ export shares. Format and
//! choices: `docs/FABRICATION_OUTPUTS.md`, "IPC-2581".

mod features;
mod parts;
mod stack;
mod xml;

use pcb_geom::PointNm;
use signex_types::format::parse_pcb_layer;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::PcbLayer;

use super::collect::{self, Images};
use super::documents::collect_documents;
use super::drill::{self, DrillFile, Hole};
use super::frame::{FabFrame, millionths};
use super::layers::{CopperStack, FabLayer};
use super::marks::Mark;
use super::notes::Tally;
use super::roles::board_stack;
use super::FabExtras;
use features::{Dictionary, contour, layer_feature, xy};
use parts::{Parts, write_nets};
use stack::{Declared, LayerPlan, Source, image_layer, write_layers};
use xml::{Xml, qualified};

/// The schema's namespace.
const NAMESPACE: &str = "http://webstds.ipc.org/2581";
/// The history record's dates. The schema requires them, and a real date
/// would make two generations of one board differ, so none is recorded.
const NOT_RECORDED: &str = "1970-01-01T00:00:00Z";

/// The IPC-2581 file of `board`, its step named after `name`. Items the
/// geometry refuses are counted in `tally`, as the other files count them.
pub(crate) fn write_ipc2581(
    board: &PcbBoard,
    extras: &FabExtras,
    copper: &CopperStack,
    name: &str,
    tally: &mut Tally,
) -> String {
    let frame = FabFrame::of(board);
    let images = collect::collect(board, extras, copper, tally);
    let documents = collect_documents(board, frame);
    let holes = drill::collect_holes(board, copper, &mut tally.skipped);
    let drills = drill::drill_files(&holes, copper.count());
    let stack = board_stack(board);
    let plan = LayerPlan {
        board,
        copper,
        stack: &stack,
        used: used_layers(board, &images, &documents),
        v_score: images.contains_key(&FabLayer::VScore),
        drills: &drills,
    };
    let declared = plan.declare();
    let step = qualified(name);
    let bom = format!("{step}_BOM");
    let parts = Parts::of(board);
    let mut dictionary = Dictionary::default();

    // The CAD data first: it fills the dictionary the content lists.
    let mut ecad = Xml::fragment(1);
    ecad.open("Ecad", &[("name", step.clone())]);
    ecad.leaf("CadHeader", &[("units", "MILLIMETER".into())]);
    ecad.open("CadData", &[]);
    write_layers(&mut ecad, &declared);
    plan.write_stackup(&mut ecad, &declared);
    ecad.open("Step", &[("name", step.clone())]);
    ecad.leaf("Datum", &xy(PointNm::new(0, 0)));
    profile(&mut ecad, board, frame, &mut tally.skipped);
    parts.write_step_parts(&mut ecad, &mut dictionary, frame, &mut tally.skipped);
    let copper_marks = images
        .iter()
        .filter(|(layer, _)| layer.is_copper())
        .flat_map(|(_, marks)| marks);
    write_nets(&mut ecad, copper_marks);
    let mut slots = 0;
    for layer in &declared {
        match layer.source {
            Source::Board(board_layer) => {
                let marks = board_marks(&images, &documents, board_layer);
                layer_feature(&mut ecad, &mut dictionary, &layer.name, marks);
            }
            Source::VScore => {
                let marks = images.get(&FabLayer::VScore).map_or(&[][..], Vec::as_slice);
                layer_feature(&mut ecad, &mut dictionary, &layer.name, marks);
            }
            Source::Drill(file) => drill_feature(&mut ecad, layer, file, &holes, &mut slots),
            Source::Dielectric(_) => {}
        }
    }

    let mut doc = Xml::document();
    doc.open(
        "IPC-2581",
        &[("revision", "C".into()), ("xmlns", NAMESPACE.into())],
    );
    doc.open("Content", &[("roleRef", "Owner".into())]);
    doc.leaf("FunctionMode", &[("mode", "USERDEF".into())]);
    doc.leaf("StepRef", &[("name", step.clone())]);
    for layer in &declared {
        doc.leaf("LayerRef", &[("name", layer.name.clone())]);
    }
    if !parts.is_empty() {
        doc.leaf("BomRef", &[("name", bom.clone())]);
    }
    dictionary.write(&mut doc);
    doc.close();
    logistics(&mut doc);
    if !parts.is_empty() {
        parts.write_bom(&mut doc, &bom, &step);
    }
    debug_assert_eq!(doc.depth(), 1, "the CAD data is spliced under the root");
    doc.splice(ecad);
    doc.finish()
}

/// The board layers that carry marks or that the layer table lists.
fn used_layers(
    board: &PcbBoard,
    images: &Images,
    documents: &[(PcbLayer, Vec<Mark>)],
) -> Vec<PcbLayer> {
    images
        .keys()
        .filter_map(|image| image_layer(*image))
        .chain(documents.iter().map(|(layer, _)| *layer))
        .chain(board.layers.iter().filter_map(|def| parse_pcb_layer(&def.name)))
        .collect()
}

/// The marks drawn on a board layer: its image's, or its drawings'.
fn board_marks<'a>(
    images: &'a Images,
    documents: &'a [(PcbLayer, Vec<Mark>)],
    layer: PcbLayer,
) -> &'a [Mark] {
    let image = images
        .iter()
        .find(|(image, _)| image_layer(**image) == Some(layer))
        .map(|(_, marks)| marks.as_slice());
    let drawing = || {
        documents
            .iter()
            .find(|(l, _)| *l == layer)
            .map(|(_, marks)| marks.as_slice())
    };
    image.or_else(drawing).unwrap_or(&[])
}

/// The header sections: who sent the file and with what. No person or
/// date is recorded.
fn logistics(doc: &mut Xml) {
    doc.open("LogisticHeader", &[]);
    doc.leaf(
        "Role",
        &[("id", "Owner".into()), ("roleFunction", "SENDER".into())],
    );
    doc.leaf("Enterprise", &[("id", "Signex".into()), ("code", "NONE".into())]);
    doc.leaf(
        "Person",
        &[
            ("name", "Signex".into()),
            ("enterpriseRef", "Signex".into()),
            ("roleRef", "Owner".into()),
        ],
    );
    doc.close();
    doc.open(
        "HistoryRecord",
        &[
            ("number", "1".into()),
            ("origination", NOT_RECORDED.into()),
            ("software", "Signex".into()),
            ("lastChange", NOT_RECORDED.into()),
        ],
    );
    doc.open(
        "FileRevision",
        &[("fileRevisionId", "1".into()), ("comment", String::new())],
    );
    doc.open(
        "SoftwarePackage",
        &[
            ("name", "Signex".into()),
            ("vendor", "Alp Lab".into()),
            ("revision", "1".into()),
        ],
    );
    doc.leaf("Certification", &[("certificationStatus", "SELFTEST".into())]);
    doc.close();
    doc.close();
    doc.close();
}

/// The step profile: the board outline and every through cutout of the
/// board sketch. A ring the geometry refuses is counted in `skipped`.
fn profile(xml: &mut Xml, board: &PcbBoard, frame: FabFrame, skipped: &mut usize) {
    let ring = |points: &[signex_types::pcb::Point]| {
        points
            .iter()
            .map(|p| frame.of_mm(*p, "profile"))
            .collect::<Result<Vec<_>, _>>()
    };
    let Ok(outline) = ring(&board.outline) else {
        *skipped += 1;
        return;
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
    contour(xml, "Profile", &outline, &cutouts);
}

/// One drill layer's holes and slots.
fn drill_feature(
    xml: &mut Xml,
    layer: &Declared,
    file: DrillFile,
    holes: &[Hole],
    slots: &mut usize,
) {
    let held: Vec<&Hole> = holes.iter().filter(|hole| file.holds(hole)).collect();
    if held.is_empty() {
        return;
    }
    let plating = if file.plated { "PLATED" } else { "NONPLATED" };
    xml.open("LayerFeature", &[("layerRef", layer.name.clone())]);
    xml.open("Set", &[("plate", file.plated.to_string())]);
    for (i, hole) in held.into_iter().enumerate() {
        let zero = || "0".to_string();
        let Some(to) = hole.slot_to else {
            let [x, y] = xy(hole.at);
            xml.leaf(
                "Hole",
                &[
                    ("name", format!("H{}", i + 1)),
                    ("diameter", millionths(hole.diameter)),
                    ("platingStatus", plating.into()),
                    ("plusTol", zero()),
                    ("minusTol", zero()),
                    x,
                    y,
                ],
            );
            continue;
        };
        *slots += 1;
        xml.open(
            "SlotCavity",
            &[
                ("name", format!("S{slots}")),
                ("platingStatus", plating.into()),
                ("plusTol", zero()),
                ("minusTol", zero()),
            ],
        );
        let center = PointNm::new((hole.at.x + to.x) / 2, (hole.at.y + to.y) / 2);
        let (dx, dy) = ((to.x - hole.at.x) as f64, (to.y - hole.at.y) as f64);
        let length = dx.hypot(dy).round() as i64;
        let udeg = (dy.atan2(dx).to_degrees().rem_euclid(360.0) * 1.0e6).round() as i64
            % 360_000_000;
        xml.leaf("Location", &xy(center));
        if udeg != 0 {
            xml.leaf("Xform", &[("rotation", millionths(udeg))]);
        }
        xml.leaf(
            "Oval",
            &[
                ("width", millionths(length + hole.diameter)),
                ("height", millionths(hole.diameter)),
            ],
        );
        xml.close();
    }
    xml.close();
    xml.close();
}
