//! The sketch mode bar and bake preview card, one chrome on both hosts
//! (#2518): Bake preview · Discard changes · Bake, with Discard changes
//! asking first when there are edits. The footprint editor mounts
//! [`footprint_overlay`]; the board sketch view builds the same bar with
//! its own messages through [`mode_bar`]. The card lists why a sketch does
//! not bake and what fails, with its place (mockup row 190); a footprint's
//! card sits in Properties ([`FootprintBakeCard`]), so it covers no canvas.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Space, column, container, row};
use iced::{Border, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::sketch::baked_kinds::BakedKind;
use signex_types::sketch::{SketchHost, SketchRole};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_dispatch::current_role_of;
use crate::library::editor::footprint::sketch_mode::roles::role_label;
use crate::library::editor::footprint::sketch_mode::{BakePreview, SketchExit};
use crate::library::editor::footprint::state::board_checks::{GapOffer, mm, open_end_text};
use crate::library::editor::footprint::state::{CheckStatus, EditorMode};
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::{DialogButtonKind, dialog_button};
use signex_bake::open_ends::{OpenEnd, warning_names_end};
use signex_sketch::id::SketchEntityId;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of a count row's label column, in body text heights.
const COUNT_LABEL_EM: f32 = 9.0;
/// Widest the bake preview card grows, in body text heights.
pub(crate) const PREVIEW_CARD_MAX_EM: f32 = 26.0;
/// The mode bar's distance from the canvas edges, on both hosts.
pub(crate) const MODE_BAR_INSET: f32 = SP.xl;

/// How far the mode bar reaches up from the canvas bottom: its inset, its
/// own padding and a row of dialog buttons. An overlay that ends this far
/// up leaves the bar in reach.
pub(crate) const fn mode_bar_reach() -> f32 {
    MODE_BAR_INSET + 2.0 * SP.s + crate::styles::ChromeMetrics::DEFAULT.button
}

/// The messages the mode bar sends; the host decides what they mean.
pub struct ModeBarMessages<M> {
    pub preview: M,
    pub discard: M,
    pub confirm_discard: M,
    pub keep_editing: M,
    /// `None` greys Bake out.
    pub bake: Option<M>,
}

/// What the mode bar shows besides its buttons.
pub struct ModeBarLook<'a, M> {
    /// The mode's name: "Sketch" or "Board Sketch".
    pub title: Localized,
    /// A line after the title, such as why the last bake was refused.
    pub status: Option<Element<'a, M>>,
    /// Bake's tooltip: what Bake does on this host.
    pub bake_tooltip: Localized,
}

/// The mode bar: the mode's name, then the one way out. While a discard
/// waits for its answer the buttons are the question instead.
pub fn mode_bar<'a, M: Clone + 'a>(
    look: ModeBarLook<'a, M>,
    exit: &SketchExit,
    msgs: ModeBarMessages<M>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, M> {
    let dot_size = TS.caption * 0.6;
    let accent = theme_ext::accent(tokens);
    let dot = container(Space::new())
        .width(Length::Fixed(dot_size))
        .height(Length::Fixed(dot_size))
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(accent)),
            border: Border {
                radius: crate::styles::Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..Default::default()
        });
    let mut bar = row![
        dot,
        text(look.title)
            .size(TS.body)
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..signex_widgets::ui_font::regular()
            })
            .color(theme_ext::text_primary(tokens)),
    ]
    .spacing(SP.m)
    .align_y(iced::Alignment::Center);
    if let Some(status) = look.status {
        bar = bar.push(status);
    }
    if exit.confirming_discard {
        bar = bar
            .push(
                text(t!(i18n, ids::EDITORS_SKETCH_DISCARD_CONFIRM))
                    .size(TS.body)
                    .color(theme_ext::text_primary(tokens))
                    .boxed(),
            )
            .push(dialog_button(
                t!(i18n, ids::EDITORS_SKETCH_KEEP_EDITING),
                DialogButtonKind::Secondary,
                Some(msgs.keep_editing),
                tokens,
            ))
            .push(dialog_button(
                t!(i18n, ids::EDITORS_SKETCH_DISCARD_CHANGES),
                DialogButtonKind::Danger,
                Some(msgs.confirm_discard),
                tokens,
            ));
    } else {
        let preview = if exit.preview {
            ids::EDITORS_SKETCH_BACK_TO_SKETCH
        } else {
            ids::EDITORS_SKETCH_BAKE_PREVIEW
        };
        let bake = dialog_button(
            t!(i18n, ids::EDITORS_SKETCH_BAKE),
            DialogButtonKind::Primary,
            msgs.bake,
            tokens,
        );
        bar = bar
            .push(dialog_button(
                t!(i18n, preview),
                DialogButtonKind::Secondary,
                Some(msgs.preview),
                tokens,
            ))
            .push(dialog_button(
                t!(i18n, ids::EDITORS_SKETCH_DISCARD_CHANGES),
                DialogButtonKind::Danger,
                Some(msgs.discard),
                tokens,
            ))
            .push(tip(bake, look.bake_tooltip, tokens));
    }
    container(bar)
        .padding([SP.s, SP.m])
        .style(crate::styles::modal_card(tokens))
        .boxed()
}

