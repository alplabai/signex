//! The Gerber Viewer tab's state (mockup rows 193 and 193f): what is open,
//! how each layer looks, the drawing order, the selection and the view.
//! Pure data and pure rules; the reads run in `read.rs`, off the UI thread.

use std::path::PathBuf;
use std::sync::Arc;

use pcb_geom::PointNm;
use signex_gerber::{FileFunction, Package, Side};

use super::recent::RecentPackages;

/// The unit Properties shows lengths in. The files keep their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LengthUnit {
    #[default]
    Mm,
    Mil,
}

/// Nanometres in a millimetre and in a mil.
const NM_PER_MM: f64 = 1.0e6;
const NM_PER_MIL: f64 = 25_400.0;

impl LengthUnit {
    pub const ALL: [Self; 2] = [Self::Mm, Self::Mil];

    /// `nm` in this unit, as Properties prints it: three decimals in mm,
    /// one in mil.
    pub fn show(self, nm: i64) -> String {
        match self {
            Self::Mm => format!("{:.3}", nm as f64 / NM_PER_MM),
            Self::Mil => format!("{:.1}", nm as f64 / NM_PER_MIL),
        }
    }
}

/// Where the board view looks: `zoom` 1 fits the board, `pan` moves the
/// view centre from the board centre, in nm of the file frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub zoom: f64,
    pub pan: (f64, f64),
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: (0.0, 0.0),
        }
    }
}

/// One object of the package: its layer (an index of `Package::layers`)
/// and its place in that layer's objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectRef {
    pub layer: usize,
    pub object: usize,
}

/// How one layer is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerLook {
    /// Red, green and blue.
    pub colour: [u8; 3],
    pub visible: bool,
}

/// A layer row held by its grip: where it was and the row under the
/// pointer, both places in the drawing order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GripDrag {
    pub from: usize,
    pub over: usize,
}

/// The colours a swatch offers, the defaults first.
pub const SWATCHES: [[u8; 3]; 10] = [
    [0xD0, 0x40, 0x40],
    [0xC8, 0xA0, 0x2C],
    [0x3C, 0xA8, 0x6A],
    [0x38, 0x6C, 0xD0],
    [0x40, 0x9C, 0x58],
    [0xE8, 0xE8, 0xE8],
    [0xA8, 0xA8, 0xB8],
    [0xE0, 0x00, 0xE0],
    [0x6F, 0xD0, 0xFF],
    [0x9D, 0xB3, 0xF2],
];

/// Inner copper colours, by position from the top.
const INNER: [[u8; 3]; 2] = [SWATCHES[1], SWATCHES[2]];
const PLATED_HOLE: [u8; 3] = [0xD8, 0xD8, 0xD8];

/// The colour a layer opens in, from its function (mockup row 193).
pub fn default_colour(function: Option<&FileFunction>) -> [u8; 3] {
    match function {
        Some(FileFunction::Copper { number, side }) => match side {
            signex_gerber::CopperSide::Top => SWATCHES[0],
            signex_gerber::CopperSide::Bottom => SWATCHES[3],
            signex_gerber::CopperSide::Inner => INNER[(*number as usize) % INNER.len()],
        },
        Some(FileFunction::Legend(_)) => SWATCHES[5],
        Some(FileFunction::Paste(_)) => SWATCHES[6],
        Some(FileFunction::Soldermask(_)) => SWATCHES[4],
        Some(FileFunction::Profile { .. }) => SWATCHES[7],
        Some(FileFunction::Drill { plated: true, .. }) => PLATED_HOLE,
        Some(FileFunction::Drill { plated: false, .. }) => SWATCHES[8],
        Some(FileFunction::Other(_)) | None => SWATCHES[9],
    }
}

/// Whether a layer is a drill file.
pub fn is_drill(function: Option<&FileFunction>) -> bool {
    matches!(function, Some(FileFunction::Drill { .. }))
}

/// Whether a layer draws in the band above the copper whatever its list
/// place (mockup row 193): the board outline, a drill file or a drawing.
pub fn is_overlay(function: Option<&FileFunction>) -> bool {
    matches!(
        function,
        Some(FileFunction::Profile { .. } | FileFunction::Drill { .. } | FileFunction::Other(_))
            | None
    )
}

/// A package on the tab.
#[derive(Debug, Clone)]
pub struct OpenPackage {
    pub package: Arc<Package>,
    /// What was opened: one folder, one zip, or the files.
    pub paths: Vec<PathBuf>,
    /// The drawing order, index 0 on top; each value a layer index.
    pub order: Vec<usize>,
    /// By layer index.
    pub looks: Vec<LayerLook>,
    /// The layer row last clicked: Properties shows it with nothing
    /// selected.
    pub current: Option<usize>,
    pub selected: Option<ObjectRef>,
    /// Seen from the bottom: the board mirrored, each pair's order swapped.
    pub flipped: bool,
    /// The holes drawn over everything.
    pub drills_on_top: bool,
    /// The X2 net Highlight net keeps at full strength.
    pub net: Option<String>,
    pub camera: Camera,
    /// The layer whose swatch row is open.
    pub colour_open: Option<usize>,
    pub grip: Option<GripDrag>,
    /// Counts every change to what the board draws; the board view redraws
    /// its cached picture when it moves.
    pub revision: u64,
    /// The box of every layer, found once at open.
    bounds: Option<(PointNm, PointNm)>,
}

