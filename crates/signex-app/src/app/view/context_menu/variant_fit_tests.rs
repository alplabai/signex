//! #2662, mockup row 32: on a repeated sheet the In variant flyout's
//! Fitted and Not fitted offer "This channel only (CH2)" and "All
//! channels", each one undo step of the General history; Editor offers only
//! All channels; before a compile, or in Base, the rows are greyed.

use std::path::PathBuf;

use signex_i18n::I18n;
use signex_types::pcb::SymbolLink;
use signex_types::project::PartRef;
use signex_types::schematic::{
    ChannelOccurrence, InstancePath, SchematicSheet, SelectedItem, SelectedKind, Symbol,
};
use signex_widgets::active_bar_dropdown::{DropdownEntry, DropdownItem};
use uuid::Uuid;

use super::menus::MenuVariant;
use super::submenu::in_variant_entries;
use super::variant_fit::channel_fit_rows;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with};
use crate::app::{Message, ProjectId, Signex, TabInfo, TabKind};
use crate::panels::compiled_design::{
    ChannelCopies, ChannelPartFit, ChannelStripView, ChannelTabView,
};

const R5: Uuid = Uuid::from_u128(0x2662_0005);

/// Channel `i`'s copy of R5.
fn copy(i: u32) -> PartRef {
    let path = InstancePath::from_chain(vec![
        Uuid::from_u128(0x2662_0000),
        Uuid::from_u128(0x2662_0000 + u128::from(i)),
    ]);
    PartRef::new(format!("CH{i}_R5"), Some(SymbolLink::new(path, [R5])))
}

/// The strip of `sheet` repeated as CH1 to CH3, every copy fitted, with
/// `active` shown.
fn strip(sheet: PathBuf, active: Option<u32>) -> ChannelStripView {
    let tab = |i: u32| ChannelTabView {
        channel: ChannelOccurrence {
            name: format!("CH{i}"),
            index: i,
            number: i,
            of: 3,
        },
        parts: Some(1),
        nets: Some(0),
        designators: vec![format!("CH{i}_R5")],
        miniature: None,
        fits: vec![ChannelPartFit {
            part: copy(i),
            is_fitted: true,
        }],
    };
    ChannelStripView {
        sheet,
        file: "a.snxsch".into(),
        tabs: (1..=3).map(tab).collect(),
        active,
        designator_map: Vec::new(),
        variant: Some("Lite".into()),
        is_fit_locked: false,
    }
}

fn labels(rows: &[DropdownEntry<Message>]) -> Vec<String> {
    rows.iter()
        .filter_map(|row| match row {
            DropdownEntry::Item(item) => Some(item.label.clone()),
            _ => None,
        })
        .collect()
}