/// `content` with `tip` on hover, in the house tooltip look.
fn tip<'a, M: 'a>(content: Element<'a, M>, tip: Localized, tokens: &ThemeTokens) -> Element<'a, M> {
    let r = crate::styles::tokens::resolve(tokens);
    let tip = container(
        text(tip)
            .size(TS.caption)
            .color(theme_ext::text_primary(tokens)),
    )
    .padding([SP.xs, SP.s])
    .style(signex_widgets::active_bar_dropdown::tooltip_style(
        r.raised,
        r.line_strong,
    ));
    signex_widgets::tooltip(content, tip, signex_widgets::tooltip::Position::Top).boxed()
}

/// The catalog id naming a bake target in the preview.
pub fn target_label_id(kind: BakedKind) -> MessageId {
    match kind {
        BakedKind::Pads => ids::EDITORS_SKETCH_TARGET_PADS,
        BakedKind::SilkF => ids::EDITORS_SKETCH_TARGET_SILK_TOP,
        BakedKind::SilkB => ids::EDITORS_SKETCH_TARGET_SILK_BOTTOM,
        BakedKind::Courtyard => ids::EDITORS_SKETCH_TARGET_COURTYARD,
        BakedKind::MaskOpenings => ids::EDITORS_SKETCH_TARGET_MASK_OPENINGS,
        BakedKind::MaskExcludes => ids::EDITORS_SKETCH_TARGET_MASK_EXCLUDES,
        BakedKind::PasteApertures => ids::EDITORS_SKETCH_TARGET_PASTE_APERTURES,
        BakedKind::Pours => ids::EDITORS_SKETCH_TARGET_POURS,
        BakedKind::Keepouts => ids::EDITORS_SKETCH_TARGET_KEEPOUTS,
        BakedKind::Cutouts => ids::EDITORS_SKETCH_TARGET_CUTOUTS,
        BakedKind::VScores => ids::EDITORS_SKETCH_TARGET_V_SCORES,
        BakedKind::BoardOutline => ids::EDITORS_SKETCH_TARGET_BOARD_OUTLINE,
        BakedKind::MountingHoles => ids::EDITORS_SKETCH_TARGET_MOUNTING_HOLES,
        BakedKind::BoardPours => ids::EDITORS_SKETCH_TARGET_POURS,
        BakedKind::Mechanical => ids::EDITORS_SKETCH_TARGET_MECHANICAL,
        BakedKind::ComponentAnchors => ids::EDITORS_SKETCH_TARGET_COMPONENT_ANCHORS,
    }
}

/// One line of a check or error list: a status dot and its text. A line
/// that names a place is a button that sends `select` (mockup row 179d, mockup row 190).
/// Shared by the preview card and the Outline checks section.
pub fn status_line<'a, M: Clone + 'a>(
    status: CheckStatus,
    label: Localized,
    select: Option<M>,
) -> Element<'a, M> {
    let colour = move |theme: &iced::Theme| {
        let palette = theme.palette();
        match status {
            CheckStatus::Passed => palette.success.base.color,
            CheckStatus::Failed => palette.danger.base.color,
            CheckStatus::NotChecked => palette.background.base.text.scale_alpha(MUTED_ALPHA),
        }
    };
    let dot_size = TS.caption * 0.6;
    let dot = container(Space::new())
        .width(Length::Fixed(dot_size))
        .height(Length::Fixed(dot_size))
        .style(move |theme: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(colour(theme))),
            border: Border {
                radius: crate::styles::Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..Default::default()
        });
    let label = text(label)
        .size(TS.caption)
        .style(move |theme: &iced::Theme| iced::widget::text::Style {
            color: Some(match status {
                CheckStatus::Failed => colour(theme),
                _ => theme.palette().background.base.text,
            }),
            selection: None,
        });
    let content = row![dot, label]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    match select {
        Some(msg) => signex_widgets::button(content)
            .padding([SP.xs, SP.s])
            .width(Length::Fill)
            .on_press(msg)
            .style(|theme: &iced::Theme, _| iced::widget::button::Style {
                background: Some(iced::Background::Color(
                    theme
                        .palette()
                        .danger
                        .base
                        .color
                        .scale_alpha(SELECTABLE_TINT),
                )),
                ..iced::widget::button::Style::default()
            })
            .boxed(),
        None => container(content)
            .padding([SP.xs, SP.s])
            .width(Length::Fill)
            .boxed(),
    }
}