impl OpenPackage {
    pub fn new(package: Arc<Package>, paths: Vec<PathBuf>) -> Self {
        let looks = package
            .layers
            .iter()
            .map(|layer| LayerLook {
                colour: default_colour(layer.function.as_ref()),
                visible: true,
            })
            .collect();
        Self {
            order: package.default_order(),
            bounds: package_bounds(&package),
            package,
            paths,
            looks,
            current: None,
            selected: None,
            flipped: false,
            drills_on_top: false,
            net: None,
            camera: Camera::default(),
            colour_open: None,
            grip: None,
            revision: 0,
        }
    }

    /// A layer row's name was pressed: Properties shows that layer.
    pub fn pick_layer(&mut self, layer: usize) {
        self.current = Some(layer);
        self.selected = None;
    }

    /// A swatch opens its colour row, or closes it when it is open.
    pub fn toggle_swatch(&mut self, layer: usize) {
        self.colour_open = (self.colour_open != Some(layer)).then_some(layer);
    }

    pub fn press_grip(&mut self, from: usize) {
        self.grip = Some(GripDrag { from, over: from });
    }

    pub fn enter_row(&mut self, over: usize) {
        if let Some(grip) = self.grip.as_mut() {
            grip.over = over;
        }
    }

    /// The button was let go: a held row moves to the row last under the
    /// pointer.
    pub fn release_grip(&mut self) {
        if let Some(grip) = self.grip.take() {
            self.move_layer(grip.from, grip.over);
        }
    }

    /// Highlight net takes the selected object's net, or lets it go.
    pub fn toggle_net_highlight(&mut self) {
        self.net = match self.net {
            Some(_) => None,
            None => self.selected_net().map(str::to_string),
        };
    }

    /// A click on the board selects what is under it; a row still held
    /// by a release missed elsewhere is let go.
    pub fn click_at(&mut self, at: PointNm, tolerance: i64) {
        self.selected = self.object_at(at, tolerance);
        self.colour_open = None;
        self.grip = None;
    }

    /// Move the row at `from` in the drawing order to `to`.
    pub fn move_layer(&mut self, from: usize, to: usize) {
        if from >= self.order.len() || to >= self.order.len() || from == to {
            return;
        }
        let layer = self.order.remove(from);
        self.order.insert(to, layer);
    }

    pub fn toggle_visible(&mut self, layer: usize) {
        if let Some(look) = self.looks.get_mut(layer) {
            look.visible = !look.visible;
        }
    }

    pub fn set_colour(&mut self, layer: usize, colour: [u8; 3]) {
        if let Some(look) = self.looks.get_mut(layer) {
            look.colour = colour;
        }
        self.colour_open = None;
    }

    /// The layers bottom first, as they are drawn: the list order turned
    /// over, each pair's places swapped while flipped, the outline, drill
    /// files and drawings above the copper (mockup row 193), and with
    /// Drills on the drill files last.
    pub fn draw_order(&self) -> Vec<usize> {
        let mut order = self.order.clone();
        if self.flipped {
            self.swap_pairs(&mut order);
        }
        order.reverse();
        let function = |layer: &usize| self.package.layers[*layer].function.as_ref();
        let (overlay, stack): (Vec<usize>, Vec<usize>) = order
            .into_iter()
            .partition(|layer| is_overlay(function(layer)));
        order = stack.into_iter().chain(overlay).collect();
        if self.drills_on_top {
            let (drills, rest): (Vec<usize>, Vec<usize>) = order
                .into_iter()
                .partition(|layer| is_drill(function(layer)));
            order = rest.into_iter().chain(drills).collect();
        }
        order
    }

    fn swap_pairs(&self, order: &mut [usize]) {
        let place = |layer: usize, order: &[usize]| order.iter().position(|l| *l == layer);
        for (layer, entry) in self.package.layers.iter().enumerate() {
            let Some(partner) = entry.pair.filter(|p| *p > layer) else {
                continue;
            };
            if let (Some(a), Some(b)) = (place(layer, order), place(partner, order)) {
                order.swap(a, b);
            }
        }
    }

    /// Whether a layer draws at full strength: the active face (the top,
    /// or the bottom while flipped) and anything not on a face.
    pub fn is_near_side(&self, layer: usize) -> bool {
        let function = self.package.layers[layer].function.as_ref();
        let far = if self.flipped {
            Side::Top
        } else {
            Side::Bottom
        };
        match function {
            Some(FileFunction::Copper {
                side: signex_gerber::CopperSide::Inner,
                ..
            }) => false,
            Some(function) => function.side() != Some(far),
            None => true,
        }
    }

