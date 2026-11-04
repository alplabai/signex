//! Supply tab — manufacturer-part grid (primary + alternates) and a
//! distributor-listing grid, built from the shared dialog kit.
//!
//! The shape this view edits:
//!
//! - `state.row.primary_mpn`        : `ManufacturerPart` — headline part.
//! - `state.row.alternates`         : `Vec<ManufacturerPart>` — AVL.
//! - `state.row.supply`             : `Vec<DistributorListing>` — sourcing rows.
//!
//! Two grids: **Manufacturer parts** (Role / Manufacturer / MPN / Status /
//! Notes, the primary row first and marked) and **Distributor listings**
//! (Distributor / SKU / Link). Every value mutation flows through
//! `EditorMsg::Supply*` → the library dispatcher's `dispatch/library/`
//! handlers, which write to the editor's draft row and mark it dirty.

use iced::widget::{column, pick_list, scrollable, text, text_input};
use iced::{Element, Length};
use signex_library::{AlternateStatus, DistributorSource, ManufacturerPart};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress};
use super::kit::{self, SP, TS};
use super::params::editor_event;

const STATUS_OPTS: [AlternateStatus; 4] = [
    AlternateStatus::Primary,
    AlternateStatus::Approved,
    AlternateStatus::Conditional,
    AlternateStatus::Disqualified,
];

const DISTRIBUTOR_OPTS: [DistributorSource; 7] = [
    DistributorSource::DigiKey,
    DistributorSource::Mouser,
    DistributorSource::Lcsc,
    DistributorSource::Jlcpcb,
    DistributorSource::Octopart,
    DistributorSource::Oemsecrets,
    DistributorSource::Other,
];

/// Wrapper so the pick_list `Display` impl prints a friendly label
/// instead of the bare debug variant name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StatusPick(AlternateStatus);

impl std::fmt::Display for StatusPick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self.0 {
            AlternateStatus::Primary => "Primary",
            AlternateStatus::Approved => "Approved",
            AlternateStatus::Conditional => "Conditional",
            AlternateStatus::Disqualified => "Disqualified",
            // `AlternateStatus` is `#[non_exhaustive]` — fall back to debug
            // for any future variants rather than failing to compile.
            other => return write!(f, "{other:?}"),
        };
        f.write_str(s)
    }
}

/// Wrapper so the pick_list `Display` impl prints a friendly label
/// instead of the bare debug variant name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DistributorPick(DistributorSource);

impl std::fmt::Display for DistributorPick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(distributor_label(self.0))
    }
}

fn distributor_label(s: DistributorSource) -> &'static str {
    match s {
        DistributorSource::DigiKey => "DigiKey",
        DistributorSource::Mouser => "Mouser",
        DistributorSource::Lcsc => "LCSC",
        DistributorSource::Jlcpcb => "JLCPCB",
        DistributorSource::Octopart => "Octopart",
        DistributorSource::Oemsecrets => "OEMsecrets",
        DistributorSource::Other => "Other",
    }
}

/// Best-effort reverse of `distributor_label` — turn the canonical string
/// stored on `DistributorListing.distributor` back into a
/// `DistributorSource` for the pick_list selection. Unknown / legacy
/// strings select `Other` so the picker can still drive the row.
fn distributor_from_label(s: &str) -> DistributorSource {
    match s {
        "DigiKey" => DistributorSource::DigiKey,
        "Mouser" => DistributorSource::Mouser,
        "LCSC" => DistributorSource::Lcsc,
        "JLCPCB" => DistributorSource::Jlcpcb,
        "Octopart" => DistributorSource::Octopart,
        "OEMsecrets" => DistributorSource::Oemsecrets,
        _ => DistributorSource::Other,
    }
}

/// Convert a picked `DistributorSource` to the canonical string written
/// onto `DistributorListing.distributor`. Used by the dispatcher.
pub(crate) fn distributor_source_to_string(s: DistributorSource) -> String {
    distributor_label(s).to_string()
}

const PART_COLS: [(&str, Length); 6] = [
    ("Role", Length::FillPortion(2)),
    ("Manufacturer", Length::FillPortion(4)),
    ("MPN", Length::FillPortion(4)),
    ("Status", Length::FillPortion(3)),
    ("Notes", Length::FillPortion(4)),
    ("", Length::FillPortion(2)),
];

const LISTING_COLS: [(&str, Length); 4] = [
    ("Distributor", Length::FillPortion(3)),
    ("SKU", Length::FillPortion(4)),
    ("Link", Length::FillPortion(6)),
    ("", Length::FillPortion(2)),
];