/// Alpha of a line that did not run, against the text colour.
const MUTED_ALPHA: f32 = 0.55;
/// Tint behind a line a click selects from.
const SELECTABLE_TINT: f32 = 0.06;

/// What the preview card lists besides its counts: why the sketch does not
/// bake and what else fails, each with the message that selects its place.
pub struct PreviewIssues<M> {
    /// Selects the entity the bake's refusal names (a board).
    pub refusal_select: Option<M>,
    /// The heading of `lines`: Outline checks on a board, Sketch warnings
    /// on a footprint.
    pub heading: MessageId,
    pub lines: Vec<(CheckStatus, Localized, Option<M>)>,
    /// Under `lines`.
    pub caption: Option<MessageId>,
    /// An open profile's gap (mockup row 190): whether it heals, and the message
    /// Close gap sends when it does.
    pub gap: Option<GapLine<M>>,
}

/// The open-profile line of the preview card: the gap and the heal
/// tolerance, and Close gap's message when the gap heals.
pub struct GapLine<M> {
    pub text: Localized,
    pub close: Option<M>,
}

impl<M> GapLine<M> {
    /// The line for `offer`; `close` makes the message Close gap sends.
    pub fn of(
        offer: &GapOffer,
        i18n: &I18n,
        close: impl FnOnce(SketchEntityId, SketchEntityId) -> M,
    ) -> Self {
        use signex_i18n::RenderTranslatable as _;
        Self {
            text: offer.text().localized(i18n),
            close: offer.closes().then(|| close(offer.keep, offer.merge)),
        }
    }

    /// The line for an open end. A paired end of a solved sketch is a gap
    /// with its offer; Close gap needs a solve, so any other end only says
    /// where the profile is open.
    pub fn of_open_end(
        end: &OpenEnd,
        solved: bool,
        i18n: &I18n,
        close: impl FnOnce(SketchEntityId, SketchEntityId) -> M,
    ) -> Option<Self> {
        use signex_i18n::RenderTranslatable as _;
        if let Some(offer) = GapOffer::of(end).filter(|_| solved) {
            return Some(Self::of(&offer, i18n, close));
        }
        Some(Self {
            text: open_end_text(end)?.localized(i18n),
            close: None,
        })
    }
}

/// The bake preview card: a count per target of the host, or why the
/// sketch does not bake (a click selects the place it names), then the
/// host's `issues`. The canvas draws the result meanwhile.
pub fn preview_card<'a, M: Clone + 'a>(
    preview: &BakePreview,
    issues: PreviewIssues<M>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, M> {
    let primary = theme_ext::text_primary(tokens);
    let muted = theme_ext::text_secondary(tokens);
    let heading = |id: MessageId| -> Element<'a, M> {
        text(t!(i18n, id)).size(TS.body).color(primary).boxed()
    };
    let caption = |id: MessageId| -> Element<'a, M> {
        text(t!(i18n, id)).size(TS.caption).color(muted).boxed()
    };
    let mut card = column![heading(ids::EDITORS_SKETCH_BAKE_RESULT)].spacing(SP.s);
    match preview {
        BakePreview::Counts(counts) => {
            for &(kind, count) in counts {
                card = card.push(
                    row![
                        text(t!(i18n, target_label_id(kind)))
                            .size(TS.body)
                            .color(muted)
                            .width(Length::Fixed(TS.body * COUNT_LABEL_EM)),
                        text(t!(i18n, ids::EDITORS_SKETCH_BAKE_COUNT, "count" => count))
                            .size(TS.body)
                            .color(primary),
                    ]
                    .spacing(SP.m)
                    .boxed(),
                );
            }
            card = card.push(caption(ids::SKETCH_BAKE_DRAWN));
            // A footprint bakes past an open profile; the gap is why one
            // shape is missing.
            if issues.gap.is_some() {
                card = card.push(heading(ids::SKETCH_BAKE_ERRORS));
            }
        }
        BakePreview::Refused(reason) => {
            let refused = t!(i18n, ids::EDITORS_SKETCH_BAKE_REFUSED, "reason" => reason.as_str());
            card = card
                .push(heading(ids::SKETCH_BAKE_ERRORS))
                .push(status_line(
                    CheckStatus::Failed,
                    refused,
                    issues.refusal_select.clone(),
                ));
        }
    }
    if let Some(gap) = issues.gap {
        card = card.push(gap_line(gap, tokens, i18n));
    }
    let selectable = issues.refusal_select.is_some()
        || issues.lines.iter().any(|(_, _, select)| select.is_some());
    if !issues.lines.is_empty() {
        card = card.push(heading(issues.heading));
        for (status, label, select) in issues.lines {
            card = card.push(status_line(status, label, select));
        }
        if let Some(id) = issues.caption {
            card = card.push(caption(id));
        }
    }
    if selectable {
        card = card.push(caption(ids::SKETCH_BAKE_SELECT_HINT));
    }
    card = card.push(caption(ids::EDITORS_SKETCH_BAKE_NEVER));
    // Fills the Properties column, capped so long lines wrap (mockup row 179d).
    container(card)
        .padding([SP.s, SP.m])
        .width(Length::Fill.max(TS.body * PREVIEW_CARD_MAX_EM))
        .style(crate::styles::modal_card(tokens))
        .boxed()
}

