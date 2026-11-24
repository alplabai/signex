//! The hex field of a colour row (mockup row 46). What is typed stays a
//! draft until Enter or until the field is left; then it becomes the pick
//! the row's popover sends, so one typed colour is one undo step. Text that
//! names no colour is refused: the field keeps it, outlined, and nothing
//! changes. Esc drops the draft and the stored colour shows again.
//!
//! A colour is six hex digits, any case, `#` optional, as the net colour
//! field reads it. The short `#RGB` form and `#RRGGBBAA` are refused: the
//! field shows no alpha, so it takes none, and a typed colour keeps the
//! alpha the object already has.

use super::PanelMsg;

/// Which colour row a draft belongs to, with the object it colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorRowKey {
    /// The Color row of the wire tool's Tab form.
    PrePlacementWire,
    /// The Color row of a selected wire or bus.
    Run(uuid::Uuid),
    /// The Border row of a selected sheet symbol.
    ChildSheetBorder(uuid::Uuid),
    /// The Fill row of a selected sheet symbol.
    ChildSheetFill(uuid::Uuid),
    /// The Color row of a selected drawing.
    DrawingStroke(uuid::Uuid),
}

impl ColorRowKey {
    /// Whether the row is still shown while `selected` is the single
    /// selected object and the Tab form is open or not.
    pub fn is_shown_for(self, selected: Option<uuid::Uuid>, has_tab_form: bool) -> bool {
        match self {
            Self::PrePlacementWire => has_tab_form,
            Self::Run(id)
            | Self::ChildSheetBorder(id)
            | Self::ChildSheetFill(id)
            | Self::DrawingStroke(id) => selected == Some(id),
        }
    }
}

/// The text typed in one colour row's hex field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorHexDraft {
    pub key: ColorRowKey,
    pub text: String,
    /// Enter or leaving the field found no colour in `text`.
    pub refused: bool,
}

/// What a colour row's hex field sends.
#[derive(Debug, Clone)]
pub enum ColorHexMsg {
    /// A keystroke: kept as the row's draft.
    Typed(ColorRowKey, String),
    /// Enter in the field, or the field lost focus. `text` is the draft the
    /// view built `apply` from; `apply` is the row's pick for that colour,
    /// `None` when the text names no colour or the colour the row has.
    Left {
        key: ColorRowKey,
        text: String,
        apply: Option<Box<PanelMsg>>,
    },
    /// Esc in the field: the draft is dropped.
    Restored(ColorRowKey),
}

