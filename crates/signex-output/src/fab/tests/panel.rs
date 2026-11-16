//! #2410: V-score lines on their own image, and a whole panel through the
//! same writers as one board.

use signex_types::pcb_panel::{BoardPanel, PanelSeparation};
use signex_types::pcb_sketch::{BoardSketch, PcbVScore};
use uuid::Uuid;

use super::fixture::{fixture_board, p};
use super::{file, generated};

#[test]
fn a_sketch_v_score_is_written_on_the_v_cut_image_and_not_noted() {
    let mut board = fixture_board();
    let mut sketch = BoardSketch::default();
    sketch.v_scores.push(PcbVScore {
        uuid: Uuid::from_u128(70),
        from: p(10.0, 0.0),
        to: p(10.0, 10.0),
        depth_fraction: 1.0 / 3.0,
        min_web: 0.0,
        side: Default::default(),
    });
    board.sketch = Some(sketch);
    let set = generated(&board);
    let image = file(&set, "fab-vscore.gbr").expect("the V-cut image");
    assert!(image.contains("%TF.FileFunction,Vcut*%"), "{image}");
    assert!(image.contains("Other,VScore"), "{image}");
    assert!(set.notes.is_empty(), "{:?}", set.notes);
    let job = file(&set, "fab-job.gbrjob").expect("job");
    assert!(job.contains("fab-vscore.gbr"), "{job}");
}

#[test]
fn a_panel_is_written_as_one_board_with_its_copies_holes_and_scores() {
    let board = fixture_board();
    let panel = BoardPanel::default();
    let whole = pcb_geom::panel::panel_board(&board, &panel).expect("panelised");
    let set = generated(&whole);
    let profile = file(&set, "fab-profile.gbr").expect("profile");
    assert!(profile.contains("Profile"), "{profile}");
    let npth = file(&set, "fab-npth.drl").expect("tooling holes and bites");
    assert!(npth.contains("T1C0.500\n"), "the bites: {npth}");
    assert!(npth.contains("C2.000\n"), "the tooling holes: {npth}");
    // Four copies of R1 and J1; the fiducials are graphical, not placed.
    let centroid = file(&set, "fab-centroid.csv").expect("centroid");
    assert_eq!(centroid.matches("R1,").count(), 4, "{centroid}");
    assert!(!centroid.contains("FID"), "{centroid}");
    assert!(file(&set, "fab-vscore.gbr").is_none());

    let scored = BoardPanel {
        separation: PanelSeparation::VScore,
        spacing_x: 0.0,
        spacing_y: 0.0,
        ..panel
    };
    let whole = pcb_geom::panel::panel_board(&board, &scored).expect("panelised");
    let set = generated(&whole);
    assert!(file(&set, "fab-vscore.gbr").is_some());
}
