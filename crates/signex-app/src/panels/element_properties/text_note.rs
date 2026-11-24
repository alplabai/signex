//! Properties of a selected text note (mockup row 47): the illustration,
//! Location (Position, Rotation in four steps), Basic Properties (the Text
//! as a multi-line box that grows to six lines, then scrolls) and Text
//! Properties (Justify H and V as segmented rows, Text Size, Font). State ▸
//! Locked follows from the shared router. The Text box keeps a draft in the
//! model and applies it as one edit when it loses focus (Escape, a press
//! outside it), on Ctrl+Enter, or when the note is no longer selected
//! (`PlacedPanel::sync_note`); the pointer leaving it commits nothing.

use super::super::*;
use super::font_row::font_family_row;
use super::placed_rows::{RowLook, edit_box, push_lock_note, push_position_rows};
use super::rows::{PanelStyle, h_align_from_row, info_row, justify_h_row, justify_v_row};
use crate::panels::{NoteLayout, PlacedEditMsg, PlacedField};
use crate::styles::{Spacing, TypeScale};
use iced::Widget;
use iced::widget::text_editor::{Binding, KeyPress};
use signex_i18n::{Localized, ids, t};
use signex_types::measure::length_field_text;
use signex_types::schematic::{HAlign, SelectedItem, SelectedKind, VAlign};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The quarter turns the Rotation row offers, in degrees.
const QUARTER_TURNS: [u32; 4] = [0, 90, 180, 270];

/// The justification a text note's rows report (`info_keys::JUSTIFY_H` /
/// `JUSTIFY_V`), as the segmented rows pick it.
pub(super) fn justification(info: &[(String, String)]) -> (HAlign, VAlign) {
    let v = match justify_v_row(info).as_str() {
        "Top" => VAlign::Top,
        "Bottom" => VAlign::Bottom,
        _ => VAlign::Center,
    };
    (h_align_from_row(&justify_h_row(info)), v)
}

/// The quarter turn nearest `degrees`, as every Rotation control marks it.
fn nearest_quarter_turn(degrees: f64) -> u32 {
    crate::panels::rotation_steps::nearest_rotation_step(degrees) as u32
}