/// The colour `text` names as `[r, g, b]`, or `None` (see the module docs).
pub fn parse_typed_hex(text: &str) -> Option<[u8; 3]> {
    let trimmed = text.trim();
    let digits = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if digits.len() != 6 || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(digits, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// `#RRGGBB`, upper case: how a colour row shows a colour. A hex code is
/// design data, not language.
pub fn hex_code(rgba: [u8; 4]) -> String {
    format!("#{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2])
}

/// What one colour row's hex field needs: its key and, while typed in, its
/// draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexEntry {
    pub key: ColorRowKey,
    pub draft: Option<ColorHexDraft>,
}

impl HexEntry {
    /// The field of row `key`, showing `draft` only when it is this row's.
    pub fn of(key: ColorRowKey, draft: Option<&ColorHexDraft>) -> Self {
        Self {
            key,
            draft: draft.filter(|d| d.key == key).cloned(),
        }
    }

    /// The text the field shows: the draft, else the code of `current`, else
    /// nothing, so the default name shows as the placeholder.
    pub fn text(&self, current: Option<[u8; 4]>) -> String {
        match (&self.draft, current) {
            (Some(draft), _) => draft.text.clone(),
            (None, Some(rgba)) => hex_code(rgba),
            (None, None) => String::new(),
        }
    }

    /// Whether the field holds text it refused.
    pub fn is_refused(&self) -> bool {
        self.draft.as_ref().is_some_and(|d| d.refused)
    }

    /// The message of a keystroke.
    pub fn typed(&self) -> impl Fn(String) -> PanelMsg + 'static {
        let key = self.key;
        move |text| PanelMsg::ColorHex(ColorHexMsg::Typed(key, text))
    }

    /// The message of Esc.
    pub fn restored(&self) -> PanelMsg {
        PanelMsg::ColorHex(ColorHexMsg::Restored(self.key))
    }

    /// The message of Enter or of leaving the field. `on_pick` is the row's
    /// popover pick; the typed colour keeps the alpha of `current`.
    pub fn left(
        &self,
        current: Option<[u8; 4]>,
        on_pick: &dyn Fn([u8; 4]) -> PanelMsg,
    ) -> PanelMsg {
        let text = self
            .draft
            .as_ref()
            .map(|d| d.text.clone())
            .unwrap_or_default();
        let alpha = current.map_or(u8::MAX, |c| c[3]);
        let apply = self
            .draft
            .as_ref()
            .and_then(|d| parse_typed_hex(&d.text))
            .map(|[r, g, b]| [r, g, b, alpha])
            .filter(|rgba| Some(*rgba) != current)
            .map(|rgba| Box::new(on_pick(rgba)));
        PanelMsg::ColorHex(ColorHexMsg::Left {
            key: self.key,
            text,
            apply,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: ColorRowKey = ColorRowKey::PrePlacementWire;

    fn typed(text: &str) -> HexEntry {
        HexEntry {
            key: KEY,
            draft: Some(ColorHexDraft {
                key: KEY,
                text: text.into(),
                refused: false,
            }),
        }
    }

    fn picked(rgba: [u8; 4]) -> PanelMsg {
        PanelMsg::SetPrePlacementWireColor(Some(signex_types::theme::Color::new(
            rgba[0], rgba[1], rgba[2], rgba[3],
        )))
    }

    fn applied(msg: PanelMsg) -> Option<PanelMsg> {
        match msg {
            PanelMsg::ColorHex(ColorHexMsg::Left { apply, .. }) => apply.map(|m| *m),
            other => panic!("not a Left: {other:?}"),
        }
    }

    #[test]
    fn six_hex_digits_in_any_case_are_a_colour() {
        assert_eq!(parse_typed_hex("#FFF7E0"), Some([0xFF, 0xF7, 0xE0]));
        assert_eq!(parse_typed_hex("#fff7e0"), Some([0xFF, 0xF7, 0xE0]));
        assert_eq!(parse_typed_hex("c0392B"), Some([0xC0, 0x39, 0x2B]));
        assert_eq!(parse_typed_hex("  #000000 "), Some([0, 0, 0]));
    }

    #[test]
    fn anything_else_is_refused() {
        for text in [
            "",
            "#",
            "#FFF",
            "FFF",
            "#FFF7E",
            "#FFF7E0FF",
            "#GGGGGG",
            "red",
            "#FF F7E0",
        ] {
            assert_eq!(parse_typed_hex(text), None, "{text:?}");
        }
    }

    #[test]
    fn the_field_shows_the_draft_else_the_code_else_nothing() {
        let rgba = Some([0xC0, 0x39, 0x2B, 255]);
        assert_eq!(typed("#12").text(rgba), "#12");
        assert_eq!(HexEntry::of(KEY, None).text(rgba), "#C0392B");
        assert_eq!(HexEntry::of(KEY, None).text(None), "");
    }

    #[test]
    fn another_rows_draft_is_not_shown() {
        let id = uuid::Uuid::from_u128(7);
        let draft = typed("#123456").draft;
        let other = HexEntry::of(ColorRowKey::Run(id), draft.as_ref());
        assert_eq!(other.draft, None);
    }

    #[test]
    fn leaving_a_typed_colour_sends_the_rows_pick_with_its_alpha() {
        let left = typed("#fff7e0").left(Some([0, 0, 0, 128]), &picked);
        assert!(
            matches!(
                applied(left),
                Some(PanelMsg::SetPrePlacementWireColor(Some(c)))
                    if c == signex_types::theme::Color::new(0xFF, 0xF7, 0xE0, 128)
            ),
            "the typed colour, keeping alpha 128"
        );
        let from_default = typed("#FFF7E0").left(None, &picked);
        assert!(matches!(
            applied(from_default),
            Some(PanelMsg::SetPrePlacementWireColor(Some(c))) if c.a == 255
        ));
    }

    #[test]
    fn refused_text_or_the_same_colour_sends_no_pick() {
        assert!(applied(typed("#FFF").left(None, &picked)).is_none());
        let same = Some([0xFF, 0xF7, 0xE0, 255]);
        assert!(applied(typed("#fff7e0").left(same, &picked)).is_none());
        assert!(applied(HexEntry::of(KEY, None).left(same, &picked)).is_none());
    }

    #[test]
    fn a_row_shows_only_for_its_own_object() {
        let id = uuid::Uuid::from_u128(3);
        assert!(ColorRowKey::Run(id).is_shown_for(Some(id), false));
        assert!(!ColorRowKey::ChildSheetFill(id).is_shown_for(None, false));
        assert!(!ColorRowKey::PrePlacementWire.is_shown_for(Some(id), false));
        assert!(ColorRowKey::PrePlacementWire.is_shown_for(None, true));
    }
}
