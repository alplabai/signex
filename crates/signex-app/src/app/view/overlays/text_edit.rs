//! The in-place text editor: a text input sitting on the label being edited,
//! in the canvas font and size, with its justification anchor marked and a
//! short key hint above it for the first few uses.

use super::*;
use iced::widget::{Space, column, container, row, stack};
use signex_types::schematic::{HAlign, SelectedKind};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Width of the soft focus ring around the editor, the same ring the panel
/// fields get when focused (distinct from the solid red of an error).
const FOCUS_RING: f32 = 3.0;

/// Corner radius of the editor box.
const EDITOR_RADIUS: f32 = 2.0;

/// How many opened editors show the key hint before it hides.
const TEXT_EDIT_HINT_USES: u32 = 3;

impl Signex {
    /// The editor over the label being edited in `window_id`. The object's
    /// world position goes through the live camera to window coordinates
    /// each frame, so the editor follows pan and zoom.
    pub(in crate::app::view) fn text_edit_overlay(
        &self,
        window_id: iced::window::Id,
    ) -> Option<Element<'_, Message>> {
        let document = &self.document_state;
        let interaction = &self.interaction_state;
        let engine = document.engine_for_window(window_id, &self.ui_state)?;
        let edit_state = interaction.editing_text.as_ref()?;
        let text = edit_state.text.clone();
        let placeholder: signex_widgets::Label<'static> =
            match crate::app::handlers::canvas::editor_placeholder(edit_state.kind) {
                Some(id) => self.ui_state.i18n.message(id, &[]).into(),
                None => signex_i18n::UserText::new(String::new()).into(),
            };
        // Object world position -> window position through the live camera
        // and the canvas rectangle: this is the text's justification anchor.
        let camera = interaction.canvas_for_window(window_id).live_camera();
        let (anchor_x, anchor_y) = canvas_region::world_to_window(
            (edit_state.world_x, edit_state.world_y),
            camera,
            self.canvas_bounds(),
        );
        // Font size in pixels matches the rendered label (10 pt ≈ 1.8 mm).
        let font_px = (camera.2 * 1.8).clamp(10.0, 64.0);
        // Estimate width from the text in the canvas face to keep the input
        // snug, with two characters of room to type into.
        let canvas_font_name = document.panel_ctx.canvas_font_name.as_str();
        let canvas_family = signex_types::font::nonblank_family(canvas_font_name);
        // An empty editor shows its placeholder, so it is sized for that.
        let shown = if edit_state.text.is_empty() {
            crate::app::handlers::canvas::editor_placeholder(edit_state.kind)
                .map_or(String::new(), |id| {
                    self.ui_state.i18n.message_plain(id, &[])
                })
        } else {
            edit_state.text.clone()
        };
        let approx_w = editor_width(
            signex_types::font::estimate_text_width_px(&shown, font_px, canvas_family),
            font_px,
        );
        let justify = drawn_justify(engine.document(), edit_state);
        let left = editor_left(anchor_x - SP.xs, approx_w, justify) - FOCUS_RING;
        // Sit the input's baseline on the label text.
        let top = anchor_y - font_px - SP.xs - FOCUS_RING;

        let tokens = &document.panel_ctx.tokens;
        let input = signex_widgets::text_input(placeholder, text)
            .id(crate::app::contracts::TEXT_EDIT_INPUT_ID.clone())
            .on_input(|t| Message::TextEdit(TextEditMsg::Changed(t)))
            .on_submit(Message::TextEdit(TextEditMsg::Submit))
            .font(crate::fonts::canvas_iced_font(canvas_font_name))
            .size(font_px)
            .padding([1.0, SP.xs])
            .width(approx_w)
            .style(editor_style(tokens));
        let ringed = container(input)
            .padding(FOCUS_RING)
            .style(focus_ring_style(tokens));

        let mut layers = vec![
            positioned(left, top, ringed.boxed()),
            positioned(
                anchor_x - SP.s,
                anchor_y - SP.s,
                anchor_cross(crate::styles::ti(tokens.accent)),
            ),
        ];
        if self.ui_state.text_edit_opens <= TEXT_EDIT_HINT_USES {
            let hint_h = TS.caption * crate::styles::LINE_HEIGHT + 2.0 * SP.xs;
            let hint_id = hint_message(
                crate::app::handlers::canvas::next_part_field(engine.document(), edit_state)
                    .is_some(),
            );
            let hint = self.ui_state.i18n.message_plain(hint_id, &[]);
            let hint_w = signex_types::font::estimate_text_width_px(&hint, TS.caption, None)
                + 2.0 * SP.m
                + 2.0;
            layers.push(positioned(
                hint_left(left, hint_w, self.canvas_bounds()),
                top - hint_h - SP.xs,
                key_hint(hint_id, tokens, &self.ui_state.i18n),
            ));
        }
        Some(iced::widget::Stack::with_children(layers).boxed())
    }
}

