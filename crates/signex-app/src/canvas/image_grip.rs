//! The eight resize handles of a selected image (#1116, mockup row 204):
//! square handles on the corners and the sides of its box. A drag moves the
//! handle's edges; while the image keeps its aspect a corner keeps it and a
//! side does nothing, and the drop is one edit (`ImagesMsg::HandleDropped`).
//! The drag steps on the snap grid. A locked image has no handles. The
//! geometry is `signex_types::schematic::resized_by_handle`; the drawing is
//! `draw/image_handles.rs`.

use super::*;
use signex_types::schematic::{
    ImageHandle, Point as SheetPoint, SchDrawing, SchImage, SchematicSheet, SelectedKind,
    resized_by_handle,
};

/// How near (screen px) a press must land to a handle to take it.
pub(in crate::canvas) const IMAGE_HANDLE_HIT_PX: f32 = 6.0;

/// A handle drag in flight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImageDrag {
    pub image: uuid::Uuid,
    pub handle: ImageHandle,
    /// Where the press landed, world mm.
    origin: (f64, f64),
    /// The box when the drag began: top-left and size.
    from: (SheetPoint, (f64, f64)),
    keep_aspect: bool,
    /// The box as the drag has it now.
    pub now: (SheetPoint, (f64, f64)),
}

impl ImageDrag {
    /// The image's scale at the drag's width, as a percentage of its file's
    /// own size; `None` when that size is not known.
    pub(in crate::canvas) fn scale_percent(&self, natural_w: Option<f64>) -> Option<f64> {
        natural_w.map(|w| self.now.1.0 / w * 100.0)
    }

    /// What the release does: one edit when the box changed, else nothing.
    pub(in crate::canvas) fn dropped(self) -> canvas::Action<Message> {
        if self.now == self.from {
            return canvas::Action::capture();
        }
        canvas::Action::publish(Message::from(crate::app::ImagesMsg::HandleDropped {
            id: self.image,
            position: self.now.0,
            size: self.now.1,
        }))
    }
}

/// The selected, unlocked images of `sheet`.
pub(in crate::canvas) fn selected_images<'s>(
    sheet: &'s SchematicSheet,
    selected: &'s [signex_types::schematic::SelectedItem],
) -> impl Iterator<Item = &'s SchImage> {
    selected
        .iter()
        .filter(|item| item.kind == SelectedKind::Drawing)
        .filter_map(|item| sheet.drawings.iter().find(|d| d.uuid() == item.uuid))
        .filter_map(|drawing| match drawing {
            SchDrawing::Image(image) if !image.locked => Some(image),
            _ => None,
        })
}

impl SchematicCanvas<'_> {
    /// The drag a press at the screen point `at` starts, if it lands on a
    /// handle of a selected image; the nearest handle wins.
    pub(in crate::canvas) fn image_handle_at(
        &self,
        at: iced::Point,
        bounds: Rectangle,
        world: (f64, f64),
    ) -> Option<ImageDrag> {
        let sheet = self.active_snapshot()?;
        let camera = self.camera();
        let mut best: Option<(f32, ImageDrag)> = None;
        for image in selected_images(sheet, &self.selected) {
            let size = (image.width, image.height);
            for handle in ImageHandle::ALL {
                let p = handle.at(image.position, size);
                let screen =
                    camera.world_to_screen(iced::Point::new(p.x as f32, p.y as f32), bounds);
                let distance = (screen.x - at.x).hypot(screen.y - at.y);
                if distance <= IMAGE_HANDLE_HIT_PX && best.is_none_or(|(d, _)| distance < d) {
                    let drag = ImageDrag {
                        image: image.uuid,
                        handle,
                        origin: world,
                        from: (image.position, size),
                        keep_aspect: image.keep_aspect,
                        now: (image.position, size),
                    };
                    best = Some((distance, drag));
                }
            }
        }
        best.map(|(_, drag)| drag)
    }

    /// Follow the cursor while a handle is held: the delta steps on the snap
    /// grid.
    pub(in crate::canvas) fn update_image_drag(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let drag = state.image_drag.as_mut()?;
        let at = cursor.position_in(bounds)?;
        let world = self.camera().screen_to_world(at, bounds);
        let mut delta = (
            f64::from(world.x) - drag.origin.0,
            f64::from(world.y) - drag.origin.1,
        );
        if self.prefs.snap_enabled && self.prefs.snap_grid_mm > 0.0 {
            let grid = self.prefs.snap_grid_mm;
            delta = (
                (delta.0 / grid).round() * grid,
                (delta.1 / grid).round() * grid,
            );
        }
        drag.now = resized_by_handle(
            drag.from.0,
            drag.from.1,
            drag.handle,
            delta,
            drag.keep_aspect,
        );
        let event = CanvasEvent::CursorAt {
            x: world.x,
            y: world.y,
            zoom_pct: self.camera().zoom_percent(),
        };
        Some(canvas::Action::publish(Message::CanvasEvent(event)).and_capture())
    }
}
