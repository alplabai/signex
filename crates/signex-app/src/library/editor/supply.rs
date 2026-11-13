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

use iced::widget::{Column, Widget, column, pick_list, scrollable};
use iced::{Element, Length};
use signex_i18n::{I18n, ids, t};
use signex_library::{AlternateStatus, DistributorSource, ManufacturerPart};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::theme_ext;
use signex_widgets::{raw, text, text_input};

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

fn status_label(s: &AlternateStatus) -> String {
    match s {
        AlternateStatus::Primary => "Primary".to_string(),
        AlternateStatus::Approved => "Approved".to_string(),
        AlternateStatus::Conditional => "Conditional".to_string(),
        AlternateStatus::Disqualified => "Disqualified".to_string(),
        // `AlternateStatus` is `#[non_exhaustive]` — fall back to debug
        // for any future variants rather than failing to compile.
        other => format!("{other:?}"),
    }
}

fn distributor_pick_label(s: &DistributorSource) -> String {
    distributor_label(*s).to_string()
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

const PART_COLS: [kit::TableColumn; 6] = [
    (
        Some(ids::EDITORS_COMPONENT_COLUMN_ROLE),
        Length::FillPortion(2),
    ),
    (
        Some(ids::EDITORS_COMPONENT_COLUMN_MANUFACTURER),
        Length::FillPortion(4),
    ),
    (
        Some(ids::EDITORS_COMPONENT_COLUMN_MPN),
        Length::FillPortion(4),
    ),
    (
        Some(ids::EDITORS_COMPONENT_COLUMN_STATUS),
        Length::FillPortion(3),
    ),
    (
        Some(ids::EDITORS_COMPONENT_COLUMN_NOTES),
        Length::FillPortion(4),
    ),
    (None, Length::FillPortion(2)),
];

const LISTING_COLS: [kit::TableColumn; 4] = [
    (
        Some(ids::EDITORS_COMPONENT_COLUMN_DISTRIBUTOR),
        Length::FillPortion(3),
    ),
    (
        Some(ids::EDITORS_COMPONENT_COLUMN_SKU),
        Length::FillPortion(4),
    ),
    (
        Some(ids::EDITORS_COMPONENT_COLUMN_LINK),
        Length::FillPortion(6),
    ),
    (None, Length::FillPortion(2)),
];

/// Role cell text: the primary part, or `Alt n` (1-based) for an alternate.
fn role_label(alternate_idx: Option<usize>) -> String {
    match alternate_idx {
        None => "Primary".to_string(),
        Some(i) => format!("Alt {}", i + 1),
    }
}

/// Empty-state text for the alternates / listings grids.
fn empty_text(listings: bool) -> signex_i18n::MessageId {
    if listings {
        ids::EDITORS_COMPONENT_LISTINGS_EMPTY
    } else {
        ids::EDITORS_COMPONENT_PARTS_EMPTY
    }
}

pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let body = column![
        parts_section(state, tokens, i18n, &address),
        listings_section(state, tokens, i18n, &address),
    ]
    .spacing(SP.xl)
    .width(Length::Fill)
    .boxed();

    scrollable(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed()
}

// ─────────────────────────── Cell builders ─────────────────────────────

fn input<'a>(
    placeholder: &'a str,
    value: &'a str,
    address: &EditorAddress,
    make: impl Fn(String) -> EditorMsg + 'a,
) -> Element<'a, LibraryMessage> {
    let address = address.clone();
    text_input(raw(placeholder), value)
        .on_input(move |s| editor_event(&address, make(s)))
        .padding([SP.s, SP.m])
        .size(TS.label)
        .boxed()
}

fn status_picker<'a>(
    current: AlternateStatus,
    address: &EditorAddress,
    make: impl Fn(AlternateStatus) -> EditorMsg + 'a,
) -> Element<'a, LibraryMessage> {
    let address = address.clone();
    pick_list(Some(current), STATUS_OPTS, status_label)
        .graphite()
        .on_select(move |s| editor_event(&address, make(s)))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed()
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
    text(raw(label)).size(TS.body).color(color).boxed()
}

fn blank<'a>() -> Element<'a, LibraryMessage> {
    text(raw("")).size(TS.body).boxed()
}

// ─────────────────────── Manufacturer parts grid ───────────────────────

fn parts_section<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
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

    let mut grid: Column<Element<'a, LibraryMessage>> =
        column![kit::table_header(&PART_COLS, i18n, tokens), primary_row];
    for (idx, alt) in state.row.alternates.iter().enumerate() {
        grid = grid.push(alternate_row(idx, alt, tokens, address));
    }
    if state.row.alternates.is_empty() {
        grid = grid.push(kit::empty_state(t!(i18n, empty_text(false)), None, tokens));
    }

    column![
        kit::section_header(
            t!(i18n, ids::EDITORS_COMPONENT_PARTS_HEADING),
            Some(add),
            tokens
        ),
        kit::card(grid.boxed(), tokens),
    ]
    .spacing(SP.m)
    .boxed()
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
    i18n: &I18n,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let add = kit::primary_button(
        "Add listing",
        Some(editor_event(address, EditorMsg::SupplyListingAdd)),
        tokens,
    );
    let mut grid: Column<Element<'a, LibraryMessage>> =
        column![kit::table_header(&LISTING_COLS, i18n, tokens)];
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
        grid = grid.push(kit::empty_state(t!(i18n, empty_text(true)), None, tokens));
    }
    column![
        kit::section_header(
            t!(i18n, ids::EDITORS_COMPONENT_LISTINGS_HEADING),
            Some(add),
            tokens
        ),
        kit::card(grid.boxed(), tokens),
    ]
    .spacing(SP.m)
    .boxed()
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
        Some(distributor_from_label(distributor_str)),
        DISTRIBUTOR_OPTS,
        distributor_pick_label,
    )
    .graphite()
    .on_select(move |value| {
        editor_event(
            &picker_address,
            EditorMsg::SupplyListingSetDistributor { idx, value },
        )
    })
    .padding([SP.s, SP.m])
    .width(Length::Fill);

    kit::table_row(
        vec![
            (distributor_picker.boxed(), dist_w),
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
        let english = |listings| I18n::en_us().message(empty_text(listings), &[]).to_string();
        assert_eq!(english(false), "No alternates yet.");
        assert_eq!(english(true), "No distributor listings yet.");
        for listings in [false, true] {
            let text = english(listings);
            assert!(text.is_ascii() && !text.contains('+'), "{text}");
        }
    }

    #[test]
    fn grids_have_issue_columns_and_a_trailing_action() {
        let en = I18n::en_us();
        let render =
            |c: &kit::TableColumn| c.0.map_or_else(String::new, |id| t!(en, id).into_string());
        let titles: Vec<String> = PART_COLS.iter().map(render).collect();
        assert_eq!(
            &titles[..5],
            ["ROLE", "MANUFACTURER", "MPN", "STATUS", "NOTES"]
        );
        assert_eq!(titles[5], "");
        let titles: Vec<String> = LISTING_COLS.iter().map(render).collect();
        assert_eq!(&titles[..3], ["DISTRIBUTOR", "SKU", "LINK"]);
        assert_eq!(titles[3], "");
    }

    #[test]
    fn distributor_labels_round_trip() {
        for d in DISTRIBUTOR_OPTS {
            assert_eq!(distributor_from_label(distributor_label(d)), d);
        }
    }
}
