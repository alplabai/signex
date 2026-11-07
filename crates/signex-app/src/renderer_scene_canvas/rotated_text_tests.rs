use super::*;
use signex_gfx::primitive::text::{TextHAlign, TextVAlign};
use std::f32::consts::FRAC_PI_2;

fn item(rotation: f32) -> TextItem {
    TextItem {
        content: "PIN1".to_string(),
        position: [0.0, 0.0],
        size_mm: 1.0,
        color: [1.0, 1.0, 1.0, 1.0],
        bold: false,
        italic: false,
        rotation,
        h_align: TextHAlign::Right,
        v_align: TextVAlign::Bottom,
        family: None,
    }
}

fn sized(content: &str, size_mm: f32, rotation: f32) -> TextItem {
    TextItem {
        content: content.to_string(),
        size_mm,
        color: [1.0; 4],
        rotation,
        ..Default::default()
    }
}

fn options() -> SceneDrawOptions {
    SceneDrawOptions {
        scale_px_per_mm: 30.0,
        device_px_per_logical_px: 1.0,
        text_min_px: 0.0,
        text_max_px: f32::INFINITY,
        frame_origin: Point::ORIGIN,
    }
}

/// The GPU stages what the atlas can draw and the canvas draws the rest,
/// so a text that is drawn at all is never lost and never drawn twice
/// (#199 S1), over the real predicates: rotated, oversize and tiny items
/// included.
#[test]
fn gpu_staged_plus_canvas_is_exactly_the_drawn_texts() {
    let options = options();
    let texts = [
        sized("horizontal", 1.27, 0.0),
        sized("negative-zero", 1.27, -0.0),
        sized("vertical", 1.27, std::f32::consts::FRAC_PI_2),
        sized("arbitrary", 1.27, 0.61),
        sized("vertical-down", 1.27, -std::f32::consts::FRAC_PI_2),
        sized("flipped", 1.27, std::f32::consts::PI),
        sized("oversize", 40.0, 0.0),
        sized("oversize-rotated", 40.0, 0.61),
        sized("invisible", f32::NAN, 0.0),
    ];
    let staged: Vec<&str> = texts
        .iter()
        .filter(|t| gpu_stages_text(t, options))
        .map(|t| t.content.as_str())
        .collect();
    let canvas: Vec<&str> = texts
        .iter()
        .filter(|t| cpu_draws_beside_gpu(t, options))
        .map(|t| t.content.as_str())
        .collect();
    assert_eq!(staged, ["horizontal", "negative-zero"]);
    assert_eq!(
        canvas,
        [
            "vertical",
            "arbitrary",
            "vertical-down",
            "flipped",
            "oversize",
            "oversize-rotated"
        ]
    );
    assert!(staged.iter().all(|c| !canvas.contains(c)), "disjoint");
    let mut union: Vec<&str> = staged.iter().chain(&canvas).copied().collect();
    let mut drawn: Vec<&str> = texts
        .iter()
        .map(|t| t.content.as_str())
        .filter(|c| *c != "invisible")
        .collect();
    union.sort_unstable();
    drawn.sort_unstable();
    assert_eq!(union, drawn, "nothing missed, nothing duplicated");
    assert!(
        !staged.contains(&"invisible") && !canvas.contains(&"invisible"),
        "an invisible text is drawn by neither"
    );
}

/// iced opens a new layer at every switch between atlas text and outline
/// meshes, cache hit or not (#2128), so a bucket switches exactly once.
#[test]
fn atlas_texts_draw_before_outline_texts_each_in_scene_order() {
    let texts = [
        sized("a", 1.27, 0.0),
        sized("r1", 1.27, FRAC_PI_2),
        sized("b", 1.27, 0.0),
        sized("big", 40.0, 0.0),
        sized("r2", 1.27, 0.61),
        sized("c", 1.27, -0.0),
    ];
    let order: Vec<&str> = text_draw_order(&texts, options())
        .iter()
        .map(|t| t.content.as_str())
        .collect();
    assert_eq!(order, ["a", "b", "c", "r1", "big", "r2"]);
}

#[test]
fn quarter_turn_labels_pivot_on_the_anchor_with_their_own_sign() {
    let anchor = Point::new(100.0, 50.0);
    for angle in [FRAC_PI_2, -FRAC_PI_2, 3.0 * FRAC_PI_2] {
        let (shift, rot) = rotated_text_transform(&item(angle), anchor).expect("rotated");
        assert_eq!((shift.x, shift.y), (100.0, 50.0));
        assert_eq!(rot.0, angle);
    }
}

#[test]
fn axis_aligned_labels_are_placed_directly() {
    assert!(rotated_text_transform(&item(0.0), Point::ORIGIN).is_none());
}
