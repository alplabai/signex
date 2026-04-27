//! Supply tab — primary MPN + ranked alternates + distributor listings
//! editor. Retargeted to `ComponentRow` (DBLib model) per
//! `v0.9-refactor-2-plan.md` §11.4.
//!
//! The shape this view edits:
//!
//! - `state.row.primary_mpn`        : `ManufacturerPart` — headline part.
//! - `state.row.alternates`         : `Vec<ManufacturerPart>` — AVL.
//! - `state.row.supply`             : `Vec<DistributorListing>` — sourcing rows.
//!
//! Layout is three muted-header sections:
//!
//! 1. **Primary MPN** — 4-row form (Manufacturer / MPN / Status / Notes).
//! 2. **Alternates** — one inline row per alternate (manufacturer, MPN,
//!    status pick_list, notes, remove [×]) plus a `+ Add Alternate`
//!    trigger.
//! 3. **Distributor Listings** — table-like rows (distributor pick_list,
//!    sku, url, remove [×]) plus a `+ Add Listing` trigger.
//!
//! Every value mutation flows through `EditorMsg::Supply*` → the
//! library dispatcher's `apply_inline_edit` arms (see `dispatch/library.rs`),
//! which write directly to `editor.draft.*` and bump `editor.dirty`.

use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Border, Element, Length, Theme};
use signex_library::{AlternateStatus, DistributorSource, ManufacturerPart};
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::state::{ComponentPreviewState, EditorAddress};

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

pub fn view<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: EditorAddress,
) -> Element<'a, LibraryMessage> {
    let body = column![
        section_header("Primary MPN", tokens),
        Space::new().height(6),
        primary_form(state, tokens, &address),
        Space::new().height(14),
        section_header("Alternates", tokens),
        Space::new().height(6),
        alternates_section(state, tokens, &address),
        Space::new().height(14),
        section_header("Distributor Listings", tokens),
        Space::new().height(6),
        listings_section(state, tokens, &address),
    ]
    .spacing(0)
    .width(Length::Fill);

    container(scrollable(body).width(Length::Fill).height(Length::Fill))
        .padding(14)
        .style(crate::styles::modal_card(tokens))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn section_header<'a>(label: &'a str, tokens: &'a ThemeTokens) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    text(label).size(11).color(muted).into()
}

// ─────────────────────────── Primary MPN form ──────────────────────────

fn primary_form<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let primary = &state.row.primary_mpn;
    let notes_str = primary.notes.clone().unwrap_or_default();

    let mfg_field = labelled_input(
        "Manufacturer",
        primary.manufacturer.clone(),
        "Texas Instruments",
        EditorMsg::SupplyPrimarySetManufacturer,
        tokens,
        address,
    );
    let mpn_field = labelled_input(
        "MPN",
        primary.mpn.clone(),
        "LM358MX",
        EditorMsg::SupplyPrimarySetMpn,
        tokens,
        address,
    );

    let lib_path = address.library_path.clone();
    let table = address.table.clone();
    let row_id = address.row_id;
    let status_picker = pick_list(
        STATUS_OPTS.map(StatusPick),
        Some(StatusPick(primary.status)),
        move |StatusPick(s)| LibraryMessage::EditorEvent {
            library_path: lib_path.clone(),
            table: table.clone(),
            row_id,
            msg: EditorMsg::SupplyPrimarySetStatus(s),
        },
    )
    .text_size(12)
    .padding([4, 8]);
    let status_block: Element<'a, LibraryMessage> =
        column![text("Status").size(10).color(muted), status_picker,]
            .spacing(4)
            .into();

    let notes_field = labelled_input(
        "Notes",
        notes_str,
        "preferred vendor / lead-time / etc.",
        EditorMsg::SupplyPrimarySetNotes,
        tokens,
        address,
    );

    column![
        mfg_field,
        Space::new().height(8),
        mpn_field,
        Space::new().height(8),
        status_block,
        Space::new().height(8),
        notes_field,
    ]
    .spacing(0)
    .width(Length::Fill)
    .into()
}

// ────────────────────────── Alternates section ─────────────────────────

fn alternates_section<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let mut col = column![].spacing(6);
    if state.row.alternates.is_empty() {
        col = col.push(
            text("No alternates yet — click \u{2009}+ Add Alternate\u{2009} to start.")
                .size(11)
                .color(muted),
        );
    } else {
        for (idx, alt) in state.row.alternates.iter().enumerate() {
            col = col.push(alternate_row(idx, alt, tokens, address));
        }
    }

    col = col.push(Space::new().height(2));
    col = col.push(add_button(
        "+ Add Alternate",
        EditorMsg::SupplyAlternateAdd,
        tokens,
        address,
    ));
    col.into()
}