/// The gap and the tolerance, then Close gap when the gap heals.
fn gap_line<'a, M: Clone + 'a>(
    gap: GapLine<M>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, M> {
    let has_button = gap.close.is_some();
    let mut line = row![status_line(CheckStatus::Failed, gap.text, None)]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
    if has_button {
        line = line.push(dialog_button(
            t!(i18n, ids::SKETCH_BAKE_CLOSE_GAP),
            DialogButtonKind::Primary,
            gap.close,
            tokens,
        ));
    }
    line.boxed()
}

/// The footprint editor's mode bar at the canvas's bottom left. The open
/// bake preview's card is in Properties, not over the canvas (mockup row
/// 190). Empty outside sketch mode.
pub fn footprint_overlay<'a>(
    editor: &'a FootprintDocument,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let fp = |msg: FootprintEditorMsg| LibraryMessage::PrimitiveEditorEvent {
        path: editor.path.clone(),
        msg: PrimitiveEdit::Footprint(msg),
    };
    let exit_state = &editor.state.sketch_exit;
    let look = ModeBarLook {
        title: t!(i18n, ids::EDITORS_SKETCH_MODE_TITLE),
        status: None,
        bake_tooltip: t!(i18n, ids::EDITORS_SKETCH_BAKE_TOOLTIP),
    };
    let msgs = ModeBarMessages {
        preview: fp(FootprintEditorMsg::SketchBakePreviewToggled),
        discard: fp(FootprintEditorMsg::SketchDiscardRequested),
        confirm_discard: fp(FootprintEditorMsg::SketchDiscardConfirmed),
        keep_editing: fp(FootprintEditorMsg::SketchDiscardCancelled),
        bake: Some(fp(FootprintEditorMsg::SketchBakeRequested)),
    };
    let mut stack = column![].spacing(SP.m);
    // Jump to pad / Back to pad sit directly above the mode bar.
    if let Some(link) = super::pad_link::link_button(editor, tokens, i18n) {
        stack = stack.push(link);
    }
    stack = stack.push(mode_bar(look, exit_state, msgs, tokens, i18n));
    container(stack)
        .padding(MODE_BAR_INSET)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Left)
        .align_y(Vertical::Bottom)
        .boxed()
}

/// A footprint's open bake preview (mockup row 190): what Bake writes or
/// why it does not, and what else the card lists, read from the editor into
/// the Properties panel context. Properties shows the card, so it covers no
/// canvas.
#[derive(Debug, Clone, PartialEq)]
pub struct FootprintBakeCard {
    pub preview: BakePreview,
    /// The first open profile end, with the role of its first edge.
    pub open_end: Option<(OpenEnd, Option<SketchRole>)>,
    /// Whether the sketch is solved: Close gap needs a solve.
    pub solved: bool,
    /// The sketch warnings, less the one the open end's line already says.
    pub warnings: Vec<String>,
}

