//! The Roles section of a footprint sketch (mockup row 190): one row per
//! closed profile with its role, what bakes without an entity, and the way
//! to preview or run the bake. Rules: `docs/FOOTPRINT_SKETCH.md`, "Roles".
//! A board sketch's Bake panel reuses the rows as Bake as (mockup row 179).

use iced::widget::{Column, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_types::sketch::SketchHost;

use super::super::{FootprintModeKind, PanelMsg, SketchFixMsg};
use super::body::{Ctx, caption, ghost, is_open, section};
use crate::library::editor::footprint::sketch_mode::roles::{ProfileRoleRow, role_labels};
use crate::library::messages::RoleTag;
use crate::styles::Spacing;

const SP: Spacing = Spacing::DEFAULT;

const SECTION: &str = "fp_sketch_roles";
const BOARD_SECTION: &str = "board_sketch_bake_as";

pub(super) fn view_roles<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    if fp.mode_kind != FootprintModeKind::Sketch || fp.sketch_host != SketchHost::Footprint {
        return col;
    }
    let title = t!(i18n, ids::SKETCH_BAKE_ROLES);
    col = col.push(section(SECTION, &title, None, cx.collapsed, tokens, i18n));
    if !is_open(SECTION, cx.collapsed) {
        return col;
    }
    profile_rows(col, cx)
        .push(caption(t!(i18n, ids::SKETCH_BAKE_ROLES_MASK), tokens))
        .push(caption(
            t!(i18n, ids::SKETCH_BAKE_ROLES_CENTERLINES, "count" => fp.centerline_count),
            tokens,
        ))
        .push(bake_buttons(cx))
}

/// The Bake as section of a board sketch, in its Bake panel and its
/// Properties body (mockup row 179):
/// the footprint's profile list with the board's roles, without its Bake
/// buttons, mask and centerline lines. Nothing on a footprint.
pub(super) fn view_board_bake_as<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    if fp.mode_kind != FootprintModeKind::Sketch || fp.sketch_host != SketchHost::Board {
        return col;
    }
    let title = t!(i18n, ids::SKETCH_BAKE_BAKE_AS);
    col = col.push(section(
        BOARD_SECTION,
        &title,
        None,
        cx.collapsed,
        tokens,
        i18n,
    ));
    if !is_open(BOARD_SECTION, cx.collapsed) {
        return col;
    }
    profile_rows(col, cx)
}

/// One picker row per closed profile, or the empty line.
fn profile_rows<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let fp = cx.fp;
    if fp.profile_roles.is_empty() {
        col = col.push(caption(
            t!(cx.i18n, ids::SKETCH_BAKE_ROLES_EMPTY),
            cx.tokens,
        ));
    }
    for (index, profile) in fp.profile_roles.iter().enumerate() {
        col = col.push(role_row(cx, index + 1, profile));
    }
    col
}

fn role_row<'a>(
    cx: Ctx<'a, '_>,
    index: usize,
    profile: &'a ProfileRoleRow,
) -> Element<'a, PanelMsg> {
    let i18n = cx.i18n;
    let name = match &profile.pad_number {
        Some(number) => t!(
            i18n,
            ids::SKETCH_BAKE_ROLES_PAD_PROFILE,
            "number" => number.as_str(),
            "edges" => profile.edges
        ),
        None => t!(
            i18n,
            ids::SKETCH_BAKE_ROLES_PROFILE,
            "index" => index,
            "edges" => profile.edges
        ),
    }
    .into_string();
    let labels = role_labels(cx.fp.sketch_host, i18n);
    let label_of = move |r: &RoleTag| {
        labels
            .iter()
            .find(|(role, _)| role == r)
            .map(|(_, label)| label.clone())
            .unwrap_or_default()
    };
    let seed = profile.seed;
    super::rows::pick_row_labelled(
        &name,
        &profile.options,
        profile.role,
        label_of,
        move |role| PanelMsg::SketchFix(SketchFixMsg::ProfileRolePicked { seed, role }),
        cx.palette.muted,
    )
}

fn bake_buttons<'a>(cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    let (tokens, i18n) = (cx.tokens, cx.i18n);
    container(
        row![
            ghost(
                t!(i18n, ids::EDITORS_SKETCH_BAKE_PREVIEW),
                Some(PanelMsg::SketchFix(SketchFixMsg::BakePreviewClicked)),
                tokens,
            ),
            ghost(
                t!(i18n, ids::EDITORS_SKETCH_BAKE),
                Some(PanelMsg::SketchFix(SketchFixMsg::BakeClicked)),
                tokens,
            ),
        ]
        .spacing(SP.s),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
#[path = "roles_tests.rs"]
mod tests;
