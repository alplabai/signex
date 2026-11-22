//! Function audit, wave B: the board surfaces, mockup rows 93 to 106 and 108
//! (board Properties, Layers panel, layer strip, Layer Stack, PCB panel and
//! the PCB active bar). Each row's controls are driven through
//! `Signex::update` and checked by what they change.

#[path = "b_board/active_bar.rs"]
mod active_bar;
#[path = "b_board/component.rs"]
mod component;
#[path = "b_board/copper.rs"]
mod copper;
#[path = "b_board/drags.rs"]
mod drags;
#[path = "b_board/drawings.rs"]
mod drawings;
#[path = "b_board/layer_stack.rs"]
mod layer_stack;
#[path = "b_board/layers.rs"]
mod layers;
#[path = "b_board/options.rs"]
mod options;
#[path = "b_board/support.rs"]
mod support;
