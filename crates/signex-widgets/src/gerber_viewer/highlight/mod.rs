use super::*;

const HIGHLIGHT_COLOR: Color = Color::from_rgb(1.0, 193.0 / 255.0, 7.0 / 255.0);
const NON_MATCHING_ALPHA: f32 = 0.18;

mod attribute;
mod clear;
mod component;
mod d_code;
mod net;

pub(super) use attribute::attribute_highlight_color;
pub(super) use component::component_highlight_color;
pub(super) use d_code::d_code_highlight_color;
pub(super) use net::net_highlight_color;
