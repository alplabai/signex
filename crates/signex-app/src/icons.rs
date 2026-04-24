//! Central icon registry with runtime theme-aware tinting.
//!
//! Design
//! ------
//! There is **one canonical SVG tree** at `assets/icons/…`. Every accent
//! path in those SVGs uses the Signex brand amber `#f59e0b` as a
//! sentinel colour. At fetch time the sentinel is string-replaced with
//! the current theme's accent hex and the resulting bytes are handed to
//! `iced::widget::svg::Handle::from_memory`, which de-duplicates by
//! content hash — so repeat renders of the same (icon, theme) pair reuse
//! the GPU texture cache and the replace cost is paid once.
//!
//! For themes that only change the **accent colour**, no icon copy is
//! needed: the `canonical_icon!` macro expands to a tint-only lookup.
//!
//! For a theme that needs a genuinely **different shape** for a specific
//! icon (logo-style, brand-dependent glyph, …), hand-write the function
//! with an explicit match arm that pulls from a per-theme override
//! directory. Example:
//!
//! ```ignore
//! pub fn icon_logo(theme: ThemeId) -> svg::Handle {
//!     match theme {
//!         ThemeId::Alplab => {
//!             // Shape override — cyan-baked SVG with different strokes.
//!             svg::Handle::from_memory(
//!                 include_bytes!("../assets/icons/alplab/logo.svg").as_slice(),
//!             )
//!         }
//!         _ => canonical("logo.svg", theme),
//!     }
//! }
//! ```
//!
//! The reserved `assets/icons/alplab/` tree exists as a pre-tinted
//! starting point for authoring such overrides.

use iced::widget::svg;
use signex_types::theme::{ThemeId, theme_tokens};

/// The Signex brand amber. Any accent path in a canonical SVG that is
/// coloured with this literal hex gets replaced at load time by the
/// active theme's accent. Keeping the sentinel equal to the default
/// theme's accent means the canonical tree also reads correctly in
/// design tools like Figma with no runtime processing.
const ACCENT_SENTINEL: &str = "#f59e0b";

