//! Excellon drill files, one per drilled span and plating, with slots, and
//! the drill table. Format: metric, absolute, decimal point, three decimals
//! (micrometres). See `docs/FABRICATION_OUTPUTS.md`, "Drill".

use pcb_geom::{
    GeomError, PointNm, layer_bit, mm_to_nm, pad_world_center, pad_world_rotation_deg, via_layers,
};
use signex_types::pcb::{Footprint, Pad, PadType, PcbBoard, Via};

use super::frame::{FabFrame, mm3};
use super::gerber::GENERATION_SOFTWARE;
use super::layers::CopperStack;

/// The copper layers a hole runs between: 1-based stack numbers, top first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Span {
    pub from: usize,
    pub to: usize,
}

/// One drilled hole or routed slot, in the fabrication frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hole {
    pub at: PointNm,
    pub diameter: i64,
    pub plated: bool,
    pub span: Span,
    /// The far end of a slot: the tool runs from `at` to here.
    pub slot_to: Option<PointNm>,
}

/// What a drill file holds, in the X2 words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DrillKind {
    /// Plated, through the whole stack.
    Pth,
    /// Not plated, through the whole stack.
    Npth,
    /// Plated, from an outer layer to an inner one.
    Blind,
    /// Plated, between two inner layers.
    Buried,
}

/// One drill file: the plated holes of one span, or the non-plated holes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrillFile {
    pub plated: bool,
    pub span: Span,
}

impl DrillFile {
    fn kind(self, layers: usize) -> DrillKind {
        let Span { from, to } = self.span;
        match (self.plated, from == 1, to == layers) {
            (false, _, _) => DrillKind::Npth,
            (true, true, true) => DrillKind::Pth,
            (true, false, false) => DrillKind::Buried,
            (true, _, _) => DrillKind::Blind,
        }
    }

    /// The X2 `.FileFunction` value: `Plated,1,4,PTH`, `Plated,1,2,Blind`.
    pub(crate) fn function(self, layers: usize) -> String {
        let plating = if self.plated { "Plated" } else { "NonPlated" };
        let kind = match self.kind(layers) {
            DrillKind::Pth => "PTH",
            DrillKind::Npth => "NPTH",
            DrillKind::Blind => "Blind",
            DrillKind::Buried => "Buried",
        };
        format!("{plating},{},{},{kind}", self.span.from, self.span.to)
    }

    /// The file-name suffix, before `.drl`: `pth`, `npth`, `blind-l1-l2`.
    pub(crate) fn suffix(self, layers: usize) -> String {
        let Span { from, to } = self.span;
        match self.kind(layers) {
            DrillKind::Pth => "pth".to_string(),
            DrillKind::Npth => "npth".to_string(),
            DrillKind::Blind => format!("blind-l{from}-l{to}"),
            DrillKind::Buried => format!("buried-l{from}-l{to}"),
        }
    }

    /// The drill table's word for the file: `PTH`, `NPTH`, `Blind L1-L2`.
    fn table_word(self, layers: usize) -> String {
        let Span { from, to } = self.span;
        match self.kind(layers) {
            DrillKind::Pth => "PTH".to_string(),
            DrillKind::Npth => "NPTH".to_string(),
            DrillKind::Blind => format!("Blind L{from}-L{to}"),
            DrillKind::Buried => format!("Buried L{from}-L{to}"),
        }
    }

    fn holds(self, hole: &Hole) -> bool {
        hole.plated == self.plated && hole.span == self.span
    }
}

/// Every hole of the board, in board order: pads, vias, then the board
/// sketch's mounting holes. A hole of no diameter is not drilled. A blind
/// or micro via whose layers name no span of two stack layers is skipped,
/// never drilled through the whole stack.
pub(crate) fn collect_holes(
    board: &PcbBoard,
    stack: &CopperStack,
    skipped: &mut usize,
) -> Vec<Hole> {
    let through = Span {
        from: 1,
        to: stack.count(),
    };
    let mut holes = Vec::new();
    let frame = FabFrame::of(board);
    // Free pads (#2373) are drilled in the board's own frame.
    let free_frame = signex_types::pcb_free::free_pad_frame();
    let free = board.free.pads.iter().map(|pad| (&free_frame, pad));
    let owned = board
        .footprints
        .iter()
        .flat_map(|fp| fp.pads.iter().map(move |pad| (fp, pad)));
    for (footprint, pad) in owned.chain(free) {
        match pad_hole(footprint, pad, through, frame) {
            Ok(Some(hole)) => holes.push(hole),
            Ok(None) => {}
            Err(_) => *skipped += 1,
        }
    }
    let vias = board
        .vias
        .iter()
        .map(|via| (via.position, via.drill, true, via_span(via, stack)));
    let mounting = board
        .sketch
        .iter()
        .flat_map(|s| s.holes.iter())
        .map(|hole| (hole.position, hole.drill, hole.plated, Some(through)));
    for (position, drill, plated, span) in vias.chain(mounting) {
        let built = PointNm::from_mm(position, "hole.position")
            .and_then(|at| Ok((at, mm_to_nm(drill, "hole.drill")?.abs())));
        match (built, span) {
            (Ok((_, 0)), _) => {}
            (Ok((at, diameter)), Some(span)) => holes.push(Hole {
                at: frame.point(at),
                diameter,
                plated,
                span,
                slot_to: None,
            }),
            (Ok(_), None) | (Err(_), _) => *skipped += 1,
        }
    }
    holes
}

