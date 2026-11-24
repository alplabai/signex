//! The F5 net colour palette and the custom net colour picker (#2528): the
//! message handler, the rows the palette lists and the picker's Hex field.
//!
//! A palette colour is an edit of the active sheet like a pen click: the
//! wires and junctions of the net under each of its labels take the colour
//! through [`Signex::commit_net_colors`], one undo step per pick.

use super::super::super::*;
use crate::app::handlers::canvas::net_color_click::{net_elements_in, wire_at};
use crate::app::mutation_gateway::MutationOutcome;
use signex_i18n::{ids, t};
use signex_types::schematic::{Label, LabelType, SchematicSheet};
use signex_types::theme::Color;

/// The colours of a palette row's chooser.
pub(crate) const CHOOSER_COLORS: [(u8, u8, u8); 10] = [
    (0xE0, 0x54, 0x54),
    (0xE0, 0xB0, 0x4A),
    (0x78, 0xC2, 0x6A),
    (0x42, 0xB8, 0xE0),
    (0x6F, 0x77, 0xE0),
    (0xB0, 0x6F, 0xE0),
    (0xE0, 0x6F, 0xB0),
    (0xC2, 0xA0, 0x78),
    (0x78, 0xC2, 0xA0),
    (0xA0, 0xA0, 0xA0),
];

/// Rows the palette window is sized for before its list scrolls.
const VISIBLE_ROWS: usize = 12;

/// One palette row: a net label text and the colour its net shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaletteRow {
    pub net: String,
    pub color: Option<Color>,
}

fn is_net_label(label: &Label) -> bool {
    matches!(
        label.label_type,
        LabelType::Net | LabelType::Global | LabelType::Hierarchical
    )
}

