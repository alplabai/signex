//! Bounded cache of shaped text buffers.
//!
//! Shaping is the expensive part of staging a text item and its result is a
//! pure function of the key below, so an unchanged label costs one hash
//! lookup per frame instead of a re-shape. Font and line-height are quantised
//! (see [`quantise_px`]) so a zoom reuses shapes instead of missing every
//! label on every frame.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::primitive::text::{TextHAlign, TextItem};

/// Most shaped buffers kept; the whole cache is cleared when it fills.
pub(crate) const SHAPED_CACHE_CAPACITY: usize = 4096;

/// Sub-pixel steps per pixel for font size and line height (1/8 px).
const PX_STEPS: f32 = 8.0;

/// Snap a pixel size to 1/8 px. Both the key and the shaping use the snapped
/// value, so the key always describes the metrics the buffer was shaped at.
pub(crate) fn quantise_px(px: f32) -> f32 {
    quantised_steps(px) as f32 / PX_STEPS
}

fn quantised_steps(px: f32) -> u32 {
    (px * PX_STEPS).round() as u32
}

/// Everything `shaped_buffer` reads, borrowed: content, metrics, face
/// attributes, the wrap box (viewport) and the horizontal alignment. Built per
/// item per frame without allocating.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ShapedKey<'a> {
    content: &'a str,
    family: &'a str,
    font_steps: u32,
    line_height_steps: u32,
    bold: bool,
    italic: bool,
    h_align: u8,
    wrap_px_bits: [u32; 2],
}

impl<'a> ShapedKey<'a> {
    pub(crate) fn new(
        item: &'a TextItem,
        family: &'a str,
        font_px: f32,
        line_height_px: f32,
        wrap_px: [f32; 2],
    ) -> Self {
        Self {
            content: &item.content,
            family: item.family_or(family),
            font_steps: quantised_steps(font_px),
            line_height_steps: quantised_steps(line_height_px),
            bold: item.bold,
            italic: item.italic,
            h_align: match item.h_align {
                TextHAlign::Left => 0,
                TextHAlign::Center => 1,
                TextHAlign::Right => 2,
            },
            wrap_px_bits: [wrap_px[0].to_bits(), wrap_px[1].to_bits()],
        }
    }

    fn hash64(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    fn to_owned_key(self) -> OwnedKey {
        OwnedKey {
            content: self.content.to_owned(),
            family: self.family.to_owned(),
            ..OwnedKey::scalars(&self)
        }
    }
}

/// Owned copy of a [`ShapedKey`], stored beside the buffer to verify a hash hit.
struct OwnedKey {
    content: String,
    family: String,
    font_steps: u32,
    line_height_steps: u32,
    bold: bool,
    italic: bool,
    h_align: u8,
    wrap_px_bits: [u32; 2],
}

impl OwnedKey {
    fn scalars(key: &ShapedKey<'_>) -> Self {
        Self {
            content: String::new(),
            family: String::new(),
            font_steps: key.font_steps,
            line_height_steps: key.line_height_steps,
            bold: key.bold,
            italic: key.italic,
            h_align: key.h_align,
            wrap_px_bits: key.wrap_px_bits,
        }
    }

    fn matches(&self, key: &ShapedKey<'_>) -> bool {
        self.content == key.content
            && self.family == key.family
            && self.font_steps == key.font_steps
            && self.line_height_steps == key.line_height_steps
            && self.bold == key.bold
            && self.italic == key.italic
            && self.h_align == key.h_align
            && self.wrap_px_bits == key.wrap_px_bits
    }
}

/// Shaped buffers plus the font-database size they were shaped against.
///
/// Entries are indexed by the key's 64-bit hash and verified against an owned
/// copy, so a hit allocates nothing and a hash collision is only a miss. A full
/// cache is cleared outright: O(1) amortised, where per-miss LRU bookkeeping or
/// an oldest-entry scan costs more than the few re-shapes a clear causes. Live
/// `Arc`s held by the current frame survive a clear.
pub(crate) struct ShapedBufferCache {
    entries: HashMap<u64, (OwnedKey, Arc<cryoglyph::Buffer>)>,
    capacity: usize,
    font_faces: usize,
}

impl ShapedBufferCache {
    pub(crate) fn new() -> Self {
        Self::with_capacity(SHAPED_CACHE_CAPACITY)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity: capacity.max(1),
            font_faces: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// The cached buffer for `key`, or one shaped by `shape` and stored.
    ///
    /// Assumes every font mutation changes `font_system.db().len()`; a face
    /// replaced at the same count would keep stale shapes.
    pub(crate) fn get_or_shape(
        &mut self,
        font_system: &mut cryoglyph::FontSystem,
        key: ShapedKey<'_>,
        shape: impl FnOnce(&mut cryoglyph::FontSystem) -> cryoglyph::Buffer,
    ) -> Arc<cryoglyph::Buffer> {
        let faces = font_system.db().len();
        if faces != self.font_faces {
            self.entries.clear();
            self.font_faces = faces;
        }
        let hash = key.hash64();
        if let Some((owned, buffer)) = self.entries.get(&hash)
            && owned.matches(&key)
        {
            return Arc::clone(buffer);
        }
        let buffer = Arc::new(shape(font_system));
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&hash) {
            self.entries.clear();
        }
        self.entries
            .insert(hash, (key.to_owned_key(), Arc::clone(&buffer)));
        buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(content: &str) -> TextItem {
        TextItem {
            content: content.to_string(),
            ..Default::default()
        }
    }

    fn key(item: &TextItem) -> ShapedKey<'_> {
        ShapedKey::new(item, "Iosevka", 20.0, 24.0, [800.0, 600.0])
    }

