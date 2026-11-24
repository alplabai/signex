//! The board 3D view (#2390, mockup row 175): one board shown in 3D in the main
//! window, with its toolbar's state (the side shown, the flip in progress,
//! explode, lighting, the screenshot waiting for a frame). View state like
//! pan and zoom: nothing here touches the board, its undo history or its
//! dirty mark. See `docs/RENDERING_RULES.md`, "board-3d".

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use signex_gfx::camera3d::{FLIP_DURATION_MS, LightingPreset, OrbitCamera, ViewPreset, ViewSide};
use signex_renderer::board3d::{
    Board3dColors, Board3dInput, BoardScene3d, ComponentModel, Explode, build_board_scene,
};
use signex_renderer::pcb::PcbLayerView;
use signex_types::part::FootprintSourceRef;
use signex_types::pcb::PcbBoard;

use crate::app::Board3dShot;

/// Viewport shape assumed until the view reports its size.
const ASSUMED_VIEWPORT: [f32; 2] = [1200.0, 800.0];

/// What a built scene was built from: rebuilt when any of it changes.
#[derive(Clone, PartialEq)]
struct SceneKey {
    revision: u64,
    view: PcbLayerView,
    models: u64,
}

/// A built scene and the generation that names its geometry.
#[derive(Clone)]
pub struct BuiltScene {
    pub scene: Arc<BoardScene3d>,
    pub generation: u64,
}

/// The flip in progress: the camera turns from `from` to `to`; `started` is
/// the first frame's time.
#[derive(Clone, Copy, Debug)]
struct FlipTween {
    from: OrbitCamera,
    to: OrbitCamera,
    started: Option<Instant>,
}

/// A screenshot waiting for a frame drawn without the toolbar. `save_to`
/// is the PNG to write (`None` copies it); `asked` once the capture ran.
/// A transparent one is captured twice, over black then over white
/// (`over_black` holds the first), and the two passes give each pixel's
/// coverage.
#[derive(Clone, Debug)]
struct Capture {
    save_to: Option<PathBuf>,
    asked: bool,
    transparent: bool,
    over_black: Option<Arc<Board3dShot>>,
}

/// The flat backdrop a transparent screenshot's pass is drawn over, in
/// place of the lighting preset's gradient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatteBackdrop {
    Black,
    White,
}

/// What a captured frame completes.
#[derive(Debug, Clone, PartialEq)]
pub enum CaptureResult {
    /// The pass over black was kept; the next frame is captured over white.
    Pending,
    /// The screenshot as captured, or nothing when the window had none.
    Plain(Option<Arc<Board3dShot>>),
    /// A transparent screenshot's two passes.
    Matte {
        black: Arc<Board3dShot>,
        white: Arc<Board3dShot>,
    },
}

/// One board in the 3D view.
pub struct Board3dSession {
    /// The board shown; the view is drawn while it is the main window's tab.
    pub board_path: PathBuf,
    pub camera: OrbitCamera,
    /// The view's size in logical pixels, as it last reported it.
    pub viewport: [f32; 2],
    /// Where the view's top-left corner sits in the window, logical pixels.
    pub origin: [f32; 2],
    /// The face shown: the toolbar's Flip turns the board over.
    pub side: ViewSide,
    /// The toolbar preset the view looks from; a turn by hand clears it.
    pub active_preset: Option<ViewPreset>,
    /// How far the board's layers and parts stand apart.
    pub explode: Explode,
    /// The Explode popover is open.
    pub explode_open: bool,
    /// The lighting preset, which also picks the backdrop.
    pub lighting: LightingPreset,
    /// The generation of the lighting toast on show.
    pub lighting_toast: Option<u64>,
    /// The screenshot menu is open.
    pub shot_menu_open: bool,
    /// Parts whose model could not be read since the toast was dismissed:
    /// the source, as text, and why.
    pub unread: Vec<(String, String)>,
    toast_generation: u64,
    flip: Option<FlipTween>,
    capture: Option<Capture>,
    /// The view has not reported its size since it was framed; the first
    /// size a redraw reports frames it again, unless a gesture came first.
    unsized_fit: bool,
    /// The models read for the library footprints the board's parts came
    /// from; a source that could not be read has none and draws as a body.
    models: HashMap<FootprintSourceRef, ComponentModel>,
    /// Sources asked for, answered or not, so each is read once.
    requested: HashSet<FootprintSourceRef>,
    /// Bumped whenever `models` changes.
    models_version: u64,
    /// The scene built for the last key; built on demand by the view and by
    /// picking, as the board canvas keeps its own scene cache.
    built: RefCell<Option<(SceneKey, BuiltScene)>>,
    /// The exploded scene, keyed by the assembled scene's generation and
    /// the explode it was moved by.
    exploded: RefCell<Option<(u64, Explode, BuiltScene)>>,
}

