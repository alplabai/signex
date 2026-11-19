//! Row symbol thumbnails of the Components panel (#2535): the cache, the
//! off-thread read that fills it, and the small view-only line-art drawing
//! (transparent, no paper or grid, like the Properties previews).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use iced::widget::canvas::{self, Geometry};
use iced::{Element, Length, Rectangle, Size, Theme, Widget, mouse};
use signex_library::primitive::PrimitiveRef;
use signex_types::schematic::{Aabb, LibSymbol};
use signex_types::theme::CanvasColors;

use super::details::load::MountedAdapters;
use super::details::symbol_canvas::{preview_draw_options, preview_sheet};
use crate::schematic_runtime::{
    SceneOverrides, ScreenTransform, build_schematic_scene, symbol_bounds,
};

/// Most symbols one read asks for; the next library message asks for more.
pub(crate) const MAX_PER_READ: usize = 64;

/// Share of the thumbnail side kept free around the symbol.
const MARGIN_RATIO: f32 = 0.08;

/// The thumbnails read so far. `None` marks a symbol that could not be read,
/// so it is not asked for again until the mounted libraries change.
#[derive(Debug, Clone, Default)]
pub struct SymbolThumbs {
    read: HashMap<PrimitiveRef, Option<Arc<LibSymbol>>>,
    /// Asked for and not landed yet.
    asked: HashSet<PrimitiveRef>,
    /// Mounted library count the failed reads were made against.
    mounted: usize,
}

impl SymbolThumbs {
    /// The thumbnail of `reference`, when read.
    pub fn get(&self, reference: &PrimitiveRef) -> Option<&Arc<LibSymbol>> {
        self.read.get(reference)?.as_ref()
    }

    /// Forget failed reads once the mounted library count changed: a symbol
    /// in a library that has mounted since can now be read.
    pub fn note_mounted(&mut self, count: usize) {
        if self.mounted != count {
            self.mounted = count;
            self.read.retain(|_, symbol| symbol.is_some());
        }
    }

    /// Of `wanted`, the references neither read nor being read, at most
    /// [`MAX_PER_READ`], each marked as asked.
    pub fn take_missing(
        &mut self,
        wanted: impl IntoIterator<Item = PrimitiveRef>,
    ) -> Vec<PrimitiveRef> {
        let mut missing = Vec::new();
        for reference in wanted {
            if missing.len() >= MAX_PER_READ {
                break;
            }
            if self.read.contains_key(&reference) || !self.asked.insert(reference) {
                continue;
            }
            missing.push(reference);
        }
        missing
    }

    /// Store a finished read.
    pub fn finish(&mut self, read: Vec<(PrimitiveRef, Option<Arc<LibSymbol>>)>) {
        for (reference, symbol) in read {
            self.asked.remove(&reference);
            self.read.insert(reference, symbol);
        }
    }
}

/// Read the symbols `refs` name through the mounted adapters. Pure IO, run
/// in `spawn_blocking`.
pub fn load_thumbnails(
    refs: &[PrimitiveRef],
    libraries: &MountedAdapters,
) -> Vec<(PrimitiveRef, Option<Arc<LibSymbol>>)> {
    refs.iter()
        .map(|reference| {
            let symbol = libraries
                .iter()
                .find(|(id, _)| *id == reference.library_id)
                .and_then(|(_, adapter)| adapter.get_symbol(reference.uuid).ok())
                .map(|symbol| {
                    Arc::new(symbol.to_lib_symbol(crate::library::place::lib_id_of(reference)))
                });
            (*reference, symbol)
        })
        .collect()
}

/// The transform that centres `extent` in `size` with a small margin.
fn thumb_transform(extent: Aabb, size: Size, device_px_per_logical_px: f32) -> ScreenTransform {
    let width = (extent.max_x - extent.min_x).max(f64::EPSILON) as f32;
    let height = (extent.max_y - extent.min_y).max(f64::EPSILON) as f32;
    let free_w = (size.width * (1.0 - 2.0 * MARGIN_RATIO)).max(1.0);
    let free_h = (size.height * (1.0 - 2.0 * MARGIN_RATIO)).max(1.0);
    let scale = (free_w / width).min(free_h / height);
    let mid_x = ((extent.min_x + extent.max_x) / 2.0) as f32;
    let mid_y = ((extent.min_y + extent.max_y) / 2.0) as f32;
    ScreenTransform {
        offset_x: size.width / 2.0 - mid_x * scale,
        offset_y: size.height / 2.0 - mid_y * scale,
        scale,
        device_px_per_logical_px,
    }
}