pub(super) fn push_text_note_sections<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    ctx: &'a PanelContext,
    uuid: Option<uuid::Uuid>,
    st: PanelStyle,
) -> Column<Element<'a, PanelMsg>> {
    let PanelStyle { muted, palette, .. } = st;
    let i18n: &'a signex_i18n::I18n = &ctx.i18n;
    let get = |key: &str| info_row(&ctx.selection_info, key);
    let rotation = super::rows::rotation_degrees(&get("Rotation"));
    let (justify_h, justify_v) = justification(&ctx.selection_info);
    let note_font_family = Some(get("Font Family")).filter(|family| !family.is_empty());
    let Some(id) = uuid else {
        return col;
    };
    let item = SelectedItem::new(id, SelectedKind::TextNote);
    let draft = ctx.placed.note.as_ref().filter(|d| d.item == item);
    // The illustration follows what is typed, not only what is stored.
    let shown_text = draft.map_or_else(|| get("Text"), |d| d.content.text());
    col = col.push(super::illustrations::show(
        super::illustrations::text_note(&shown_text, justify_h, justify_v, i18n),
        ctx,
    ));

    let look = RowLook {
        muted,
        input_bg: palette.input_bg,
    };
    let read_only = ctx.placed.locked.is_some();
    let picked = move |layout: NoteLayout| {
        PanelMsg::PlacedEdit(PlacedEditMsg::NoteLayoutPicked { item, layout })
    };

    // ── Location ──: Position (the shared rows) and Rotation in four steps.
    col = col.push(
        ctx.collapsible(
            "sel_location",
            ids::PANELS_PLACED_LOCATION,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                c = push_lock_note(c, ctx, look);
                c = push_position_rows(c, ctx, look);
                let steps = QUARTER_TURNS.map(|degrees| {
                    let label = t!(i18n, ids::PROPERTIES_DEGREES_SHORT, "degrees" => degrees);
                    (degrees, label)
                });
                c.push(segmented_row(
                    t!(i18n, ids::PANELS_PLACED_ROTATION),
                    steps,
                    nearest_quarter_turn(rotation),
                    move |degrees| picked(NoteLayout::Rotation(f64::from(degrees))),
                    st,
                    read_only,
                ))
            },
        )
        .boxed(),
    );

    // ── Basic Properties ──: the multi-line Text box.
    col = col.push(
        ctx.collapsible(
            "sel_basic",
            ids::PROPERTIES_SECTION_BASIC,
            &ctx.collapsed_sections,
            move || {
                let c = Column::new().spacing(0).width(Length::Fill);
                let Some(draft) = draft else {
                    // The box is seeded on the next selection refresh.
                    return c.push(form_input_row(
                        t!(i18n, ids::PROPERTIES_TEXT),
                        &shown_text,
                        muted,
                        palette.input_bg,
                    ));
                };
                c.push(text_box(i18n, draft, item, muted, read_only))
            },
        )
        .boxed(),
    );

    // ── Text Properties ──: Justify H / V, Text Size and Font.
    col.push(
        ctx.collapsible(
            "sel_text",
            ids::PROPERTIES_SECTION_TEXT,
            &ctx.collapsed_sections,
            move || {
                let mut c = Column::new().spacing(0).width(Length::Fill);
                let across = [
                    (HAlign::Left, t!(i18n, ids::PANELS_MULTI_JUSTIFY_LEFT)),
                    (HAlign::Center, t!(i18n, ids::PANELS_MULTI_JUSTIFY_CENTER)),
                    (HAlign::Right, t!(i18n, ids::PANELS_MULTI_JUSTIFY_RIGHT)),
                ];
                c = c.push(segmented_row(
                    t!(i18n, ids::PROPERTIES_JUSTIFY_H),
                    across,
                    justify_h,
                    move |h| picked(NoteLayout::JustifyH(h)),
                    st,
                    read_only,
                ));
                let down = [
                    (VAlign::Top, t!(i18n, ids::PROPERTIES_JUSTIFY_TOP)),
                    (VAlign::Center, t!(i18n, ids::PROPERTIES_JUSTIFY_MIDDLE)),
                    (VAlign::Bottom, t!(i18n, ids::PROPERTIES_JUSTIFY_BOTTOM)),
                ];
                c = c.push(segmented_row(
                    t!(i18n, ids::PROPERTIES_JUSTIFY_V),
                    down,
                    justify_v,
                    move |v| picked(NoteLayout::JustifyV(v)),
                    st,
                    read_only,
                ));
                if let Some(size_mm) = ctx.placed.note_size_mm {
                    let unit = ctx.unit.to_string();
                    let label = t!(i18n, ids::PROPERTIES_TEXT_SIZE_UNIT, "unit" => unit.as_str());
                    let shown = length_field_text(size_mm, ctx.unit);
                    c = c.push(edit_box(
                        label.into(),
                        item,
                        PlacedField::TextSize,
                        shown,
                        &ctx.placed,
                        look,
                    ));
                }
                c.push(font_family_row(
                    t!(i18n, ids::PROPERTIES_FONT),
                    note_font_family.as_deref(),
                    muted,
                    move |family| {
                        PanelMsg::EditTextFontFamily(
                            signex_engine::TextTarget::TextNote(id),
                            family,
                        )
                    },
                    i18n,
                ))
            },
        )
        .boxed(),
    )
}

/// The tallest the Text box gets: `NoteDraft::MAX_VISIBLE_LINES` lines and
/// its padding. Only a maximum: a minimum would leave the height unbounded
/// inside the panel's scrollable.
fn box_height_cap() -> f32 {
    let line = TS.body * signex_widgets::theme_ext::LINE_HEIGHT;
    line * crate::panels::NoteDraft::MAX_VISIBLE_LINES as f32 + 2.0 * SP.s
}