impl Board3dSession {
    /// The 3D view of the board at `board_path`, from Iso on the top side
    /// (mockup row 175); the caller frames it.
    pub fn open(board_path: &Path) -> Self {
        let side = ViewSide::default();
        let mut camera = OrbitCamera::default();
        camera.set_preset_on(ViewPreset::Iso, side);
        Self {
            board_path: board_path.to_path_buf(),
            camera,
            viewport: ASSUMED_VIEWPORT,
            origin: [0.0, 0.0],
            side,
            active_preset: Some(ViewPreset::Iso),
            explode: Explode::ASSEMBLED,
            explode_open: false,
            lighting: LightingPreset::default(),
            lighting_toast: None,
            shot_menu_open: false,
            unread: Vec::new(),
            toast_generation: 0,
            flip: None,
            capture: None,
            unsized_fit: true,
            models: HashMap::new(),
            requested: HashSet::new(),
            models_version: 0,
            built: RefCell::new(None),
            exploded: RefCell::new(None),
        }
    }

    /// The scene of `board` at `revision` as `view` shows it, moved apart by
    /// the explode, built when anything it depends on changed since the last
    /// call. Drawing and picking both read it.
    pub fn scene(
        &self,
        board: &PcbBoard,
        revision: u64,
        view: &PcbLayerView,
        colors: Board3dColors,
    ) -> BuiltScene {
        let assembled = self.assembled_scene(board, revision, view, colors);
        if self.explode.is_assembled() {
            return assembled;
        }
        let mut exploded = self.exploded.borrow_mut();
        if let Some((generation, explode, scene)) = exploded.as_ref()
            && *generation == assembled.generation
            && *explode == self.explode
        {
            return scene.clone();
        }
        let scene = BuiltScene {
            scene: Arc::new(assembled.scene.exploded(self.explode)),
            generation: crate::scene_generation::next_scene_generation(),
        };
        *exploded = Some((assembled.generation, self.explode, scene.clone()));
        scene
    }

    fn assembled_scene(
        &self,
        board: &PcbBoard,
        revision: u64,
        view: &PcbLayerView,
        colors: Board3dColors,
    ) -> BuiltScene {
        let key = SceneKey {
            revision,
            view: view.clone(),
            models: self.models_version,
        };
        let mut built = self.built.borrow_mut();
        if let Some((cached, scene)) = built.as_ref()
            && *cached == key
        {
            return scene.clone();
        }
        let scene = BuiltScene {
            scene: Arc::new(build_board_scene(&Board3dInput {
                board,
                view,
                models: &self.models,
                colors,
            })),
            generation: crate::scene_generation::next_scene_generation(),
        };
        if scene.scene.dropped_holes > 0 {
            crate::diagnostics::log_debug(format!(
                "[board-3d] {} left {} holes out of the board: each meets a hole cut before it",
                self.board_path.display(),
                scene.scene.dropped_holes
            ));
        }
        *built = Some((key, scene.clone()));
        scene
    }

    /// Frame `scene` in the view, from the direction it is seen from now.
    pub fn fit(&mut self, scene: &BoardScene3d) {
        self.end_flip();
        if let Some((lo, hi)) = scene.bounds() {
            let [w, h] = self.viewport;
            self.camera.fit(lo, hi, w / h.max(1.0));
        }
    }

    /// Look from `preset` on the side shown, framing `scene` again.
    pub fn set_preset(&mut self, preset: ViewPreset, scene: &BoardScene3d) {
        self.end_flip();
        self.camera.set_preset_on(preset, self.side);
        self.fit(scene);
        self.active_preset = Some(preset);
    }