fn alternate_row<'a>(
    idx: usize,
    alt: &'a ManufacturerPart,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);

    let lib_path_for_mfg = address.library_path.clone();
    let table_for_mfg = address.table.clone();
    let row_id = address.row_id;
    let mfg_input = text_input("Manufacturer", &alt.manufacturer)
        .padding([4, 8])
        .size(12)
        .width(Length::FillPortion(3))
        .on_input(move |s| LibraryMessage::EditorEvent {
            library_path: lib_path_for_mfg.clone(),
            table: table_for_mfg.clone(),
            row_id,
            msg: EditorMsg::SupplyAlternateSetManufacturer { idx, value: s },
        });

    let lib_path_for_mpn = address.library_path.clone();
    let table_for_mpn = address.table.clone();
    let mpn_input = text_input("MPN", &alt.mpn)
        .padding([4, 8])
        .size(12)
        .width(Length::FillPortion(3))
        .on_input(move |s| LibraryMessage::EditorEvent {
            library_path: lib_path_for_mpn.clone(),
            table: table_for_mpn.clone(),
            row_id,
            msg: EditorMsg::SupplyAlternateSetMpn { idx, value: s },
        });

    let lib_path_for_status = address.library_path.clone();
    let table_for_status = address.table.clone();
    let status_picker = pick_list(
        STATUS_OPTS.map(StatusPick),
        Some(StatusPick(alt.status)),
        move |StatusPick(s)| LibraryMessage::EditorEvent {
            library_path: lib_path_for_status.clone(),
            table: table_for_status.clone(),
            row_id,
            msg: EditorMsg::SupplyAlternateSetStatus { idx, value: s },
        },
    )
    .text_size(12)
    .padding([4, 8])
    .width(Length::Fixed(140.0));

    let lib_path_for_notes = address.library_path.clone();
    let table_for_notes = address.table.clone();
    let notes_value = alt.notes.clone().unwrap_or_default();
    let notes_input = text_input("notes", &notes_value)
        .padding([4, 8])
        .size(12)
        .width(Length::FillPortion(4))
        .on_input(move |s| LibraryMessage::EditorEvent {
            library_path: lib_path_for_notes.clone(),
            table: table_for_notes.clone(),
            row_id,
            msg: EditorMsg::SupplyAlternateSetNotes { idx, value: s },
        });

    let remove = remove_button(
        EditorMsg::SupplyAlternateRemove { idx },
        text_c,
        border,
        address,
    );

    container(
        row![
            mfg_input,
            Space::new().width(6),
            mpn_input,
            Space::new().width(6),
            status_picker,
            Space::new().width(6),
            notes_input,
            Space::new().width(6),
            remove,
        ]
        .align_y(iced::Alignment::Center)
        .padding([3, 4]),
    )
    .style(move |_: &Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            1.0, 1.0, 1.0, 0.02,
        ))),
        border: Border {
            width: 1.0,
            radius: 2.0.into(),
            color: iced::Color::from_rgba(1.0, 1.0, 1.0, 0.06),
        },
        ..Default::default()
    })
    .into()
}

// ──────────────────────── Distributor listings ─────────────────────────

fn listings_section<'a>(
    state: &'a ComponentPreviewState,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let border = theme_ext::border_color(tokens);

    let header = row![
        text("Distributor")
            .size(10)
            .color(muted)
            .width(Length::Fixed(160.0)),
        Space::new().width(6),
        text("SKU")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(3)),
        Space::new().width(6),
        text("URL")
            .size(10)
            .color(muted)
            .width(Length::FillPortion(5)),
        Space::new().width(6),
        text("").size(10).width(Length::Fixed(28.0)),
    ]
    .padding([2, 4]);

    let mut rows = column![header].spacing(4);
    if state.row.supply.is_empty() {
        rows = rows.push(text("No distributor listings yet.").size(11).color(muted));
    } else {
        for (idx, listing) in state.row.supply.iter().enumerate() {
            rows = rows.push(listing_row(
                idx,
                listing.distributor.as_str(),
                listing.sku.as_str(),
                listing.url.as_deref().unwrap_or(""),
                tokens,
                address,
            ));
        }
    }

    let table_card = container(rows)
        .padding(6)
        .width(Length::Fill)
        .style(move |_: &Theme| iced::widget::container::Style {
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..Default::default()
        });

    column![
        table_card,
        Space::new().height(6),
        add_button(
            "+ Add Listing",
            EditorMsg::SupplyListingAdd,
            tokens,
            address
        ),
    ]
    .spacing(0)
    .into()
}

