//! Layout probes of the Edit Component modal at its card size (mockup row
//! 130): the quiet Reload at the footer's left, and a left column whose
//! binding cards fit without scrolling, the Internal PN conflict included,
//! with the conflict reason as the Parameters column's foot banner.

use std::path::{Path, PathBuf};

use iced::Size;
use signex_library::{PrimitiveKind, PrimitiveSummary, RowId};
use signex_types::theme::{ThemeId, theme_tokens};

use super::super::messages::{BrowserEditMsg, LibraryMessage};
use super::super::state::{EditRowModalState, EditorAddress};
use super::{Binding, COLUMN_PAD, EditRowContext, LEFT_COLUMN_W, view};
use crate::layout_probe::{LaidOutText, lay_out_texts, messages_on_click};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The card's own size: the probe window holds exactly the modal.
const CARD: Size = Size::new(
    crate::library::modal_chrome::EDIT_ROW_W,
    crate::library::modal_chrome::EDIT_ROW_H,
);

const RELOAD: &str = "Reload saved version";
const PICK_FOOTPRINT: &str = "Pick Footprint\u{2026}";
const CONFLICT_REASON: &str = "Internal PN \u{201C}U1\u{201D} is already used in components.";

fn modal() -> (PathBuf, EditRowModalState) {
    let row = crate::library::component_preview::test_row();
    let path = PathBuf::from("/tmp/stock.snxlib");
    let address = EditorAddress::new(path.clone(), "components".into(), RowId::from(row.row_id));
    (path, EditRowModalState::new(address, row))
}

fn summary(kind: PrimitiveKind, file: &str, name: &str) -> PrimitiveSummary {
    PrimitiveSummary {
        uuid: uuid::Uuid::now_v7(),
        name: name.into(),
        kind,
        used_by_count: 0,
        file: Some(file.into()),
    }
}

/// Both bindings found, with the mockup's longest footprint label, and the
/// Internal PN conflict when `conflict`.
fn context<'a>(
    symbol: &'a PrimitiveSummary,
    footprint: &'a PrimitiveSummary,
    conflict: bool,
) -> EditRowContext<'a> {
    EditRowContext {
        classes: Vec::new(),
        template: None,
        library_name: Some("Stock"),
        symbol: Binding::Found(symbol),
        footprint: Binding::Found(footprint),
        pn_conflict: conflict.then(|| "components".to_string()),
    }
}

/// What the probe reads off one laid-out modal: its texts without Fluent's
/// isolation marks, and what a click on `click` publishes.
fn probe(
    path: &Path,
    edit: &EditRowModalState,
    conflict: bool,
    click: &str,
) -> (Vec<LaidOutText>, Vec<LibraryMessage>) {
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let i18n = signex_i18n::I18n::en_us_ref();
    let symbol = summary(PrimitiveKind::Symbol, "resistor.snxsym", "R");
    let footprint = summary(
        PrimitiveKind::Footprint,
        "chip-passives.snxfpt",
        "R_0402_1005",
    );
    let build = || {
        view(
            path,
            edit,
            &tokens,
            i18n,
            false,
            context(&symbol, &footprint, conflict),
        )
    };
    let texts = lay_out_texts(build(), CARD)
        .into_iter()
        .map(|t| LaidOutText {
            text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
            bounds: t.bounds,
        })
        .collect();
    (texts, messages_on_click(build(), CARD, click))
}

fn find<'t>(texts: &'t [LaidOutText], label: &str) -> &'t LaidOutText {
    crate::layout_probe::find(texts, label)
}

fn reload_requested(clicked: &[LibraryMessage]) -> bool {
    clicked.iter().any(|m| {
        matches!(
            m,
            LibraryMessage::BrowserEdit {
                msg: BrowserEditMsg::ReloadRequested,
                ..
            }
        )
    })
}