/// The Text box: grows with its wrapped lines up to
/// `NoteDraft::MAX_VISIBLE_LINES`, then scrolls. Ctrl+Enter, Escape or a
/// press outside it applies a typed draft.
fn text_box<'a>(
    i18n: &signex_i18n::I18n,
    draft: &'a crate::panels::NoteDraft,
    item: SelectedItem,
    muted: Color,
    read_only: bool,
) -> Element<'a, PanelMsg> {
    let mut editor = iced::widget::text_editor(&draft.content)
        .size(TS.body)
        .padding(SP.s)
        .height(Length::Fit.max(box_height_cap()))
        .key_binding(move |key| note_binding(key, item));
    if !read_only {
        editor = editor.on_action(move |action| {
            PanelMsg::PlacedEdit(PlacedEditMsg::NoteActed { item, action })
        });
    }
    // A press outside takes the editor's focus: the draft is applied then.
    let submitted = PanelMsg::PlacedEdit(PlacedEditMsg::NoteSubmitted { item });
    let boxed = signex_widgets::press_outside::PressOutside::new(
        editor.boxed(),
        draft.edited.then_some(submitted),
    );
    container(
        row![
            property_label(t!(i18n, ids::PROPERTIES_TEXT), muted),
            container(boxed).width(Length::FillPortion(PROPERTY_CONTROL_PORTION)),
        ]
        .spacing(SP.m)
        .align_y(iced::Alignment::Start),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

/// Ctrl+Enter applies the draft and Escape applies it as it unfocuses the
/// box; every other key keeps the editor's own binding (Enter breaks the
/// line).
fn note_binding(key: KeyPress, item: SelectedItem) -> Option<Binding<PanelMsg>> {
    use iced::keyboard::{Key, key::Named};
    let named = |name: Named| matches!(key.key.as_ref(), Key::Named(n) if n == name);
    let (enter, escape) = (named(Named::Enter), named(Named::Escape));
    let submitted = Binding::Custom(PanelMsg::PlacedEdit(PlacedEditMsg::NoteSubmitted { item }));
    if key.is_focused && enter && key.modifiers.command() {
        return Some(submitted);
    }
    if key.is_focused && escape {
        return Some(Binding::Sequence(vec![Binding::Unfocus, submitted]));
    }
    Binding::from_key_press(key)
}

/// A label and a segmented control over `options`, `current` on. Read-only
/// rows (a locked note) are veiled and take no input.
fn segmented_row<'a, V, const N: usize>(
    label: Localized,
    options: [(V, Localized); N],
    current: V,
    on_pick: impl Fn(V) -> PanelMsg + 'a,
    st: PanelStyle,
    read_only: bool,
) -> Element<'a, PanelMsg>
where
    V: Copy + PartialEq + 'a,
{
    let cells: Vec<Element<'a, PanelMsg>> = options
        .into_iter()
        .map(|(value, name)| {
            seg_btn(
                name,
                value == current,
                on_pick(value),
                st.palette.input_bg,
                st.primary,
                st.muted,
            )
        })
        .collect();
    let control = container(signex_widgets::FitRow::new(cells).fill(true))
        .padding(SP.xs)
        .style(crate::panels::form_look::segmented_frame)
        .width(Length::FillPortion(PROPERTY_CONTROL_PORTION))
        .boxed();
    let veil = signex_widgets::enable_section::dependents_veil(st.palette.panel_bg, false);
    let control = match veil {
        Some(veil) => signex_widgets::inert::unless_read_only_veiled(read_only, veil, control),
        None => control,
    };
    container(
        row![property_label(label, st.muted), control]
            .spacing(SP.m)
            .align_y(iced::Alignment::Center),
    )
    .padding([2, PROPERTY_ROW_PAD_X])
    .width(Length::Fill)
    .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(h: &str, v: &str) -> Vec<(String, String)> {
        vec![
            (signex_engine::info_keys::JUSTIFY_H.to_string(), h.into()),
            (signex_engine::info_keys::JUSTIFY_V.to_string(), v.into()),
        ]
    }

    /// #2024 - the segmented rows read the justification under the keys the
    /// engine writes, each row its own value.
    #[test]
    fn the_justify_rows_read_the_engines_keys() {
        assert_eq!(
            justification(&rows("Left", "Top")),
            (HAlign::Left, VAlign::Top)
        );
        assert_eq!(
            justification(&rows("Right", "Bottom")),
            (HAlign::Right, VAlign::Bottom)
        );
        assert_eq!(
            justification(&rows("Center", "Center")),
            (HAlign::Center, VAlign::Center)
        );
    }

    /// Mockup row 47: one long line wraps and the box grows to show it, up
    /// to its six-line cap.
    #[test]
    fn the_text_box_grows_with_wrapped_lines_up_to_its_cap() {
        let i18n = signex_i18n::I18n::en_us();
        let item = SelectedItem::new(uuid::Uuid::from_u128(1), SelectedKind::TextNote);
        let sheet = std::path::Path::new("a.snxsch");
        let height = |typed: &str| {
            let draft = crate::panels::NoteDraft::new(sheet, item, typed);
            let view = text_box(&i18n, &draft, item, Color::BLACK, false);
            crate::layout_probe::root_size(view, iced::Size::new(300.0, 2000.0)).height
        };
        let one_line = height("short");
        assert!(height(&"long words ".repeat(8)) > one_line);
        let cap = box_height_cap() + 4.0;
        assert!(height(&"long words ".repeat(200)) <= cap);
    }

    /// #2800, mockup row 47: the first click on a text note seeds its Text
    /// box, so Properties shows the multi-line box at once, not the one-line
    /// field kept for a box not yet seeded.
    #[test]
    fn the_first_click_on_a_note_shows_the_multi_line_box() {
        use super::super::child_sheet::tests::{app_showing, click};
        let stored = "Place C14 near U3\nKeep the SW node small";
        let note = signex_types::schematic::TextNote {
            uuid: uuid::Uuid::now_v7(),
            text: stored.into(),
            position: signex_types::schematic::Point::new(60.0, 60.0),
            rotation: 0.0,
            font_size: 2.54,
            font_family: None,
            justify_h: HAlign::Center,
            justify_v: VAlign::Center,
            locked: false,
        };
        let id = note.uuid;
        let sheet = signex_types::schematic::SchematicSheet {
            paper_size: "A4".into(),
            text_notes: vec![note],
            ..Default::default()
        };
        let mut app = app_showing(sheet);
        click(&mut app, (60.0, 60.0));
        let ctx = &app.document_state.panel_ctx;
        assert_eq!(ctx.selected_kind, Some(SelectedKind::TextNote));
        let draft = ctx.placed.note.as_ref().expect("the Text box is seeded");
        assert_eq!(draft.item, SelectedItem::new(id, SelectedKind::TextNote));
        assert_eq!(draft.content.text(), stored);
        assert_eq!(draft.visible_lines(), 2);
        let view = super::super::selected::view_selected_element_properties(
            ctx,
            Color::WHITE,
            Color::WHITE,
            Color::BLACK,
        );
        // The probe lists the multi-line box with the inputs (a text editor
        // reports itself as one); it is the Text box, not the one-line
        // field, when it stands taller than the one-line Text Size field.
        let inputs = crate::layout_probe::lay_out_inputs(view, iced::Size::new(320.0, 4000.0));
        let text = crate::layout_probe::find(&inputs, stored).bounds;
        let one_line = crate::layout_probe::find(&inputs, "2.540").bounds;
        assert!(
            text.height > one_line.height,
            "no one-line Text field: {inputs:?}"
        );
    }

    #[test]
    fn rotation_shows_the_nearest_quarter_turn() {
        assert_eq!(nearest_quarter_turn(0.0), 0);
        assert_eq!(nearest_quarter_turn(89.6), 90);
        assert_eq!(nearest_quarter_turn(-90.0), 270);
        assert_eq!(nearest_quarter_turn(359.0), 0);
    }
}