/// The stack layers a via runs between; `None` when it reaches fewer than
/// two of them.
fn via_span(via: &Via, stack: &CopperStack) -> Option<Span> {
    let mask = via_layers(via);
    let mut numbers = stack
        .layers()
        .iter()
        .filter(|l| mask & (1 << layer_bit(**l)) != 0)
        .filter_map(|l| stack.number(*l));
    let from = numbers.next()?;
    let to = numbers.next_back()?;
    Some(Span { from, to })
}

/// A pad's hole. An oval drill is a slot along the pad's longer side,
/// keeping the annular ring the pad has across it: its length is the
/// drill width plus the pad's length minus its width.
fn pad_hole(
    footprint: &Footprint,
    pad: &Pad,
    span: Span,
    frame: FabFrame,
) -> Result<Option<Hole>, GeomError> {
    let Some(drill) = &pad.drill else {
        return Ok(None);
    };
    let diameter = mm_to_nm(drill.diameter, "pad.drill")?.abs();
    if diameter == 0 {
        return Ok(None);
    }
    let center = pad_world_center(footprint, pad)?;
    let plated = pad.pad_type != PadType::NpThru;
    let w = mm_to_nm(pad.size.x, "pad.size")?.abs();
    let h = mm_to_nm(pad.size.y, "pad.size")?.abs();
    let reach = (w.max(h) - w.min(h)) / 2;
    if drill.shape != "oval" || reach == 0 {
        return Ok(Some(Hole {
            at: frame.point(center),
            diameter,
            plated,
            span,
            slot_to: None,
        }));
    }
    let (sin, cos) = pad_world_rotation_deg(footprint, pad)?
        .to_radians()
        .sin_cos();
    let (ax, ay) = if w > h { (1.0, 0.0) } else { (0.0, 1.0) };
    let tip = PointNm::new(
        ((ax * cos - ay * sin) * reach as f64).round() as i64,
        ((ax * sin + ay * cos) * reach as f64).round() as i64,
    );
    Ok(Some(Hole {
        at: frame.point(center.minus(tip)),
        diameter,
        plated,
        span,
        slot_to: Some(frame.point(center.plus(tip))),
    }))
}

/// The drill files `holes` fill, in writing order: the plated through
/// file, the plated spans top first, then the non-plated file.
pub(crate) fn drill_files(holes: &[Hole], layers: usize) -> Vec<DrillFile> {
    let mut files: Vec<DrillFile> = holes
        .iter()
        .map(|h| DrillFile {
            plated: h.plated,
            span: h.span,
        })
        .collect();
    files.sort_unstable_by_key(|f| {
        let through = f.span.from == 1 && f.span.to == layers;
        (!f.plated, !through, f.span)
    });
    files.dedup();
    files
}

/// The tools of one file: each distinct diameter, smallest first.
fn tools(holes: &[&Hole]) -> Vec<i64> {
    let mut tools: Vec<i64> = holes.iter().map(|h| h.diameter).collect();
    tools.sort_unstable();
    tools.dedup();
    tools
}

/// The Excellon file of `file`'s holes.
pub(crate) fn write_drill(holes: &[Hole], file: DrillFile, layers: usize) -> String {
    let mine: Vec<&Hole> = holes.iter().filter(|h| file.holds(h)).collect();
    let tools = tools(&mine);
    let mut out = String::from("M48\n");
    out.push_str(&format!(
        "; #@! TF.GenerationSoftware,{GENERATION_SOFTWARE}\n"
    ));
    out.push_str(&format!(
        "; #@! TF.FileFunction,{}\n",
        file.function(layers)
    ));
    out.push_str("FMAT,2\nMETRIC\n");
    for (index, diameter) in tools.iter().enumerate() {
        out.push_str(&format!("T{}C{}\n", index + 1, mm3(*diameter)));
    }
    out.push_str("%\nG90\nG05\n");
    for (index, diameter) in tools.iter().enumerate() {
        out.push_str(&format!("T{}\n", index + 1));
        for hole in mine.iter().filter(|h| h.diameter == *diameter) {
            let at = format!("X{}Y{}", mm3(hole.at.x), mm3(hole.at.y));
            match hole.slot_to {
                Some(to) => {
                    out.push_str(&format!("{at}G85X{}Y{}\n", mm3(to.x), mm3(to.y)));
                }
                None => out.push_str(&format!("{at}\n")),
            }
        }
    }
    out.push_str("T0\nM30\n");
    out
}

/// The drill table: one row per tool of each file, with its hole and slot
/// counts.
pub(crate) fn write_table(holes: &[Hole], files: &[DrillFile], layers: usize) -> String {
    let mut out = String::from("File,Tool,Diameter (mm),Holes,Slots\n");
    for file in files {
        let mine: Vec<&Hole> = holes.iter().filter(|h| file.holds(h)).collect();
        let word = file.table_word(layers);
        for (index, diameter) in tools(&mine).iter().enumerate() {
            let of_tool = mine.iter().filter(|h| h.diameter == *diameter);
            let slots = of_tool.clone().filter(|h| h.slot_to.is_some()).count();
            let all = of_tool.count();
            out.push_str(&format!(
                "{word},T{},{},{},{slots}\n",
                index + 1,
                mm3(*diameter),
                all - slots
            ));
        }
    }
    out
}
