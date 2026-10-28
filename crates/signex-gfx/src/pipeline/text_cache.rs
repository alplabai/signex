//! Bounded LRU cache of shaped text buffers.
//!
//! Shaping is the expensive part of staging a text item and its result is a
//! pure function of the key below, so an unchanged label costs one hash
//! lookup per frame instead of a re-shape.

use std::collections::HashMap;
use std::sync::Arc;

use crate::primitive::text::{TextHAlign, TextItem};

/// Most shaped buffers kept; the least recently used is evicted beyond it.
pub(crate) const SHAPED_CACHE_CAPACITY: usize = 4096;

/// Everything `shaped_buffer` reads: content, metrics, face attributes, the
/// wrap box (viewport) and the horizontal alignment.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct ShapedKey {
    content: String,
    family: String,
    font_px_bits: u32,
    line_height_px_bits: u32,
    bold: bool,
    italic: bool,
    h_align: u8,
    wrap_px_bits: [u32; 2],
}

impl ShapedKey {
    pub(crate) fn new(
        item: &TextItem,
        family: &str,
        font_px: f32,
        line_height_px: f32,
        wrap_px: [f32; 2],
    ) -> Self {
        Self {
            content: item.content.clone(),
            family: item.family_or(family).to_owned(),
            font_px_bits: font_px.to_bits(),
            line_height_px_bits: line_height_px.to_bits(),
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
}

/// Least-recently-used map with a hard entry bound.
pub(crate) struct LruCache<K, V> {
    entries: HashMap<K, (V, u64)>,
    capacity: usize,
    tick: u64,
}

impl<K: Clone + Eq + std::hash::Hash, V: Clone> LruCache<K, V> {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity: capacity.max(1),
            tick: 0,
        }
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn get(&mut self, key: &K) -> Option<V> {
        self.tick += 1;
        let tick = self.tick;
        self.entries.get_mut(key).map(|(value, used)| {
            *used = tick;
            value.clone()
        })
    }

    pub(crate) fn insert(&mut self, key: K, value: V) {
        self.tick += 1;
        if !self.entries.contains_key(&key) && self.entries.len() >= self.capacity {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                self.entries.remove(&oldest);
            }
        }
        self.entries.insert(key, (value, self.tick));
    }
}

/// Shaped buffers plus the font-database size they were shaped against; a
/// changed database (a face loaded later) drops every entry.
pub(crate) struct ShapedBufferCache {
    lru: LruCache<ShapedKey, Arc<cryoglyph::Buffer>>,
    font_faces: usize,
}

impl ShapedBufferCache {
    pub(crate) fn new() -> Self {
        Self {
            lru: LruCache::new(SHAPED_CACHE_CAPACITY),
            font_faces: 0,
        }
    }

    /// The cached buffer for `key`, or one shaped by `shape` and stored.
    pub(crate) fn get_or_shape(
        &mut self,
        font_system: &mut cryoglyph::FontSystem,
        key: ShapedKey,
        shape: impl FnOnce(&mut cryoglyph::FontSystem) -> cryoglyph::Buffer,
    ) -> Arc<cryoglyph::Buffer> {
        let faces = font_system.db().len();
        if faces != self.font_faces {
            self.lru.clear();
            self.font_faces = faces;
        }
        if let Some(hit) = self.lru.get(&key) {
            return hit;
        }
        let buffer = Arc::new(shape(font_system));
        self.lru.insert(key, Arc::clone(&buffer));
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

    fn key(item: &TextItem) -> ShapedKey {
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
    fn eviction_drops_least_recently_used_and_holds_the_bound() {
        let mut cache: LruCache<u32, u32> = LruCache::new(3);
        for n in 0..3 {
            cache.insert(n, n);
        }
        assert_eq!(cache.get(&0), Some(0)); // 1 is now the oldest
        cache.insert(3, 3);
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&0), Some(0));
        for n in 10..100 {
            cache.insert(n, n);
            assert!(cache.len() <= 3);
        }
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