fn row<'a>(rows: &'a [DropdownEntry<Message>], label: &str) -> &'a DropdownItem<Message> {
    rows.iter()
        .find_map(|row| match row {
            DropdownEntry::Item(item) if item.label == label => Some(item),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no row {label:?} in {:?}", labels(rows)))
}

#[test]
fn a_channel_tab_offers_this_channel_and_all_channels() {
    let copies = strip(PathBuf::from("a.snxsch"), Some(2)).copies_of(R5);
    assert_eq!(copies.shown, Some(("CH2".to_string(), copy(2))));
    assert_eq!(copies.all, [copy(1), copy(2), copy(3)]);
    let rows = channel_fit_rows(I18n::en_us_ref(), &copies, false);
    assert_eq!(
        labels(&rows),
        [
            "Fitted · This channel only (CH2)",
            "Fitted · All channels",
            "Not fitted · This channel only (CH2)",
            "Not fitted · All channels",
        ]
    );
    assert!(rows.iter().all(|entry| match entry {
        DropdownEntry::Item(item) => !item.disabled && item.on_press.is_some(),
        _ => true,
    }));
}

#[test]
fn editor_offers_all_channels_only_and_base_or_no_compile_grey_the_rows() {
    let editor = strip(PathBuf::from("a.snxsch"), None).copies_of(R5);
    let rows = channel_fit_rows(I18n::en_us_ref(), &editor, false);
    assert_eq!(
        labels(&rows),
        ["Fitted · All channels", "Not fitted · All channels"]
    );

    let in_base = channel_fit_rows(I18n::en_us_ref(), &editor, true);
    let greyed = row(&in_base, "Not fitted · All channels");
    assert!(greyed.disabled && greyed.on_press.is_none());

    let uncompiled = ChannelCopies::default();
    let rows = channel_fit_rows(I18n::en_us_ref(), &uncompiled, false);
    let greyed = row(&rows, "Fitted · All channels");
    assert!(greyed.disabled && greyed.on_press.is_none());
}

/// A project with Lite active whose sheet `a.snxsch`, shown on its CH2
/// tab, places R5, selected; the channel strip lists its three copies.
fn app() -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    let path = dir.path().join("a.snxsch");
    let sheet = SchematicSheet {
        symbols: vec![Symbol {
            uuid: R5,
            lib_id: "Device:R".to_string(),
            reference: "R5".to_string(),
            value: "31.6k".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    open_with(&mut app.document_state, &path, sheet);
    let id = app.document_state.projects[0].id;
    app.document_state.tabs.push(TabInfo {
        title: "a".to_string(),
        path: path.clone(),
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    app.document_state.active_path = Some(path.clone());
    let data = &mut app.document_state.projects[0].data;
    data.variant_definitions = vec!["Lite".to_string()];
    data.active_variant = Some("Lite".to_string());
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(R5, SelectedKind::Symbol)];
    app.document_state.panel_ctx.compiled_design.strip = Some(strip(path, Some(2)));
    (app, id, dir)
}

/// The menu's rows, as the main window's flyout builds them.
fn flyout(app: &Signex, id: ProjectId) -> Vec<DropdownEntry<Message>> {
    let window = app.ui_state.main_window_id.expect("main window");
    let copies = app.canvas_menu_channel_copies(window);
    assert!(copies.is_some(), "the sheet is the repeated one");
    let variant = MenuVariant {
        project_id: id,
        name: "Lite".to_string(),
    };
    in_variant_entries(I18n::en_us_ref(), &variant, Some("R5"), copies.as_ref())
}

#[test]
fn this_channel_only_sets_the_shown_channels_copy_as_one_step() {
    let (mut app, id, _dir) = app();
    let rows = flyout(&app, id);
    let press = row(&rows, "Not fitted · This channel only (CH2)")
        .on_press
        .clone()
        .expect("a live row");
    let _ = app.update(press);
    let settings = &app.document_state.projects[0].data.variant_settings;
    assert_eq!(settings.parts.len(), 1);
    let own = settings.part(Some("Lite"), copy(2).placed());
    assert_eq!(own.and_then(|own| own.fitted), Some(false));
    assert!(settings.part(Some("Lite"), copy(1).placed()).is_none());
    assert!(app.step_project_tab_history(id, true));
    assert!(
        app.document_state.projects[0]
            .data
            .variant_settings
            .is_empty()
    );
}

#[test]
fn all_channels_sets_every_copy_as_one_step() {
    let (mut app, id, _dir) = app();
    let rows = flyout(&app, id);
    let press = row(&rows, "Not fitted · All channels")
        .on_press
        .clone()
        .expect("a live row");
    let _ = app.update(press);
    let settings = &app.document_state.projects[0].data.variant_settings;
    assert_eq!(settings.parts.len(), 3);
    for i in 1..=3 {
        let own = settings.part(Some("Lite"), copy(i).placed());
        assert_eq!(own.and_then(|own| own.fitted), Some(false), "CH{i}");
    }
    assert!(app.step_project_tab_history(id, true));
    assert!(
        app.document_state.projects[0]
            .data
            .variant_settings
            .is_empty(),
        "one step takes every channel's copy back"
    );
}

/// A plain sheet keeps the greyed rows: no strip names its sheet.
#[test]
fn a_plain_sheet_has_no_channel_rows() {
    let (mut app, _id, _dir) = app();
    app.document_state.panel_ctx.compiled_design.strip = None;
    let window = app.ui_state.main_window_id.expect("main window");
    assert_eq!(app.canvas_menu_channel_copies(window), None);
}
