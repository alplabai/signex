//! Start screen project thumbnails (review #1): a small line-art render of
//! the project's board, or of its root sheet when it has no board, cached on
//! save and shown on the project rows.
//!
//! The snapshot is built from the model in `update` ([`snapshot`]), kept in
//! [`ProjectThumbnails`] and written to the per-user cache off the UI thread
//! ([`cache`]); start-up reads the cache for the recent and pinned projects.
//! The tile ([`tile`]) only draws what the state holds. Cache location and
//! format: `docs/PROJECT_THUMBNAILS.md`.

pub mod cache;
pub mod snapshot;
mod tile;

#[cfg(test)]
pub(crate) mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use serde::{Deserialize, Serialize};
use signex_i18n::{ids, t};

use crate::app::{Message, ProjectMsg, Signex};

pub use tile::{Inks, ink_colour, sheet_canvas, tile};

/// The longest side of a thumbnail's view box; every coordinate lies in
/// `0..=VIEW_BOX_SIDE`.
pub const VIEW_BOX_SIDE: f32 = 1000.0;

/// What a thumbnail shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThumbnailKind {
    /// The project's board, on the board canvas.
    Board,
    /// The project's root sheet, on the schematic paper.
    Sheet,
}

/// The colour role of a line-art primitive. The tile turns it into a colour
/// at draw time, so a cached render follows the theme it is shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ink {
    BoardOutline,
    TopCopper,
    BottomCopper,
    /// Through-hole pads and vias.
    MultiLayer,
    TopSilkscreen,
    BottomSilkscreen,
    Wire,
    SymbolBody,
    PowerPort,
}

impl Ink {
    /// Paint order: lower first, so the top face draws over the bottom one
    /// and the board edge stays visible over everything.
    pub fn draw_order(self) -> u8 {
        match self {
            Self::BottomSilkscreen => 0,
            Self::BottomCopper => 1,
            Self::TopCopper => 2,
            Self::MultiLayer => 3,
            Self::TopSilkscreen => 4,
            Self::BoardOutline => 5,
            Self::Wire => 6,
            Self::SymbolBody => 7,
            Self::PowerPort => 8,
        }
    }
}

/// The geometry of one primitive, in view-box units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Geometry {
    Polyline { points: Vec<[f32; 2]>, closed: bool },
    Circle { center: [f32; 2], radius: f32 },
}

/// One stroke (or filled area) of the line art.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Primitive {
    pub ink: Ink,
    /// Drawn as an area (pads, vias) rather than an outline.
    #[serde(default)]
    pub filled: bool,
    pub geometry: Geometry,
}

/// A project preview: what it shows, the view box its primitives are laid
/// out in (width, height; the longer is [`VIEW_BOX_SIDE`]) and the line art,
/// in paint order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thumbnail {
    pub kind: ThumbnailKind,
    pub view_box: [f32; 2],
    pub primitives: Vec<Primitive>,
}

/// What a project row's thumbnail tile shows.
#[derive(Debug, Clone, PartialEq)]
pub enum ThumbnailSlot {
    /// No cached render yet: the project was not saved since previews exist.
    NoPreview,
    /// The project file is gone; never a stale picture.
    MissingOnDisk,
    Ready(Arc<Thumbnail>),
}

/// The thumbnails known this session, by project path as the recent and
/// pinned lists spell it.
#[derive(Debug, Clone, Default)]
pub struct ProjectThumbnails {
    by_project: HashMap<PathBuf, Arc<Thumbnail>>,
}

impl ProjectThumbnails {
    /// What the tile of `project` shows. A project missing on disk shows the
    /// missing tile even when a render of it is known.
    pub fn slot(&self, project: &Path, is_missing: bool) -> ThumbnailSlot {
        if is_missing {
            return ThumbnailSlot::MissingOnDisk;
        }
        match self.by_project.get(project) {
            Some(thumbnail) => ThumbnailSlot::Ready(Arc::clone(thumbnail)),
            None => ThumbnailSlot::NoPreview,
        }
    }

    /// The render of `project`, if one is known.
    pub fn get(&self, project: &Path) -> Option<&Arc<Thumbnail>> {
        self.by_project.get(project)
    }

    /// Keep `thumbnail` as the render of `project`, replacing an older one.
    pub fn insert(&mut self, project: PathBuf, thumbnail: Arc<Thumbnail>) {
        self.by_project.insert(project, thumbnail);
    }

    /// Keep a render read from the cache, unless a save this session already
    /// made a newer one. Returns whether it was kept.
    pub fn insert_cached(&mut self, project: PathBuf, thumbnail: Arc<Thumbnail>) -> bool {
        if self.by_project.contains_key(&project) {
            return false;
        }
        self.by_project.insert(project, thumbnail);
        true
    }

    /// Move the render kept under `from` to `to`: the save keyed it by the
    /// open project's path, the lists spell it canonically.
    pub fn rekey(&mut self, from: &Path, to: PathBuf) {
        if from == to {
            return;
        }
        if let Some(thumbnail) = self.by_project.remove(from) {
            self.by_project.insert(to, thumbnail);
        }
    }