/// The net label texts of `sheet` (net, global and hierarchical), each once,
/// by name.
pub(crate) fn net_label_names(sheet: &SchematicSheet) -> Vec<String> {
    sheet
        .labels
        .iter()
        .filter(|l| is_net_label(l))
        .map(|l| l.text.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// The colour `net` shows on `sheet`: the colour of a wire under one of its
/// labels. A colour covers the whole net, so one wire tells.
pub(crate) fn net_color_on(sheet: &SchematicSheet, net: &str) -> Option<Color> {
    sheet
        .labels
        .iter()
        .filter(|l| is_net_label(l) && l.text == net)
        .filter_map(|l| wire_at(sheet, l.position.x, l.position.y))
        .find_map(|wire| sheet.net_colors.get(&wire).copied())
}

/// The wires and junctions of every net a label named `net` sits on.
pub(crate) fn net_label_elements(sheet: &SchematicSheet, net: &str) -> Vec<uuid::Uuid> {
    let mut ids: Vec<uuid::Uuid> = Vec::new();
    for label in sheet
        .labels
        .iter()
        .filter(|l| is_net_label(l) && l.text == net)
    {
        for id in net_elements_in(sheet, label.position.x, label.position.y) {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids
}

/// The rows the palette lists for `sheet`: names containing `filter` (any
/// case), coloured nets first, then by name.
pub(crate) fn palette_rows(sheet: &SchematicSheet, filter: &str) -> Vec<PaletteRow> {
    let needle = filter.trim().to_lowercase();
    let mut rows: Vec<PaletteRow> = net_label_names(sheet)
        .into_iter()
        .filter(|net| needle.is_empty() || net.to_lowercase().contains(&needle))
        .map(|net| PaletteRow {
            color: net_color_on(sheet, &net),
            net,
        })
        .collect();
    rows.sort_by(|a, b| {
        b.color
            .is_some()
            .cmp(&a.color.is_some())
            .then_with(|| a.net.cmp(&b.net))
    });
    rows
}

/// Height of the palette window's content for `rows` nets: the filter, the
/// rows up to [`VISIBLE_ROWS`] (or the empty card) and the footer.
pub(crate) fn palette_window_height(rows: usize) -> f32 {
    use crate::styles::{ChromeMetrics, Spacing, TypeScale};
    let (m, sp, ts) = (ChromeMetrics::DEFAULT, Spacing::DEFAULT, TypeScale::DEFAULT);
    let filter = m.field + 2.0 * sp.m;
    let list = if rows == 0 {
        // The empty-state card: title, body and its padding.
        (ts.label + ts.dense) * crate::styles::LINE_HEIGHT + sp.s + 2.0 * sp.l + sp.m
    } else {
        rows.min(VISIBLE_ROWS) as f32 * m.row + 2.0 * sp.s
    };
    filter + list + crate::styles::dense_footer_height()
}

/// Whether `event` is a press of any mouse button or a finger.
pub(crate) fn is_pointer_press(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(_))
            | iced::Event::Touch(iced::touch::Event::FingerPressed { .. })
    )
}

/// `c` as a `#RRGGBB` hex code.
pub(crate) fn hex_of(c: iced::Color) -> String {
    let t = theme_color_of(c);
    format!("#{:02X}{:02X}{:02X}", t.r, t.g, t.b)
}

/// The colour a typed hex code names: six hex digits, `#` optional. `None`
/// for anything else.
pub(crate) fn parse_hex(text: &str) -> Option<iced::Color> {
    let [r, g, b] = crate::panels::parse_typed_hex(text)?;
    Some(iced::Color::from_rgb8(r, g, b))
}

/// The opaque document colour of an iced colour.
pub(crate) fn theme_color_of(c: iced::Color) -> Color {
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::new(channel(c.r), channel(c.g), channel(c.b), 255)
}

impl Signex {
    /// The colour `net` shows on the active sheet.
    pub(crate) fn palette_net_color(&self, net: &str) -> Option<Color> {
        self.document_state
            .active_engine()
            .and_then(|engine| net_color_on(engine.document(), net))
    }

    /// The palette rows of the active sheet, filtered by the palette state.
    pub(crate) fn palette_rows(&self) -> Vec<PaletteRow> {
        self.document_state
            .active_engine()
            .map(|engine| palette_rows(engine.document(), &self.ui_state.net_color_palette.filter))
            .unwrap_or_default()
    }

    /// How many nets the active sheet labels, filter or not.
    pub(crate) fn palette_net_count(&self) -> usize {
        self.document_state
            .active_engine()
            .map_or(0, |engine| net_label_names(engine.document()).len())
    }

    /// Per-net colour override + F5-palette handler (namespaced family,
    /// ADR-0001 D3).
    pub(crate) fn dispatch_net_color_message(&mut self, msg: NetColorMsg) -> Task<Message> {
        match msg {
            NetColorMsg::Open => {
                self.ui_state.net_color_palette_open = true;
                // Each open starts unfiltered with no chooser, also after the
                // window was closed by its title bar.
                *self.ui_state.net_color_palette = Default::default();
                self.handle_detach_modal(crate::app::state::ModalId::NetColorPalette)
            }
            NetColorMsg::Close => {
                self.ui_state.net_color_palette_open = false;
                *self.ui_state.net_color_palette = Default::default();
                self.close_detached_modal(crate::app::state::ModalId::NetColorPalette)
            }
            NetColorMsg::Set { net, color } => {
                self.set_palette_net_color(&net, color);
                Task::none()
            }
            NetColorMsg::CustomShow(show) => {
                let custom = &mut self.ui_state.net_color_custom;
                if show && !custom.show {
                    custom.draft_at_open = custom.draft;
                }
                // Cancel: the pick is not offered again on the next open.
                if !show && custom.show {
                    custom.draft = custom.draft_at_open;
                }
                custom.show = show;
                if !show {
                    custom.net = None;
                    custom.sheet = None;
                    custom.hex = None;
                }
                Task::none()
            }
            NetColorMsg::CustomDraft(c) => {
                self.ui_state.net_color_custom.draft = c;
                self.ui_state.net_color_custom.hex = None;
                Task::none()
            }
            NetColorMsg::CustomSubmit(c) => {
                self.submit_custom_net_color(c);
                Task::none()
            }
            NetColorMsg::CustomChannel(chan, s) => {
                self.set_custom_channel(chan, &s);
                Task::none()
            }
            NetColorMsg::FilterChanged(text) => {
                self.ui_state.net_color_palette.filter = text;
                Task::none()
            }
            NetColorMsg::RowOpened(net) => {
                let chooser = &mut self.ui_state.net_color_palette.chooser;
                *chooser = if chooser.as_deref() == Some(net.as_str()) {
                    None
                } else {
                    Some(net)
                };
                Task::none()
            }
            NetColorMsg::ChooserDismissed => {
                self.ui_state.net_color_palette.chooser = None;
                Task::none()
            }
            NetColorMsg::PressedInWindow(window) => {
                let palette = self.detached_window_of(crate::app::state::ModalId::NetColorPalette);
                if palette != Some(window) {
                    self.ui_state.net_color_palette.chooser = None;
                }
                Task::none()
            }
            NetColorMsg::CustomOpenedFor(net) => {
                self.ui_state.net_color_palette.chooser = None;
                let seed = self
                    .palette_net_color(&net)
                    .map(|c| iced::Color::from_rgb8(c.r, c.g, c.b));
                let sheet = self.document_state.active_path.clone();
                let custom = &mut self.ui_state.net_color_custom;
                if !custom.show {
                    custom.draft_at_open = custom.draft;
                }
                if let Some(seed) = seed {
                    custom.draft = seed;
                }
                custom.net = Some(net);
                custom.sheet = sheet;
                custom.hex = None;
                custom.show = true;
                Task::none()
            }
            NetColorMsg::CustomHexChanged(text) => {
                if let Some(c) = parse_hex(&text) {
                    self.ui_state.net_color_custom.draft = c;
                }
                self.ui_state.net_color_custom.hex = Some(text);
                Task::none()
            }
            NetColorMsg::ClearedAll => {
                self.ui_state.net_color_palette.chooser = None;
                if let Some(path) = self.document_state.active_path.clone() {
                    self.run_clear_all_net_colors(Some(&path));
                }
                Task::none()
            }
        }
    }

    /// Colour `net` on the active sheet (`None` clears it) in one undo step,
    /// and close the row's chooser.
    fn set_palette_net_color(&mut self, net: &str, color: Option<Color>) {
        self.ui_state.net_color_palette.chooser = None;
        let elements = self
            .document_state
            .active_engine()
            .map(|engine| net_label_elements(engine.document(), net))
            .unwrap_or_default();
        // An alpha of 0 clears. A refusal (a read-only sheet) is already
        // said in Messages by the commit.
        let applied = !elements.is_empty() && {
            let paint = color.unwrap_or(Color::new(0, 0, 0, 0));
            self.commit_net_colors(&elements, paint).0 != MutationOutcome::Failed
        };
        // The per-name record follows the sheet: an empty or refused commit
        // leaves it.
        match color {
            Some(c) if applied => {
                self.ui_state.net_colors.insert(net.to_owned(), c);
            }
            None if applied => {
                self.ui_state.net_colors.remove(net);
            }
            _ => {}
        }
        // Every window showing a sheet repaints, the undocked ones too (#1350).
        for canvas in self.interaction_state.all_schematic_canvases_mut() {
            canvas.clear_content_cache();
        }
    }

    /// The custom picker's Use Color: colour the picker's net when it came
    /// from a palette row, else arm the net-colour pen with the colour. A
    /// row's net is coloured only on the sheet it was opened on.
    fn submit_custom_net_color(&mut self, c: iced::Color) {
        let custom = &mut self.ui_state.net_color_custom;
        custom.show = false;
        custom.draft = c;
        custom.hex = None;
        let color = theme_color_of(c);
        let sheet = custom.sheet.take();
        if let Some(net) = custom.net.take() {
            if sheet != self.document_state.active_path {
                let i18n = &self.ui_state.i18n;
                crate::diagnostics::log_warning(
                    t!(i18n, ids::DIALOG_NET_COLOR_SHEET_CHANGED, "net" => net.as_str())
                        .into_string(),
                );
                return;
            }
            self.set_palette_net_color(&net, Some(color));
            return;
        }
        self.cancel_placement_session();
        self.ui_state.pending_net_color = Some(color);
        self.interaction_state.active_canvas_mut().pending_net_color = Some(color);
    }

    /// One R/G/B field of the custom picker. Parsed as u8: values above 255
    /// (however large) clamp to 255. Input that does not parse (the empty
    /// string while the user types) keeps the channel's previous value, so
    /// a half-typed edit never zeroes the channel.
    fn set_custom_channel(&mut self, chan: crate::app::contracts::Channel, text: &str) {
        let Ok(parsed) = text.trim().parse::<u64>() else {
            return;
        };
        let v = parsed.min(255) as f32 / 255.0;
        let custom = &mut self.ui_state.net_color_custom;
        custom.hex = None;
        match chan {
            crate::app::contracts::Channel::R => custom.draft.r = v,
            crate::app::contracts::Channel::G => custom.draft.g = v,
            crate::app::contracts::Channel::B => custom.draft.b = v,
        }
    }
}