/// Build the theme accent as a lowercase `#rrggbb` literal so the
/// bytewise string replace lines up with the sentinel.
fn accent_hex(theme: ThemeId) -> String {
    let c = theme_tokens(theme).accent;
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Swap the sentinel hex for the theme accent and hand the bytes to
/// iced. When the theme accent already equals the sentinel (Signex
/// default) the canonical bytes go straight through — no allocation.
fn tinted_handle(canonical: &'static [u8], theme: ThemeId) -> svg::Handle {
    let accent = accent_hex(theme);
    if accent == ACCENT_SENTINEL {
        return svg::Handle::from_memory(canonical);
    }
    // SVGs are tiny (100–400 bytes each) and always valid UTF-8, so
    // the `from_utf8` path is a few hundred cycles per miss. iced
    // hashes the resulting bytes and reuses its texture, so this only
    // actually runs on the first render after a theme switch.
    let tinted = std::str::from_utf8(canonical)
        .unwrap_or("")
        .replace(ACCENT_SENTINEL, &accent);
    svg::Handle::from_memory(tinted.into_bytes())
}

/// Emit a `pub fn <name>(ThemeId) -> svg::Handle` that resolves to the
/// canonical SVG at the given asset-relative path, tinted to the
/// theme's accent.
macro_rules! canonical_icon {
    ($name:ident, $path:literal) => {
        pub fn $name(theme: ThemeId) -> svg::Handle {
            const BYTES: &[u8] = include_bytes!(concat!("../assets/icons/", $path));
            tinted_handle(BYTES, theme)
        }
    };
}

// ─── Flat top-level (active-bar, dock, and shape_* shapes) ─────────

canonical_icon!(icon_addpart, "addpart.svg");
canonical_icon!(icon_align, "align.svg");
canonical_icon!(icon_chevron_45, "chevron_45.svg");
canonical_icon!(icon_close, "close.svg");
canonical_icon!(icon_collapse_down, "collapse_down.svg");
canonical_icon!(icon_collapse_left, "collapse_left.svg");
canonical_icon!(icon_collapse_right, "collapse_right.svg");
canonical_icon!(icon_collapse_up, "collapse_up.svg");
canonical_icon!(icon_component, "component.svg");
canonical_icon!(icon_directives, "directives.svg");
canonical_icon!(icon_expand_left, "expand_left.svg");
canonical_icon!(icon_expand_right, "expand_right.svg");
canonical_icon!(icon_filter, "filter.svg");
canonical_icon!(icon_harness, "harness.svg");
canonical_icon!(icon_move, "move.svg");
canonical_icon!(icon_netcolor, "netcolor.svg");
canonical_icon!(icon_noconnect, "noconnect.svg");
canonical_icon!(icon_port, "port.svg");
canonical_icon!(icon_power, "power.svg");
canonical_icon!(icon_select, "select.svg");
canonical_icon!(icon_shape_arc, "shape_arc.svg");
canonical_icon!(icon_shape_circle, "shape_circle.svg");
canonical_icon!(icon_shape_elliptical_arc, "shape_elliptical_arc.svg");
canonical_icon!(icon_shape_line, "shape_line.svg");
canonical_icon!(icon_shape_polygon, "shape_polygon.svg");
canonical_icon!(icon_shape_rect, "shape_rect.svg");
canonical_icon!(icon_shapes, "shapes.svg");
canonical_icon!(icon_sheetsym, "sheetsym.svg");
canonical_icon!(icon_sheetsymbol, "sheetsymbol.svg");
canonical_icon!(icon_text, "text.svg");
canonical_icon!(icon_undock, "undock.svg");
canonical_icon!(icon_wire, "wire.svg");

// ─── Chrome (window controls, search) ──────────────────────────────

canonical_icon!(icon_chrome_search, "chrome/search.svg");
canonical_icon!(icon_chrome_window_close, "chrome/window_close.svg");
canonical_icon!(icon_chrome_window_max, "chrome/window_max.svg");
canonical_icon!(icon_chrome_window_min, "chrome/window_min.svg");
canonical_icon!(icon_chrome_window_restore, "chrome/window_restore.svg");

// ─── Dropdown (active-bar group menus) ─────────────────────────────

canonical_icon!(icon_dd_align_bottom, "dropdown/align_bottom.svg");
canonical_icon!(icon_dd_align_grid, "dropdown/align_grid.svg");
canonical_icon!(icon_dd_align_hcenter, "dropdown/align_hcenter.svg");
canonical_icon!(icon_dd_align_left, "dropdown/align_left.svg");
canonical_icon!(icon_dd_align_right, "dropdown/align_right.svg");
canonical_icon!(icon_dd_align_top, "dropdown/align_top.svg");
canonical_icon!(icon_dd_align_menu, "dropdown/align_menu.svg");
canonical_icon!(icon_dd_align_vcenter, "dropdown/align_vcenter.svg");
canonical_icon!(icon_dd_arc, "dropdown/arc.svg");
canonical_icon!(icon_dd_bezier, "dropdown/bezier.svg");
canonical_icon!(icon_dd_blanket, "dropdown/blanket.svg");
canonical_icon!(icon_dd_bring_front, "dropdown/bring_front.svg");
canonical_icon!(icon_dd_bring_front_of, "dropdown/bring_front_of.svg");
canonical_icon!(icon_dd_bus, "dropdown/bus.svg");
canonical_icon!(icon_dd_bus_entry, "dropdown/bus_entry.svg");
canonical_icon!(icon_dd_circle, "dropdown/circle.svg");
canonical_icon!(icon_dd_clear_filter, "dropdown/clear_filter.svg");
canonical_icon!(icon_dd_comment, "dropdown/comment.svg");
canonical_icon!(icon_dd_copy, "dropdown/copy.svg");
canonical_icon!(icon_dd_cross_probe, "dropdown/cross_probe.svg");
canonical_icon!(icon_dd_cut, "dropdown/cut.svg");
canonical_icon!(icon_dd_delete, "dropdown/delete.svg");
canonical_icon!(icon_dd_device_sheet, "dropdown/device_sheet.svg");
canonical_icon!(icon_dd_diff_pair, "dropdown/diff_pair.svg");
canonical_icon!(icon_dd_dist_horiz, "dropdown/dist_horiz.svg");
canonical_icon!(icon_dd_dist_vert, "dropdown/dist_vert.svg");
canonical_icon!(icon_dd_drag, "dropdown/drag.svg");
canonical_icon!(icon_dd_drag_sel, "dropdown/drag_sel.svg");
canonical_icon!(icon_dd_ellipse, "dropdown/ellipse.svg");
canonical_icon!(icon_dd_find_similar, "dropdown/find_similar.svg");
canonical_icon!(icon_dd_flip_x, "dropdown/flip_x.svg");
canonical_icon!(icon_dd_flip_y, "dropdown/flip_y.svg");
canonical_icon!(icon_dd_gnd, "dropdown/gnd.svg");
canonical_icon!(icon_dd_graphic, "dropdown/graphic.svg");
canonical_icon!(icon_dd_harness, "dropdown/harness.svg");
canonical_icon!(icon_dd_harness_conn, "dropdown/harness_conn.svg");
canonical_icon!(icon_dd_harness_entry, "dropdown/harness_entry.svg");
canonical_icon!(icon_dd_line, "dropdown/line.svg");
canonical_icon!(icon_dd_move, "dropdown/move.svg");
canonical_icon!(icon_dd_move_sel, "dropdown/move_sel.svg");
canonical_icon!(icon_dd_move_to_front, "dropdown/move_to_front.svg");
canonical_icon!(icon_dd_move_xy, "dropdown/move_xy.svg");
canonical_icon!(icon_dd_net_color_clear, "dropdown/net_color_clear.svg");
canonical_icon!(icon_dd_net_color_clear_all, "dropdown/net_color_clear_all.svg");
canonical_icon!(icon_dd_net_color_custom, "dropdown/net_color_custom.svg");
canonical_icon!(icon_dd_net_label, "dropdown/net_label.svg");
canonical_icon!(icon_dd_no_erc, "dropdown/no_erc.svg");
canonical_icon!(icon_dd_note, "dropdown/note.svg");
canonical_icon!(icon_dd_off_sheet, "dropdown/off_sheet.svg");
canonical_icon!(icon_dd_open_child_sheet, "dropdown/open_child_sheet.svg");
canonical_icon!(icon_dd_other, "dropdown/other.svg");
canonical_icon!(icon_dd_param_set, "dropdown/param_set.svg");
canonical_icon!(icon_dd_part_actions, "dropdown/part_actions.svg");
canonical_icon!(icon_dd_paste, "dropdown/paste.svg");
canonical_icon!(icon_dd_pin_mapping, "dropdown/pin_mapping.svg");
canonical_icon!(icon_dd_place_menu, "dropdown/place.svg");
canonical_icon!(icon_dd_polygon, "dropdown/polygon.svg");
canonical_icon!(icon_dd_port, "dropdown/port.svg");
canonical_icon!(icon_dd_preferences, "dropdown/preferences.svg");
canonical_icon!(icon_dd_project_options, "dropdown/project_options.svg");
canonical_icon!(icon_dd_properties, "dropdown/properties.svg");
canonical_icon!(icon_dd_pwr_arrow, "dropdown/pwr_arrow.svg");
canonical_icon!(icon_dd_pwr_bar, "dropdown/pwr_bar.svg");
canonical_icon!(icon_dd_pwr_circle, "dropdown/pwr_circle.svg");
canonical_icon!(icon_dd_pwr_earth, "dropdown/pwr_earth.svg");
canonical_icon!(icon_dd_pwr_minus5, "dropdown/pwr_minus5.svg");
canonical_icon!(icon_dd_pwr_plus12, "dropdown/pwr_plus12.svg");
canonical_icon!(icon_dd_pwr_plus5, "dropdown/pwr_plus5.svg");
canonical_icon!(icon_dd_pwr_signal_gnd, "dropdown/pwr_signal_gnd.svg");
canonical_icon!(icon_dd_pwr_wave, "dropdown/pwr_wave.svg");
canonical_icon!(icon_dd_rect, "dropdown/rect.svg");
canonical_icon!(icon_dd_references, "dropdown/references.svg");
canonical_icon!(icon_dd_reuse_block, "dropdown/reuse_block.svg");
canonical_icon!(icon_dd_rotate, "dropdown/rotate.svg");
canonical_icon!(icon_dd_rotate_cw, "dropdown/rotate_cw.svg");
canonical_icon!(icon_dd_round_rect, "dropdown/round_rect.svg");
canonical_icon!(icon_dd_select_all, "dropdown/select_all.svg");
canonical_icon!(icon_dd_select_connection, "dropdown/select_connection.svg");
canonical_icon!(icon_dd_select_inside, "dropdown/select_inside.svg");
canonical_icon!(icon_dd_select_lasso, "dropdown/select_lasso.svg");
canonical_icon!(icon_dd_select_outside, "dropdown/select_outside.svg");
canonical_icon!(icon_dd_select_toggle, "dropdown/select_toggle.svg");
canonical_icon!(icon_dd_select_touching_line, "dropdown/select_touching_line.svg");
canonical_icon!(icon_dd_select_touching_rect, "dropdown/select_touching_rect.svg");
canonical_icon!(icon_dd_send_back, "dropdown/send_back.svg");
canonical_icon!(icon_dd_send_back_of, "dropdown/send_back_of.svg");
canonical_icon!(icon_dd_sheet_actions, "dropdown/sheet_actions.svg");
canonical_icon!(icon_dd_sheet_entry, "dropdown/sheet_entry.svg");
canonical_icon!(icon_dd_sheet_symbol, "dropdown/sheet_symbol.svg");
canonical_icon!(icon_dd_smart_paste, "dropdown/smart_paste.svg");
canonical_icon!(icon_dd_snippets, "dropdown/snippets.svg");
canonical_icon!(icon_dd_supplier_links, "dropdown/supplier_links.svg");
canonical_icon!(icon_dd_text_frame, "dropdown/text_frame.svg");
canonical_icon!(icon_dd_text_string, "dropdown/text_string.svg");
canonical_icon!(icon_dd_unions, "dropdown/unions.svg");
canonical_icon!(icon_dd_vcc, "dropdown/vcc.svg");
canonical_icon!(icon_dd_wire, "dropdown/wire.svg");

// ─── Justify picker (9-cell property panel) ────────────────────────

canonical_icon!(icon_justify_b, "justify/b.svg");
canonical_icon!(icon_justify_bl, "justify/bl.svg");
canonical_icon!(icon_justify_br, "justify/br.svg");
canonical_icon!(icon_justify_c, "justify/c.svg");
canonical_icon!(icon_justify_l, "justify/l.svg");
canonical_icon!(icon_justify_r, "justify/r.svg");
canonical_icon!(icon_justify_t, "justify/t.svg");
canonical_icon!(icon_justify_tl, "justify/tl.svg");
canonical_icon!(icon_justify_tr, "justify/tr.svg");
