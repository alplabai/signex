//! Row symbol thumbnails of the Components panel (#2535): the cache, the
//! off-thread read that fills it, and the small view-only line-art drawing
//! (transparent, no paper or grid, like the Properties previews). A library
//! re-read (a save, an external edit) marks its thumbnails stale; the old
//! drawing stays up until the fresh read lands (#2631).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use iced::widget::canvas::{self, Geometry};
use iced::{Element, Length, Rectangle, Size, Theme, Widget, mouse};
use signex_library::primitive::PrimitiveRef;
use signex_types::schematic::{Aabb, LibSymbol};
use signex_types::theme::CanvasColors;
use uuid::Uuid;

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
    /// Read (or being read) before their library was re-read: asked for
    /// again, still drawn from `read` until the fresh copy lands.
    stale: HashSet<PrimitiveRef>,
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

    /// The library `library_id` was re-read: its symbols may have changed
    /// on disk, so every thumbnail of it is read again when next wanted.
    pub fn mark_library_stale(&mut self, library_id: Uuid) {
        let of_library = |reference: &&PrimitiveRef| reference.library_id == library_id;
        let marked: Vec<PrimitiveRef> = self
            .read
            .keys()
            .chain(self.asked.iter())
            .filter(of_library)
            .copied()
            .collect();
        self.stale.extend(marked);
    }

    /// Of `wanted`, the references not read (or read stale) and not being
    /// read, at most [`MAX_PER_READ`], each marked as asked.
    pub fn take_missing(
        &mut self,
        wanted: impl IntoIterator<Item = PrimitiveRef>,
    ) -> Vec<PrimitiveRef> {
        let mut missing = Vec::new();
        for reference in wanted {
            if missing.len() >= MAX_PER_READ {
                break;
            }
            let fresh = self.read.contains_key(&reference) && !self.stale.contains(&reference);
            if fresh || !self.asked.insert(reference) {
                continue;
            }
            // Marked stale again while this read runs: asked once more after.
            self.stale.remove(&reference);
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

/// What a cached thumbnail drawing was built from: the read copy (a fresh
/// read is a new `Arc`), the theme colours and the scale. The box size is
/// the cache's own check.
#[derive(Debug, Clone)]
struct ThumbShown {
    lib: Arc<LibSymbol>,
    colors: CanvasColors,
    device_px_per_logical_px: f32,
}

impl ThumbShown {
    fn of(thumb: &SymbolThumb) -> Self {
        Self {
            lib: Arc::clone(&thumb.lib),
            colors: thumb.colors,
            device_px_per_logical_px: thumb.device_px_per_logical_px,
        }
    }

    fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.lib, &other.lib)
            && self.colors == other.colors
            && self.device_px_per_logical_px == other.device_px_per_logical_px
    }
}

/// The drawing kept between frames, so a hover or another row's redraw
/// does not rebuild and tessellate every visible thumbnail.
#[derive(Default)]
struct ThumbCache {
    cache: canvas::Cache,
    shown: RefCell<Option<ThumbShown>>,
}

impl ThumbCache {
    /// Drop the drawing when it was built from anything but `current`
    /// (re-read symbol, theme switch, another row reusing this widget).
    fn follow(&self, current: ThumbShown) {
        let mut shown = self.shown.borrow_mut();
        if !shown.as_ref().is_some_and(|s| s.same_as(&current)) {
            self.cache.clear();
            *shown = Some(current);
        }
    }
}

impl<M> canvas::Program<M> for SymbolThumb {
    type State = ThumbCache;

    fn draw(
        &self,
        state: &ThumbCache,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        state.follow(ThumbShown::of(self));
        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            self.paint(frame, bounds.size());
        });
        vec![geometry]
    }
}