/// Mockup row 130: Reload saved version sits at the footer's left with no
/// refused save behind it, and asks for the reload.
#[test]
fn reload_is_at_the_footer_left_without_a_stale_save() {
    let (path, edit) = modal();
    assert!(!edit.stale_base);
    let (texts, clicked) = probe(&path, &edit, false, RELOAD);
    let reload = find(&texts, RELOAD);
    let cancel = find(&texts, "Cancel");
    assert!(
        reload.bounds.x < cancel.bounds.x,
        "Reload is left of Cancel: {texts:#?}"
    );
    assert!(
        (reload.bounds.center_y() - cancel.bounds.center_y()).abs() < 1.0,
        "Reload shares the footer line: {texts:#?}"
    );
    assert!(reload_requested(&clicked), "{clicked:?}");
}

/// How far the pixels of `area` reach from its top-left pixel towards
/// `ink` (0 = none, 1 = the ink itself). Panics on a pixel off that line,
/// so a label in another colour (the accent, the danger red) fails.
fn reach_towards(pixels: &[u8], window: Size, area: iced::Rectangle, ink: iced::Color) -> f32 {
    let stride = window.width as usize * 4;
    let (left, right) = (area.x as usize, (area.x + area.width) as usize);
    let (top, bottom) = (area.y as usize, (area.y + area.height) as usize);
    let channels = |p: &[u8]| [f32::from(p[0]), f32::from(p[1]), f32::from(p[2])];
    let back = channels(&pixels[top * stride + left * 4..]);
    let target = channels(&ink.into_rgba8());
    let to_ink = [0, 1, 2].map(|i| target[i] - back[i]);
    let length: f32 = to_ink.iter().map(|d| d * d).sum();
    let mut reach = 0.0_f32;
    for y in top..bottom {
        for x in left..right {
            let pixel = channels(&pixels[y * stride + x * 4..]);
            let delta = [0, 1, 2].map(|i| pixel[i] - back[i]);
            let t = (0..3).map(|i| delta[i] * to_ink[i]).sum::<f32>() / length;
            assert!(
                (0..3).all(|i| (delta[i] - t * to_ink[i]).abs() <= 16.0) && t > -0.1,
                "pixel ({x}, {y}) is {pixel:?}, not between {back:?} and {target:?}"
            );
            reach = reach.max(t);
        }
    }
    reach
}

/// Mockup row 130 (#3011): Reload saved version is the quiet `.btn.quiet`,
/// its label drawn in the secondary ink, not the accent or danger red, in
/// both Graphite themes.
#[test]
fn reload_is_drawn_in_the_quiet_secondary_ink() {
    let (path, edit) = modal();
    let i18n = signex_i18n::I18n::en_us_ref();
    let symbol = summary(PrimitiveKind::Symbol, "resistor.snxsym", "R");
    let footprint = summary(PrimitiveKind::Footprint, "chip-passives.snxfpt", "R");
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(theme);
        let build = || {
            view(
                &path,
                &edit,
                &tokens,
                i18n,
                false,
                context(&symbol, &footprint, false),
            )
        };
        let label = lay_out_texts(build(), CARD)
            .into_iter()
            .find(|t| t.text.replace(['\u{2068}', '\u{2069}'], "") == RELOAD)
            .unwrap_or_else(|| panic!("{theme:?}: no {RELOAD:?}"))
            .bounds;
        let pixels = crate::layout_probe::draw_pixels(build(), CARD);
        let ink = crate::styles::ti(tokens.text_secondary);
        let reach = reach_towards(&pixels, CARD, label, ink);
        assert!(reach >= 0.5, "{theme:?}: the label reaches {reach}");
    }
}

/// Mockup row 130 (conflict): Reload stays at the footer's left beside the
/// problem summary while Save waits.
#[test]
fn reload_stays_in_the_footer_in_the_conflict_state() {
    let (path, edit) = modal();
    let (texts, clicked) = probe(&path, &edit, true, RELOAD);
    let reload = find(&texts, RELOAD);
    let summary = find(&texts, "1 problem: Internal PN");
    assert!(
        reload.bounds.x + reload.bounds.width <= summary.bounds.x,
        "Reload comes first, then the summary: {texts:#?}"
    );
    assert!(reload_requested(&clicked), "{clicked:?}");
    let (_, clicked) = probe(&path, &edit, true, "Save");
    assert!(clicked.is_empty(), "Save waits: {clicked:?}");
}

