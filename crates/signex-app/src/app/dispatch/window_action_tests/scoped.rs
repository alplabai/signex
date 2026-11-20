//! #1966: Properties messages typed in an undocked window that write its
//! sheet or its panel context run in that window's swap.

use super::*;

#[test]
fn a_child_sheet_name_typed_in_a_window_lands_in_that_windows_properties() {
    let mut f = fixture();
    f.app.refresh_panel_ctx();
    assert!(
        f.app
            .document_state
            .window_panel_ctx
            .contains_key(&f.window)
    );
    let id = uuid::Uuid::from_u128(11);
    let typed = typed_in_window(
        &f,
        crate::panels::PanelMsg::ChildSheetNameTyping(id, "Power".into()),
    );
    assert!(is_scoped(&typed, f.window));
    let _ = f.app.update(typed);
    assert_eq!(
        f.app.document_state.window_panel_ctx[&f.window].child_sheet_name_buf,
        Some((id, "Power".to_string()))
    );
    assert!(
        f.app
            .document_state
            .panel_ctx
            .child_sheet_name_buf
            .is_none()
    );
}

#[test]
fn a_child_sheet_name_committed_in_a_window_lands_in_that_windows_engine() {
    let mut f = fixture();
    let id = uuid::Uuid::from_u128(11);
    f.app
        .document_state
        .engines
        .get_mut(&f.window_path)
        .unwrap()
        .execute(signex_engine::Command::PlaceChildSheet {
            child_sheet: child_sheet(id),
        })
        .unwrap();
    f.app.refresh_panel_ctx();
    for message in [
        crate::panels::PanelMsg::ChildSheetNameTyping(id, "Power".into()),
        crate::panels::PanelMsg::CommitChildSheetName(id),
    ] {
        let scoped = typed_in_window(&f, message);
        let _ = f.app.update(scoped);
    }
    let window_sheet = f.app.document_state.engines[&f.window_path].document();
    assert_eq!(window_sheet.child_sheets[0].name, "Power");
    let main_sheet = f.app.document_state.engines[&f.main_path].document();
    assert!(main_sheet.child_sheets.is_empty());
}

#[test]
fn every_properties_message_that_writes_the_shown_sheet_or_its_context_is_scoped() {
    use crate::library::PlacedPartAction;
    use crate::panels::PanelMsg as P;
    use signex_widgets::tree_view::TreeMsg;
    let f = fixture();
    let id = uuid::Uuid::from_u128(11);
    let colour = iced::Color::from_rgb(0.1, 0.2, 0.3);
    let scoped = [
        P::ChildSheetNameTyping(id, String::new()),
        P::ChildSheetFileTyping(id, String::new()),
        P::CommitChildSheetName(id),
        P::CommitChildSheetFile(id),
        P::ToggleChildSheetBorderPicker(id),
        P::ToggleChildSheetFillPicker(id),
        P::OpenChildSheetAdvancedPicker(id, true),
        P::CancelChildSheetColorPicker,
        P::EditChildSheetBorderColor(id, colour),
        P::EditChildSheetFillColor(id, colour),
        P::ChildSheetStrokeWidthTyping(id, String::new()),
        P::CommitChildSheetStrokeWidth(id),
        P::ResetChildSheetStyle(id),
        P::UpdateSymbolFromLibrary(id),
        P::PlacedPartAction(PlacedPartAction::EditSymbol),
        // The part's ⋯ menu and Browse… act on the window's own sheet.
        P::Part(crate::panels::PartPanelMsg::MenuActionPicked(
            PlacedPartAction::EditSymbol,
        )),
        P::SelectionForm(crate::panels::SelectionFormMsg::SheetFileBrowseClicked(id)),
        P::SelectionForm(crate::panels::SelectionFormMsg::SheetFileBrowsed(id, None)),
        P::SetPrePlacementText(String::new()),
        P::SetPrePlacementDesignator(String::new()),
        P::SetPrePlacementRotation(0.0),
        P::SetPrePlacementFont(String::new()),
        P::SetPrePlacementFontSize(10),
        P::TogglePrePlacementBold,
        P::TogglePrePlacementItalic,
        P::TogglePrePlacementUnderline,
        P::ConfirmPrePlacement,
        P::PrePlacementPosition(crate::panels::PrePlacementPositionMsg::FieldTyped(
            crate::panels::PositionField::X,
            String::new(),
        )),
        P::Navigator(TreeMsg::Toggle(vec!["x".into()])),
        // #1312 review: a board window's footprint fields.
        P::PcbFootprintText(crate::panels::PcbFootprintTextMsg::Typed {
            board: "b.snxpcb".into(),
            footprint: id,
            field: signex_engine::pcb::designators::FootprintTextField::Value,
            text: String::new(),
        }),
        P::PcbFootprintText(crate::panels::PcbFootprintTextMsg::Submitted {
            board: "b.snxpcb".into(),
            footprint: id,
            field: signex_engine::pcb::designators::FootprintTextField::Value,
        }),
        // #2354: a board window's Component view edits that window's board.
        P::PcbComponent(crate::panels::PcbComponentMsg::LockToggled {
            board: "b.snxpcb".into(),
            footprints: vec![id],
            locked: true,
        }),
    ];
    for message in scoped {
        let wrapped = typed_in_window(&f, message.clone());
        assert!(is_scoped(&wrapped, f.window), "{message:?} runs on main");
    }
    // A Navigator row click opens a sheet: it stays on the main window.
    let click = typed_in_window(&f, P::Navigator(TreeMsg::Select(vec!["x".into()])));
    assert!(!is_scoped(&click, f.window));
}

#[test]
fn the_panel_inputs_stamp_moves_with_every_net_class_field() {
    let dir = tempfile::tempdir().unwrap();
    let names = ["a.snxsch", "b.snxsch"];
    let app = crate::app::handlers::menu::export::tests::app_workspace(
        &dir.path().to_string_lossy(),
        &names,
    );
    let mut f = fixture_in(app, dir.path());
    f.app.document_state.projects[0]
        .data
        .net_classes
        .push(signex_types::net_class::NetClass::named("Power"));
    let mut seen = vec![f.app.window_panel_inputs_stamp()];
    let edits: [fn(&mut signex_types::net_class::NetClass); 5] = [
        |c| c.name = "Power2".into(),
        |c| c.nets.push("VCC".into()),
        |c| c.patterns.push("V*".into()),
        |c| c.clearance = 0.2,
        |c| c.style.wire_width_mm = Some(0.3),
    ];
    for edit in edits {
        edit(&mut f.app.document_state.projects[0].data.net_classes[0]);
        let stamp = f.app.window_panel_inputs_stamp();
        assert!(
            !seen.contains(&stamp),
            "a net-class edit left the stamp as it was"
        );
        seen.push(stamp);
    }
}