/// Horizontal justification the label is drawn with, as seen on screen; the
/// editor grows away from that side. Rotated text and net labels keep the
/// editor's left edge on the anchor.
fn drawn_justify(
    sheet: &signex_types::schematic::SchematicSheet,
    state: &crate::app::TextEditState,
) -> HAlign {
    let upright = |rotation: f64| rotation.rem_euclid(180.0) < 0.5;
    let symbol = || sheet.symbols.iter().find(|s| s.uuid == state.uuid);
    // A field's drawn justification, if it is drawn upright.
    let field = |prop: &signex_types::schematic::TextProp,
                 sym: &signex_types::schematic::Symbol| {
        let (rotation, h, _) = signex_types::schematic::field_effective_style(prop, sym);
        upright(rotation).then_some(h)
    };
    let justify = match state.kind {
        SelectedKind::TextNote => sheet
            .text_notes
            .iter()
            .find(|t| t.uuid == state.uuid)
            .filter(|t| upright(t.rotation))
            .map(|t| t.justify_h),
        SelectedKind::SymbolRefField => {
            symbol().and_then(|s| s.ref_text.as_ref().and_then(|prop| field(prop, s)))
        }
        SelectedKind::SymbolValField => {
            symbol().and_then(|s| sheet.drawn_value_text(s).and_then(|prop| field(&prop, s)))
        }
        _ => None,
    };
    justify.unwrap_or(HAlign::Left)
}

/// Window x of the editor's left edge for text anchored at `anchor_x`: a
/// left-justified text grows right, a right-justified one grows left and a
/// centred one both ways, so the saved position never shifts.
fn editor_left(anchor_x: f32, width: f32, justify: HAlign) -> f32 {
    match justify {
        HAlign::Left => anchor_x,
        HAlign::Center => anchor_x - width / 2.0,
        HAlign::Right => anchor_x - width,
    }
}

/// Width of the editor for text `text_w` wide: snug, with two characters of
/// room to type into.
fn editor_width(text_w: f32, font_px: f32) -> f32 {
    text_w + 2.0 * font_px * signex_types::font::ESTIMATE_FALLBACK_ADVANCE_EM
}

/// Window x of the key hint: over the editor, kept inside the canvas.
fn hint_left(editor_left: f32, hint_w: f32, canvas: iced::Rectangle) -> f32 {
    editor_left
        .min(canvas.x + canvas.width - hint_w)
        .max(canvas.x)
}

/// `widget` with its top-left corner at window `(x, y)`, clamped on screen.
/// Each layer fills the window: a stack limits its later layers to the size
/// of its first, which squeezed the key hint to the editor's width.
fn positioned<'a>(x: f32, y: f32, widget: Element<'a, Message>) -> Element<'a, Message> {
    column![
        Space::new().height(y.max(0.0)),
        row![Space::new().width(x.max(0.0)), widget],
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .boxed()
}

/// The editor box: paper fill, 1 px accent border, 2 px radius; selected
/// text on the soft accent so the whole-text selection on open reads.
fn editor_style(
    tokens: &signex_types::theme::ThemeTokens,
) -> impl Fn(&iced::Theme, iced::widget::text_input::Status) -> iced::widget::text_input::Style + 'static
{
    let r = crate::styles::tokens::resolve(tokens);
    let paper = crate::styles::ti(tokens.paper);
    let ink = crate::styles::ti(tokens.text);
    let accent = crate::styles::ti(tokens.accent);
    let (placeholder, selection) = (r.ink4, r.accent_soft);
    move |_: &iced::Theme, _status| iced::widget::text_input::Style {
        background: iced::Background::Color(paper),
        border: iced::Border {
            color: accent,
            width: 1.0,
            radius: EDITOR_RADIUS.into(),
            ..iced::Border::default()
        },
        placeholder,
        value: ink,
        selection,
    }
}

/// The 3 px soft accent ring around the editor.
fn focus_ring_style(
    tokens: &signex_types::theme::ThemeTokens,
) -> impl Fn(&iced::Theme) -> container::Style + 'static {
    let ring = crate::styles::tokens::resolve(tokens).accent_soft;
    move |_: &iced::Theme| container::Style {
        background: Some(iced::Background::Color(ring)),
        border: iced::Border {
            radius: (EDITOR_RADIUS + FOCUS_RING).into(),
            ..iced::Border::default()
        },
        ..container::Style::default()
    }
}

/// Small accent cross marking the justification anchor.
fn anchor_cross<'a>(accent: iced::Color) -> Element<'a, Message> {
    let side = 2.0 * SP.s + crate::styles::HAIRLINE;
    let line = move |w: f32, h: f32| -> Element<'a, Message> {
        container(
            container(Space::new())
                .width(Length::Fixed(w))
                .height(Length::Fixed(h))
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(iced::Background::Color(accent)),
                    ..container::Style::default()
                }),
        )
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .boxed()
    };
    stack![
        line(side, crate::styles::HAIRLINE),
        line(crate::styles::HAIRLINE, side)
    ]
    .boxed()
}

