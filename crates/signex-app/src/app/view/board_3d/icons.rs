//! The 3D view toolbar's stroke icons (mockup row 175), drawn in `currentColor` and
//! tinted with the ink of the button they sit on.

use std::sync::OnceLock;

use iced::widget::{Svg, svg};
use iced::{Color, Theme};

/// One of the toolbar's icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Icon {
    Fit,
    Reset,
    Flip,
    Explode,
    Camera,
    Download,
    Warn,
    Close,
}

const FIT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const RESET: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M3 8a5 5 0 105-5H5M5 1.2L3.2 3 5 4.8" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const FLIP: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M8 2v12" stroke="currentColor" stroke-width="1.2" stroke-dasharray="1.5 1.5"/><path d="M6 4L2.5 12H6zM10 4l3.5 8H10z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>"#;
const EXPLODE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M3 4.5l5-2 5 2-5 2zM3 8l5 2 5-2M3 11.5l5 2 5-2" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/></svg>"#;
const CAMERA: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M2.5 5.5h2.6l1.2-1.8h3.4l1.2 1.8h2.6v7h-11z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><circle cx="8" cy="8.8" r="2.2" fill="none" stroke="currentColor" stroke-width="1.3"/></svg>"#;
const DOWNLOAD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M8 2.5v8M4.5 7.5L8 11l3.5-3.5M3 13.5h10" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const WARN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M8 2.5l6 10.5H2z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><path d="M8 6.5v3M8 11.3v.2" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;
const CLOSE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M4.5 4.5l7 7M11.5 4.5l-7 7" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;

impl Icon {
    const ALL: [Self; 8] = [
        Self::Fit,
        Self::Reset,
        Self::Flip,
        Self::Explode,
        Self::Camera,
        Self::Download,
        Self::Warn,
        Self::Close,
    ];

    fn bytes(self) -> &'static [u8] {
        match self {
            Self::Fit => FIT,
            Self::Reset => RESET,
            Self::Flip => FLIP,
            Self::Explode => EXPLODE,
            Self::Camera => CAMERA,
            Self::Download => DOWNLOAD,
            Self::Warn => WARN,
            Self::Close => CLOSE,
        }
    }

    /// The handle, made once: iced keys its texture cache on it.
    fn handle(self) -> svg::Handle {
        static HANDLES: OnceLock<Vec<svg::Handle>> = OnceLock::new();
        let handles = HANDLES.get_or_init(|| {
            Self::ALL
                .iter()
                .map(|icon| svg::Handle::from_memory(icon.bytes()))
                .collect()
        });
        let index = Self::ALL.iter().position(|i| *i == self).unwrap_or(0);
        handles[index].clone()
    }
}

/// `icon` at `size` px, tinted `ink`.
pub(super) fn icon(icon: Icon, size: f32, ink: Color) -> Svg<'static> {
    svg(icon.handle())
        .width(size)
        .height(size)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
}
