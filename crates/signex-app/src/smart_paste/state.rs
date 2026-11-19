//! The Smart Paste dialog's state and messages (#155). The fields hold what
//! the user typed; `input` checks them. What the copies are called and the
//! clipboard summary are worked out in `update` (they read the sheet and
//! the project's designators) and kept here for the view.

use std::path::PathBuf;

use super::plan::{
    DesignatorChoice, InputProblem, MAX_COPIES, NetAfter, NetChoice, PasteInput, PlaceMode,
    after_paste, parse_mm,
};

/// The dialog's size (mockup: 880 x 600).
pub const DIALOG_SIZE: (f32, f32) = (880.0, 600.0);

/// What the clipboard holds, for the subtitle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipboardSummary {
    /// The copied parts' designators.
    pub parts: Vec<String>,
    pub wires: usize,
    pub labels: usize,
}

/// One drawable of the preview, in mm relative to the clipboard's anchor.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewShape {
    Line {
        from: (f64, f64),
        to: (f64, f64),
    },
    /// A part's box and its designator.
    Part {
        min: (f64, f64),
        max: (f64, f64),
        reference: String,
    },
    /// A net label and the copied net it names (`None` for a power label).
    Label {
        at: (f64, f64),
        net: String,
        power: bool,
    },
}

#[derive(Debug, Clone)]
pub struct SmartPasteState {
    /// The sheet the paste was asked on; the dialog closes when it changes.
    pub target: PathBuf,
    pub mode: PlaceMode,
    pub offset_x: String,
    pub offset_y: String,
    pub copies: String,
    pub step_x: String,
    pub step_y: String,
    pub designators: DesignatorChoice,
    pub nets: NetChoice,
    pub pattern: String,
    pub start: String,
    pub summary: ClipboardSummary,
    /// The copied nets, `true` for a power net.
    pub copied_nets: Vec<(String, bool)>,
    /// "R4 → R6, R5 → R7": what Next free calls the parts, per copy.
    pub next_free_text: String,
    /// The clipboard's drawables and their box, for the preview.
    pub shapes: Vec<PreviewShape>,
    pub bounds: ((f64, f64), (f64, f64)),
}

impl SmartPasteState {
    /// A dialog for a clipboard whose copy is `width` mm wide: the copy is
    /// offset one width and a grid step to the right, and the array step
    /// is the same.
    #[must_use]
    pub fn new(target: PathBuf, step_mm: f64) -> Self {
        let step = format!("{step_mm:.2}");
        Self {
            target,
            mode: PlaceMode::Offset,
            offset_x: step.clone(),
            offset_y: "0.00".into(),
            copies: "3".into(),
            step_x: step,
            step_y: "0.00".into(),
            designators: DesignatorChoice::NextFree,
            nets: NetChoice::Rewrite,
            pattern: "${Net}_${n}".into(),
            start: "2".into(),
            summary: ClipboardSummary::default(),
            copied_nets: Vec::new(),
            next_free_text: String::new(),
            shapes: Vec::new(),
            bounds: ((0.0, 0.0), (0.0, 0.0)),
        }
    }