fn listing_row<'a>(
    idx: usize,
    distributor_str: &str,
    sku: &str,
    url: &str,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);

    let lib_path_for_dist = address.library_path.clone();
    let table_for_dist = address.table.clone();
    let row_id = address.row_id;
    let distributor_picker = pick_list(
        DISTRIBUTOR_OPTS.map(DistributorPick),
        Some(DistributorPick(distributor_from_label(distributor_str))),
        move |DistributorPick(s)| LibraryMessage::EditorEvent {
            library_path: lib_path_for_dist.clone(),
            table: table_for_dist.clone(),
            row_id,
            msg: EditorMsg::SupplyListingSetDistributor { idx, value: s },
        },
    )
    .text_size(12)
    .padding([4, 8])
    .width(Length::Fixed(160.0));

    let lib_path_for_sku = address.library_path.clone();
    let table_for_sku = address.table.clone();
    let sku_input = text_input("296-1395-1-ND", sku)
        .padding([4, 8])
        .size(12)
        .width(Length::FillPortion(3))
        .on_input(move |s| LibraryMessage::EditorEvent {
            library_path: lib_path_for_sku.clone(),
            table: table_for_sku.clone(),
            row_id,
            msg: EditorMsg::SupplyListingSetSku { idx, value: s },
        });

    let lib_path_for_url = address.library_path.clone();
    let table_for_url = address.table.clone();
    let url_input = text_input("https://www.distributor.com/...", url)
        .padding([4, 8])
        .size(12)
        .width(Length::FillPortion(5))
        .on_input(move |s| LibraryMessage::EditorEvent {
            library_path: lib_path_for_url.clone(),
            table: table_for_url.clone(),
            row_id,
            msg: EditorMsg::SupplyListingSetUrl { idx, value: s },
        });

    let remove = remove_button(
        EditorMsg::SupplyListingRemove { idx },
        text_c,
        border,
        address,
    );

    row![
        distributor_picker,
        Space::new().width(6),
        sku_input,
        Space::new().width(6),
        url_input,
        Space::new().width(6),
        remove,
    ]
    .align_y(iced::Alignment::Center)
    .padding([2, 4])
    .into()
}

// ─────────────────────────── Shared widgets ────────────────────────────

fn labelled_input<'a>(
    label: &'static str,
    value: String,
    placeholder: &'static str,
    msg: fn(String) -> EditorMsg,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let muted = theme_ext::text_secondary(tokens);
    let lib_path = address.library_path.clone();
    let table = address.table.clone();
    let row_id = address.row_id;
    column![
        text(label).size(10).color(muted),
        text_input(placeholder, &value)
            .on_input(move |s| LibraryMessage::EditorEvent {
                library_path: lib_path.clone(),
                table: table.clone(),
                row_id,
                msg: msg(s),
            })
            .padding([4, 8])
            .size(12),
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn add_button<'a>(
    label: &'static str,
    msg: EditorMsg,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let text_c = theme_ext::text_primary(tokens);
    let border = theme_ext::border_color(tokens);
    button(container(text(label).size(11).color(text_c)).padding([4, 12]))
        .on_press(LibraryMessage::EditorEvent {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
            msg,
        })
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.08),
                )),
                _ => Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
            };
            iced::widget::button::Style {
                background: bg,
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}

fn remove_button<'a>(
    msg: EditorMsg,
    text_c: iced::Color,
    border: iced::Color,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    button(container(text("\u{00D7}".to_string()).size(13).color(text_c)).padding([0, 8]))
        .on_press(LibraryMessage::EditorEvent {
            library_path: address.library_path.clone(),
            table: address.table.clone(),
            row_id: address.row_id,
            msg,
        })
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                    iced::Color::from_rgba(1.0, 1.0, 1.0, 0.10),
                )),
                _ => Some(iced::Background::Color(iced::Color::from_rgba(
                    1.0, 1.0, 1.0, 0.04,
                ))),
            };
            iced::widget::button::Style {
                background: bg,
                text_color: text_c,
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
}
