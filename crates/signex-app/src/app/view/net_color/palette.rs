//! The F5 net colour palette body (#71, #2528): a filter, one row per net
//! named by a label or a power port (the swatch in a fixed column, the name
//! in mono, coloured nets first), a chooser floating beside the clicked row,
//! and the Clear all footer. The window's title bar carries the title and the
//! net count.

use iced::widget::{Space, Widget, column, container, row, scrollable, stack, svg};
use iced::{Alignment, Color, Element, Length, Theme};
use signex_i18n::{Arg, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::row_flyout::RowFlyout;
use super::{SWATCH, swatch, swatch_button};
use crate::app::handlers::editing_commands::net_color_palette::{CHOOSER_COLORS, PaletteRow};
use crate::app::state::{NetColorToast, NetColorsCleared};
use crate::app::{Message, NetColorMsg, Signex};
use crate::styles::{
    ChromeMetrics, DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split,
    dialog_icon_button, ti, tokens::resolve,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Any theme loads the monochrome check; its style tints it.
const GLYPH_THEME: signex_types::theme::ThemeId = signex_types::theme::ThemeId::GraphiteDark;

/// The chooser card: its colours on one line, inside its padding and border.
const CHOOSER_WIDTH: f32 = CHOOSER_COLORS.len() as f32 * SWATCH
    + (CHOOSER_COLORS.len() - 1) as f32 * SP.s
    + 2.0 * SP.m
    + 2.0;

fn net_message(msg: NetColorMsg) -> Message {
    Message::NetColor(msg)
}

fn to_iced(c: signex_types::theme::Color) -> Color {
    Color::from_rgb8(c.r, c.g, c.b)
}

/// What the Clear toast says was cleared.
fn toast_said(i18n: &signex_i18n::I18n, cleared: &NetColorsCleared) -> signex_i18n::Localized {
    match cleared {
        NetColorsCleared::Nets(count) => {
            t!(i18n, ids::DIALOG_NET_COLOR_TOAST_CLEARED, "count" => *count)
        }
        NetColorsCleared::Unlabelled => t!(i18n, ids::DIALOG_NET_COLOR_TOAST_CLEARED_UNLABELLED),
        NetColorsCleared::Net(net) => {
            t!(i18n, ids::DIALOG_NET_COLOR_TOAST_CLEARED_NET, "net" => net.as_str())
        }
    }
}

impl Signex {
    /// The palette body; see the module doc.
    pub(in crate::app::view) fn view_net_color_palette_body(&self) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let state = &self.ui_state.net_color_palette;

        // The shared dialog search field: the magnifier, then the input.
        let filter = container(crate::library::picker::search_field(
            t!(i18n, ids::DIALOG_NET_COLOR_FILTER),
            state.filter.as_str(),
            |s| net_message(NetColorMsg::FilterChanged(s)),
            None,
            tokens,
        ))
        .padding(iced::Padding {
            top: SP.m,
            right: SP.l,
            bottom: SP.s,
            left: SP.l,
        });

        let list: Element<'_, Message> = if self.palette_net_count() == 0 {
            container(crate::panels::empty_state_card(
                tokens,
                t!(i18n, ids::DIALOG_TITLE_NET_COLORS),
                t!(i18n, ids::DIALOG_NET_COLOR_NO_NETS),
                None,
            ))
            .padding([SP.s, SP.l])
            .boxed()
        } else {
            let mut rows = column![].width(Length::Fill);
            for net_row in self.palette_rows() {
                let open = state.chooser.as_deref() == Some(net_row.net.as_str());
                let row_element = palette_row(&net_row, open, tokens);
                rows = rows.push(if open {
                    RowFlyout::new(row_element, self.chooser(&net_row))
                        .on_outside_press(net_message(NetColorMsg::ChooserDismissed))
                        .boxed()
                } else {
                    row_element
                });
            }
            scrollable(container(rows).padding([SP.s, SP.m]))
                .direction(crate::styles::thin_vertical())
                .style(crate::styles::scrollbar_style(tokens))
                .height(Length::Fill)
                .boxed()
        };

        let has_colours = self
            .document_state
            .active_engine()
            .is_some_and(|engine| !engine.document().net_colors.is_empty());
        let footer = dialog_footer_split(
            vec![dialog_button(
                t!(i18n, ids::DIALOG_NET_COLOR_CLEAR_ALL),
                DialogButtonKind::Link,
                has_colours.then(|| net_message(NetColorMsg::ClearedAll)),
                tokens,
            )],
            vec![
                text(t!(i18n, ids::DIALOG_NET_COLOR_ROW_HINT))
                    .size(TS.caption)
                    .color(resolve(tokens).ink3)
                    .boxed(),
            ],
        );

        let card = container(column![filter, list, footer].height(Length::Fill))
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed();
        match state.toast.as_ref() {
            Some(toast) => self.with_clear_toast(card, toast),
            None => card,
        }
    }

    /// `card` with the Clear toast floating over the foot of its list, above
    /// the footer.
    fn with_clear_toast<'a>(
        &'a self,
        card: Element<'a, Message>,
        toast: &NetColorToast,
    ) -> Element<'a, Message> {
        let layer = container(self.clear_toast(toast))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                top: 0.0,
                right: SP.m,
                bottom: crate::styles::dense_footer_height() + SP.m,
                left: SP.m,
            })
            .align_y(iced::alignment::Vertical::Bottom)
            .boxed();
        stack(vec![card, layer]).boxed()
    }

    /// The toast after a Clear (mockup row 71): a check, what was cleared,
    /// and Undo with its shortcut while the clear is still the sheet's
    /// newest edit.
    fn clear_toast(&self, toast: &NetColorToast) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let check_c = ti(tokens.success);
        // A monochrome glyph: the style below tints it.
        let check = svg(crate::icons::icon_chrome_check(GLYPH_THEME))
            .width(TS.body)
            .height(TS.body)
            .style(move |_: &Theme, _| svg::Style {
                color: Some(check_c),
                ..Default::default()
            })
            .boxed();
        let undo = dialog_button(
            t!(
                i18n,
                ids::DIALOG_NET_COLOR_TOAST_UNDO,
                "undo" => crate::keymap::shown_in("Mod+Z", i18n)
            ),
            DialogButtonKind::Secondary,
            self.net_color_toast_can_undo()
                .then(|| net_message(NetColorMsg::ToastUndone)),
            tokens,
        );
        let line = row![
            check,
            text(toast_said(i18n, &toast.cleared))
                .size(TS.caption)
                .color(ti(tokens.text))
                .width(Length::Fill)
                .boxed(),
            undo,
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center);
        let surface = crate::styles::card(tokens);
        let shadow = crate::styles::floating_shadow(tokens);
        container(line)
            .width(Length::Fill)
            .padding(iced::Padding {
                top: SP.s,
                right: SP.s,
                bottom: SP.s,
                left: SP.m,
            })
            .style(move |theme: &Theme| container::Style {
                shadow,
                ..surface(theme)
            })
            .boxed()
    }

    /// The header subtitle: how many nets the active sheet labels.
    pub(crate) fn net_color_palette_subtitle(&self) -> String {
        self.ui_state.i18n.message_plain(
            ids::DIALOG_NET_COLOR_COUNT,
            &[("count", Arg::from(self.palette_net_count()))],
        )
    }

    /// The chooser card of an open row: the ten colours, Custom… and Clear.
    fn chooser(&self, net_row: &PaletteRow) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = &self.ui_state.i18n;
        let net = &net_row.net;
        let mut swatches = row![].spacing(SP.s).align_y(Alignment::Center);
        for (r, g, b) in CHOOSER_COLORS {
            let color = signex_types::theme::Color::new(r, g, b, 255);
            swatches = swatches.push(swatch_button(
                Color::from_rgb8(r, g, b),
                SWATCH,
                SWATCH,
                net_row.color == Some(color),
                net_message(NetColorMsg::Set {
                    net: net.clone(),
                    color: Some(color),
                }),
                tokens,
            ));
        }
        let actions = row![
            dialog_icon_button(
                crate::icons::icon_chrome_palette(GLYPH_THEME),
                t!(i18n, ids::DIALOG_NET_COLOR_CUSTOM),
                DialogButtonKind::Secondary,
                Some(net_message(NetColorMsg::CustomOpenedFor(net.clone()))),
                tokens,
            ),
            Space::new().width(Length::Fill),
            dialog_button(
                t!(i18n, ids::DIALOG_NET_COLOR_CLEAR),
                DialogButtonKind::Link,
                net_row.color.is_some().then(|| {
                    net_message(NetColorMsg::Set {
                        net: net.clone(),
                        color: None,
                    })
                }),
                tokens,
            ),
        ]
        .align_y(Alignment::Center);
        let surface = crate::styles::card(tokens);
        let shadow = crate::styles::floating_shadow(tokens);
        container(
            column![
                text(UserText::new(net.clone()))
                    .size(TS.caption)
                    .font(mono())
                    .color(resolve(tokens).ink3),
                swatches,
                actions,
            ]
            .spacing(SP.m),
        )
        .padding(SP.m)
        .width(Length::Fixed(CHOOSER_WIDTH))
        .style(move |theme: &Theme| container::Style {
            shadow,
            ..surface(theme)
        })
        .boxed()
    }
}

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// One net row: the swatch in a fixed column, then the name in mono. The
/// row whose chooser is open keeps the selection fill.
fn palette_row<'a>(net_row: &PaletteRow, open: bool, tokens: &ThemeTokens) -> Element<'a, Message> {
    let ghost = crate::styles::ghost_button(tokens);
    let open_fill = crate::panels::graphite::selection_fill(tokens);
    // The open row only ever closes: its press has already dismissed the
    // chooser, and a toggle in the same event batch would reopen it.
    let press = if open {
        NetColorMsg::ChooserDismissed
    } else {
        NetColorMsg::RowOpened(net_row.net.clone())
    };
    let face = row![
        swatch(net_row.color.map(to_iced), SWATCH, tokens),
        text(UserText::new(net_row.net.clone()))
            .size(TS.dense)
            .font(mono())
            .color(ti(tokens.text)),
    ]
    .spacing(SP.m)
    .align_y(Alignment::Center);
    button(face)
        .width(Length::Fill)
        .height(Length::Fixed(ChromeMetrics::DEFAULT.row))
        .padding([0.0, SP.m])
        .on_press(net_message(press))
        .style(move |theme: &Theme, status| {
            let mut style = ghost(theme, status);
            if open {
                style.background = Some(open_fill.into());
            }
            style
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::handlers::editing_commands::net_color_driver::add_sheet_with;
    use crate::layout_probe::lay_out_texts;
    use signex_types::schematic::{Label, LabelType, Point, SchematicSheet};

    fn y_of_net(app: &Signex, net: &str) -> f32 {
        lay_out_texts(
            app.view_net_color_palette_body(),
            iced::Size::new(360.0, 400.0),
        )
        .into_iter()
        .find(|laid| laid.text == net)
        .map(|laid| laid.bounds.y)
        .expect("the net has a row")
    }

    /// A sheet with one net label per name in `nets`, open in `app`.
    fn app_with_nets(nets: &[&str]) -> Signex {
        let (mut app, _task) = Signex::new();
        let mut sheet = SchematicSheet::default();
        for (i, net) in nets.iter().enumerate() {
            let at = Point::new(0.0, 10.0 * i as f64);
            sheet.wires.push(signex_types::schematic::Wire {
                uuid: uuid::Uuid::now_v7(),
                start: at,
                end: Point::new(10.0, at.y),
                ..Default::default()
            });
            sheet.labels.push(Label::new(*net, at, LabelType::Net));
        }
        add_sheet_with(&mut app, std::path::Path::new("look.snxsch"), sheet);
        app
    }

    /// The largest channel difference between the pixel column at `x` and
    /// the one at `x_ref`, over the rows `ys`: zero where no frame is drawn
    /// between them.
    fn column_contrast(
        pixels: &[u8],
        width: usize,
        x: f32,
        x_ref: f32,
        ys: std::ops::Range<usize>,
    ) -> u8 {
        let at = |x: usize, y: usize| &pixels[(y * width + x) * 4..(y * width + x) * 4 + 3];
        let (x, x_ref) = (x.floor() as usize, x_ref.floor() as usize);
        ys.flat_map(|y| {
            at(x, y)
                .iter()
                .zip(at(x_ref, y))
                .map(|(a, b)| a.abs_diff(*b))
                .collect::<Vec<_>>()
        })
        .max()
        .unwrap_or(0)
    }

    /// Mockup row 71: the filter is the shared search field, the magnifier
    /// in front of "Filter nets".
    #[test]
    fn the_filter_has_the_search_glyph_before_its_input() {
        let app = app_with_nets(&["A"]);
        let inputs = crate::layout_probe::lay_out_inputs(
            app.view_net_color_palette_body(),
            iced::Size::new(360.0, 400.0),
        );
        let filter = inputs
            .iter()
            .find(|laid| laid.text == "Filter nets")
            .expect("the filter input");
        assert!(
            filter.bounds.x >= SP.l + SP.m + TS.body,
            "the glyph sits before the input: {filter:?}"
        );
    }

    /// Mockup row 71: the window the app asks for is 360 wide and fits its
    /// rows, so the footer sits right under the list, not at the foot of a
    /// tall window.
    #[test]
    fn the_window_fits_its_rows_with_the_footer_under_the_list() {
        let app = app_with_nets(&["A", "B", "C"]);
        let size = app.modal_window_size(crate::app::state::ModalId::NetColorPalette);
        let scale = app.ui_state.ui_scale;
        assert_eq!(size.width, (360.0 * scale).ceil());
        let window = iced::Size::new(360.0, size.height / scale);
        let texts = lay_out_texts(app.view_net_color_palette_body(), window);
        let last = crate::layout_probe::find(&texts, "C").bounds;
        let footer = crate::layout_probe::find(&texts, "Clear all colors").bounds;
        let gap = footer.y - (last.y + last.height);
        assert!(gap > 0.0, "{last:?} {footer:?}");
        assert!(
            gap < ChromeMetrics::DEFAULT.row + crate::styles::dense_footer_height(),
            "the footer follows the list: gap {gap} in {window:?}"
        );
    }

    /// Mockup row 71: "Clear all colors" is a link, drawn with no frame even
    /// while there is nothing to clear (the old Ghost drew a half border).
    #[test]
    fn clear_all_is_a_link_with_no_frame() {
        let app = app_with_nets(&["A"]);
        let window = iced::Size::new(360.0, 400.0);
        let texts = lay_out_texts(app.view_net_color_palette_body(), window);
        let label = crate::layout_probe::find(&texts, "Clear all colors").bounds;
        let pixels = crate::layout_probe::draw_pixels(app.view_net_color_palette_body(), window);
        let ys = label.y as usize..(label.y + label.height) as usize;
        // The button's left edge is the footer's padding; compare it with
        // the footer fill just outside.
        let edge = SP.l;
        for x in [edge, edge + 1.0] {
            let contrast =
                column_contrast(&pixels, window.width as usize, x, edge - 3.0, ys.clone());
            assert!(contrast <= 1, "a frame at x {x}: contrast {contrast}");
        }
    }

    /// Mockup row 71: the chooser's Custom… carries the palette glyph in
    /// front of its label, and its Clear is a frameless link.
    #[test]
    fn the_chooser_has_a_palette_glyph_on_custom_and_a_link_clear() {
        let mut app = app_with_nets(&["A"]);
        let _ = app.update(net_message(NetColorMsg::Set {
            net: "A".into(),
            color: Some(signex_types::theme::Color::new(0xE0, 0x54, 0x54, 255)),
        }));
        let net_row = app.palette_rows().remove(0);
        let window = iced::Size::new(CHOOSER_WIDTH, 200.0);
        let texts = lay_out_texts(app.chooser(&net_row), window);
        assert!(net_row.color.is_some(), "Clear has an action");
        // A plain dialog button's label fills the button from the card's
        // padding; the glyph pushes it right by at least its own side.
        let custom = crate::layout_probe::find(&texts, "Custom…").bounds;
        assert!(
            custom.x - SP.m >= TS.body,
            "the glyph sits before the label: {custom:?}"
        );

        let clear = crate::layout_probe::find(&texts, "Clear").bounds;
        let pixels = crate::layout_probe::draw_pixels(app.chooser(&net_row), window);
        let ys = clear.y as usize..(clear.y + clear.height) as usize;
        // Clear sits at the right end of the card's padded content.
        let edge = CHOOSER_WIDTH - SP.m;
        for x in [edge - 1.0, edge - 2.0] {
            let contrast =
                column_contrast(&pixels, window.width as usize, x, edge + 2.0, ys.clone());
            assert!(contrast <= 1, "a frame at x {x}: contrast {contrast}");
        }
    }

    /// Mockup row 71: the chooser floats, so the list stays still.
    #[test]
    fn opening_a_chooser_leaves_the_rows_below_where_they_were() {
        let (mut app, _task) = Signex::new();
        let mut sheet = SchematicSheet::default();
        for (i, net) in ["A", "B", "C"].into_iter().enumerate() {
            let at = Point::new(0.0, 10.0 * i as f64);
            sheet.labels.push(Label::new(net, at, LabelType::Net));
        }
        add_sheet_with(&mut app, std::path::Path::new("palette.snxsch"), sheet);
        let closed = y_of_net(&app, "C");
        let _ = app.update(net_message(NetColorMsg::RowOpened("A".into())));
        assert_eq!(app.ui_state.net_color_palette.chooser.as_deref(), Some("A"));
        assert_eq!(y_of_net(&app, "C"), closed);
    }

    /// Mockup row 71: after Clear all colors the palette shows "Cleared 1
    /// net color" (en-US spelling, decision of 2026-11-24) with an Undo
    /// naming its shortcut.
    #[test]
    fn clear_all_shows_the_toast_with_undo_and_its_shortcut() {
        let (mut app, _task) = Signex::new();
        let mut sheet = SchematicSheet::default();
        let wire = signex_types::schematic::Wire {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            ..Default::default()
        };
        let wire_id = wire.uuid;
        sheet.wires.push(wire);
        sheet
            .labels
            .push(Label::new("A", Point::new(0.0, 0.0), LabelType::Net));
        add_sheet_with(&mut app, std::path::Path::new("toast.snxsch"), sheet);
        let red = signex_types::theme::Color::new(0xE0, 0x54, 0x54, 255);
        let _ = app.commit_net_colors(&[wire_id], red);
        let _ = app.update(net_message(NetColorMsg::ClearedAll));

        let undo = format!(
            "Undo {}",
            crate::keymap::shown_in("Mod+Z", &app.ui_state.i18n)
        );
        let texts: Vec<String> = lay_out_texts(
            app.view_net_color_palette_body(),
            iced::Size::new(360.0, 400.0),
        )
        .into_iter()
        .map(|laid| laid.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect();
        assert!(
            texts.iter().any(|t| t == "Cleared 1 net color"),
            "{texts:?}"
        );
        assert!(texts.contains(&undo), "{texts:?}");
    }

    /// Mockup row 71, decisions of 2026-11-24: a sheet whose only net is
    /// named by a power port lists it (no empty card), and the footer reads
    /// "Clear all colors".
    #[test]
    fn a_power_port_net_has_a_row_and_the_footer_spells_color() {
        let (mut app, _task) = Signex::new();
        let mut sheet = SchematicSheet::default();
        sheet.wires.push(signex_types::schematic::Wire {
            uuid: uuid::Uuid::now_v7(),
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
            ..Default::default()
        });
        sheet.symbols.push(signex_types::schematic::Symbol {
            uuid: uuid::Uuid::now_v7(),
            lib_id: "power:+5V".into(),
            value: "+5V".into(),
            is_power: true,
            ..Default::default()
        });
        add_sheet_with(&mut app, std::path::Path::new("power.snxsch"), sheet);
        assert_eq!(app.palette_net_count(), 1);

        let texts: Vec<String> = lay_out_texts(
            app.view_net_color_palette_body(),
            iced::Size::new(360.0, 400.0),
        )
        .into_iter()
        .map(|laid| laid.text)
        .collect();
        assert!(texts.iter().any(|t| t == "+5V"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Clear all colors"), "{texts:?}");
    }
}
