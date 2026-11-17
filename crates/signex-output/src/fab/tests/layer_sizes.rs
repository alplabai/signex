//! #2401: a pad or via with per-layer sizes is flashed at its size on each
//! copper image, as the canvas and the design rule check read it.

use signex_types::pcb::{LayerDef, LayerSizes};

use super::fixture::{fixture_board, p};
use super::{file, generated};

#[test]
fn a_pad_and_a_via_larger_or_smaller_inside_flash_their_inner_size_there() {
    let mut board = fixture_board();
    board.layers.push(LayerDef {
        id: 1,
        name: "Inner1Copper".into(),
        layer_type: "signal".into(),
        display_name: String::new(),
    });
    // J1's through pin: 1.6 mm outside, 2.4 mm on the inner layer.
    board.footprints[1].pads[0].layer_sizes = Some(LayerSizes::TopMiddleBottom {
        top: p(1.6, 1.6),
        middle: p(2.4, 2.4),
        bottom: p(1.6, 1.6),
    });
    // The via: 0.6 mm outside, 0.4 mm on the inner layer.
    board.vias[0].layer_sizes = Some(LayerSizes::TopMiddleBottom {
        top: 0.6,
        middle: 0.4,
        bottom: 0.6,
    });
    let set = generated(&board);

    let inner = file(&set, "fab-copper-l2-inner1.gbr").expect("inner copper");
    assert!(inner.contains("C,2.4*%"), "the pin's inner size: {inner}");
    assert!(inner.contains("C,0.4*%"), "the via's inner size: {inner}");
    assert!(!inner.contains("C,1.6*%"), "{inner}");
    assert!(!inner.contains("C,0.6*%"), "{inner}");
    for outer in ["fab-copper-l1-top.gbr", "fab-copper-l3-bottom.gbr"] {
        let image = file(&set, outer).expect("outer copper");
        assert!(image.contains("C,1.6*%"), "{outer}: {image}");
        assert!(image.contains("C,0.6*%"), "{outer}: {image}");
        assert!(!image.contains("C,2.4*%"), "{outer}: {image}");
        assert!(!image.contains("C,0.4*%"), "{outer}: {image}");
    }
}