/// Role cell text: the primary part, or `Alt n` (1-based) for an alternate.
fn role_label(alternate_idx: Option<usize>) -> String {
    match alternate_idx {
        None => "Primary".to_string(),
        Some(i) => format!("Alt {}", i + 1),
    }
}

/// Empty-state text for the alternates / listings grids.
fn empty_text(listings: bool) -> &'static str {
    if listings {
        "No distributor listings yet."
    } else {
        "No alternates yet."
    }
}

pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let body = column![
        parts_section(state, tokens, &address),
        listings_section(state, tokens, &address),
    ]
    .spacing(SP.xl)
    .width(Length::Fill);

    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

// ─────────────────────────── Cell builders ─────────────────────────────

fn input<'a>(
    placeholder: &'a str,
    value: &'a str,
    address: &EditorAddress,
    make: impl Fn(String) -> EditorMsg + 'a,
) -> Element<'a, LibraryMessage> {
    let address = address.clone();
    text_input(placeholder, value)
        .on_input(move |s| editor_event(&address, make(s)))
        .padding([SP.s, SP.m])
        .size(TS.label)
        .into()
}

fn status_picker<'a>(
    current: AlternateStatus,
    address: &EditorAddress,
    make: impl Fn(AlternateStatus) -> EditorMsg + 'a,
) -> Element<'a, LibraryMessage> {
    let address = address.clone();
    pick_list(
        STATUS_OPTS.map(StatusPick),
        Some(StatusPick(current)),
        move |StatusPick(s)| editor_event(&address, make(s)),
    )
    .text_size(TS.label)
    .padding([SP.s, SP.m])
    .width(Length::Fill)
    .into()
}

fn role_cell<'a>(
    label: String,
    primary: bool,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let color = if primary {
        theme_ext::accent(tokens)
    } else {
        theme_ext::text_secondary(tokens)
    };
    text(label).size(TS.body).color(color).into()
}

fn blank<'a>() -> Element<'a, LibraryMessage> {
    text("").size(TS.body).into()
}

// ─────────────────────── Manufacturer parts grid ───────────────────────

fn parts_section<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let add = kit::primary_button(
        "Add alternate",
        Some(editor_event(address, EditorMsg::SupplyAlternateAdd)),
        tokens,
    );
    let primary = &state.row.primary_mpn;
    let [role_w, mfg_w, mpn_w, status_w, notes_w, action_w] = PART_COLS.map(|(_, w)| w);

    let primary_row = kit::table_row(
        vec![
            (role_cell(role_label(None), true, tokens), role_w),
            (
                input(
                    "Texas Instruments",
                    &primary.manufacturer,
                    address,
                    EditorMsg::SupplyPrimarySetManufacturer,
                ),
                mfg_w,
            ),
            (
                input(
                    "LM358MX",
                    &primary.mpn,
                    address,
                    EditorMsg::SupplyPrimarySetMpn,
                ),
                mpn_w,
            ),
            (
                status_picker(primary.status, address, EditorMsg::SupplyPrimarySetStatus),
                status_w,
            ),
            (
                input(
                    "Preferred vendor, lead time",
                    primary.notes.as_deref().unwrap_or(""),
                    address,
                    EditorMsg::SupplyPrimarySetNotes,
                ),
                notes_w,
            ),
            (blank(), action_w),
        ],
        0,
        None,
        tokens,
    );

    let mut grid = column![kit::table_header(&PART_COLS, tokens), primary_row];
    for (idx, alt) in state.row.alternates.iter().enumerate() {
        grid = grid.push(alternate_row(idx, alt, tokens, address));
    }
    if state.row.alternates.is_empty() {
        grid = grid.push(kit::empty_state(empty_text(false), None, tokens));
    }

    column![
        kit::section_header("Manufacturer parts", Some(add), tokens),
        kit::card(grid, tokens),
    ]
    .spacing(SP.m)
    .into()
}