    /// The topmost visible object at `at` within `tolerance` nm, in the
    /// order the board draws.
    pub fn object_at(&self, at: PointNm, tolerance: i64) -> Option<ObjectRef> {
        self.draw_order()
            .into_iter()
            .rev()
            .filter(|layer| self.looks.get(*layer).is_some_and(|look| look.visible))
            .find_map(|layer| {
                let objects = &self.package.layers[layer].image.objects;
                objects
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, object)| covers(object, at, tolerance))
                    .map(|(object, _)| ObjectRef { layer, object })
            })
    }

    pub fn object(&self, at: ObjectRef) -> Option<&signex_gerber::Object> {
        self.package
            .layers
            .get(at.layer)?
            .image
            .objects
            .get(at.object)
    }

    /// The X2 net of the selected object.
    pub fn selected_net(&self) -> Option<&str> {
        self.object(self.selected?)?.attributes.net.as_deref()
    }

    /// The box holding every layer, as (min, max).
    pub fn bounds(&self) -> Option<(PointNm, PointNm)> {
        self.bounds
    }
}

/// The box holding every layer of `package`, as (min, max).
fn package_bounds(package: &Package) -> Option<(PointNm, PointNm)> {
    package
        .layers
        .iter()
        .filter_map(|layer| layer.image.bounds())
        .reduce(|a, b| {
            (
                PointNm::new(a.0.x.min(b.0.x), a.0.y.min(b.0.y)),
                PointNm::new(a.1.x.max(b.1.x), a.1.y.max(b.1.y)),
            )
        })
}

/// Whether `object` covers `at`: one of its shapes, or the points that
/// place it when it has none (drawn as a marker).
fn covers(object: &signex_gerber::Object, at: PointNm, tolerance: i64) -> bool {
    if !object.shapes.is_empty() {
        return object
            .shapes
            .iter()
            .any(|shape| shape.contains(at, tolerance));
    }
    let reach = i128::from(tolerance.max(0)).pow(2);
    object
        .anchors()
        .into_iter()
        .any(|point| point.distance_sq(at) <= reach)
}

/// The Gerber Viewer, app-wide: one tab at a time shows it.
#[derive(Debug, Clone, Default)]
pub struct GerberViewerState {
    pub open: Option<OpenPackage>,
    /// The read in flight, by job number; a later open supersedes it.
    pub reading: Option<u64>,
    pub next_job: u64,
    /// Files dragged over a window, as the window reported them.
    pub hovered: Vec<PathBuf>,
    /// Files dropped and not opened yet.
    pub dropped: Vec<PathBuf>,
    pub recent: RecentPackages,
    pub unit: LengthUnit,
    /// The Open menu shows.
    pub open_menu: bool,
    /// The clock, in Unix seconds, read in `update` when the recent list
    /// was last checked; the rows' times read against it.
    pub now: Option<i64>,
    /// The Export Job run the files came from (mockup row 193b).
    pub from_run: Option<super::RunOrigin>,
    /// The run's files could not be read: they moved since.
    pub run_missing: bool,
}

impl GerberViewerState {
    /// A new job number for a read.
    pub fn start_read(&mut self) -> u64 {
        self.next_job += 1;
        self.reading = Some(self.next_job);
        self.next_job
    }

    /// Whether a toolbar tool that needs files can act.
    pub fn has_files(&self) -> bool {
        self.open.is_some()
    }

    /// Whether Esc on the tab has something to undo: the Open menu, a held
    /// row or a selection.
    pub fn has_escape(&self) -> bool {
        self.open_menu
            || self
                .open
                .as_ref()
                .is_some_and(|open| open.grip.is_some() || open.selected.is_some())
    }

    /// Esc on the tab: the Open menu closes first, then a held row is let
    /// go where it was, then the selection clears.
    pub fn escape(&mut self) {
        if self.open_menu {
            self.open_menu = false;
            return;
        }
        let Some(open) = self.open.as_mut() else {
            return;
        };
        if open.grip.take().is_none() {
            open.selected = None;
        }
        open.revision += 1;
    }

    /// Note a dropped file. `Some(paths)` once every hovered file landed:
    /// the drop to open, as one package.
    pub fn note_drop(&mut self, path: PathBuf) -> Option<Vec<PathBuf>> {
        self.dropped.push(path);
        (self.dropped.len() >= self.hovered.len() && !self.hovered.is_empty())
            .then(|| self.take_drop())
    }

    /// The dropped files, and the drag over.
    pub fn take_drop(&mut self) -> Vec<PathBuf> {
        self.hovered.clear();
        std::mem::take(&mut self.dropped)
    }
}
