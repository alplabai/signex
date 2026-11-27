use super::*;

fn pour() -> Zone {
    serde_json::from_str(
        r#"{ "uuid": "8d15b2f9-8f86-41d7-9ec4-0f54a4f3a651", "fill_type": "pour" }"#,
    )
    .expect("zone")
}

/// #2578: a pour reads Sharp with no size until set; a size only shapes a
/// Rounded or Chamfer corner, and only when it is a positive number.
#[test]
fn corners_shape_a_pour_only_with_a_positive_size() {
    let mut zone = pour();
    assert_eq!((zone.corners, zone.corner_size), (PourCorners::Sharp, 0.0));
    assert_eq!(zone.pour_corner_shape(), None);
    zone.corner_size = 0.5;
    assert_eq!(zone.pour_corner_shape(), None, "Sharp ignores the size");
    zone.corners = PourCorners::Rounded;
    assert_eq!(zone.pour_corner_shape(), Some((PourCorners::Rounded, 0.5)));
    zone.corners = PourCorners::Chamfer;
    assert_eq!(zone.pour_corner_shape(), Some((PourCorners::Chamfer, 0.5)));
    for bad in [0.0, -1.0, f64::NAN] {
        zone.corner_size = bad;
        assert_eq!(zone.pour_corner_shape(), None, "{bad}");
    }
    zone.corner_size = 0.5;
    zone.fill_type = "solid".into();
    assert_eq!(zone.pour_corner_shape(), None, "only a pour");
}

/// #2579: a hatched pour reads its width and gap, a 0 one as the default;
/// a solid pour or another zone has no hatch.
#[test]
fn a_hatched_pour_reads_its_hatch_with_defaults_for_zero() {
    let mut zone = pour();
    assert_eq!(zone.fill_mode, PourFillMode::Solid);
    assert_eq!(zone.pour_hatch(), None);
    zone.fill_mode = PourFillMode::Hatched;
    assert_eq!(
        zone.pour_hatch(),
        Some(PourHatch {
            width_mm: HATCH_WIDTH_DEFAULT_MM,
            gap_mm: HATCH_GAP_DEFAULT_MM,
        })
    );
    zone.hatch_width = 0.3;
    zone.hatch_gap = 1.0;
    assert_eq!(
        zone.pour_hatch(),
        Some(PourHatch {
            width_mm: 0.3,
            gap_mm: 1.0,
        })
    );
    zone.fill_type = "solid".into();
    assert_eq!(zone.pour_hatch(), None);
}

/// The keys are written only when they differ from the default, and
/// round-trip once set. `has_pour_shape` is what raises the file token.
#[test]
fn the_keys_are_written_only_when_set_and_round_trip() {
    let mut zone = pour();
    let text = serde_json::to_string(&zone).expect("json");
    for key in [
        "corners",
        "corner_size",
        "fill_mode",
        "hatch_width",
        "hatch_gap",
    ] {
        assert!(!text.contains(key), "{key}: {text}");
    }
    assert!(!zone.has_pour_shape());
    zone.corner_size = 0.4;
    assert!(!zone.has_pour_shape(), "a size alone shapes nothing");
    zone.corners = PourCorners::Chamfer;
    zone.fill_mode = PourFillMode::Hatched;
    zone.hatch_width = 0.2;
    zone.hatch_gap = 0.6;
    assert!(zone.has_pour_shape());
    let text = serde_json::to_string(&zone).expect("json");
    assert!(text.contains("\"corners\":\"chamfer\""), "{text}");
    assert!(text.contains("\"fill_mode\":\"hatched\""), "{text}");
    let back: Zone = serde_json::from_str(&text).expect("zone");
    assert_eq!(back, zone);
    zone.fill_type = "solid".into();
    assert!(!zone.has_pour_shape(), "only a pour");
}

#[test]
fn the_tokens_read_back() {
    for corners in PourCorners::ALL {
        assert_eq!(PourCorners::from_token(corners.token()), Some(corners));
    }
    for mode in PourFillMode::ALL {
        assert_eq!(PourFillMode::from_token(mode.token()), Some(mode));
    }
    assert_eq!(PourCorners::from_token("round"), None);
    assert_eq!(PourFillMode::from_token("none"), None, "Shelve, not None");
}

/// Rounded or Chamfer corners with no size fill exactly as Sharp, so they
/// do not raise the file token; a size does.
#[test]
fn corners_with_no_size_do_not_count_as_a_pour_shape() {
    let mut zone = pour();
    for corners in [PourCorners::Rounded, PourCorners::Chamfer] {
        zone.corners = corners;
        zone.corner_size = 0.0;
        assert!(!zone.has_pour_shape(), "{corners:?} with no size");
        zone.corner_size = 0.5;
        assert!(zone.has_pour_shape(), "{corners:?} with a size");
    }
}