    /// Reset: Iso on the side shown, framed. Flip, explode and lighting
    /// stay.
    pub fn reset(&mut self, scene: &BoardScene3d) {
        self.set_preset(ViewPreset::Iso, scene);
    }

    /// Turn the board over: the side shown changes now, the camera turns
    /// over the next [`FLIP_DURATION_MS`] of frames.
    pub fn flip(&mut self) {
        self.end_flip();
        self.flip = Some(FlipTween {
            from: self.camera,
            to: self.camera.flipped(),
            started: None,
        });
        self.side = match self.side {
            ViewSide::Top => ViewSide::Bottom,
            ViewSide::Bottom => ViewSide::Top,
        };
        self.active_preset = None;
    }

    /// A frame at `now`: the flip in progress moves on, and ends once its
    /// time is up.
    pub fn tick(&mut self, now: Instant) {
        let Some(tween) = self.flip.as_mut() else {
            return;
        };
        let started = *tween.started.get_or_insert(now);
        let (from, to) = (tween.from, tween.to);
        let duration = Duration::from_millis(FLIP_DURATION_MS).as_secs_f32();
        let t = now.saturating_duration_since(started).as_secs_f32() / duration;
        if t >= 1.0 {
            self.end_flip();
        } else {
            self.camera = from.tween(&to, t);
        }
    }

    /// End the flip in progress at its target: a gesture, a preset or a
    /// fit during the turn starts from where the flip was going.
    pub fn end_flip(&mut self) {
        if let Some(tween) = self.flip.take() {
            self.camera = tween.to;
        }
    }

    /// The flip is still turning.
    pub fn is_flipping(&self) -> bool {
        self.flip.is_some()
    }

    /// The view needs frames: a flip turns, or a screenshot waits for a
    /// frame drawn without the toolbar.
    pub fn needs_frames(&self) -> bool {
        self.flip.is_some() || self.capture.as_ref().is_some_and(|c| !c.asked)
    }

    /// Start a screenshot: the toolbar and its overlays hide, and the next
    /// frame is captured. `save_to` is the PNG to write; `None` copies it.
    pub fn begin_capture(&mut self, save_to: Option<PathBuf>) {
        self.explode_open = false;
        self.shot_menu_open = false;
        self.capture = Some(Capture {
            save_to,
            asked: false,
            transparent: false,
            over_black: None,
        });
    }

    /// Start a screenshot with a transparent backdrop, written to `save_to`
    /// (mockup row 175): like [`Self::begin_capture`], over black and then
    /// over white.
    pub fn begin_transparent_capture(&mut self, save_to: PathBuf) {
        self.begin_capture(Some(save_to));
        if let Some(capture) = self.capture.as_mut() {
            capture.transparent = true;
        }
    }

    /// The flat backdrop the view is drawn over while a transparent
    /// screenshot is taken; `None` draws the lighting preset's.
    pub fn matte_backdrop(&self) -> Option<MatteBackdrop> {
        let capture = self.capture.as_ref().filter(|c| c.transparent)?;
        Some(if capture.over_black.is_some() {
            MatteBackdrop::White
        } else {
            MatteBackdrop::Black
        })
    }

    /// A capture came back as `shot`. The pass over black of a transparent
    /// screenshot is kept and the next frame is captured again; otherwise
    /// the capture ends, the overlays show again, and what to copy or write
    /// is returned. A pass that caught nothing ends the capture with
    /// `Plain(None)`.
    pub fn finish_pass(&mut self, shot: Option<Arc<Board3dShot>>) -> CaptureResult {
        if let Some(capture) = self.capture.as_mut().filter(|c| c.transparent)
            && let Some(caught) = shot.as_ref()
        {
            match capture.over_black.take() {
                None => {
                    capture.over_black = Some(Arc::clone(caught));
                    capture.asked = false;
                    return CaptureResult::Pending;
                }
                Some(black) => {
                    self.capture = None;
                    return CaptureResult::Matte {
                        black,
                        white: Arc::clone(caught),
                    };
                }
            }
        }
        self.capture = None;
        CaptureResult::Plain(shot)
    }

