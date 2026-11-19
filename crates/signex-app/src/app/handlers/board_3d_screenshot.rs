//! Screenshots of the board 3D view (m175): copy to the clipboard or save a
//! PNG. The view hides its toolbar and overlays, the next frame is captured
//! from the main window and cropped to the view, and the copy or the write
//! runs as a Task; Messages says how it went. See `docs/RENDERING_RULES.md`,
//! "board-3d".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::{Rectangle, Task};
use signex_i18n::{ids, t};

use super::board_3d::board_3d_message;
use crate::app::{Board3dMsg, Board3dShot, Message, Signex};

impl Signex {
    /// Save PNG…: ask where, off the update thread. The menu closes.
    pub(super) fn ask_board_3d_png(&mut self) -> Task<Message> {
        let Some(session) = self.board_3d_in_view_mut() else {
            return Task::none();
        };
        session.shot_menu_open = false;
        let stem = session
            .board_path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let folder = session.board_path.parent().map(Path::to_path_buf);
        let picker = self.ui_state.io.picker.clone();
        let i18n = self.ui_state.i18n.clone();
        Task::perform(
            async move {
                let mut dialog = picker
                    .dialog()
                    .title_in(&i18n, ids::PCB_3D_SHOT_SAVE_TITLE)
                    .filter_in(&i18n, ids::PCB_3D_SHOT_FILTER, &["png"])
                    .set_file_name(format!("{stem}-3d.png"));
                if let Some(folder) = folder {
                    dialog = dialog.set_directory(folder);
                }
                dialog.save_file().await
            },
            |path| board_3d_message(Board3dMsg::ScreenshotSavePicked(path)),
        )
    }

    /// A frame was drawn: the screenshot waiting for it, if any, is taken
    /// now, from the main window, and cropped to the view.
    pub(super) fn capture_board_3d(&mut self) -> Task<Message> {
        let window = self.ui_state.main_window_id;
        let in_view = self.board_3d_in_view().is_some();
        let Some(session) = self.interaction_state.board_3d.as_mut() else {
            return Task::none();
        };
        let Some(save_to) = session.take_capture() else {
            return Task::none();
        };
        let Some(window) = window.filter(|_| in_view) else {
            session.end_capture();
            self.report_board_3d_empty();
            return Task::none();
        };
        let (origin, viewport) = (session.origin, session.viewport);
        iced::window::screenshot(window).map(move |shot| {
            board_3d_message(Board3dMsg::ScreenshotTaken {
                save_to: save_to.clone(),
                shot: crop_shot(&shot, origin, viewport).map(Arc::new),
            })
        })
    }

    /// The capture came back: the overlays show again, and the shot is
    /// copied or written.
    pub(super) fn take_board_3d_shot(
        &mut self,
        save_to: Option<PathBuf>,
        shot: Option<Arc<Board3dShot>>,
    ) -> Task<Message> {
        if let Some(session) = self.interaction_state.board_3d.as_mut() {
            session.end_capture();
        }
        let Some(shot) = shot else {
            self.report_board_3d_empty();
            return Task::none();
        };
        match save_to {
            None => iced::clipboard::write(iced::clipboard::Image {
                rgba: shot.rgba.clone().into(),
                size: iced::Size::new(shot.size[0], shot.size[1]),
            })
            .map(|written| board_3d_message(Board3dMsg::ScreenshotCopied(written.is_ok()))),
            Some(path) => Task::perform(
                async move {
                    let to = path.clone();
                    let error = tokio::task::spawn_blocking(move || write_png(&to, &shot))
                        .await
                        .unwrap_or_else(|e| Err(e.to_string()))
                        .err();
                    (path, error)
                },
                |(path, error)| board_3d_message(Board3dMsg::ScreenshotSaved { path, error }),
            ),
        }
    }

    /// Messages says whether the clipboard took the screenshot.
    pub(super) fn report_board_3d_copy(&self, copied: bool) {
        let i18n = &self.ui_state.i18n;
        if copied {
            crate::diagnostics::log_info(t!(i18n, ids::PCB_3D_SHOT_COPIED).into_string());
        } else {
            crate::diagnostics::log_warning(t!(i18n, ids::PCB_3D_SHOT_COPY_FAILED).into_string());
        }
    }

