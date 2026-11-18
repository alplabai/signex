//! The job file: what a generation wrote, in the Gerber job JSON shape
//! (`.gbrjob`). No creation date, so one board gives the same bytes.

use pcb_geom::PointNm;
use serde_json::{Value, json};

use super::{FabFile, FabFileKind};

/// The general board facts the job file states.
pub(crate) struct JobFacts {
    pub name: String,
    /// The board outline's bounding box in the fabrication frame.
    pub extent: Option<(PointNm, PointNm)>,
    pub copper_layers: usize,
    pub thickness_mm: f64,
}

pub(crate) fn write_job(facts: &JobFacts, files: &[FabFile]) -> String {
    let mut attributes = Vec::new();
    let mut others = Vec::new();
    for file in files {
        match &file.kind {
            FabFileKind::Gerber { function, polarity }
            | FabFileKind::Drill { function, polarity } => {
                attributes.push(json!({
                    "Path": file.name,
                    "FileFunction": function,
                    "FilePolarity": polarity,
                }));
            }
            FabFileKind::DrillTable
            | FabFileKind::Centroid
            | FabFileKind::TestNetlist
            | FabFileKind::Ipc2581 => {
                others.push(json!({ "Path": file.name, "Content": file.kind.content() }));
            }
            // The ODB++ folder is listed once, by its name.
            FabFileKind::Odb => {
                let path = file.name.split('/').next().unwrap_or(&file.name);
                let listed = others.iter().any(|o| o["Path"] == path);
                if !listed {
                    others.push(json!({ "Path": path, "Content": file.kind.content() }));
                }
            }
            FabFileKind::Job => {}
        }
    }
    let size = facts.extent.map(|(min, max)| {
        json!({
            "X": nm_as_mm(max.x - min.x),
            "Y": nm_as_mm(max.y - min.y),
        })
    });
    let mut specs = serde_json::Map::new();
    specs.insert("ProjectId".into(), json!({ "Name": facts.name }));
    if let Some(size) = size {
        specs.insert("Size".into(), size);
    }
    specs.insert("LayerNumber".into(), json!(facts.copper_layers));
    specs.insert("BoardThickness".into(), json!(facts.thickness_mm));
    let job = json!({
        "Header": {
            "GenerationSoftware": { "Vendor": "Alp Lab", "Application": "Signex" },
        },
        "GeneralSpecs": Value::Object(specs),
        "FilesAttributes": attributes,
        "OtherFiles": others,
    });
    // `{:#}` is the pretty form, and printing a `Value` cannot fail.
    format!("{job:#}\n")
}

fn nm_as_mm(nm: i64) -> f64 {
    nm as f64 / 1.0e6
}