    /// The choices, checked.
    pub fn input(&self) -> Result<PasteInput, InputProblem> {
        let (copies, x, y) = match self.mode {
            PlaceMode::Offset => (1, &self.offset_x, &self.offset_y),
            PlaceMode::Array => {
                let copies = self
                    .copies
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (1..=MAX_COPIES).contains(n))
                    .ok_or(InputProblem::Copies)?;
                (copies, &self.step_x, &self.step_y)
            }
        };
        let (Some(dx), Some(dy)) = (parse_mm(x), parse_mm(y)) else {
            return Err(InputProblem::Distance);
        };
        let rewrite = match self.nets {
            NetChoice::Keep => None,
            NetChoice::Rewrite => {
                let start = self
                    .start
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| InputProblem::Start)?;
                if !self.pattern.contains("${Net}") && !self.pattern.contains("${n}") {
                    return Err(InputProblem::Pattern);
                }
                Some((self.pattern.clone(), start))
            }
        };
        Ok(PasteInput {
            copies,
            dx,
            dy,
            designators: self.designators,
            rewrite,
        })
    }

    /// How many copies the drawing shows (1 while the input is not valid).
    #[must_use]
    pub fn copy_count(&self) -> u32 {
        self.input().map_or(1, |i| i.copies)
    }

    /// The After paste table rows; empty while the input is not valid.
    #[must_use]
    pub fn after_rows(&self) -> Vec<(String, NetAfter)> {
        self.input()
            .map(|input| after_paste(&self.copied_nets, &input))
            .unwrap_or_default()
    }

    /// The offset (or step) the drawing's first copy sits at.
    #[must_use]
    pub fn first_offset(&self) -> (f64, f64) {
        let (x, y) = match self.mode {
            PlaceMode::Offset => (&self.offset_x, &self.offset_y),
            PlaceMode::Array => (&self.step_x, &self.step_y),
        };
        (parse_mm(x).unwrap_or(0.0), parse_mm(y).unwrap_or(0.0))
    }

    /// Put the first copy at `(dx, dy)` mm (a drag on the drawing).
    pub fn set_first_offset(&mut self, dx: f64, dy: f64) {
        let (x, y) = (format!("{dx:.2}"), format!("{dy:.2}"));
        match self.mode {
            PlaceMode::Offset => (self.offset_x, self.offset_y) = (x, y),
            PlaceMode::Array => (self.step_x, self.step_y) = (x, y),
        }
    }
}

/// What happened in the Smart Paste dialog.
#[derive(Debug, Clone)]
pub enum SmartPasteMsg {
    ModePicked(PlaceMode),
    OffsetXEdited(String),
    OffsetYEdited(String),
    CopiesEdited(String),
    StepXEdited(String),
    StepYEdited(String),
    DesignatorsPicked(DesignatorChoice),
    NetsPicked(NetChoice),
    PatternEdited(String),
    StartEdited(String),
    /// The first copy was dragged on the drawing to `(dx, dy)` mm, snapped.
    FirstCopyDragged {
        dx: f64,
        dy: f64,
    },
    /// Paste: the copies go on the cursor.
    PasteConfirmed,
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_mode_places_one_copy_and_array_mode_reads_copies_and_step() {
        let mut s = SmartPasteState::new(PathBuf::from("/p/a.snxsch"), 20.32);
        let one = s.input().expect("valid");
        assert_eq!((one.copies, one.dx, one.dy), (1, 20.32, 0.0));
        s.mode = PlaceMode::Array;
        s.copies = "4".into();
        s.step_y = "-5.08".into();
        let many = s.input().expect("valid");
        assert_eq!((many.copies, many.dy), (4, -5.08));
        s.copies = "0".into();
        assert_eq!(s.input(), Err(InputProblem::Copies));
    }

    #[test]
    fn keep_ignores_the_pattern_and_rewrite_checks_it() {
        let mut s = SmartPasteState::new(PathBuf::from("/p/a.snxsch"), 20.32);
        s.pattern = "plain".into();
        assert_eq!(s.input(), Err(InputProblem::Pattern));
        s.nets = NetChoice::Keep;
        assert_eq!(s.input().expect("valid").rewrite, None);
    }

    #[test]
    fn a_drag_moves_the_offset_or_the_step() {
        let mut s = SmartPasteState::new(PathBuf::from("/p/a.snxsch"), 20.32);
        s.set_first_offset(25.4, 2.54);
        assert_eq!(s.first_offset(), (25.4, 2.54));
        s.mode = PlaceMode::Array;
        s.set_first_offset(10.16, 0.0);
        assert_eq!(s.step_x, "10.16");
        assert_eq!(s.offset_x, "25.40");
    }
}
