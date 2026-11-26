//! An extension's Active Bar row (#2495): it ends its tool group's dropdown
//! below a separator, the group's own rows keep their order, and a pick runs
//! its command through `dispatch_command` like every row.

use super::tests::{board_app, entries, rows};
use super::*;
use crate::extension::test_extension::{SAY_HELLO, sample_host};

/// The labels of `menu`'s entries, `---` for a separator.
fn labels(app: &Signex, menu: PcbBarMenu) -> Vec<String> {
    entries(app, menu)
        .into_iter()
        .map(|entry| match entry {
            DropdownEntry::Item(item) => item.label,
            DropdownEntry::Header(header) => header,
            _ => "---".to_owned(),
        })
        .collect()
}

#[test]
fn an_extension_row_ends_its_groups_dropdown_and_runs_its_command() {
    let route = PcbBarMenu::Group(crate::app::pcb_bar_groups::PcbBarGroup::Route);
    let tune = PcbBarMenu::Group(crate::app::pcb_bar_groups::PcbBarGroup::Tune);
    let community = board_app();
    let (host, _seen) = sample_host();
    let mut app = board_app().with_extensions(host);

    // Another group is unchanged; the Route group gains a separator and
    // the row, after its own rows.
    assert_eq!(labels(&app, tune), labels(&community, tune));
    let before = labels(&community, route);
    let after = labels(&app, route);
    assert_eq!(after[..before.len()], before[..]);
    assert_eq!(after[before.len()..], ["---".to_owned(), "Wiki".to_owned()]);

    let row = rows(&app, route)
        .into_iter()
        .last()
        .expect("the extension row");
    assert!(!row.disabled, "its command has no context it waits for");
    let _ = app.update(row.on_press.expect("a live row"));
    assert!(
        app.ui_state
            .command_log
            .log
            .rows()
            .any(|logged| logged.command == SAY_HELLO)
    );
}
