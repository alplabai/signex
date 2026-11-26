//! The image `/XObject`s of placed images (#1116, mockup row 204): a JPEG
//! as it is (`DCTDecode`), a PNG or a GIF's first frame as deflated RGB with
//! a deflated soft mask when it has transparency, each at the file's own
//! resolution. A picture that does not decode is written as one transparent
//! pixel, so the page's `Do` always names an object.

use std::collections::HashMap;
use std::sync::Arc;

use pdf_writer::{Filter, Pdf, Ref};
use signex_types::image_asset::ImageFormat;

use crate::svg::SvgPicture;

/// An image ready to write: its samples and, when it has one, its mask.
struct Encoded {
    width: u32,
    height: u32,
    /// 1 (grey) or 3 (RGB) colour components.
    components: u8,
    filter: Filter,
    samples: Vec<u8>,
    /// Deflated 8-bit alpha, when some pixel is not opaque.
    alpha: Option<Vec<u8>>,
}

/// Width, height and component count from a JPEG's start-of-frame marker.
fn jpeg_frame(bytes: &[u8]) -> Option<(u32, u32, u8)> {
    let mut i = 2;
    while i + 9 < bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        let marker = bytes[i + 1];
        let length = usize::from(u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]));
        // SOF0..SOF15 but DHT (C4), JPG (C8) and DAC (CC).
        if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
            let height = u32::from(u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]));
            let width = u32::from(u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]));
            return Some((width, height, bytes[i + 9]));
        }
        i += 2 + length;
    }
    None
}

fn deflate(bytes: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec_zlib(bytes, 6)
}

/// `picture` as an image ready to write; `None` when it does not decode.
fn encode(picture: &SvgPicture) -> Option<Encoded> {
    if picture.format == ImageFormat::Jpeg {
        let (width, height, components) = jpeg_frame(&picture.bytes)?;
        // A CMYK JPEG's inversion is the writer's guess: refused.
        if !matches!(components, 1 | 3) {
            return None;
        }
        return Some(Encoded {
            width,
            height,
            components,
            filter: Filter::DctDecode,
            samples: picture.bytes.to_vec(),
            alpha: None,
        });
    }
    let (width, height, rgba) = crate::svg::decode_rgba(picture)?;
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    let mut alpha = Vec::with_capacity(rgba.len() / 4);
    for px in rgba.chunks_exact(4) {
        rgb.extend_from_slice(&px[..3]);
        alpha.push(px[3]);
    }
    let opaque = alpha.iter().all(|&a| a == u8::MAX);
    Some(Encoded {
        width,
        height,
        components: 3,
        filter: Filter::FlateDecode,
        samples: deflate(&rgb),
        alpha: (!opaque).then(|| deflate(&alpha)),
    })
}

/// One transparent pixel: what a picture that does not decode paints.
fn blank() -> Encoded {
    Encoded {
        width: 1,
        height: 1,
        components: 3,
        filter: Filter::FlateDecode,
        samples: deflate(&[255, 255, 255]),
        alpha: Some(deflate(&[0])),
    }
}

/// The image objects written so far, by picture, and the next free ref.
pub(super) struct ImageObjects {
    written: HashMap<usize, Ref>,
    next: i32,
}

impl ImageObjects {
    /// Objects numbered from `first_ref`, after every other object.
    pub(super) fn new(first_ref: i32) -> Self {
        Self {
            written: HashMap::new(),
            next: first_ref,
        }
    }

    fn take_ref(&mut self) -> Ref {
        let r = Ref::new(self.next);
        self.next += 1;
        r
    }

    /// The object of `picture`, written on its first use.
    pub(super) fn object(&mut self, pdf: &mut Pdf, picture: &Arc<SvgPicture>) -> Ref {
        let key = Arc::as_ptr(&picture.bytes).cast::<u8>() as usize;
        if let Some(found) = self.written.get(&key) {
            return *found;
        }
        let encoded = encode(picture).unwrap_or_else(blank);
        let id = self.take_ref();
        let mask = encoded.alpha.as_ref().map(|_| self.take_ref());
        let (w, h) = (
            i32::try_from(encoded.width).unwrap_or(1),
            i32::try_from(encoded.height).unwrap_or(1),
        );
        {
            let mut image = pdf.image_xobject(id, &encoded.samples);
            image.filter(encoded.filter);
            image.width(w);
            image.height(h);
            if encoded.components == 1 {
                image.color_space().device_gray();
            } else {
                image.color_space().device_rgb();
            }
            image.bits_per_component(8);
            if let Some(mask) = mask {
                image.s_mask(mask);
            }
        }
        if let (Some(mask), Some(alpha)) = (mask, &encoded.alpha) {
            let mut soft = pdf.image_xobject(mask, alpha);
            soft.filter(Filter::FlateDecode);
            soft.width(w);
            soft.height(h);
            soft.color_space().device_gray();
            soft.bits_per_component(8);
        }
        self.written.insert(key, id);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_jpeg_frame_gives_its_size_and_components() {
        // SOI, then a SOF0 segment: length 17, precision 8, 20 high, 40 wide, 3 components.
        let bytes = [
            0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x14, 0x00, 0x28, 0x03, 0, 0, 0, 0,
        ];
        assert_eq!(jpeg_frame(&bytes), Some((40, 20, 3)));
        assert_eq!(jpeg_frame(&[0xFF, 0xD8, 0x00]), None);
    }

    #[test]
    fn a_picture_that_does_not_decode_is_one_transparent_pixel() {
        let picture = SvgPicture {
            format: ImageFormat::Png,
            bytes: Arc::from(&b"not a png"[..]),
        };
        assert!(encode(&picture).is_none());
        let blank = blank();
        assert_eq!((blank.width, blank.height), (1, 1));
        assert!(blank.alpha.is_some());
    }
}