struct SymbolThumb {
    lib: Arc<LibSymbol>,
    colors: CanvasColors,
    device_px_per_logical_px: f32,
}

impl<M> canvas::Program<M> for SymbolThumb {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        if bounds.width > 0.0 && bounds.height > 0.0 {
            let (sheet, symbol) = preview_sheet(&self.lib);
            let transform = thumb_transform(
                symbol_bounds(&sheet, &symbol),
                bounds.size(),
                self.device_px_per_logical_px,
            );
            if let Some(scene) = build_schematic_scene(
                &sheet,
                &transform,
                &self.colors,
                Rectangle::with_size(bounds.size()),
                None,
                SceneOverrides::default(),
            ) {
                crate::renderer_scene_canvas::draw_scene_with_world_to_screen(
                    &mut frame,
                    &scene,
                    |point| transform.world_to_screen((point[0] as f64, point[1] as f64)),
                    preview_draw_options(&transform),
                );
            }
        }
        vec![frame.into_geometry()]
    }
}

/// What every row thumbnail of one frame is drawn with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ThumbLook {
    pub colors: CanvasColors,
    pub device_px_per_logical_px: f32,
}

impl ThumbLook {
    /// The row-sized thumbnail of `lib`.
    pub(crate) fn thumb<'a, M: 'a>(&self, lib: Arc<LibSymbol>) -> Element<'a, M> {
        symbol_thumb(
            lib,
            self.colors,
            super::look::THUMB_WIDTH,
            super::look::THUMB_HEIGHT,
            self.device_px_per_logical_px,
        )
    }

    /// The empty box a thumbnail not read yet takes, so the layout holds.
    pub(crate) fn placeholder<'a, M: 'a>(&self) -> Element<'a, M> {
        iced::widget::Space::new()
            .width(super::look::THUMB_WIDTH)
            .height(super::look::THUMB_HEIGHT)
            .boxed()
    }
}

/// A `width` × `height` line-art thumbnail of `lib`; view-only, publishes
/// nothing.
pub(crate) fn symbol_thumb<'a, M: 'a>(
    lib: Arc<LibSymbol>,
    colors: CanvasColors,
    width: f32,
    height: f32,
    device_px_per_logical_px: f32,
) -> Element<'a, M> {
    let canvas = canvas::Canvas::new(SymbolThumb {
        lib,
        colors,
        device_px_per_logical_px,
    })
    .width(width)
    .height(height);
    iced::widget::container::<M, Theme>(canvas)
        .width(Length::Fixed(width))
        .height(Length::Fixed(height))
        .clip(true)
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(n: u128) -> PrimitiveRef {
        PrimitiveRef::new(uuid::Uuid::from_u128(1), uuid::Uuid::from_u128(n))
    }

    #[test]
    fn missing_references_are_asked_once_and_capped() {
        let mut thumbs = SymbolThumbs::default();
        let wanted: Vec<PrimitiveRef> = (0..(MAX_PER_READ as u128 + 10)).map(reference).collect();
        let first = thumbs.take_missing(wanted.iter().copied());
        assert_eq!(first.len(), MAX_PER_READ);
        let second = thumbs.take_missing(wanted.iter().copied());
        assert_eq!(second.len(), 10, "the asked ones are not asked again");
        assert!(thumbs.take_missing(wanted.iter().copied()).is_empty());
    }

    #[test]
    fn a_failed_read_is_retried_only_after_the_mounts_change() {
        let mut thumbs = SymbolThumbs::default();
        thumbs.note_mounted(1);
        let asked = thumbs.take_missing([reference(1)]);
        thumbs.finish(asked.into_iter().map(|r| (r, None)).collect());
        assert!(thumbs.get(&reference(1)).is_none());
        assert!(thumbs.take_missing([reference(1)]).is_empty());
        thumbs.note_mounted(2);
        assert_eq!(thumbs.take_missing([reference(1)]), [reference(1)]);
    }
}
