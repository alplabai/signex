//! The board 3D view (#2390): one board shown in 3D in the main window.
//! View state like pan and zoom: nothing here touches the board, its undo
//! history or its dirty mark. See `docs/RENDERING_RULES.md`, "Board 3D view".

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_gfx::camera3d::{OrbitCamera, ViewPreset};
use signex_renderer::board3d::{
    Board3dColors, Board3dInput, BoardScene3d, ComponentModel, build_board_scene,
};
use signex_renderer::pcb::PcbLayerView;
use signex_types::part::FootprintSourceRef;
use signex_types::pcb::PcbBoard;

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

/// One board in the 3D view.
pub struct Board3dSession {
    /// The board shown; the view is drawn while it is the main window's tab.
    pub board_path: PathBuf,
    pub camera: OrbitCamera,
    /// The view's size in logical pixels, as it last reported it.
    pub viewport: [f32; 2],
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
}

impl Board3dSession {
    /// The 3D view of the board at `board_path`, from straight above; the
    /// caller frames it.
    pub fn open(board_path: &Path) -> Self {
        Self {
            board_path: board_path.to_path_buf(),
            camera: OrbitCamera::default(),
            viewport: ASSUMED_VIEWPORT,
            unsized_fit: true,
            models: HashMap::new(),
            requested: HashSet::new(),
            models_version: 0,
            built: RefCell::new(None),
        }
    }

    /// The scene of `board` at `revision` as `view` shows it, built when
    /// anything it depends on changed since the last call.
    pub fn scene(
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
        if let Some((lo, hi)) = scene.bounds() {
            let [w, h] = self.viewport;
            self.camera.fit(lo, hi, w / h.max(1.0));
        }
    }

    /// Look from `preset`, framing `scene` again.
    pub fn set_preset(&mut self, preset: ViewPreset, scene: &BoardScene3d) {
        self.camera.set_preset(preset);
        self.fit(scene);
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