    pub fn len(&self) -> usize {
        self.by_project.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_project.is_empty()
    }
}

/// The start-up cache read: the renders found and the files that could not
/// be read (a missing file is simply no preview and is not listed).
#[derive(Debug, Clone, Default)]
pub struct LoadedThumbnails {
    pub found: Vec<(PathBuf, Arc<Thumbnail>)>,
    pub unreadable: Vec<(PathBuf, String)>,
}

/// A cache write that landed: the project as the save named it, and as the
/// recent and pinned lists spell it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenThumbnail {
    pub project: PathBuf,
    pub listed_as: PathBuf,
}

/// A cache write that failed, with the reason and the spelling the recent
/// and pinned lists use (the render kept this session moves to it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailWriteFailure {
    pub project: PathBuf,
    pub listed_as: PathBuf,
    pub reason: String,
}

/// Thumbnail events, under `ProjectMsg::Thumbnail`.
#[derive(Debug, Clone)]
pub enum ProjectThumbnailMsg {
    /// The start-up read of the cached renders finished; `Err` is a worker
    /// that died.
    Loaded(Result<LoadedThumbnails, String>),
    /// A cache write finished.
    Written(Result<WrittenThumbnail, ThumbnailWriteFailure>),
}

fn message(msg: ProjectThumbnailMsg) -> Message {
    Message::Project(ProjectMsg::Thumbnail(msg))
}

impl Signex {
    /// Route a thumbnail event. Exhaustive: a new event is a compile error
    /// until it is routed.
    pub(crate) fn dispatch_project_thumbnail_message(
        &mut self,
        msg: ProjectThumbnailMsg,
    ) -> Task<Message> {
        match msg {
            ProjectThumbnailMsg::Loaded(Ok(loaded)) => {
                for (project, thumbnail) in loaded.found {
                    self.ui_state
                        .project_thumbnails
                        .insert_cached(project, thumbnail);
                }
                for (project, reason) in loaded.unreadable {
                    self.warn_thumbnail_read_failed(format!("{}: {reason}", project.display()));
                }
            }
            ProjectThumbnailMsg::Loaded(Err(reason)) => self.warn_thumbnail_read_failed(reason),
            ProjectThumbnailMsg::Written(Ok(written)) => self
                .ui_state
                .project_thumbnails
                .rekey(&written.project, written.listed_as),
            ProjectThumbnailMsg::Written(Err(failure)) => {
                // The render still shows this session, under the listed
                // spelling the Start screen rows look it up by.
                self.ui_state
                    .project_thumbnails
                    .rekey(&failure.project, failure.listed_as);
                crate::diagnostics::log_warning(
                    t!(
                        self.ui_state.i18n,
                        ids::START_THUMBNAIL_WRITE_FAILED,
                        "project" => failure.project.display().to_string(),
                        "reason" => failure.reason
                    )
                    .into_string(),
                )
            }
        }
        Task::none()
    }

    fn warn_thumbnail_read_failed(&self, reason: String) {
        crate::diagnostics::log_warning(
            t!(self.ui_state.i18n, ids::START_THUMBNAIL_READ_FAILED, "reason" => reason)
                .into_string(),
        );
    }

    /// Read the cached renders of the recent and pinned projects on a
    /// blocking worker; the answer comes back as
    /// [`ProjectThumbnailMsg::Loaded`]. Boot runs it once.
    pub fn load_project_thumbnails(&self) -> Task<Message> {
        let mut projects: Vec<PathBuf> = self.ui_state.recent_projects.entries().to_vec();
        for pinned in self.ui_state.pinned_projects.entries() {
            if !projects.contains(pinned) {
                projects.push(pinned.clone());
            }
        }
        if projects.is_empty() {
            return Task::none();
        }
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                match crate::config_root::config_root() {
                    Some(root) => cache::read_all(&root, &projects),
                    None => LoadedThumbnails::default(),
                }
            }),
            |result| message(ProjectThumbnailMsg::Loaded(result)),
        )
    }

    /// Keep `thumbnail` as the render of `project` at once and return the
    /// task that writes it to the cache.
    pub(crate) fn keep_project_thumbnail(
        &mut self,
        project: PathBuf,
        thumbnail: Thumbnail,
    ) -> Task<Message> {
        let thumbnail = Arc::new(thumbnail);
        self.ui_state
            .project_thumbnails
            .insert(project.clone(), Arc::clone(&thumbnail));
        let failed = project.clone();
        Task::perform(
            crate::app::path_op_jobs::run_blocking(move || {
                cache::write_listed(&project, &thumbnail)
            }),
            move |result| {
                // A worker that died never learnt the listed spelling: the
                // render stays under the open path.
                let result = result
                    .map_err(|reason| ThumbnailWriteFailure {
                        listed_as: failed.clone(),
                        project: failed,
                        reason,
                    })
                    .and_then(|written| written);
                message(ProjectThumbnailMsg::Written(result))
            },
        )
    }
}
