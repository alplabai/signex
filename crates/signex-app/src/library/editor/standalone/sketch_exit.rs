//! The sketch mode bar and bake preview card, one chrome on both hosts
//! (#2518): Bake preview · Discard changes · Bake, with Discard changes
//! asking first when there are edits. The footprint editor mounts
//! [`footprint_overlay`]; the board sketch view builds the same bar with
//! its own messages through [`mode_bar`]. The card lists why a sketch does
//! not bake and what fails, with its place (mockup row 190).

use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Space, column, container, row};
use iced::{Border, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::sketch::baked_kinds::BakedKind;
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, theme_ext};

use crate::app::FootprintDocument;
use crate::library::editor::footprint::sketch_mode::{BakePreview, SketchExit};
use crate::library::editor::footprint::state::CheckStatus;
use crate::library::editor::footprint::state::board_checks::GapOffer;
use crate::library::messages::{FootprintEditorMsg, LibraryMessage, PrimitiveEdit};
use crate::styles::{DialogButtonKind, dialog_button};
use signex_sketch::id::SketchEntityId;

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of a count row's label column, in body text heights.
const COUNT_LABEL_EM: f32 = 9.0;

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
    container(card)
        .padding([SP.s, SP.m])
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

/// The footprint editor's mode bar, with the preview card above it when
/// open, at the canvas's bottom left. Empty outside sketch mode.
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
    if exit_state.preview {
        let preview = BakePreview::of_footprint(editor.primitive());
        // A footprint bakes on every solve: what the last bake left out is
        // in the sketch warnings, with refused tool clicks and solver notes;
        // they name no entity.
        let issues = PreviewIssues {
            refusal_select: None,
            heading: ids::SKETCH_BAKE_WARNINGS,
            lines: editor
                .state
                .solve_warnings
                .iter()
                .map(|warning| {
                    let line = t!(i18n, ids::SKETCH_BAKE_WARNING, "warning" => warning.as_str());
                    (CheckStatus::Failed, line, None)
                })
                .collect(),
            caption: None,
            // An open profile: why its shape is missing, and Close gap.
            gap: editor
                .state
                .gap_offer(editor.primitive().sketch.as_ref())
                .map(|offer| {
                    GapLine::of(&offer, i18n, |keep, merge| {
                        fp(FootprintEditorMsg::SketchCloseGapRequested { keep, merge })
                    })
                }),
        };
        stack = stack.push(preview_card(&preview, issues, tokens, i18n));
    }
    stack = stack.push(mode_bar(look, exit_state, msgs, tokens, i18n));
    container(stack)
        .padding(SP.xl)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Left)
        .align_y(Vertical::Bottom)
        .boxed()
}