impl FootprintBakeCard {
    /// The card of `editor`; `None` unless its footprint sketch has the bake
    /// preview open. A board sketch has its own Bake panel.
    pub fn of(editor: &FootprintDocument) -> Option<Self> {
        if !Self::is_shown(editor) {
            return None;
        }
        let state = &editor.state;
        let sketch = editor.primitive().sketch.as_ref();
        let open_end = state.first_open_end(sketch);
        let warnings = state
            .solve_warnings
            .iter()
            .filter(|warning| {
                !open_end
                    .as_ref()
                    .is_some_and(|end| warning_names_end(warning, end))
            })
            .cloned()
            .collect();
        let open_end = open_end.map(|end| {
            let object = end.edges.first().and_then(|id| {
                let entity = sketch?.entities.iter().find(|e| e.id == *id)?;
                Some(current_role_of(entity).on_host(SketchHost::Footprint))
            });
            (end, object)
        });
        Some(Self {
            preview: BakePreview::of_footprint_editor(editor),
            open_end,
            solved: state.last_solve.is_some(),
            warnings,
        })
    }

    /// Whether `editor` has a card: its footprint sketch has the bake
    /// preview open.
    pub fn is_shown(editor: &FootprintDocument) -> bool {
        let state = &editor.state;
        state.sketch_exit.preview
            && state.mode == EditorMode::Sketch
            && state.sketch_host() == SketchHost::Footprint
    }

    /// What the card lists besides its counts. A footprint bakes on every
    /// solve, so what the last bake left out is in the sketch warnings, with
    /// refused tool clicks and solver notes; the open profile the gap line
    /// names is said there once. `close` makes Close gap's message.
    pub fn issues<M>(
        &self,
        i18n: &I18n,
        close: impl FnOnce(SketchEntityId, SketchEntityId) -> M,
    ) -> PreviewIssues<M> {
        let lines = self
            .warnings
            .iter()
            .map(|warning| {
                let line = t!(i18n, ids::SKETCH_BAKE_WARNING, "warning" => warning.as_str());
                (CheckStatus::Failed, line, None)
            })
            .collect();
        PreviewIssues {
            refusal_select: None,
            heading: ids::SKETCH_BAKE_WARNINGS,
            lines,
            caption: None,
            gap: self
                .open_end
                .as_ref()
                .and_then(|(end, object)| named_gap_line(end, *object, self.solved, i18n, close)),
        }
    }
}

