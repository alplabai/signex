//! The board 3D view (#2390, mockup row 175): enter and leave it, turn, slide and
//! zoom it, look from a preset, flip, explode and light it, frame it, pick
//! a part, and read the parts' models off the update thread. Screenshots
//! are in `board_3d_screenshot`. View state only: no board command, no undo
//! step, no dirty mark. See `docs/RENDERING_RULES.md`, "board-3d".

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use iced::{Subscription, Task};
use pcb_geom::ItemRef;
use signex_i18n::{ids, t};
use signex_library::adapters::local_git::LocalGitAdapter;
use signex_renderer::board3d::{AttachedModel, BodyExtrusion, ComponentModel, read_model_file};
use signex_types::part::FootprintSourceRef;
use uuid::Uuid;

use crate::app::state::{Board3dSession, BuiltScene};
use crate::app::{Board3dGesture, Board3dModelsRead, Board3dMsg, Message, PcbMsg, Signex};
use crate::library::place::open_library;
use crate::pcb_canvas::layer_style::board_3d_colors;

/// How long the lighting toast stays up.
const LIGHTING_TOAST: Duration = Duration::from_secs(2);

/// `event` as the app message that carries it.
pub(crate) fn board_3d_message(event: Board3dMsg) -> Message {
    Message::Pcb(PcbMsg::Board3d(event))
}

impl Signex {
    pub(crate) fn handle_board_3d(&mut self, event: Board3dMsg) -> Task<Message> {
        match event {
            Board3dMsg::Toggled if self.board_3d_in_view().is_some() => {
                self.interaction_state.board_3d = None;
            }
            Board3dMsg::Toggled => return self.enter_board_3d(),
            Board3dMsg::LeaveRequested => {
                if self.board_3d_in_view().is_some() {
                    self.interaction_state.board_3d = None;
                }
            }
            // Its keys and the pointer still reach the view while a
            // screenshot hides the toolbar; a flip, a new lighting or a turn,
            // slide, zoom or pick between the two passes of a transparent
            // save would spoil the matte.
            Board3dMsg::FlipRequested | Board3dMsg::LightingCycled | Board3dMsg::Pointer { .. }
                if self.board_3d_is_capturing() => {}
            Board3dMsg::PresetPicked(preset) => {
                self.with_board_3d_scene(|session, scene| session.set_preset(preset, scene));
            }
            Board3dMsg::ResetRequested => {
                self.with_board_3d_scene(|session, scene| session.reset(scene));
            }
            Board3dMsg::FitRequested => {
                self.fit_board_3d();
            }
            Board3dMsg::FlipRequested => self.edit_board_3d(Board3dSession::flip),
            Board3dMsg::AnimationTicked(now) => return self.tick_board_3d(now),
            Board3dMsg::ExplodePopoverToggled => self.edit_board_3d(|session| {
                session.explode_open = !session.explode_open;
                session.shot_menu_open = false;
            }),
            Board3dMsg::ExplodeAmountChanged(percent) => self.edit_board_3d(|session| {
                session.explode.amount = f32::from(percent.min(100)) / 100.0;
            }),
            Board3dMsg::ExplodeLayersToggled(on) => self.edit_board_3d(|session| {
                session.explode.separate_copper_and_mask = on;
            }),
            Board3dMsg::ExplodePartsToggled(on) => self.edit_board_3d(|session| {
                session.explode.lift_components = on;
            }),
            Board3dMsg::LightingCycled => return self.cycle_board_3d_lighting(),
            Board3dMsg::LightingToastExpired(generation) => {
                if let Some(session) = self.interaction_state.board_3d.as_mut() {
                    session.expire_lighting_toast(generation);
                }
            }
            Board3dMsg::ScreenshotMenuToggled => self.edit_board_3d(|session| {
                session.shot_menu_open = !session.shot_menu_open;
                session.explode_open = false;
            }),
            Board3dMsg::PopoverDismissed => self.edit_board_3d(|session| {
                session.explode_open = false;
                session.shot_menu_open = false;
            }),
            Board3dMsg::ScreenshotCopyRequested => self.start_board_3d_capture(None, false),
            Board3dMsg::ScreenshotSaveRequested => return self.ask_board_3d_png(false),
            Board3dMsg::ScreenshotSavePicked(path) => {
                if let Some(path) = path {
                    self.start_board_3d_capture(Some(path), false);
                }
            }
            Board3dMsg::ScreenshotTransparentSaveRequested => return self.ask_board_3d_png(true),
            Board3dMsg::ScreenshotTransparentSavePicked(path) => {
                if let Some(path) = path {
                    self.start_board_3d_capture(Some(path), true);
                }
            }
            Board3dMsg::ScreenshotTaken { save_to, shot } => {
                return self.take_board_3d_shot(save_to, shot);
            }
            Board3dMsg::ScreenshotCopied(copied) => self.report_board_3d_copy(copied.as_ref()),
            Board3dMsg::ScreenshotSaved { path, error } => {
                self.report_board_3d_save(&path, error.as_deref());
            }
            Board3dMsg::UnreadNoticeDismissed => {
                if let Some(session) = self.interaction_state.board_3d.as_mut() {
                    session.unread.clear();
                }
            }
            Board3dMsg::ViewportPlaced(origin) => {
                self.edit_board_3d(|session| session.note_origin(origin));
            }
            Board3dMsg::ViewportSized(viewport) => self.size_board_3d(viewport),
            Board3dMsg::Pointer { viewport, gesture } => self.board_3d_pointer(viewport, gesture),
            Board3dMsg::ModelsRead { board, read } => self.take_board_3d_models(&board, &read),
        }
        Task::none()
    }

