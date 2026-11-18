//! The ODB++ matrix: the job's layers in physical order top to bottom
//! (top components, legend, paste, mask, the copper and dielectric rows of
//! the stack, the bottom's in reverse), then the drill layers, then the
//! drawing layers. Each board layer's type comes from `fab::roles`.

use signex_types::format::pcb_layer_token;
use signex_types::pcb::PcbBoard;
use signex_types::pcb_layer::{CopperLayer, PcbLayer, TechnicalLayer};
use signex_types::pcb_stack::LayerStack;

use super::text::entity_name;
use crate::fab::drill::DrillFile;
use crate::fab::layers::{CopperStack, Face};
use crate::fab::roles::{LayerRole, StackRow, layer_label, plane_layers, stack_rows};

/// What a matrix layer holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Source {
    Board(PcbLayer),
    Dielectric,
    Components(Face),
    VScore,
    Drill(DrillFile),
}

/// One matrix row.
#[derive(Debug, Clone)]
pub(crate) struct MatrixLayer {
    pub name: String,
    pub source: Source,
    context: &'static str,
    kind: &'static str,
    /// The copper layer names a drill layer runs between.
    span: Option<(String, String)>,
}

/// Which layers the job holds, given what carries marks.
pub(crate) struct MatrixPlan<'a> {
    pub board: &'a PcbBoard,
    pub copper: &'a CopperStack,
    pub stack: &'a LayerStack,
    /// Board layers that carry marks or are in the layer table.
    pub used: Vec<PcbLayer>,
    /// Whether parts sit on the top, and on the bottom.
    pub parts_on: [bool; 2],
    pub v_score: bool,
    pub drills: &'a [DrillFile],
}