/// The open-profile line: the object and its place, then whether the gap
/// heals, with Close gap's message when it does.
fn named_gap_line<M>(
    end: &OpenEnd,
    object: Option<SketchRole>,
    solved: bool,
    i18n: &I18n,
    close: impl FnOnce(SketchEntityId, SketchEntityId) -> M,
) -> Option<GapLine<M>> {
    let line = GapLine::of_open_end(end, solved, i18n, close)?;
    let object = object.map(|role| role_label(role, SketchHost::Footprint, i18n));
    let (Some(object), Some(at), Some(_)) = (object, end.at, end.gap_mm) else {
        return Some(line);
    };
    let text = t!(
        i18n,
        ids::SKETCH_BAKE_OPEN_PROFILE,
        "object" => object.as_str(),
        "x" => mm(at.x, 2).as_str(),
        "y" => mm(at.y, 2).as_str(),
        "heal" => line.text.into_string()
    );
    Some(GapLine {
        text,
        close: line.close,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mockup row 179d: a long failing check wraps inside the capped card,
    /// so the card leaves the canvas beside it visible.
    #[test]
    fn the_preview_card_keeps_to_its_width() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let i18n = I18n::en_us();
        let warning = "self-intersection ".repeat(30);
        let issues = PreviewIssues::<()> {
            refusal_select: None,
            heading: ids::SKETCH_BAKE_CHECKS,
            lines: vec![(
                CheckStatus::Failed,
                t!(i18n, ids::SKETCH_BAKE_WARNING, "warning" => warning.as_str()),
                None,
            )],
            caption: None,
            gap: None,
        };
        let card = preview_card(
            &BakePreview::Refused("open profile".into()),
            issues,
            &tokens,
            &i18n,
        );
        let texts = crate::layout_probe::lay_out_texts(card, iced::Size::new(1200.0, 800.0));
        let cap = TS.body * PREVIEW_CARD_MAX_EM;
        assert!(!texts.is_empty());
        for laid in &texts {
            assert!(
                laid.bounds.x + laid.bounds.width <= cap,
                "{:?} passes {cap}",
                laid.bounds
            );
        }
    }

    /// A footprint in sketch mode whose selected Point drives pad "1", so
    /// the canvas offers Jump to pad.
    fn editor_offering_jump_to_pad() -> FootprintDocument {
        use signex_sketch::SketchData;
        use signex_sketch::attr::PadAttr;
        use signex_sketch::entity::{Entity, EntityKind};
        use signex_sketch::plane::{Plane, PlaneId, PlaneKind};

        let plane = PlaneId::new();
        let centre = SketchEntityId::new();
        let mut point = Entity::new(centre, plane, EntityKind::Point { x: 0.0, y: 0.0 });
        point.pad = Some(PadAttr {
            number: "1".into(),
            ..PadAttr::default()
        });
        let mut fp = signex_library::Footprint::empty("t");
        fp.sketch = Some(SketchData {
            planes: vec![Plane {
                id: plane,
                kind: PlaneKind::BoardTop,
            }],
            entities: vec![point],
            ..SketchData::default()
        });
        fp.pads.push(signex_library::Pad {
            number: "1".into(),
            ..signex_library::Pad::default()
        });
        let mut editor = FootprintDocument::new(
            std::path::PathBuf::from("t.snxfpt"),
            signex_library::FootprintFile::from_footprint(fp),
        );
        editor.state.mode = crate::library::editor::footprint::state::EditorMode::Sketch;
        editor.state.pads[0].sketch_entity_id = Some(centre);
        editor.state.selected_sketch = Some(centre);
        assert!(
            editor.state.pad_link_offer().is_some(),
            "the fixture offers a link"
        );
        editor
    }

    /// Caner 2026-11-24, mockup row 190: with the Bake preview open, the
    /// card is in Properties, not over the canvas; Jump to pad sits just
    /// above the mode bar and covers none of its text.
    #[test]
    fn the_pad_link_sits_above_the_mode_bar_with_the_preview_open() {
        let tokens = signex_types::theme::theme_tokens(signex_types::theme::ThemeId::GraphiteLight);
        let i18n = I18n::en_us();
        let mut editor = editor_offering_jump_to_pad();
        editor.state.sketch_exit.preview = true;
        assert!(
            FootprintBakeCard::of(&editor).is_some(),
            "Properties shows the card"
        );

        let overlay = footprint_overlay(&editor, &tokens, &i18n);
        let texts = crate::layout_probe::lay_out_texts(overlay, iced::Size::new(1200.0, 800.0));

        let jump = i18n.message_plain(ids::SKETCH_TOOLS_JUMP_TO_PAD, &[]);
        let heading = i18n.message_plain(ids::EDITORS_SKETCH_BAKE_RESULT, &[]);
        let title = i18n.message_plain(ids::EDITORS_SKETCH_MODE_TITLE, &[]);
        assert!(
            texts.iter().all(|t| t.text != heading),
            "the canvas still shows the card"
        );
        let link = crate::layout_probe::find(&texts, &jump).bounds;
        let bar_top = crate::layout_probe::find(&texts, &title).bounds.y;
        assert!(
            link.y + link.height <= bar_top,
            "the link {link:?} reaches below the mode bar title at {bar_top}"
        );
        for other in texts.iter().filter(|t| t.text != jump) {
            assert!(
                other.bounds.intersection(&link).is_none(),
                "the link {link:?} covers {:?} at {:?}",
                other.text,
                other.bounds
            );
        }
    }

    /// Mockup row 190: the card is in Properties, so opening the preview (a
    /// refused Bake opens it too) brings a closed Properties to the front.
    #[test]
    fn opening_the_preview_brings_properties_forward() {
        use crate::app::{Message, Signex, TabInfo, TabKind};
        use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
        use crate::panels::PanelKind;

        let editor = editor_offering_jump_to_pad();
        let path = editor.path.clone();
        let (mut app, _task) = Signex::new();
        app.document_state
            .footprint_editors
            .insert(path.clone(), editor);
        app.document_state.tabs.push(TabInfo {
            title: "t".into(),
            path: path.clone(),
            project_id: None,
            kind: TabKind::FootprintEditor(path.clone()),
        });
        app.document_state.active_tab = app.document_state.tabs.len() - 1;
        app.document_state.dock.close_panel(PanelKind::Properties);
        assert!(!app.is_panel_open(PanelKind::Properties));

        let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
            path,
            msg: PrimitiveEdit::Footprint(FootprintEditorMsg::SketchBakePreviewToggled),
        }));

        assert!(
            app.document_state.dock.is_frontmost(PanelKind::Properties),
            "the card opened where it cannot be seen"
        );
    }
}
