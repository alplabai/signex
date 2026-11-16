use super::*;

#[test]
fn the_default_panel_keeps_every_rule() {
    let panel = BoardPanel::default();
    assert_eq!(panel.check(), Ok(()));
    assert_eq!(panel.copies(), 4);
}

#[test]
fn counts_lengths_and_bite_rules_are_checked() {
    let with = |edit: fn(&mut BoardPanel)| {
        let mut panel = BoardPanel::default();
        edit(&mut panel);
        panel.check()
    };
    assert_eq!(
        with(|p| p.columns = 0),
        Err(PanelError::Count {
            field: "columns",
            min: 1,
            max: MAX_PANEL_COPIES
        })
    );
    assert!(matches!(
        with(|p| p.rows = MAX_PANEL_COPIES + 1),
        Err(PanelError::Count { field: "rows", .. })
    ));
    assert_eq!(
        with(|p| p.rails.left = -1.0),
        Err(PanelError::Negative { field: "rail_left" })
    );
    assert_eq!(
        with(|p| p.spacing_y = f64::NAN),
        Err(PanelError::NotFinite { field: "spacing_y" })
    );
    assert_eq!(
        with(|p| p.bites.pitch = 0.5),
        Err(PanelError::PitchNotAboveHole)
    );
    assert_eq!(
        with(|p| p.bites.hole_diameter = 0.01),
        Err(PanelError::BiteHoleTooSmall {
            min_mm: MIN_BITE_HOLE_MM
        })
    );
    assert_eq!(
        with(|p| p.spacing_x = 0.0),
        Err(PanelError::BitesNeedSpacing)
    );
    // V-scores take panels with no gap, and leave the bite rules aside.
    assert_eq!(
        with(|p| {
            p.separation = PanelSeparation::VScore;
            p.spacing_x = 0.0;
            p.spacing_y = 0.0;
            p.bites.pitch = 0.1;
        }),
        Ok(())
    );
}

#[test]
fn every_field_round_trips_through_its_id_and_its_text() {
    for field in PanelField::ALL {
        assert_eq!(PanelField::from_id(field.id()), Some(field));
    }
    let mut panel = BoardPanel::default();
    assert_eq!(PanelField::Columns.set_text(&mut panel, "3"), Ok(true));
    assert_eq!(panel.columns, 3);
    assert_eq!(PanelField::Columns.set_text(&mut panel, "3"), Ok(false));
    assert_eq!(PanelField::RailLeft.set_text(&mut panel, " 4.5 "), Ok(true));
    assert_eq!(panel.rails.left, 4.5);
    assert_eq!(PanelField::RailLeft.length_mm(&panel), Some(4.5));
    assert_eq!(PanelField::RailLeft.count(&panel), None);
    assert_eq!(
        PanelField::Separation.set_text(&mut panel, "v_score"),
        Ok(true)
    );
    assert_eq!(panel.separation, PanelSeparation::VScore);
    assert_eq!(
        PanelField::TabsPerEdge.set_text(&mut panel, "two"),
        Err(PanelError::BadValue {
            field: "tabs_per_edge",
            value: "two".into()
        })
    );
    assert!(PanelField::SpacingX.set_text(&mut panel, "inf").is_err());
    assert!(PanelField::Separation.set_text(&mut panel, "saw").is_err());
}
