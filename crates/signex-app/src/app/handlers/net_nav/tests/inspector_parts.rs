//! The Net Inspector's By Component tab (#1274): the tab switch, the part
//! rows of a design read from the project net index, and a pin click that
//! goes to the pin and highlights its net.

use signex_types::schematic::{Point, SelectedItem, SelectedKind};

use super::fixture::*;
use crate::app::{Message, OverlayMsg};
use crate::panels::net_inspector::{NetInspectorTab, NetInspectorView};
use crate::panels::{NetInspectorMsg, PanelKind, PanelMsg};

/// `R1` (10k, R_0603) and `C1` on the `VCC` wire, `C1` with no value or
/// footprint; the panel open on the sheet with the index built. Returns
/// the project and `R1`'s uuid.
fn open_design() -> (Project, uuid::Uuid) {
    let mut sheet = sheet_with_net("R1", "VCC", &[]);
    let r1 = sheet.symbols[0].uuid;
    sheet.symbols[0].value = "10k".to_string();
    sheet.symbols[0].footprint = "R_0603".to_string();
    let mut c1 = sheet.symbols[0].clone();
    c1.uuid = uuid::Uuid::now_v7();
    c1.reference = "C1".to_string();
    c1.value = String::new();
    c1.footprint = String::new();
    c1.position = Point::new(10.0, 0.0);
    sheet.symbols.push(c1);
    let mut p = single_sheet_project(sheet);
    let _task = p.app.update(Message::Overlay(OverlayMsg::OpenPanel(
        PanelKind::NetInspector,
    )));
    p.app.settle_net_index_blocking();
    (p, r1)
}

fn inspector(p: &mut Project, msg: NetInspectorMsg) {
    let _task = p.app.update(Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::NetInspector(msg),
    )));
}

fn view(p: &Project) -> &NetInspectorView {
    &p.app.document_state.panel_ctx.net_inspector
}

#[test]
fn the_tabs_switch_between_the_nets_and_the_parts() {
    let (mut p, _) = open_design();
    inspector(&mut p, NetInspectorMsg::FilterChanged(String::new()));
    assert_eq!(view(&p).tab, NetInspectorTab::ByNet, "nets first");
    assert_eq!(view(&p).nets.len(), 1);
    assert!(view(&p).components.is_empty(), "only the tab shown is built");

    inspector(
        &mut p,
        NetInspectorMsg::TabSelected(NetInspectorTab::ByComponent),
    );
    assert_eq!(view(&p).tab, NetInspectorTab::ByComponent);
    assert_eq!(view(&p).component_total, 2);
    assert!(view(&p).nets.is_empty());

    inspector(&mut p, NetInspectorMsg::TabSelected(NetInspectorTab::ByNet));
    assert_eq!(view(&p).tab, NetInspectorTab::ByNet);
    assert_eq!(view(&p).nets[0].name, "VCC");
}

#[test]
fn a_part_row_has_its_value_and_footprint_and_expands_to_the_net_on_each_pin() {
    let (mut p, r1) = open_design();
    inspector(
        &mut p,
        NetInspectorMsg::TabSelected(NetInspectorTab::ByComponent),
    );
    let parts = &view(&p).components;
    let designators: Vec<&str> = parts.iter().map(|r| r.designator.as_str()).collect();
    assert_eq!(designators, ["C1", "R1"], "by designator");
    let row = &parts[1];
    assert_eq!((row.value.as_str(), row.footprint.as_str()), ("10k", "R_0603"));
    assert_eq!(row.pins, 1);
    assert!(!row.expanded && row.pin_rows.is_empty());
    let key = row.key.clone();

    inspector(&mut p, NetInspectorMsg::ComponentToggled(key));
    let row = &view(&p).components[1];
    assert!(row.expanded);
    assert_eq!(row.pin_rows.len(), 1);
    let pin = &row.pin_rows[0];
    assert_eq!((pin.pin.as_str(), pin.net.as_str()), ("1", "VCC"));
    assert_eq!(pin.sheet, "u");
    assert_eq!(pin.anchor.id, r1);

    inspector(&mut p, NetInspectorMsg::FilterChanged("r_06".into()));
    let parts = &view(&p).components;
    assert_eq!(parts.len(), 1, "the footprint matches");
    assert_eq!(parts[0].designator, "R1");
    assert_eq!(view(&p).component_total, 2, "the total counts every part");
}

#[test]
fn clicking_a_pin_selects_its_part_and_highlights_its_net() {
    let (mut p, r1) = open_design();
    inspector(
        &mut p,
        NetInspectorMsg::TabSelected(NetInspectorTab::ByComponent),
    );
    inspector(&mut p, NetInspectorMsg::ComponentToggled("R1".into()));
    let pin = view(&p).components[1].pin_rows[0].clone();
    p.select_uuid(uuid::Uuid::from_u128(0xBEEF), SelectedKind::Wire);

    let click = NetInspectorMsg::MemberClicked {
        net: pin.net.clone(),
        anchor: pin.anchor.clone(),
    };
    let task = p.app.update(Message::Dock(crate::dock::DockMessage::Panel(
        PanelMsg::NetInspector(click),
    )));
    crate::app::path_ops::settle_task(&mut p.app, task);

    assert_eq!(p.selected(), [SelectedItem::new(r1, SelectedKind::Symbol)]);
    let highlight = p.app.document_state.net_nav.highlight.as_ref();
    assert_eq!(highlight.map(|h| h.name.as_str()), Some("VCC"));
}
