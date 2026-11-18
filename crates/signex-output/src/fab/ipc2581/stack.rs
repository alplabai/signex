//! The layers an IPC-2581 file declares, top to bottom through the stack
//! first, and the `Stackup` that gives their thicknesses.

use pcb_geom::mm_to_nm;
use signex_types::format::pcb_layer_token;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::PcbLayer;
use signex_types::pcb_stack::LayerStack;

use super::xml::{Xml, qualified};
use crate::fab::drill::DrillFile;
use crate::fab::frame::millionths;
use crate::fab::layers::CopperStack;
use crate::fab::roles::{LayerPlace, LayerRole, StackRow, layer_label, plane_layers, stack_rows};

/// What a declared layer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Board(PcbLayer),
    /// The `k`-th dielectric row of the stack, from 1.
    Dielectric(usize),
    VScore,
    Drill(DrillFile),
}

/// One `Layer` of the file.
#[derive(Debug, Clone)]
pub(crate) struct Declared {
    pub name: String,
    pub source: Source,
    function: &'static str,
    side: &'static str,
    /// The copper layer names a drill span runs between.
    span: Option<(String, String)>,
}

/// Which layers the file declares, given what carries marks.
pub(crate) struct LayerPlan<'a> {
    pub board: &'a PcbBoard,
    pub copper: &'a CopperStack,
    pub stack: &'a LayerStack,
    /// Board layers that carry marks or are in the layer table.
    pub used: Vec<PcbLayer>,
    pub v_score: bool,
    pub drills: &'a [DrillFile],
}

impl LayerPlan<'_> {
    /// The declared layers: the stack's rows top to bottom, copper the
    /// stack does not list, every other used layer in `PcbLayer::all`
    /// order, the V-score layer, then one drill layer per drilled span.
    pub(crate) fn declare(&self) -> Vec<Declared> {
        let planes = plane_layers(self.stack);
        let mut out: Vec<Declared> = Vec::new();
        let add = |out: &mut Vec<Declared>, layer: PcbLayer| {
            if out.iter().any(|d| d.source == Source::Board(layer)) {
                return;
            }
            let plane = layer.copper().is_some_and(|c| planes.contains(&c));
            let name = unique(out, qualified(&layer_label(self.board, layer)), layer);
            out.push(Declared {
                name,
                source: Source::Board(layer),
                function: LayerRole::of(layer, plane).ipc2581(),
                side: side_word(LayerPlace::of(layer)),
                span: None,
            });
        };
        let mut dielectrics = 0;
        for (row, _) in stack_rows(self.stack) {
            match row {
                StackRow::Layer(layer) => add(&mut out, layer),
                StackRow::Dielectric => {
                    dielectrics += 1;
                    out.push(Declared {
                        name: format!("Dielectric{dielectrics}"),
                        source: Source::Dielectric(dielectrics),
                        function: "DIELCORE",
                        side: "INTERNAL",
                        span: None,
                    });
                }
            }
        }
        for copper in self.copper.layers() {
            add(&mut out, PcbLayer::Copper(*copper));
        }
        for layer in PcbLayer::all().filter(|l| self.used.contains(l)) {
            add(&mut out, layer);
        }
        if self.v_score {
            out.push(Declared {
                name: "VScore".into(),
                source: Source::VScore,
                function: "V_CUT",
                side: "ALL",
                span: None,
            });
        }
        let count = self.copper.count();
        for file in self.drills {
            let copper_name = |n: usize| {
                let layer = self.copper.layers()[n - 1];
                qualified(&pcb_layer_token(PcbLayer::Copper(layer)))
            };
            let (from, to) = (file.span.from, file.span.to);
            out.push(Declared {
                name: format!(
                    "Drill_{}",
                    file.suffix(count).to_uppercase().replace('-', "_")
                ),
                source: Source::Drill(*file),
                function: LayerRole::Drill.ipc2581(),
                side: match (from == 1, to == count) {
                    (true, true) => "ALL",
                    (true, false) => "TOP",
                    (false, true) => "BOTTOM",
                    (false, false) => "INTERNAL",
                },
                span: Some((copper_name(from), copper_name(to))),
            });
        }
        out
    }

    /// The `Stackup`: one group holding every stack row in order.
    pub(crate) fn write_stackup(&self, xml: &mut Xml, declared: &[Declared]) {
        let mut layers = Vec::new();
        let mut dielectrics = 0;
        for (row, thickness_mm) in stack_rows(self.stack) {
            let source = match row {
                StackRow::Layer(layer) => Source::Board(layer),
                StackRow::Dielectric => {
                    dielectrics += 1;
                    Source::Dielectric(dielectrics)
                }
            };
            if let Some(d) = declared.iter().find(|d| d.source == source) {
                layers.push((d.name.clone(), nm(thickness_mm)));
            }
        }
        if layers.is_empty() {
            return;
        }
        let total = millionths(layers.iter().map(|(_, t)| t).sum());
        let zero = || "0".to_string();
        xml.open(
            "Stackup",
            &[
                ("name", "Primary".into()),
                ("overallThickness", total.clone()),
                ("tolPlus", zero()),
                ("tolMinus", zero()),
                ("whereMeasured", "METAL".into()),
                ("stackupStatus", "PROPOSED".into()),
            ],
        );
        xml.open(
            "StackupGroup",
            &[
                ("name", "Primary".into()),
                ("thickness", total),
                ("tolPlus", zero()),
                ("tolMinus", zero()),
            ],
        );
        for (i, (name, thickness)) in layers.into_iter().enumerate() {
            xml.leaf(
                "StackupLayer",
                &[
                    ("layerOrGroupRef", name),
                    ("thickness", millionths(thickness)),
                    ("tolPlus", zero()),
                    ("tolMinus", zero()),
                    ("sequence", (i + 1).to_string()),
                ],
            );
        }
        xml.close();
        xml.close();
    }
}

/// The `Layer` elements.
pub(crate) fn write_layers(xml: &mut Xml, declared: &[Declared]) {
    for layer in declared {
        let attrs = [
            ("name", layer.name.clone()),
            ("layerFunction", layer.function.to_string()),
            ("side", layer.side.to_string()),
            ("polarity", "POSITIVE".to_string()),
        ];
        match &layer.span {
            Some((from, to)) => {
                xml.open("Layer", &attrs);
                xml.leaf(
                    "Span",
                    &[("fromLayer", from.clone()), ("toLayer", to.clone())],
                );
                xml.close();
            }
            None => xml.leaf("Layer", &attrs),
        }
    }
}

fn side_word(place: LayerPlace) -> &'static str {
    match place {
        LayerPlace::Top => "TOP",
        LayerPlace::Bottom => "BOTTOM",
        LayerPlace::Inner => "INTERNAL",
        LayerPlace::Board => "ALL",
    }
}

/// `name`, or the layer's token when another layer already has `name` (a
/// mechanical layer named like another layer).
fn unique(declared: &[Declared], name: String, layer: PcbLayer) -> String {
    if declared.iter().any(|d| d.name == name) {
        qualified(&pcb_layer_token(layer))
    } else {
        name
    }
}

/// A row's thickness in nm; 0 for a value the geometry cannot hold.
fn nm(mm: f64) -> i64 {
    mm_to_nm(mm, "stack.thickness").map_or(0, i64::abs)
}