    /// Frames for the 3D view while its flip turns or a screenshot waits
    /// for a frame drawn without the toolbar; none otherwise.
    pub(crate) fn board_3d_subscription(&self) -> Subscription<Message> {
        let animating = self
            .interaction_state
            .board_3d
            .as_ref()
            .is_some_and(Board3dSession::needs_frames);
        if animating {
            iced::window::frames().map(|at| board_3d_message(Board3dMsg::AnimationTicked(at)))
        } else {
            Subscription::none()
        }
    }

    /// Esc closes the 3D view's open Explode popover or screenshot menu
    /// before it reaches the tool reset behind the view.
    pub(crate) fn board_3d_popover_escape(&self) -> Option<Message> {
        self.board_3d_in_view()
            .is_some_and(|session| session.explode_open || session.shot_menu_open)
            .then(|| board_3d_message(Board3dMsg::PopoverDismissed))
    }

    /// Change the 3D view in view with `edit`.
    fn edit_board_3d(&mut self, edit: impl FnOnce(&mut Board3dSession)) {
        if let Some(session) = self.board_3d_in_view_mut() {
            edit(session);
        }
    }

    /// Change the 3D view in view with `edit`, which reads its scene.
    fn with_board_3d_scene(
        &mut self,
        edit: impl FnOnce(&mut Board3dSession, &signex_renderer::board3d::BoardScene3d),
    ) {
        if let Some(built) = self.board_3d_scene()
            && let Some(session) = self.board_3d_in_view_mut()
        {
            edit(session, built.scene.as_ref());
        }
    }

    /// The next lighting preset, and a toast that hides after
    /// [`LIGHTING_TOAST`] unless a newer one replaced it.
    fn cycle_board_3d_lighting(&mut self) -> Task<Message> {
        let Some(session) = self.board_3d_in_view_mut() else {
            return Task::none();
        };
        let generation = session.cycle_lighting();
        // Built inside the future: `sleep` needs the runtime the task runs on.
        Task::perform(
            async { tokio::time::sleep(LIGHTING_TOAST).await },
            move |()| board_3d_message(Board3dMsg::LightingToastExpired(generation)),
        )
    }

    /// A frame was drawn: the flip moves on, and a screenshot waiting for a
    /// frame without the toolbar is taken.
    fn tick_board_3d(&mut self, now: std::time::Instant) -> Task<Message> {
        if let Some(session) = self.interaction_state.board_3d.as_mut() {
            session.tick(now);
        }
        self.capture_board_3d()
    }

    /// The 3D view of the board the main window shows, if it is on.
    pub(crate) fn board_3d_in_view(&self) -> Option<&Board3dSession> {
        let session = self.interaction_state.board_3d.as_ref()?;
        let shown = self.active_tab_path()?;
        (shown == session.board_path && self.has_active_pcb()).then_some(session)
    }

    /// Whether a key or command from `window` meets the 3D view (mockup row
    /// 175): only the main window draws it, so the `board_3d` shortcut
    /// context and the 3D view's command gate read this one answer.
    pub(crate) fn board_3d_shown_in(&self, window: Option<iced::window::Id>) -> bool {
        matches!(
            self.input_target(window),
            crate::app::dispatch::input::InputTarget::Main
        ) && self.board_3d_in_view().is_some()
    }