/// The hint's words: Tab is named only where it has a field to go to.
fn hint_message(has_next_field: bool) -> signex_i18n::MessageId {
    if has_next_field {
        signex_i18n::ids::OVERLAY_TEXT_EDIT_HINT_NEXT_FIELD
    } else {
        signex_i18n::ids::OVERLAY_TEXT_EDIT_HINT
    }
}

/// The pill above the editor naming its keys.
fn key_hint<'a>(
    hint: signex_i18n::MessageId,
    tokens: &signex_types::theme::ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, Message> {
    let r = crate::styles::tokens::resolve(tokens);
    let (raised, line, ink3) = (r.raised, crate::styles::ti(tokens.border), r.ink3);
    let shadow = crate::styles::floating_shadow(tokens);
    container(
        signex_widgets::text(signex_i18n::t!(i18n, hint))
            .size(TS.caption)
            .color(ink3)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([SP.xs, SP.m])
    .style(move |_: &iced::Theme| container::Style {
        background: Some(iced::Background::Color(raised)),
        border: iced::Border {
            width: 1.0,
            color: line,
            radius: crate::styles::Radii::GRAPHITE.small.into(),
            ..iced::Border::default()
        },
        shadow,
        ..container::Style::default()
    })
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_editor_grows_away_from_the_anchor() {
        assert_eq!(editor_left(100.0, 40.0, HAlign::Left), 100.0);
        assert_eq!(editor_left(100.0, 40.0, HAlign::Center), 80.0);
        assert_eq!(editor_left(100.0, 40.0, HAlign::Right), 60.0);
    }

    /// Mockup row 65: the box follows its text instead of a fixed floor that
    /// covered the neighbouring part.
    #[test]
    fn the_editor_is_as_wide_as_its_text_and_typing_room() {
        let room = 2.0 * 10.0 * signex_types::font::ESTIMATE_FALLBACK_ADVANCE_EM;
        assert_eq!(editor_width(12.0, 10.0), 12.0 + room);
        assert!(editor_width(12.0, 10.0) < 60.0);
    }

    /// Mockup row 65: the one-line hint stays inside the canvas.
    #[test]
    fn the_hint_stays_inside_the_canvas() {
        let canvas =
            iced::Rectangle::new(iced::Point::new(100.0, 0.0), iced::Size::new(500.0, 400.0));
        assert_eq!(hint_left(200.0, 150.0, canvas), 200.0);
        assert_eq!(hint_left(550.0, 150.0, canvas), 450.0);
        assert_eq!(hint_left(50.0, 150.0, canvas), 100.0);
    }

    /// The hint layer fills the stack, so its width is not the editor's.
    #[test]
    fn the_hint_is_laid_out_on_one_line() {
        let (app, _boot) = crate::app::Signex::new();
        let tokens = &app.document_state.panel_ctx.tokens;
        let en = signex_i18n::I18n::en_us();
        let layers = iced::widget::Stack::with_children(vec![
            positioned(0.0, 0.0, Space::new().width(20.0).height(20.0).boxed()),
            positioned(
                0.0,
                40.0,
                key_hint(signex_i18n::ids::OVERLAY_TEXT_EDIT_HINT, tokens, &en),
            ),
        ]);
        let texts =
            crate::layout_probe::lay_out_texts(layers.boxed(), iced::Size::new(800.0, 200.0));
        let hint = texts
            .iter()
            .find(|t| t.text.starts_with("Enter"))
            .expect("the hint is drawn");
        assert!(hint.bounds.width > 100.0, "{:?}", hint.bounds);
        assert!(hint.bounds.height < 2.0 * TS.caption * crate::styles::LINE_HEIGHT);
    }

    /// A text with no next field has only two keys that work, so its hint
    /// names only those.
    #[test]
    fn the_hint_names_only_the_keys_that_work() {
        let hint = signex_i18n::I18n::en_us_ref()
            .message_plain(signex_i18n::ids::OVERLAY_TEXT_EDIT_HINT, &[]);
        assert_eq!(hint, "Enter apply · Esc cancel");
    }

    /// Mockup row 65: the hint names Tab only where a next field exists.
    #[test]
    fn the_hint_names_tab_where_it_has_a_field_to_go_to() {
        let en = signex_i18n::I18n::en_us_ref();
        assert_eq!(
            en.message_plain(hint_message(true), &[]),
            "Enter apply · Esc cancel · Tab next field"
        );
        assert_eq!(
            en.message_plain(hint_message(false), &[]),
            "Enter apply · Esc cancel"
        );
    }
}
