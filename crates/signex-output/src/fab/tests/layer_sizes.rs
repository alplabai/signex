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

#[test]
fn a_test_record_gives_the_size_on_its_probed_layer() {
    let mut board = fixture_board();
    // J1's through pin is probed from the top: 2 mm there, 1.6 mm below.
    board.footprints[1].pads[0].layer_sizes = Some(LayerSizes::TopMiddleBottom {
        top: p(2.0, 2.0),
        middle: p(1.0, 1.0),
        bottom: p(1.6, 1.6),
    });
    board.vias[0].layer_sizes = Some(LayerSizes::TopMiddleBottom {
        top: 0.8,
        middle: 0.4,
        bottom: 0.6,
    });
    let set = generated(&board);
    let ipc = file(&set, "fab-netlist.ipc").expect("test netlist");
    assert!(ipc.contains("X2000Y2000"), "J1's top size: {ipc}");
    assert!(!ipc.contains("X1600Y1600"), "not its base size: {ipc}");
    assert!(ipc.contains("X0800Y0800"), "the via's top size: {ipc}");
}