    /// [`Self::board_3d_in_view`], mutable.
    pub(crate) fn board_3d_in_view_mut(&mut self) -> Option<&mut Board3dSession> {
        let shown = self.active_tab_path()?;
        if !self.has_active_pcb() {
            return None;
        }
        self.interaction_state
            .board_3d
            .as_mut()
            .filter(|session| session.board_path == shown)
    }

    /// The board's file moved from `from` to `to` (a rename): its 3D view
    /// follows it, as its sketch mode does.
    pub(crate) fn rekey_board_3d(&mut self, from: &Path, to: &Path) {
        if let Some(session) = self
            .interaction_state
            .board_3d
            .as_mut()
            .filter(|session| session.board_path == from)
        {
            session.board_path = to.to_path_buf();
        }
    }

    /// The board at `path` closed: its 3D view ends with the tab, so the
    /// file opens flat again.
    pub(crate) fn end_board_3d_of(&mut self, path: &Path) {
        if self
            .interaction_state
            .board_3d
            .as_ref()
            .is_some_and(|session| session.board_path == path)
        {
            self.interaction_state.board_3d = None;
        }
    }

    /// The scene of the board in the 3D view, built when the board, its
    /// layer view or its models changed since it was last built.
    pub(crate) fn board_3d_scene(&self) -> Option<BuiltScene> {
        let session = self.board_3d_in_view()?;
        let path = session.board_path.as_path();
        let engine = self.document_state.board_engine(path)?;
        let view = self.pcb_layer_view(path);
        Some(session.scene(engine.board(), engine.revision(), &view, board_3d_colors()))
    }

    /// Frame the board in the 3D view. Whether a 3D view was in view.
    pub(crate) fn fit_board_3d(&mut self) -> bool {
        let Some(built) = self.board_3d_scene() else {
            return false;
        };
        if let Some(session) = self.board_3d_in_view_mut() {
            session.fit(&built.scene);
        }
        true
    }

    /// Show the board in view in 3D, framed from above, and read its parts'
    /// models. Sketch mode of that board is left first (its changes
    /// applied); a refused apply keeps the board flat.
    fn enter_board_3d(&mut self) -> Task<Message> {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return Task::none();
        };
        if !self.leave_board_sketch_of(&path) {
            return Task::none();
        }
        self.interaction_state.board_3d = Some(Board3dSession::open(&path));
        self.fit_board_3d();
        self.read_board_3d_models(path)
    }

    /// Read, off the update thread, the models of the library footprints
    /// the parts of the board at `path` came from that were not asked for.
    fn read_board_3d_models(&mut self, path: PathBuf) -> Task<Message> {
        let Some(board) = self.document_state.pcb_board(&path) else {
            return Task::none();
        };
        let Some(session) = self.interaction_state.board_3d.as_mut() else {
            return Task::none();
        };
        let sources = session.take_unrequested(board);
        if sources.is_empty() {
            return Task::none();
        }
        let open_libraries: Vec<(Uuid, PathBuf)> = self
            .library
            .open_libraries
            .iter()
            .map(|l| (l.library_id, l.root.clone()))
            .collect();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_models(&sources, &open_libraries))
                    .await
                    .unwrap_or_else(|e| Board3dModelsRead {
                        models: Vec::new(),
                        failures: vec![("-".to_string(), format!("the model read failed: {e}"))],
                    })
            },
            move |read| {
                Message::Pcb(PcbMsg::Board3d(Board3dMsg::ModelsRead {
                    board: path.clone(),
                    read: Arc::new(read),
                }))
            },
        )
    }

    fn take_board_3d_models(&mut self, board: &Path, read: &Board3dModelsRead) {
        for (source, why) in &read.failures {
            let line = t!(
                self.ui_state.i18n,
                ids::PCB_3D_MODEL_UNREAD,
                "source" => source.as_str(),
                "why" => why.as_str()
            );
            crate::diagnostics::log_warning(line.into_string());
        }
        if let Some(session) = self
            .interaction_state
            .board_3d
            .as_mut()
            .filter(|session| session.board_path == board)
        {
            session.add_models(read.models.iter().cloned());
            // The same lines also show as the view's warn toast (mockup row 109).
            session.unread.extend(read.failures.iter().cloned());
        }
    }

    /// The view reported its size; the first report frames it again for its
    /// real shape.
    fn size_board_3d(&mut self, viewport: [f32; 2]) {
        let Some(built) = self.board_3d_scene() else {
            return;
        };
        if let Some(session) = self.board_3d_in_view_mut()
            && session.note_viewport(viewport)
        {
            session.fit(&built.scene);
        }
    }

    fn board_3d_pointer(&mut self, viewport: [f32; 2], gesture: Board3dGesture) {
        let Some(built) = self.board_3d_scene() else {
            return;
        };
        let Some(session) = self.board_3d_in_view_mut() else {
            return;
        };
        session.resize(viewport);
        session.end_flip();
        session.shot_menu_open = false;
        let [width, height] = session.viewport;
        match gesture {
            Board3dGesture::Orbited { dx, dy } => {
                session.camera.orbit(dx, dy);
                session.active_preset = None;
            }
            Board3dGesture::Panned { dx, dy } => session.camera.pan(dx, dy, height),
            Board3dGesture::Zoomed(steps) => session.camera.zoom(steps),
            Board3dGesture::Clicked(at) => {
                let (origin, dir) = session.camera.ray(at, width, height);
                let hit = built.scene.pick(origin, dir);
                self.select_from_board_3d(hit);
            }
        }
    }

    /// A click in 3D selects the footprint of the part it hit, in the flat
    /// view too; a click on the board or on nothing clears the selection.
    fn select_from_board_3d(&mut self, hit: Option<Uuid>) {
        match hit {
            Some(footprint) => self.select_pcb_items(&[ItemRef::Footprint(footprint)]),
            None => {
                let cleared = self
                    .with_pcb_selection(|selection, _| selection.clear_selection())
                    .unwrap_or(false);
                if cleared {
                    self.refresh_pcb_highlight();
                }
            }
        }
    }
}