/// While the reload asks, the footer shows the inline confirm in its place.
#[test]
fn the_reload_confirm_replaces_the_action_inline() {
    let (path, mut edit) = modal();
    edit.reload_confirming = true;
    let (texts, clicked) = probe(&path, &edit, false, "Reload");
    assert!(texts.iter().all(|t| t.text != RELOAD), "{texts:#?}");
    find(&texts, "Discard your edits and reload?");
    find(&texts, "Keep editing");
    assert!(
        clicked.iter().any(|m| matches!(
            m,
            LibraryMessage::BrowserEdit {
                msg: BrowserEditMsg::ReloadConfirmed,
                ..
            }
        )),
        "{clicked:?}"
    );
}

/// Mockup row 130: the binding cards fit the left column, each line on one
/// line, with and without the Internal PN conflict; the conflict reason is
/// the error banner at the foot of the Parameters column, never in the left
/// column.
#[test]
fn the_binding_cards_fit_the_left_column() {
    let (path, edit) = modal();
    for conflict in [false, true] {
        let (texts, _) = probe(&path, &edit, conflict, "Cancel");
        let one_line = |label: &str, size: f32| {
            let t = find(&texts, label);
            assert!(
                t.bounds.height < 2.0 * size,
                "{label:?} sits on one line (conflict {conflict}): {t:#?}"
            );
            t
        };
        one_line("Symbol ref", TS.caption);
        let title = one_line("Footprint ref", TS.caption);
        // Ellipsised when the card is narrow, so matched by its start.
        let bound = texts
            .iter()
            .find(|t| t.text.starts_with("chip-passives.snxfpt"))
            .unwrap_or_else(|| panic!("no footprint binding label: {texts:#?}"));
        assert!(
            bound.bounds.height < 2.0 * TS.body,
            "the binding label sits on one line (conflict {conflict}): {bound:#?}"
        );
        let pick = find(&texts, PICK_FOOTPRINT);
        assert!(
            bound.bounds.x + bound.bounds.width <= pick.bounds.x,
            "the bound label stops before Pick Footprint… (conflict {conflict})"
        );
        assert!(title.bounds.width > 0.0);

        let card_bottom =
            (bound.bounds.y + bound.bounds.height).max(pick.bounds.y + pick.bounds.height) + SP.m;
        let footer_top = find(&texts, "Cancel").bounds.y - SP.l;
        assert!(
            card_bottom + COLUMN_PAD[0] <= footer_top + 0.5,
            "the Footprint card ends above the footer, no scroll (conflict {conflict}): \
             card bottom {card_bottom}, footer top {footer_top}"
        );
        let left_edge = LEFT_COLUMN_W;
        assert!(
            texts
                .iter()
                .filter(|t| t.bounds.x < left_edge)
                .all(|t| !t.text.contains(CONFLICT_REASON)),
            "the left column holds no conflict text (conflict {conflict}): {texts:#?}"
        );
        let banner = texts.iter().find(|t| t.text == CONFLICT_REASON);
        if !conflict {
            assert!(banner.is_none(), "no banner without a conflict: {texts:#?}");
            continue;
        }
        let banner = banner.unwrap_or_else(|| panic!("no conflict banner: {texts:#?}"));
        assert!(
            banner.bounds.x >= left_edge,
            "the banner sits in the Parameters column: {banner:#?}"
        );
        assert!(
            banner.bounds.height < 2.0 * TS.body,
            "the banner reason sits on one line: {banner:#?}"
        );
        let banner_bottom = banner.bounds.y + banner.bounds.height;
        assert!(
            banner_bottom <= footer_top + 0.5 && footer_top - banner_bottom < 4.0 * TS.body,
            "the banner is pinned to the foot of the column, above the footer: \
             banner bottom {banner_bottom}, footer top {footer_top}"
        );
    }
}