fn alternate_row<'a>(
    idx: usize,
    alt: &'a ManufacturerPart,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let [role_w, mfg_w, mpn_w, status_w, notes_w, action_w] = PART_COLS.map(|(_, w)| w);
    kit::table_row(
        vec![
            (role_cell(role_label(Some(idx)), false, tokens), role_w),
            (
                input("Manufacturer", &alt.manufacturer, address, move |value| {
                    EditorMsg::SupplyAlternateSetManufacturer { idx, value }
                }),
                mfg_w,
            ),
            (
                input("MPN", &alt.mpn, address, move |value| {
                    EditorMsg::SupplyAlternateSetMpn { idx, value }
                }),
                mpn_w,
            ),
            (
                status_picker(alt.status, address, move |value| {
                    EditorMsg::SupplyAlternateSetStatus { idx, value }
                }),
                status_w,
            ),
            (
                input(
                    "Notes",
                    alt.notes.as_deref().unwrap_or(""),
                    address,
                    move |value| EditorMsg::SupplyAlternateSetNotes { idx, value },
                ),
                notes_w,
            ),
            (
                kit::danger_button(
                    "Remove",
                    Some(editor_event(
                        address,
                        EditorMsg::SupplyAlternateRemove { idx },
                    )),
                    tokens,
                ),
                action_w,
            ),
        ],
        idx + 1,
        None,
        tokens,
    )
}

// ──────────────────────── Distributor listings ─────────────────────────

fn listings_section<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let add = kit::primary_button(
        "Add listing",
        Some(editor_event(address, EditorMsg::SupplyListingAdd)),
        tokens,
    );
    let mut grid = column![kit::table_header(&LISTING_COLS, tokens)];
    for (idx, listing) in state.row.supply.iter().enumerate() {
        grid = grid.push(listing_row(
            idx,
            listing.distributor.as_str(),
            listing.sku.as_str(),
            listing.url.as_deref().unwrap_or(""),
            tokens,
            address,
        ));
    }
    if state.row.supply.is_empty() {
        grid = grid.push(kit::empty_state(empty_text(true), None, tokens));
    }

    column![
        kit::section_header("Distributor listings", Some(add), tokens),
        kit::card(grid, tokens),
    ]
    .spacing(SP.m)
    .into()
}

fn listing_row<'a>(
    idx: usize,
    distributor_str: &str,
    sku: &'a str,
    url: &'a str,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let [dist_w, sku_w, url_w, action_w] = LISTING_COLS.map(|(_, w)| w);
    let picker_address = address.clone();
    let distributor_picker = pick_list(
        DISTRIBUTOR_OPTS.map(DistributorPick),
        Some(DistributorPick(distributor_from_label(distributor_str))),
        move |DistributorPick(value)| {
            editor_event(
                &picker_address,
                EditorMsg::SupplyListingSetDistributor { idx, value },
            )
        },
    )
    .text_size(TS.label)
    .padding([SP.s, SP.m])
    .width(Length::Fill);

    kit::table_row(
        vec![
            (distributor_picker.into(), dist_w),
            (
                input("296-1395-1-ND", sku, address, move |value| {
                    EditorMsg::SupplyListingSetSku { idx, value }
                }),
                sku_w,
            ),
            (
                input(
                    "https://www.distributor.com/...",
                    url,
                    address,
                    move |value| EditorMsg::SupplyListingSetUrl { idx, value },
                ),
                url_w,
            ),
            (
                kit::danger_button(
                    "Remove",
                    Some(editor_event(
                        address,
                        EditorMsg::SupplyListingRemove { idx },
                    )),
                    tokens,
                ),
                action_w,
            ),
        ],
        idx,
        None,
        tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_labels_mark_primary_and_number_alternates() {
        assert_eq!(role_label(None), "Primary");
        assert_eq!(role_label(Some(0)), "Alt 1");
        assert_eq!(role_label(Some(2)), "Alt 3");
    }

    #[test]
    fn empty_state_text_has_no_glyph_hacks() {
        for listings in [false, true] {
            let t = empty_text(listings);
            assert!(t.is_ascii() && !t.contains('+'), "{t}");
        }
    }

    #[test]
    fn grids_have_issue_columns_and_a_trailing_action() {
        let titles: Vec<&str> = PART_COLS.iter().map(|c| c.0).collect();
        assert_eq!(
            &titles[..5],
            ["Role", "Manufacturer", "MPN", "Status", "Notes"]
        );
        assert_eq!(titles[5], "");
        let titles: Vec<&str> = LISTING_COLS.iter().map(|c| c.0).collect();
        assert_eq!(&titles[..3], ["Distributor", "SKU", "Link"]);
    }

    #[test]
    fn distributor_labels_round_trip() {
        for d in DISTRIBUTOR_OPTS {
            assert_eq!(distributor_from_label(distributor_label(d)), d);
        }
    }
}