/// Read the model of each of `sources` from the open libraries, each
/// library opened once. Blocking IO.
pub(crate) fn read_models(
    sources: &[FootprintSourceRef],
    open_libraries: &[(Uuid, PathBuf)],
) -> Board3dModelsRead {
    let mut libraries: HashMap<Uuid, Result<(LocalGitAdapter, PathBuf), String>> = HashMap::new();
    let mut read = Board3dModelsRead::default();
    for source in sources {
        let library = libraries
            .entry(source.library)
            .or_insert_with(|| open_library(source.library, open_libraries));
        let model = match &*library {
            Ok((adapter, root)) => read_model(source, adapter, root),
            Err(why) => Err(why.clone()),
        };
        match model {
            Ok(model) => read.models.push((source.clone(), model)),
            Err(why) => read.failures.push((source.to_string(), why)),
        }
    }
    read
}

/// The model of the library footprint `source` from its open library
/// `adapter` at `root`: its attached model file, placed by the attachment's
/// offset and turn; else its 3D Body.
fn read_model(
    source: &FootprintSourceRef,
    adapter: &LocalGitAdapter,
    root: &Path,
) -> Result<ComponentModel, String> {
    use signex_library::LibraryAdapter as _;

    let footprint = adapter
        .get_footprint(source.footprint)
        .map_err(|e| e.to_string())?;
    if let Some(attachment) = footprint.step_attachment.as_ref() {
        let hash = attachment.content_hash.as_str();
        // The hash names a file in `step/`; only a hex digest may, so a
        // hostile footprint file cannot point the read elsewhere.
        if hash.is_empty() || hash.len() > 128 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("its model name {hash} is not a hex digest"));
        }
        let dir = root
            .parent()
            .ok_or_else(|| format!("{} has no folder", root.display()))?;
        let path = dir.join("step").join(format!("{hash}.step"));
        let meshes = read_model_file(&path, &attachment.filename)?;
        return Ok(ComponentModel::Model(AttachedModel {
            meshes,
            offset_mm: attachment.offset_xyz,
            rotation_deg: attachment.rotation_xyz,
        }));
    }
    let body = &footprint.body_3d;
    Ok(ComponentModel::Body(BodyExtrusion {
        outline: body.outline.as_ref().map(|outline| outline.points.clone()),
        height_mm: f64::from(body.height_mm),
        offset_z_mm: f64::from(body.offset_z_mm),
        colors: Some((body.top_color, body.side_color)),
    }))
}