impl MatrixPlan<'_> {
    pub(crate) fn declare(&self) -> Vec<MatrixLayer> {
        use TechnicalLayer as T;
        let mut out: Vec<MatrixLayer> = Vec::new();
        if self.parts_on[0] {
            out.push(components(Face::Top));
        }
        for layer in [T::SilkscreenTop, T::PasteTop, T::SolderMaskTop] {
            self.add_if_used(&mut out, PcbLayer::Technical(layer));
        }
        let mut dielectrics = 0;
        for row in self.middle() {
            match row {
                StackRow::Layer(layer) => self.add(&mut out, layer),
                StackRow::Dielectric => {
                    dielectrics += 1;
                    out.push(MatrixLayer {
                        name: format!("dielectric{dielectrics}"),
                        source: Source::Dielectric,
                        context: "BOARD",
                        kind: "DIELECTRIC",
                        span: None,
                    });
                }
            }
        }
        for layer in [T::SolderMaskBottom, T::PasteBottom, T::SilkscreenBottom] {
            self.add_if_used(&mut out, PcbLayer::Technical(layer));
        }
        if self.parts_on[1] {
            out.push(components(Face::Bottom));
        }
        self.add_drills(&mut out);
        if self.v_score {
            let name = unique(&out, "vscore".into(), "v_score".into());
            out.push(MatrixLayer {
                name,
                source: Source::VScore,
                context: "MISC",
                kind: "DOCUMENT",
                span: None,
            });
        }
        for layer in PcbLayer::all() {
            self.add_if_used(&mut out, layer);
        }
        out
    }

    /// The copper and dielectric rows top to bottom: the stack's, with
    /// any copper layer it does not list put in copper order.
    fn middle(&self) -> Vec<StackRow> {
        let mut rows: Vec<StackRow> = stack_rows(self.stack)
            .into_iter()
            .map(|(row, _)| row)
            .filter(|row| match row {
                StackRow::Layer(layer) => layer.is_copper(),
                StackRow::Dielectric => true,
            })
            .collect();
        let copper_of = |row: &StackRow| match row {
            StackRow::Layer(PcbLayer::Copper(c)) => Some(*c),
            _ => None,
        };
        for copper in self.copper.layers() {
            if rows.iter().any(|r| copper_of(r) == Some(*copper)) {
                continue;
            }
            let below = rows
                .iter()
                .position(|r| copper_of(r).is_some_and(|c: CopperLayer| c > *copper))
                .unwrap_or(rows.len());
            rows.insert(below, StackRow::Layer(PcbLayer::Copper(*copper)));
        }
        rows
    }

    fn add_if_used(&self, out: &mut Vec<MatrixLayer>, layer: PcbLayer) {
        if self.used.contains(&layer) {
            self.add(out, layer);
        }
    }

    /// A board layer, once; the outline is the step profile, not a layer.
    fn add(&self, out: &mut Vec<MatrixLayer>, layer: PcbLayer) {
        if out.iter().any(|l| l.source == Source::Board(layer)) {
            return;
        }
        let plane = layer
            .copper()
            .is_some_and(|c| plane_layers(self.stack).contains(&c));
        let role = LayerRole::of(layer, plane);
        let Some(kind) = role.odb_matrix_type() else {
            return;
        };
        let context = match role {
            LayerRole::Signal
            | LayerRole::Plane
            | LayerRole::Legend
            | LayerRole::SolderMask
            | LayerRole::SolderPaste => "BOARD",
            _ => "MISC",
        };
        let name = unique(
            out,
            entity_name(&layer_label(self.board, layer)),
            entity_name(&pcb_layer_token(layer)),
        );
        out.push(MatrixLayer {
            name,
            source: Source::Board(layer),
            context,
            kind,
            span: None,
        });
    }

    /// One drill layer per drilled span, running between its copper.
    fn add_drills(&self, out: &mut Vec<MatrixLayer>) {
        let count = self.copper.count();
        for file in self.drills {
            let copper_name = |n: usize| {
                let layer = PcbLayer::Copper(self.copper.layers()[n - 1]);
                out.iter()
                    .find(|l| l.source == Source::Board(layer))
                    .map(|l| l.name.clone())
                    .unwrap_or_else(|| entity_name(&pcb_layer_token(layer)))
            };
            let span = (copper_name(file.span.from), copper_name(file.span.to));
            let wanted = format!("drill_{}", file.suffix(count).replace('-', "_"));
            out.push(MatrixLayer {
                name: unique(out, wanted.clone(), format!("{wanted}_1")),
                source: Source::Drill(*file),
                context: "BOARD",
                kind: LayerRole::Drill.odb_matrix_type().unwrap_or("DRILL"),
                span: Some(span),
            });
        }
    }
}

fn components(face: Face) -> MatrixLayer {
    MatrixLayer {
        name: component_layer(face).to_string(),
        source: Source::Components(face),
        context: "BOARD",
        kind: "COMPONENT",
        span: None,
    }
}

/// The name of a side's component layer.
pub(crate) fn component_layer(face: Face) -> &'static str {
    match face {
        Face::Top => "comp_+_top",
        Face::Bottom => "comp_+_bot",
    }
}

/// `name`, or `fallback` when another layer already has `name` (a
/// mechanical layer named like another layer).
fn unique(out: &[MatrixLayer], name: String, fallback: String) -> String {
    if out.iter().any(|l| l.name == name) {
        fallback
    } else {
        name
    }
}

/// The `matrix/matrix` file of step `step`.
pub(crate) fn write_matrix(layers: &[MatrixLayer], step: &str) -> String {
    let mut out = format!("UNITS=MM\nSTEP {{\n   COL=1\n   NAME={step}\n}}\n");
    for (i, layer) in layers.iter().enumerate() {
        let (start, end) = layer.span.clone().unwrap_or_default();
        out.push_str(&format!(
            "LAYER {{\n   ROW={}\n   CONTEXT={}\n   TYPE={}\n   NAME={}\n   \
             POLARITY=POSITIVE\n   START_NAME={start}\n   END_NAME={end}\n   OLD_NAME=\n}}\n",
            i + 1,
            layer.context,
            layer.kind,
            layer.name,
        ));
    }
    out
}