    #[test]
    fn key_differs_on_each_shaping_attribute() {
        let base = item("NET_A");
        let k = key(&base);
        assert_eq!(k, key(&item("NET_A")));
        assert_ne!(k, key(&item("NET_B")));
        let mut bold = base.clone();
        bold.bold = true;
        assert_ne!(k, key(&bold));
        let mut italic = base.clone();
        italic.italic = true;
        assert_ne!(k, key(&italic));
        let mut right = base.clone();
        right.h_align = TextHAlign::Right;
        assert_ne!(k, key(&right));
        let mut fam = base.clone();
        fam.family = Some("Roboto".into());
        assert_ne!(k, key(&fam));
        assert_ne!(
            k,
            ShapedKey::new(&base, "Iosevka", 21.0, 24.0, [800.0, 600.0])
        );
        assert_ne!(
            k,
            ShapedKey::new(&base, "Iosevka", 20.0, 25.0, [800.0, 600.0])
        );
        assert_ne!(
            k,
            ShapedKey::new(&base, "Iosevka", 20.0, 24.0, [801.0, 600.0])
        );
        assert_ne!(
            k,
            ShapedKey::new(&base, "Iosevka", 20.0, 24.0, [800.0, 601.0])
        );
        // Position, colour and size_mm are not shaping inputs.
        let mut moved = base.clone();
        moved.position = [5.0, 5.0];
        moved.color = [0.5; 4];
        assert_eq!(k, key(&moved));
    }

    #[test]
    fn quantisation_snaps_to_an_eighth_pixel() {
        assert_eq!(quantise_px(20.0), 20.0);
        assert_eq!(quantise_px(20.04), 20.0);
        assert_eq!(quantise_px(20.07), 20.125);
        assert_eq!(quantise_px(quantise_px(13.3)), quantise_px(13.3));
    }

    fn empty_font_system() -> cryoglyph::FontSystem {
        cryoglyph::FontSystem::new_with_locale_and_db(
            "en-US".to_string(),
            cryoglyph::fontdb::Database::new(),
        )
    }

    fn blank(fs: &mut cryoglyph::FontSystem) -> cryoglyph::Buffer {
        cryoglyph::Buffer::new(fs, cryoglyph::Metrics::new(10.0, 12.0))
    }

    fn lookup(
        cache: &mut ShapedBufferCache,
        fs: &mut cryoglyph::FontSystem,
        it: &TextItem,
        font_px: f32,
        shapes: &mut u32,
    ) {
        cache.get_or_shape(
            fs,
            ShapedKey::new(it, "Iosevka", font_px, font_px * 1.2, [800.0, 600.0]),
            |fs| {
                *shapes += 1;
                blank(fs)
            },
        );
    }

    #[test]
    fn full_cache_clears_and_holds_the_bound() {
        let mut fs = empty_font_system();
        let mut cache = ShapedBufferCache::with_capacity(3);
        let mut shapes = 0;
        for n in 0..3 {
            lookup(
                &mut cache,
                &mut fs,
                &item(&format!("L{n}")),
                10.0,
                &mut shapes,
            );
        }
        assert_eq!(cache.len(), 3);
        lookup(&mut cache, &mut fs, &item("L3"), 10.0, &mut shapes);
        assert_eq!(cache.len(), 1, "a full cache is cleared before the insert");
        for n in 10..200 {
            lookup(
                &mut cache,
                &mut fs,
                &item(&format!("L{n}")),
                10.0,
                &mut shapes,
            );
            assert!(cache.len() <= 3);
        }
    }

