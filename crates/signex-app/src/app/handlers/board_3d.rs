//! The board 3D view (#2390): enter and leave it, turn, slide and zoom it,
//! look from a preset, frame it, pick a part, and read the parts' models
//! off the update thread. View state only: no board command, no undo step,
//! no dirty mark. See `docs/RENDERING_RULES.md`, "Board 3D view".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use pcb_geom::ItemRef;
use signex_i18n::{ids, t};
use signex_renderer::board3d::{AttachedModel, BodyExtrusion, ComponentModel, read_model_file};
use signex_types::part::FootprintSourceRef;
use uuid::Uuid;

use crate::app::state::{Board3dSession, BuiltScene};
use crate::app::{Board3dGesture, Board3dModelsRead, Board3dMsg, Message, PcbMsg, Signex};
use crate::pcb_canvas::layer_style::board_3d_colors;

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
            Board3dMsg::PresetPicked(preset) => {
                if let Some(built) = self.board_3d_scene()
                    && let Some(session) = self.board_3d_in_view_mut()
                {
                    session.set_preset(preset, &built.scene);
                }
            }
            Board3dMsg::FitRequested => {
                self.fit_board_3d();
            }
            Board3dMsg::Pointer { viewport, gesture } => self.board_3d_pointer(viewport, gesture),
            Board3dMsg::ModelsRead { board, read } => self.take_board_3d_models(&board, &read),
        }
        Task::none()
    }

    /// The 3D view of the board the main window shows, if it is on.
    pub(crate) fn board_3d_in_view(&self) -> Option<&Board3dSession> {
        let session = self.interaction_state.board_3d.as_ref()?;
        let shown = self.active_tab_path()?;
        (shown == session.board_path && self.has_active_pcb()).then_some(session)
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
        }
    }

    fn board_3d_pointer(&mut self, viewport: [f32; 2], gesture: Board3dGesture) {
        let Some(built) = self.board_3d_scene() else {
            return;
        };
        let Some(session) = self.board_3d_in_view_mut() else {
            return;
        };
        if session.note_viewport(viewport) {
            session.fit(&built.scene);
        }
        let [width, height] = session.viewport;
        match gesture {
            Board3dGesture::Orbited { dx, dy } => session.camera.orbit(dx, dy),
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

/// Read the model of each of `sources` from the open libraries. Blocking IO.
fn read_models(
    sources: &[FootprintSourceRef],
    open_libraries: &[(Uuid, PathBuf)],
) -> Board3dModelsRead {
    let mut read = Board3dModelsRead::default();
    for source in sources {
        match read_model(source, open_libraries) {
            Ok(model) => read.models.push((source.clone(), model)),
            Err(why) => read.failures.push((source.to_string(), why)),
        }
    }
    read
}

/// The model of the library footprint `source`: its attached model file,
/// placed by the attachment's offset and turn; else its 3D Body.
fn read_model(
    source: &FootprintSourceRef,
    open_libraries: &[(Uuid, PathBuf)],
) -> Result<ComponentModel, String> {
    use signex_library::LibraryAdapter as _;
    use signex_library::adapters::local_git::LocalGitAdapter;

    let (_, root) = open_libraries
        .iter()
        .find(|(id, _)| *id == source.library)
        .ok_or_else(|| format!("its library {} is not open", source.library))?;
    let adapter = LocalGitAdapter::open(root)
        .map_err(|e| format!("could not open {}: {e}", root.display()))?;
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
