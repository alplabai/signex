//! The page colours of the in-app preview (mockup row 195, decided as
//! drawn): the preview page follows the theme, so under Graphite Dark it is
//! a dark page with light ink, while the PDF always prints black on white.
//! A compiled page is drawn on white ([`crate::CompiledNotebook::render_page`])
//! and then recoloured here: grey pixels move along the line from the
//! theme's paper to its ink, coloured pixels (a Pass badge, a plot trace)
//! keep their colour.

use crate::compile::PageImage;

/// How far a pixel's channels may differ and still count as grey: the
/// antialiased edge of black text on white stays within it.
const GREY_SPREAD: u8 = 24;

/// The colours a page is shown in: the paper, where the page is white, and
/// the ink, where it is black. RGB, eight bits per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageColours {
    pub paper: [u8; 3],
    pub ink: [u8; 3],
}

impl PageColours {
    /// The colours the PDF prints in: black on white.
    pub const PRINT: Self = Self {
        paper: [255, 255, 255],
        ink: [0, 0, 0],
    };

    /// `image` in these colours. With [`Self::PRINT`] it is the image as
    /// drawn.
    pub fn apply(self, image: &PageImage) -> PageImage {
        if self == Self::PRINT {
            return image.clone();
        }
        let mut rgba = image.rgba.clone();
        for pixel in rgba.chunks_exact_mut(4) {
            let [r, g, b] = [pixel[0], pixel[1], pixel[2]];
            let (low, high) = (r.min(g).min(b), r.max(g).max(b));
            if high - low > GREY_SPREAD {
                continue;
            }
            // 255 is paper, 0 is ink.
            let lightness = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
            let ends = self.ink.into_iter().zip(self.paper);
            for (out, (ink, paper)) in pixel[..3].iter_mut().zip(ends) {
                *out = mix(ink, paper, lightness);
            }
        }
        PageImage {
            width: image.width,
            height: image.height,
            rgba,
        }
    }
}

/// `from` at 0 to `to` at 255.
fn mix(from: u8, to: u8, at: u16) -> u8 {
    let (from, to) = (i32::from(from), i32::from(to));
    let value = from + (to - from) * i32::from(at) / 255;
    u8::try_from(value.clamp(0, 255)).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DARK: PageColours = PageColours {
        paper: [30, 32, 36],
        ink: [220, 222, 226],
    };

    fn image(pixels: &[[u8; 4]]) -> PageImage {
        PageImage {
            width: u32::try_from(pixels.len()).unwrap_or(u32::MAX),
            height: 1,
            rgba: pixels.iter().flatten().copied().collect(),
        }
    }

    #[test]
    fn white_becomes_the_paper_and_black_the_ink() {
        let shown = DARK.apply(&image(&[[255, 255, 255, 255], [0, 0, 0, 255]]));
        assert_eq!(&shown.rgba[0..3], &DARK.paper);
        assert_eq!(&shown.rgba[4..7], &DARK.ink);
        assert_eq!(shown.rgba[3], 255, "alpha is kept");
    }

    #[test]
    fn a_grey_edge_lands_between_paper_and_ink() {
        let shown = DARK.apply(&image(&[[128, 128, 128, 255]]));
        let ends = DARK.paper.into_iter().zip(DARK.ink);
        for (value, (low, high)) in shown.rgba[..3].iter().zip(ends) {
            assert!(
                *value > low && *value < high,
                "{value} between {low} and {high}"
            );
        }
    }

    #[test]
    fn a_coloured_pixel_keeps_its_colour() {
        let green = [40, 160, 80, 255];
        let shown = DARK.apply(&image(&[green]));
        assert_eq!(shown.rgba, green);
    }

    #[test]
    fn print_colours_leave_the_page_as_drawn() {
        let page = image(&[[255, 255, 255, 255], [10, 10, 10, 255], [200, 0, 0, 255]]);
        assert_eq!(PageColours::PRINT.apply(&page), page);
    }
}