    #[test]
    fn more_items_than_capacity_stays_bounded_and_linear() {
        let mut fs = empty_font_system();
        let mut cache = ShapedBufferCache::new();
        let items: Vec<TextItem> = (0..SHAPED_CACHE_CAPACITY * 3)
            .map(|n| item(&format!("NET_{n}")))
            .collect();
        let mut shapes = 0;
        let started = std::time::Instant::now();
        for _ in 0..2 {
            for it in &items {
                lookup(&mut cache, &mut fs, it, 10.0, &mut shapes);
                assert!(cache.len() <= SHAPED_CACHE_CAPACITY);
            }
        }
        // 24k O(1) inserts; an O(capacity) scan per miss would be ~100M steps.
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "over-capacity inserts must not scan the cache"
        );
        assert!(
            shapes as usize >= items.len(),
            "every item shaped at least once"
        );
    }

    #[test]
    fn a_zoom_frame_with_a_new_font_px_reuses_the_shapes() {
        let mut fs = empty_font_system();
        let mut cache = ShapedBufferCache::new();
        let items: Vec<TextItem> = (0..50).map(|n| item(&format!("N{n}"))).collect();
        let mut shapes = 0;
        for it in &items {
            lookup(&mut cache, &mut fs, it, 12.00, &mut shapes);
        }
        assert_eq!(shapes, 50);
        // Next zoom frame: a different float, same 1/8 px step.
        for it in &items {
            lookup(&mut cache, &mut fs, it, 12.03, &mut shapes);
        }
        assert_eq!(shapes, 50, "sub-step zoom change must hit");
        // A real change of a full step misses.
        lookup(&mut cache, &mut fs, &items[0], 12.2, &mut shapes);
        assert_eq!(shapes, 51);
    }

    #[test]
    fn a_changed_face_count_clears_through_get_or_shape() {
        let mut fs = empty_font_system();
        let mut cache = ShapedBufferCache::new();
        let it = item("NET_A");
        let mut shapes = 0;
        lookup(&mut cache, &mut fs, &it, 10.0, &mut shapes);
        lookup(&mut cache, &mut fs, &it, 10.0, &mut shapes);
        assert_eq!(shapes, 1);
        fs.db_mut()
            .load_font_source(cryoglyph::fontdb::Source::Binary(Arc::new(
                &include_bytes!("../../tests/fonts/Iosevka-Regular.ttf")[..],
            )));
        lookup(&mut cache, &mut fs, &it, 10.0, &mut shapes);
        assert_eq!(shapes, 2, "a new face must drop stale shapes");
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn cache_hit_returns_the_same_layout_as_fresh_shaping() {
        use crate::pipeline::text::{attrs_for_item, shaped_buffer};
        let mut fs = {
            let mut db = cryoglyph::fontdb::Database::new();
            db.load_font_source(cryoglyph::fontdb::Source::Binary(Arc::new(
                &include_bytes!("../../tests/fonts/Iosevka-Regular.ttf")[..],
            )));
            cryoglyph::FontSystem::new_with_locale_and_db("en-US".to_string(), db)
        };
        let mut it = item("alpha\nlonger line");
        it.h_align = TextHAlign::Center;
        let attrs = attrs_for_item(&it, "Iosevka");
        let metrics = cryoglyph::Metrics::new(20.0, 24.0);
        let mut shapes = 0;
        let mut cache = ShapedBufferCache::new();
        let mut get = |fs: &mut cryoglyph::FontSystem, shapes: &mut u32| {
            cache.get_or_shape(fs, key(&it), |fs| {
                *shapes += 1;
                shaped_buffer(fs, &it, metrics, &attrs, [800.0, 600.0])
            })
        };
        let first = get(&mut fs, &mut shapes);
        let hit = get(&mut fs, &mut shapes);
        assert_eq!(shapes, 1, "second lookup must not re-shape");
        assert!(Arc::ptr_eq(&first, &hit));

        let fresh = shaped_buffer(&mut fs, &it, metrics, &attrs, [800.0, 600.0]);
        let layout = |b: &cryoglyph::Buffer| -> Vec<(f32, f32, f32)> {
            b.layout_runs()
                .flat_map(|r| {
                    r.glyphs
                        .iter()
                        .map(|g| (g.x, g.w, r.line_top))
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        assert_eq!(layout(&hit), layout(&fresh));
        assert!(!layout(&fresh).is_empty());
    }
}