    /// Messages says where the PNG went, or why it was not written.
    pub(super) fn report_board_3d_save(&self, path: &Path, error: Option<&str>) {
        let i18n = &self.ui_state.i18n;
        let shown = path.display().to_string();
        match error {
            None => crate::diagnostics::log_info(
                t!(i18n, ids::PCB_3D_SHOT_SAVED, "path" => shown).into_string(),
            ),
            Some(why) => crate::diagnostics::log_warning(
                t!(i18n, ids::PCB_3D_SHOT_SAVE_FAILED, "path" => shown, "why" => why).into_string(),
            ),
        }
    }

    fn report_board_3d_empty(&self) {
        let line = t!(self.ui_state.i18n, ids::PCB_3D_SHOT_EMPTY);
        crate::diagnostics::log_warning(line.into_string());
    }
}

/// The view's part of a window screenshot: the view at `origin`, `viewport`
/// logical pixels, in the shot's physical pixels and clamped to it. `None`
/// when the shot is empty or the view lies outside it.
fn crop_shot(
    shot: &iced::window::Screenshot,
    origin: [f32; 2],
    viewport: [f32; 2],
) -> Option<Board3dShot> {
    if shot.rgba.is_empty() {
        return None;
    }
    let size = [shot.size.width, shot.size.height];
    let region = crop_region(origin, viewport, shot.scale_factor, size)?;
    let cropped = shot.crop(region).ok()?;
    Some(Board3dShot {
        rgba: cropped.rgba.to_vec(),
        size: [cropped.size.width, cropped.size.height],
    })
}

/// The view's rectangle in physical pixels of a `size` shot taken at
/// `scale`, clamped to the shot; `None` when nothing of it is left.
pub(crate) fn crop_region(
    origin: [f32; 2],
    viewport: [f32; 2],
    scale: f32,
    size: [u32; 2],
) -> Option<Rectangle<u32>> {
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let px = |v: f32| {
        let v = (v * scale).round();
        if v.is_finite() && v > 0.0 {
            v as u32
        } else {
            0
        }
    };
    let x = px(origin[0]).min(size[0]);
    let y = px(origin[1]).min(size[1]);
    let width = px(viewport[0]).min(size[0] - x);
    let height = px(viewport[1]).min(size[1] - y);
    (width > 0 && height > 0).then_some(Rectangle {
        x,
        y,
        width,
        height,
    })
}

/// `shot` as PNG bytes.
pub(crate) fn encode_png(shot: &Board3dShot) -> Result<Vec<u8>, String> {
    use image::ImageEncoder as _;

    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            &shot.rgba,
            shot.size[0],
            shot.size[1],
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// Write `shot` to `path` as a PNG, atomically (a temporary file renamed
/// over it). Blocking IO.
fn write_png(path: &Path, shot: &Board3dShot) -> Result<(), String> {
    let bytes = encode_png(shot)?;
    signex_types::atomic_io::atomic_write(path, &bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_view_is_cropped_in_physical_pixels() {
        let region = crop_region([10.0, 20.0], [100.0, 50.0], 2.0, [400, 300]);
        assert_eq!(
            region,
            Some(Rectangle {
                x: 20,
                y: 40,
                width: 200,
                height: 100,
            })
        );
    }

    #[test]
    fn a_view_past_the_shot_is_clamped_and_one_outside_is_none() {
        let clamped = crop_region([50.0, 50.0], [100.0, 100.0], 1.0, [120, 80]);
        assert_eq!(
            clamped,
            Some(Rectangle {
                x: 50,
                y: 50,
                width: 70,
                height: 30,
            })
        );
        assert_eq!(
            crop_region([200.0, 0.0], [10.0, 10.0], 1.0, [120, 80]),
            None
        );
        assert_eq!(
            crop_region([0.0, 0.0], [10.0, 10.0], f32::NAN, [120, 80]),
            None
        );
    }

    #[test]
    fn a_shot_encodes_as_a_png_that_reads_back() {
        let shot = Board3dShot {
            rgba: [[255, 0, 0, 255], [0, 255, 0, 255]].concat(),
            size: [2, 1],
        };
        let bytes = encode_png(&shot).expect("encodes");
        let read = image::load_from_memory(&bytes).expect("decodes").to_rgba8();
        assert_eq!(read.dimensions(), (2, 1));
        assert_eq!(read.into_raw(), shot.rgba);
    }
}