    /// The capture a frame should run now, marked as run: `Some(save_to)`.
    pub fn take_capture(&mut self) -> Option<Option<PathBuf>> {
        let capture = self.capture.as_mut().filter(|c| !c.asked)?;
        capture.asked = true;
        Some(capture.save_to.clone())
    }

    /// A capture for `save_to` was taken and its pass is awaited. A pass
    /// that arrives when this is false belongs to a capture that ended.
    pub fn awaits_pass(&self, save_to: Option<&Path>) -> bool {
        self.capture
            .as_ref()
            .is_some_and(|c| c.asked && c.save_to.as_deref() == save_to)
    }

    /// The screenshot came back, or will not: the overlays show again.
    pub fn end_capture(&mut self) {
        self.capture = None;
    }

    /// A screenshot is under way: the toolbar and its overlays are hidden.
    pub fn is_capturing(&self) -> bool {
        self.capture.is_some()
    }

    /// The next lighting preset; its toast shows under a new generation,
    /// which is returned.
    pub fn cycle_lighting(&mut self) -> u64 {
        self.lighting = self.lighting.next();
        self.toast_generation += 1;
        self.lighting_toast = Some(self.toast_generation);
        self.toast_generation
    }

    /// The lighting toast of `generation` timed out; a newer one stays.
    pub fn expire_lighting_toast(&mut self, generation: u64) {
        if self.lighting_toast == Some(generation) {
            self.lighting_toast = None;
        }
    }

    /// The view reported its size on a redraw. `true` when this is the
    /// first size since it was framed, so the caller frames it again for
    /// the real shape.
    pub fn note_viewport(&mut self, viewport: [f32; 2]) -> bool {
        if viewport[0] > 0.0 && viewport[1] > 0.0 {
            self.viewport = viewport;
            return std::mem::take(&mut self.unsized_fit);
        }
        false
    }

    /// The view reported where it sits in the window.
    pub fn note_origin(&mut self, origin: [f32; 2]) {
        if origin.iter().all(|v| v.is_finite()) {
            self.origin = origin;
        }
    }

    /// A gesture saw the view at `viewport`. It never frames the view
    /// again: a refit before the gesture's own step would make it jump.
    pub fn resize(&mut self, viewport: [f32; 2]) {
        if viewport[0] > 0.0 && viewport[1] > 0.0 {
            self.viewport = viewport;
            self.unsized_fit = false;
        }
    }

    /// The sources of `board`'s footprints not asked for yet; they are
    /// marked asked.
    pub fn take_unrequested(&mut self, board: &PcbBoard) -> Vec<FootprintSourceRef> {
        let mut out = Vec::new();
        for source in board
            .footprints
            .iter()
            .filter_map(|f| f.library_ref.as_ref())
        {
            if self.requested.insert(source.clone()) {
                out.push(source.clone());
            }
        }
        out
    }

    /// Models read for some sources: the scene is built again with them.
    pub fn add_models(
        &mut self,
        read: impl IntoIterator<Item = (FootprintSourceRef, ComponentModel)>,
    ) {
        let before = self.models.len();
        self.models.extend(read);
        if self.models.len() != before {
            self.models_version += 1;
        }
    }

    /// The model read for `source`, if any.
    pub fn model(&self, source: &FootprintSourceRef) -> Option<&ComponentModel> {
        self.models.get(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mockup row 175: the view opens on Iso, the tinted preset, looking
    /// from Iso at the top side.
    #[test]
    fn a_new_view_opens_on_iso() {
        let session = Board3dSession::open(Path::new("board.snxpcb"));
        assert_eq!(session.active_preset, Some(ViewPreset::Iso));
        assert_eq!(session.side, ViewSide::Top);
        let mut iso = OrbitCamera::default();
        iso.set_preset(ViewPreset::Iso);
        assert_eq!(
            (session.camera.yaw_deg, session.camera.pitch_deg),
            (iso.yaw_deg, iso.pitch_deg)
        );
        assert_ne!(session.camera.pitch_deg, OrbitCamera::default().pitch_deg);
    }
}
