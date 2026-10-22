use std::collections::{BTreeMap, BTreeSet};

use signex_gerber::LayerType;
use signex_types::format::SnxPcb;
use signex_types::pcb::{
    BoardGraphic, Footprint, LayerDef, PCB_DEFAULT_CLEARANCE_MM, PCB_DEFAULT_THICKNESS_MM,
    PCB_DEFAULT_TRACE_WIDTH_MM, PCB_DEFAULT_VIA_DIAMETER_MM, PCB_DEFAULT_VIA_DRILL_MM, PCB_GRID_MM,
    PCB_TRACK_MIN_MM, PCB_VIA_MIN_DIAMETER_MM, PCB_VIA_MIN_DRILL_MM, Pad, PadShape, PadType,
    PcbBoard, PcbSetup, Point as PcbPoint, Segment,
};
use uuid::Uuid;

use super::*;

#[derive(Debug, Clone)]
pub struct GerberPcbExport {
    pub board: PcbBoard,
    pub report: GerberPcbExportReport,
}

impl GerberPcbExport {
    pub fn write_string(&self) -> Result<String, String> {
        SnxPcb::new(self.board.clone())
            .write_string()
            .map_err(|error| format!("Could not serialize native PCB: {error}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GerberPcbExportReport {
    pub converted_flashes: usize,
    pub converted_lines: usize,
    pub skipped: Vec<GerberPcbSkippedItem>,
    pub lossy: bool,
}

impl GerberPcbExportReport {
    pub fn converted_count(&self) -> usize {
        self.converted_flashes + self.converted_lines
    }

    pub fn skipped_count(&self) -> usize {
        self.skipped.iter().map(|item| item.count).sum()
    }

    pub fn summary(&self) -> String {
        format!(
            "Lossy conversion: {} flash(es) and {} line(s) converted; {} item(s) skipped.",
            self.converted_flashes,
            self.converted_lines,
            self.skipped_count(),
        )
    }

    pub fn detailed_summary(&self) -> String {
        if self.skipped.is_empty() {
            return self.summary();
        }

        let skipped = self
            .skipped
            .iter()
            .map(|item| format!("{} ({})", item.reason, item.count))
            .collect::<Vec<_>>()
            .join("; ");
        format!("{} Skipped: {skipped}", self.summary())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GerberPcbSkippedItem {
    pub reason: String,
    pub count: usize,
}

impl GerberViewerState {
    pub fn export_native_pcb(&self) -> GerberPcbExport {
        let mut board = empty_lossy_board();
        let mut report = ExportReportBuilder::default();
        let mut native_layers = BTreeSet::new();
        let mut pads_by_layer = BTreeMap::<String, Vec<Pad>>::new();

        for viewer_layer in &self.layers {
            let Some(target) = target_layer(viewer_layer.layer.layer_type) else {
                report.skip_count(
                    format!("{}: unsupported layer role", viewer_layer.layer.name,),
                    viewer_layer.layer.geometry.primitives.len(),
                );
                continue;
            };
            native_layers.insert(target.name().to_owned());

            for primitive in &viewer_layer.layer.geometry.primitives {
                export_primitive(
                    primitive,
                    &viewer_layer.layer.name,
                    &target,
                    &mut board,
                    &mut pads_by_layer,
                    &mut report,
                );
            }
        }

        board.layers = native_layers
            .into_iter()
            .enumerate()
            .map(|(id, name)| LayerDef {
                id: id as u8,
                layer_type: if name.ends_with(".Cu") {
                    "copper".to_owned()
                } else {
                    "graphic".to_owned()
                },
                name,
            })
            .collect();
        board.footprints = pads_by_layer
            .into_iter()
            .map(|(layer, pads)| Footprint {
                uuid: Uuid::new_v4(),
                reference: format!("GERBER_{}", layer.replace(['.', '-'], "_").to_uppercase(),),
                value: "Lossy Gerber import".to_owned(),
                footprint_id: "signex:gerber-import".to_owned(),
                position: PcbPoint::ZERO,
                rotation: 0.0,
                layer,
                locked: false,
                pads,
                graphics: Vec::new(),
                properties: Vec::new(),
            })
            .collect();

        GerberPcbExport {
            board,
            report: report.finish(),
        }
    }
}

fn empty_lossy_board() -> PcbBoard {
    PcbBoard {
        uuid: Uuid::new_v4(),
        version: 1,
        generator: "signex-gerber-import (lossy)".to_owned(),
        thickness: PCB_DEFAULT_THICKNESS_MM,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: Some(PcbSetup {
            grid_size: PCB_GRID_MM,
            trace_width: PCB_DEFAULT_TRACE_WIDTH_MM,
            via_diameter: PCB_DEFAULT_VIA_DIAMETER_MM,
            via_drill: PCB_DEFAULT_VIA_DRILL_MM,
            clearance: PCB_DEFAULT_CLEARANCE_MM,
            track_min_width: PCB_TRACK_MIN_MM,
            via_min_diameter: PCB_VIA_MIN_DIAMETER_MM,
            via_min_drill: PCB_VIA_MIN_DRILL_MM,
        }),
        nets: Vec::new(),
        footprints: Vec::new(),
        segments: Vec::new(),
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
    }
}

fn export_primitive(
    primitive: &GerberPrimitive,
    source_layer: &str,
    target: &TargetLayer,
    board: &mut PcbBoard,
    pads_by_layer: &mut BTreeMap<String, Vec<Pad>>,
    report: &mut ExportReportBuilder,
) {
    match primitive {
        GerberPrimitive::Stroke {
            start,
            end,
            width,
            polarity: PrimitivePolarity::Dark,
            ..
        } if valid_point(*start) && valid_point(*end) && valid_size(*width) => {
            match target {
                TargetLayer::Copper(layer) => {
                    board.segments.push(Segment {
                        uuid: Uuid::new_v4(),
                        start: pcb_point(*start),
                        end: pcb_point(*end),
                        width: *width,
                        layer: layer.clone(),
                        net: 0,
                    });
                }
                TargetLayer::Graphic(layer) => {
                    board.graphics.push(BoardGraphic {
                        graphic_type: "line".to_owned(),
                        layer: layer.clone(),
                        width: *width,
                        start: Some(pcb_point(*start)),
                        end: Some(pcb_point(*end)),
                        center: None,
                        radius: 0.0,
                        points: Vec::new(),
                    });
                }
            }
            report.converted_lines += 1;
        }
        GerberPrimitive::Flash {
            position,
            aperture,
            polarity: PrimitivePolarity::Dark,
            ..
        } if valid_point(*position) => {
            let TargetLayer::Copper(layer) = target else {
                report.skip(format!("{source_layer}: flashes on non-copper layers",));
                return;
            };
            let Some((shape, size)) = pad_geometry(aperture) else {
                report.skip(format!("{source_layer}: polygon or macro flash",));
                return;
            };
            pads_by_layer.entry(layer.clone()).or_default().push(Pad {
                uuid: Uuid::new_v4(),
                number: String::new(),
                pad_type: PadType::Smd,
                shape,
                position: pcb_point(*position),
                size,
                drill: None,
                layers: vec![layer.clone()],
                net: None,
                roundrect_ratio: 0.0,
            });
            report.converted_flashes += 1;
        }
        GerberPrimitive::Stroke {
            polarity: PrimitivePolarity::Clear,
            ..
        }
        | GerberPrimitive::Flash {
            polarity: PrimitivePolarity::Clear,
            ..
        }
        | GerberPrimitive::Region {
            polarity: PrimitivePolarity::Clear,
            ..
        } => {
            report.skip(format!("{source_layer}: clear-polarity primitive"));
        }
        GerberPrimitive::Region { .. } => {
            report.skip(format!("{source_layer}: filled region"));
        }
        GerberPrimitive::DrillHit { .. } | GerberPrimitive::DrillSlot { .. } => {
            report.skip(format!("{source_layer}: drill primitive"));
        }
        GerberPrimitive::Stroke { .. } | GerberPrimitive::Flash { .. } => {
            report.skip(format!("{source_layer}: invalid primitive geometry"));
        }
    }
}

fn pad_geometry(aperture: &ApertureShape) -> Option<(PadShape, PcbPoint)> {
    match aperture {
        ApertureShape::Circle { diameter } if valid_size(*diameter) => {
            Some((PadShape::Circle, PcbPoint::new(*diameter, *diameter)))
        }
        ApertureShape::Rectangle { width, height } if valid_size(*width) && valid_size(*height) => {
            Some((PadShape::Rect, PcbPoint::new(*width, *height)))
        }
        ApertureShape::Obround { width, height } if valid_size(*width) && valid_size(*height) => {
            Some((PadShape::Oval, PcbPoint::new(*width, *height)))
        }
        ApertureShape::Polygon { .. } | ApertureShape::Macro { .. } => None,
        _ => None,
    }
}

fn valid_point(point: signex_gerber::Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn valid_size(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn pcb_point(point: signex_gerber::Point) -> PcbPoint {
    PcbPoint::new(point.x, point.y)
}

enum TargetLayer {
    Copper(String),
    Graphic(String),
}

impl TargetLayer {
    fn name(&self) -> &str {
        match self {
            Self::Copper(name) | Self::Graphic(name) => name,
        }
    }
}

fn target_layer(layer_type: LayerType) -> Option<TargetLayer> {
    match layer_type {
        LayerType::Top => Some(TargetLayer::Copper("F.Cu".to_owned())),
        LayerType::Bottom => Some(TargetLayer::Copper("B.Cu".to_owned())),
        LayerType::Inner(index) if index > 0 => Some(TargetLayer::Copper(format!("In{index}.Cu"))),
        LayerType::PasteTop => Some(TargetLayer::Graphic("F.Paste".to_owned())),
        LayerType::PasteBottom => Some(TargetLayer::Graphic("B.Paste".to_owned())),
        LayerType::MaskTop => Some(TargetLayer::Graphic("F.Mask".to_owned())),
        LayerType::MaskBottom => Some(TargetLayer::Graphic("B.Mask".to_owned())),
        LayerType::SilkScreenTop => Some(TargetLayer::Graphic("F.SilkS".to_owned())),
        LayerType::SilkScreenBottom => Some(TargetLayer::Graphic("B.SilkS".to_owned())),
        LayerType::Dimensions | LayerType::Milling => {
            Some(TargetLayer::Graphic("Edge.Cuts".to_owned()))
        }
        LayerType::VCut => Some(TargetLayer::Graphic("V.Cuts".to_owned())),
        LayerType::Drill
        | LayerType::SidePlating
        | LayerType::KeepOut
        | LayerType::Info
        | LayerType::UndefinedGerber
        | LayerType::Inner(_) => None,
    }
}

#[derive(Default)]
struct ExportReportBuilder {
    converted_flashes: usize,
    converted_lines: usize,
    skipped: BTreeMap<String, usize>,
}

impl ExportReportBuilder {
    fn skip(&mut self, reason: String) {
        *self.skipped.entry(reason).or_default() += 1;
    }

    fn skip_count(&mut self, reason: String, count: usize) {
        if count > 0 {
            *self.skipped.entry(reason).or_default() += count;
        }
    }

    fn finish(self) -> GerberPcbExportReport {
        GerberPcbExportReport {
            converted_flashes: self.converted_flashes,
            converted_lines: self.converted_lines,
            skipped: self
                .skipped
                .into_iter()
                .map(|(reason, count)| GerberPcbSkippedItem { reason, count })
                .collect(),
            lossy: true,
        }
    }
}