impl SymbolThumb {
    /// The line art of the symbol, centred in `size`.
    fn paint(&self, frame: &mut canvas::Frame, size: Size) {
        if size.width > 0.0 && size.height > 0.0 {
            let (sheet, symbol) = preview_sheet(&self.lib);
            let transform = thumb_transform(
                symbol_bounds(&sheet, &symbol),
                size,
                self.device_px_per_logical_px,
            );
            if let Some(scene) = build_schematic_scene(
                &sheet,
                &transform,
                &self.colors,
                Rectangle::with_size(size),
                None,
                SceneOverrides::default(),
            ) {
                crate::renderer_scene_canvas::draw_scene_with_world_to_screen(
                    frame,
                    &scene,
                    |point| transform.world_to_screen((point[0] as f64, point[1] as f64)),
                    preview_draw_options(&transform),
                );
            }
        }
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
    iced::widget::container::<_, Theme>(canvas)
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

    fn symbol(name: &str) -> Arc<LibSymbol> {
        Arc::new(serde_json::from_value(serde_json::json!({ "id": name })).expect("a bare symbol"))
    }

    /// A re-read library's thumbnails are asked again, the old copy drawn
    /// until the fresh one lands; another library's stay as they are.
    #[test]
    fn a_reread_library_rereads_its_thumbnails() {
        let mut thumbs = SymbolThumbs::default();
        let other = PrimitiveRef::new(uuid::Uuid::from_u128(2), uuid::Uuid::from_u128(1));
        let asked = thumbs.take_missing([reference(1), other]);
        let old = symbol("old");
        thumbs.finish(asked.into_iter().map(|r| (r, Some(old.clone()))).collect());

        thumbs.mark_library_stale(uuid::Uuid::from_u128(1));
        assert_eq!(thumbs.take_missing([reference(1), other]), [reference(1)]);
        assert!(
            thumbs
                .get(&reference(1))
                .is_some_and(|s| Arc::ptr_eq(s, &old)),
            "the old drawing stays up while the fresh read runs"
        );
        assert!(thumbs.take_missing([reference(1)]).is_empty(), "asked once");
        let new = symbol("new");
        thumbs.finish(vec![(reference(1), Some(new.clone()))]);
        assert!(
            thumbs
                .get(&reference(1))
                .is_some_and(|s| Arc::ptr_eq(s, &new))
        );
        assert!(thumbs.take_missing([reference(1), other]).is_empty());
    }

    /// A re-read that lands while an older read runs asks once more after
    /// it: the running read may have seen the file before the change.
    #[test]
    fn a_reread_during_a_read_asks_again_after_it() {
        let mut thumbs = SymbolThumbs::default();
        let asked = thumbs.take_missing([reference(1)]);
        thumbs.mark_library_stale(uuid::Uuid::from_u128(1));
        thumbs.finish(
            asked
                .into_iter()
                .map(|r| (r, Some(symbol("old"))))
                .collect(),
        );
        assert_eq!(thumbs.take_missing([reference(1)]), [reference(1)]);
    }

    /// The cached drawing is kept for the same copy, colours and scale, and
    /// dropped by a fresh read or a theme switch.
    #[test]
    fn the_drawing_is_kept_until_the_symbol_or_theme_changes() {
        use signex_types::theme::{ThemeId, canvas_colors};
        let shown = |lib: &Arc<LibSymbol>, theme| ThumbShown {
            lib: Arc::clone(lib),
            colors: canvas_colors(theme),
            device_px_per_logical_px: 1.0,
        };
        let lib = symbol("R");
        let light = shown(&lib, ThemeId::GraphiteLight);
        assert!(light.same_as(&shown(&lib, ThemeId::GraphiteLight)));
        assert!(!light.same_as(&shown(&symbol("R"), ThemeId::GraphiteLight)));
        assert!(!light.same_as(&shown(&lib, ThemeId::GraphiteDark)));
        let mut scaled = shown(&lib, ThemeId::GraphiteLight);
        scaled.device_px_per_logical_px = 2.0;
        assert!(!light.same_as(&scaled));

        let cache = ThumbCache::default();
        cache.follow(light.clone());
        cache.follow(light.clone());
        assert!(
            cache
                .shown
                .borrow()
                .as_ref()
                .is_some_and(|s| s.same_as(&light))
        );
        cache.follow(shown(&lib, ThemeId::GraphiteDark));
        assert!(
            !cache
                .shown
                .borrow()
                .as_ref()
                .is_some_and(|s| s.same_as(&light))
        );
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
