//! Tree view widget — Altium-style project/component browser tree.
//!
//! COSMIC composition pattern: `TreeView` struct with builder methods.
//! Matches the React reference (ProjectPanel.tsx) spacing pixel-for-pixel:
//!   - 14px indent per depth + 10px base left padding
//!   - 6px gap between elements (gap-1.5)
//!   - 5px vertical row padding
//!   - 12px body font, 10px badge
//!   - text-secondary default → text-primary on selected
//!   - hover bg, selection bg, "(empty)" indicator

use std::sync::OnceLock;

use iced::widget::{Column, Row, Space, button, container, mouse_area, scrollable, svg, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeTokens;

// ─── SVG Chevron Icons (cached handles) ──────────────────────

const SVG_CHEVRON_RIGHT: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M3 1l5 4-5 4z" fill="currentColor"/></svg>"#;
const SVG_CHEVRON_DOWN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 10 10"><path d="M1 3l4 5 4-5z" fill="currentColor"/></svg>"#;

fn chevron_right_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(SVG_CHEVRON_RIGHT))
        .clone()
}

fn chevron_down_handle() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(SVG_CHEVRON_DOWN))
        .clone()
}

use crate::PushIf;
use crate::theme_ext;

// ─── Data Model ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeIcon {
    Folder,
    FolderOpen,
    File,
    Schematic,
    Pcb,
    Library,
    Component,
    Sheet,
    Net,
    Pin,
    // ─── Signex native file formats ─────────────────────────────
    // Each renders as a full-color SVG (see `svg_bytes`). Glyph +
    // colour methods below return sensible fallbacks but the SVG
    // path is what actually paints in the tree.
    /// `.snxprj` — Signex project file.
    SnxProject,
    /// `.snxsch` — Signex schematic.
    SnxSchematic,
    /// `.snxpcb` — Signex PCB.
    SnxPcb,
    /// `.snxfpt` — Signex footprint.
    SnxFootprint,
    /// `.snxsim` — Signex simulation.
    SnxSimulation,
    /// `.snxlib` — Signex library.
    SnxLibrary,
    /// `.snxsym` — Signex symbol.
    SnxSymbol,
    /// `.snxpkg` — Signex package / distributable bundle.
    Package,
    /// `.snxmat` — Signex PCB material / stackup sidecar.
    Material,
    /// `.snxcfg` — Signex project config.
    Config,
    /// `.snxmod` — Signex encrypted SPICE model (distinct from
    /// `.snxsim`, which is the testbench, not the model).
    Model,
}

// ─── Tree icon assets ─────────────────────────────────────────
//
// Every `TreeIcon` variant renders as a bundled SVG. Three groups:
//
//  * **Generic tree icons** — folder / file / library / component /
//    sheet / net / pin. Live at
//    `crates/signex-widgets/assets/tree-icons/`.
//  * **Signex native `.snx***` file family** — shared with the
//    installer's file-association artwork at
//    `crates/signex-app/assets/icons/files/`. Reached cross-crate via
//    `include_bytes!` so one copy of the artwork serves both the
//    tree view and the .ico/.icns raster pipeline.
//  * **Standard handoff formats** — `.standard_sch` / `.standard_pcb` /
//    `.standard_sym` / `.standard_mod` render with the matching Signex-
//    brand glyph (same chamfered silhouette + amber wedge), so the
//    project tree stays visually consistent regardless of whether
//    files are native or Standard. The `TreeIcon::Schematic` / `::Pcb`
//    variants remain in the enum for backward-compat but now share
//    the `.snxsch` / `.snxpcb` SVGs.

// Per-variant `OnceLock` cache. iced handles are Arc-backed so
// cloning a cached handle into every render frame is near-free.
macro_rules! cached_svg_handle {
    ($bytes:expr) => {{
        static H: OnceLock<svg::Handle> = OnceLock::new();
        H.get_or_init(|| svg::Handle::from_memory($bytes)).clone()
    }};
}

// Generic tree icons (SVG).
const SVG_TREE_FOLDER: &[u8] = include_bytes!("../assets/tree-icons/folder.svg");
const SVG_TREE_FOLDER_OPEN: &[u8] = include_bytes!("../assets/tree-icons/folder_open.svg");
const SVG_TREE_FILE: &[u8] = include_bytes!("../assets/tree-icons/file.svg");
const SVG_TREE_LIBRARY: &[u8] = include_bytes!("../assets/tree-icons/library.svg");
const SVG_TREE_COMPONENT: &[u8] = include_bytes!("../assets/tree-icons/component.svg");
const SVG_TREE_SHEET: &[u8] = include_bytes!("../assets/tree-icons/sheet.svg");
const SVG_TREE_NET: &[u8] = include_bytes!("../assets/tree-icons/net.svg");
const SVG_TREE_PIN: &[u8] = include_bytes!("../assets/tree-icons/pin.svg");
const SVG_TREE_PACKAGE: &[u8] = include_bytes!("../assets/tree-icons/package.svg");
const SVG_TREE_MATERIAL: &[u8] = include_bytes!("../assets/tree-icons/material.svg");
const SVG_TREE_CONFIG: &[u8] = include_bytes!("../assets/tree-icons/config.svg");
const SVG_TREE_MODEL: &[u8] = include_bytes!("../assets/tree-icons/model.svg");

// Signex native `.snx***` file family (SVG). Shared with the
// installer's file-association artwork; update both paths together
// if the asset layout changes.
const SVG_SNX_PROJECT: &[u8] = include_bytes!("../../signex-app/assets/icons/files/snxprj.svg");
const SVG_SNX_SCHEMATIC: &[u8] = include_bytes!("../../signex-app/assets/icons/files/snxsch.svg");
const SVG_SNX_PCB: &[u8] = include_bytes!("../../signex-app/assets/icons/files/snxpcb.svg");
const SVG_SNX_FOOTPRINT: &[u8] = include_bytes!("../../signex-app/assets/icons/files/snxfpt.svg");
const SVG_SNX_SIMULATION: &[u8] = include_bytes!("../../signex-app/assets/icons/files/snxsim.svg");
const SVG_SNX_LIBRARY: &[u8] = include_bytes!("../../signex-app/assets/icons/files/snxlib.svg");
const SVG_SNX_SYMBOL: &[u8] = include_bytes!("../../signex-app/assets/icons/files/snxsym.svg");

impl TreeIcon {
    /// Return the cached SVG handle for this icon. Each variant
    /// memoises its handle through a `OnceLock` so bytes are only
    /// wrapped once per process — subsequent calls are a cheap
    /// `Arc::clone`.
    pub fn svg(self) -> svg::Handle {
        match self {
            Self::Folder => cached_svg_handle!(SVG_TREE_FOLDER),
            Self::FolderOpen => cached_svg_handle!(SVG_TREE_FOLDER_OPEN),
            Self::File => cached_svg_handle!(SVG_TREE_FILE),
            Self::Library => cached_svg_handle!(SVG_TREE_LIBRARY),
            Self::Component => cached_svg_handle!(SVG_TREE_COMPONENT),
            Self::Sheet => cached_svg_handle!(SVG_TREE_SHEET),
            Self::Net => cached_svg_handle!(SVG_TREE_NET),
            Self::Pin => cached_svg_handle!(SVG_TREE_PIN),
            // Standard handoff formats — share the Signex-brand glyph
            // for visual consistency in the project tree. The enum
            // variants remain for backward-compat with older call
            // sites that construct `TreeIcon::Schematic/::Pcb`
            // directly.
            Self::Schematic => cached_svg_handle!(SVG_SNX_SCHEMATIC),
            Self::Pcb => cached_svg_handle!(SVG_SNX_PCB),
            // Signex native `.snx***` family.
            Self::SnxProject => cached_svg_handle!(SVG_SNX_PROJECT),
            Self::SnxSchematic => cached_svg_handle!(SVG_SNX_SCHEMATIC),
            Self::SnxPcb => cached_svg_handle!(SVG_SNX_PCB),
            Self::SnxFootprint => cached_svg_handle!(SVG_SNX_FOOTPRINT),
            Self::SnxSimulation => cached_svg_handle!(SVG_SNX_SIMULATION),
            Self::SnxLibrary => cached_svg_handle!(SVG_SNX_LIBRARY),
            Self::SnxSymbol => cached_svg_handle!(SVG_SNX_SYMBOL),
            Self::Package => cached_svg_handle!(SVG_TREE_PACKAGE),
            Self::Material => cached_svg_handle!(SVG_TREE_MATERIAL),
            Self::Config => cached_svg_handle!(SVG_TREE_CONFIG),
            Self::Model => cached_svg_handle!(SVG_TREE_MODEL),
        }
    }

    /// Pick a `TreeIcon` for a filename. Both Signex `.snx***` and
    /// Standard `.standard_*` extensions route to the same Signex-brand
    /// glyph family so the project tree reads as one cohesive visual
    /// family regardless of whether the underlying file is native or
    /// Standard. Unknown extensions fall back to `File`.
    pub fn for_path(filename: &str) -> Self {
        let lower = filename.to_ascii_lowercase();
        if let Some(ext) = lower.rsplit('.').next() {
            match ext {
                // Native Signex files.
                "snxprj" => Self::SnxProject,
                "snxsch" => Self::SnxSchematic,
                "snxpcb" => Self::SnxPcb,
                "snxfpt" => Self::SnxFootprint,
                "snxsim" => Self::SnxSimulation,
                "snxlib" => Self::SnxLibrary,
                "snxsym" => Self::SnxSymbol,
                "snxpkg" => Self::Package,
                "snxmat" => Self::Material,
                "snxcfg" => Self::Config,
                "snxmod" => Self::Model,
                // Standard handoff formats — map to the matching Signex
                // glyph. `.standard_sym` is a symbol library (multiple
                // symbols) so it pairs with the library glyph;
                // `.standard_mod` is a single footprint.
                "standard_pro" => Self::SnxProject,
                "standard_sch" => Self::SnxSchematic,
                "standard_pcb" => Self::SnxPcb,
                "standard_sym" => Self::SnxLibrary,
                "standard_mod" => Self::SnxFootprint,
                _ => Self::File,
            }
        } else {
            Self::File
        }
    }
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub label: String,
    pub icon: TreeIcon,
    pub expanded: bool,
    pub children: Vec<TreeNode>,
    pub badge: Option<String>,
    /// Node represents a folder that can hold children (show expand even if empty).
    pub is_folder: bool,
    /// Accent the row when rendering (bold label at depth 0). Callers
    /// set this to mark the active project root in multi-project
    /// workspaces; inner nodes ignore it.
    pub accent: bool,
    /// File represented by this leaf is currently open in a tab.
    /// Drives a small right-side "open" marker — Altium parity.
    pub is_open: bool,
    /// File has unsaved changes. Drives a red dot on the right —
    /// Altium parity. Implies `is_open` (only open files can be
    /// dirty), but the renderer doesn't enforce that.
    pub is_dirty: bool,
    /// Leaf is the currently-active document (the tab the user is
    /// viewing). Renders with a highlighted row background so the
    /// user can find their place in a multi-sheet project at a
    /// glance — Altium parity.
    pub is_active: bool,
}

impl TreeNode {
    pub fn leaf(label: impl Into<String>, icon: TreeIcon) -> Self {
        Self {
            label: label.into(),
            icon,
            expanded: false,
            children: Vec::new(),
            badge: None,
            is_folder: false,
            accent: false,
            is_open: false,
            is_dirty: false,
            is_active: false,
        }
    }

    pub fn branch(label: impl Into<String>, icon: TreeIcon, children: Vec<TreeNode>) -> Self {
        Self {
            label: label.into(),
            icon,
            expanded: true,
            children,
            badge: None,
            is_folder: true,
            accent: false,
            is_open: false,
            is_dirty: false,
            is_active: false,
        }
    }

    pub fn with_badge(mut self, badge: impl Into<String>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Builder variant: mark the node as accented (bold at depth 0).
    pub fn with_accent(mut self, accent: bool) -> Self {
        self.accent = accent;
        self
    }

    /// Builder: mark this leaf as currently open in a tab.
    pub fn with_open(mut self, open: bool) -> Self {
        self.is_open = open;
        self
    }

    /// Builder: mark this leaf as having unsaved changes.
    pub fn with_dirty(mut self, dirty: bool) -> Self {
        self.is_dirty = dirty;
        self
    }

    /// Builder: mark this leaf as the currently-active document.
    pub fn with_active(mut self, active: bool) -> Self {
        self.is_active = active;
        self
    }
}

// ─── Messages ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TreeMsg {
    Toggle(Vec<usize>),
    Select(Vec<usize>),
    /// Right-click on a node. Anchor coordinates are resolved by the
    /// parent (from `last_mouse_pos`) since iced 0.14's `mouse_area` does
    /// not forward cursor position with `on_right_press`.
    ContextMenu(Vec<usize>),
    /// Right-click in the tree area but not on any node — offer generic
    /// tree actions (Expand all / Collapse all / Refresh).
    BackgroundContextMenu,
}

// ─── Layout Constants (matched to Altium Designer) ────────────

const INDENT_PER_DEPTH: f32 = 12.0; // Altium: ~12px per depth at compact density
const BASE_PAD_LEFT: f32 = 4.0; // minimal base indent
const ELEM_GAP: f32 = 2.0; // Altium: very tight gaps
const ICON_LABEL_GAP: f32 = 4.0; // Tight gap right of the icon (Altium parity)
const CHEVRON_W: f32 = 8.0; // triangle column — half-step smaller to match smaller icon
const ICON_SZ: f32 = 12.0; // Chamfered SVG silhouettes — Altium parity at compact density
const FONT_SZ: f32 = 10.5; // body text — Altium project tree is typically 9-10pt
const BADGE_SZ: f32 = 9.0; // muted counts — track FONT_SZ down a half-step
const PAD_V: u16 = 2; // Altium-ish: 16-18px row height — 1-2 px breathing room above + below the icon
const PAD_R: u16 = 6; // right margin — leaves room for open / dirty indicators
/// Right-side "file is currently open" indicator — small filled square,
/// matches Altium's per-row open-document marker. Pulled out as a const
/// so the rest of the tree row layout can plan around its width.
const OPEN_DOT_SZ: f32 = 6.0;
/// Right-side "file has unsaved changes" indicator — bright red dot.
const DIRTY_DOT_SZ: f32 = 6.0;
/// Spacer between the label / badge and the right-side indicators.
const RIGHT_INDICATOR_GAP: f32 = 6.0;

// ─── TreeView (COSMIC composition struct) ─────────────────────

/// Composable tree view widget.
///
/// ```rust,ignore
/// TreeView::new(&roots, &tokens)
///     .selected(&path)
///     .view()
///     .map(PanelMsg::Tree)
/// ```
pub struct TreeView<'a> {
    roots: &'a [TreeNode],
    selected: Option<&'a [usize]>,
    tokens: &'a ThemeTokens,
}

impl<'a> TreeView<'a> {
    pub fn new(roots: &'a [TreeNode], tokens: &'a ThemeTokens) -> Self {
        Self {
            roots,
            selected: None,
            tokens,
        }
    }

    pub fn selected(mut self, path: &'a [usize]) -> Self {
        self.selected = Some(path);
        self
    }

    /// Build the tree into a scrollable Element.
    pub fn view(self) -> Element<'static, TreeMsg> {
        let mut col: Column<'static, TreeMsg> = Column::new().spacing(0.0).width(Length::Fill);
        for (i, node) in self.roots.iter().enumerate() {
            col = render_node(col, node, 0, &[i], self.selected, self.tokens);
        }
        // Right-click on the scrollable body (below the last row or in any
        // gap between rows that row-level mouse_areas don't cover) triggers
        // the background context menu (Expand all / Collapse all / Refresh).
        // Row right-clicks are captured earlier by per-row mouse_areas and
        // take priority — they never propagate to this outer handler.
        mouse_area(scrollable(col).width(Length::Fill))
            .on_right_press(TreeMsg::BackgroundContextMenu)
            .into()
    }
}

// ─── Row Rendering ────────────────────────────────────────────

fn render_node(
    mut col: Column<'static, TreeMsg>,
    node: &TreeNode,
    depth: usize,
    path: &[usize],
    selected: Option<&[usize]>,
    tokens: &ThemeTokens,
) -> Column<'static, TreeMsg> {
    let path_vec: Vec<usize> = path.to_vec();
    let is_sel = selected.is_some_and(|s| s == path);
    let has_kids = !node.children.is_empty();
    let is_expandable = has_kids || node.is_folder;

    // --- Colors ---
    // Tree labels render in primary text (white on dark themes) so
    // they read cleanly next to the full-colour SVG icons. Older
    // builds used `text_secondary` back when icons were muted
    // Unicode glyphs.
    let txt_c = if is_sel {
        Color::WHITE
    } else {
        theme_ext::text_primary(tokens)
    };
    // Icons are now rendered as full-colour bitmap / SVG assets — no
    // per-variant theme tinting. The theme still drives text + hover
    // + selection bg below.
    let badge_c = {
        let base = theme_ext::text_secondary(tokens);
        Color::from_rgba(base.r, base.g, base.b, 0.5)
    };
    let sel_bg = theme_ext::selection_color(tokens);
    let hov_bg = theme_ext::hover_color(tokens);

    // --- Assemble row ---
    let pad_left = (depth as f32) * INDENT_PER_DEPTH + BASE_PAD_LEFT;

    let mut r: Row<'static, TreeMsg> = Row::new()
        .spacing(ELEM_GAP)
        .align_y(iced::Alignment::Center);

    // Left indent
    r = r.push_if(pad_left > 0.0, || Space::new().width(pad_left));

    // Chevron — SVG icon (Unicode triangles render as colored emoji on Windows)
    if is_expandable {
        let handle = if node.expanded {
            chevron_down_handle()
        } else {
            chevron_right_handle()
        };
        r = r.push(
            svg(handle)
                .width(CHEVRON_W)
                .height(CHEVRON_W)
                .style(|_: &Theme, _| svg::Style {
                    color: Some(Color::WHITE),
                }),
        );
    } else {
        r = r.push(Space::new().width(CHEVRON_W));
    }

    // Icon — full-colour bundled SVG. Cached per variant; render via
    // `iced::widget::svg`. Standard handoff formats share glyphs with
    // their Signex-native counterparts (see `TreeIcon::svg`).
    //
    // Active-project marker: when this row is the accented root in a
    // multi-project workspace (`node.accent && depth == 0`), tint the
    // icon with the theme accent so the active project pops without
    // relying solely on the bold-label cue. svg::Style.color flat-tints
    // the SVG, which is what we want at this size — the silhouette
    // stays readable, the colour signals "this is active".
    let icon_w = svg(node.icon.svg()).width(ICON_SZ).height(ICON_SZ);
    let icon_w = if node.accent && depth == 0 {
        let accent = theme_ext::accent_color(tokens);
        icon_w.style(move |_: &Theme, _| svg::Style {
            color: Some(accent),
        })
    } else {
        icon_w
    };
    r = r.push(icon_w);
    // Dedicated icon → label gap. `ELEM_GAP` alone (2 px) leaves the
    // label visually kerned into the icon; a wider spacer just after
    // the icon opens the gap without affecting the chevron→icon
    // alignment that's already tight-on-purpose.
    r = r.push(Space::new().width(ICON_LABEL_GAP));

    // Label (flex, no wrap — truncation handled by scrollable parent).
    // Accented root nodes (active project in a multi-project workspace)
    // render bold so the user can scan the workspace at a glance; inner
    // depths ignore the flag.
    let mut label = text(node.label.clone())
        .size(FONT_SZ)
        .color(txt_c)
        .wrapping(iced::widget::text::Wrapping::None);
    if node.accent && depth == 0 {
        label = label.font(iced::Font {
            weight: iced::font::Weight::Bold,
            ..iced::Font::DEFAULT
        });
    }
    r = r.push(label);

    // Badge (right-aligned, very muted, single line)
    if let Some(badge) = &node.badge {
        r = r.push(iced::widget::space::horizontal());
        r = r.push(
            text(badge.clone())
                .size(BADGE_SZ)
                .color(badge_c)
                .wrapping(iced::widget::text::Wrapping::None),
        );
    }

    // Right-side indicators (Altium parity):
    //   • dim grey dot  → file is currently open in a tab
    //   • bright red dot → file has unsaved changes
    // Both ride to the far right of the row, after the badge. The
    // `is_dirty` dot replaces the open dot when both apply since the
    // dirty state already implies open and only one indicator's worth
    // of width fits comfortably in narrow Projects panels.
    if node.is_open || node.is_dirty {
        if node.badge.is_none() {
            r = r.push(iced::widget::space::horizontal());
        } else {
            r = r.push(Space::new().width(RIGHT_INDICATOR_GAP));
        }
        let dot_color = if node.is_dirty {
            // Windows-native destructive red — same hue used by the
            // chrome window-close hover so dirty / close stay
            // visually consistent.
            Color::from_rgba(0.85, 0.30, 0.30, 1.0)
        } else {
            // Open-but-clean reads as a neutral white dot — the theme
            // accent is reserved for the active-project marker so
            // mixing accent colour into the open indicator made the
            // tree feel noisy at a glance.
            Color::WHITE
        };
        let sz = if node.is_dirty {
            DIRTY_DOT_SZ
        } else {
            OPEN_DOT_SZ
        };
        r = r.push(
            container(Space::new().width(sz).height(sz))
                .width(sz)
                .height(sz)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(dot_color)),
                    border: iced::Border {
                        radius: (sz / 2.0).into(),
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                }),
        );
    }

    // Click action
    let msg = if is_expandable {
        TreeMsg::Toggle(path_vec.clone())
    } else {
        TreeMsg::Select(path_vec)
    };

    // Button with hover/selection styling. Active-row gets a
    // dimmer-than-selection bg so the active document reads at a
    // glance without competing with the explicit click-to-select
    // colour.
    let is_active = node.is_active;
    let active_bg = {
        let s = sel_bg;
        Color::from_rgba(s.r, s.g, s.b, 0.45)
    };
    let row_btn = button(r)
        .padding([PAD_V, PAD_R])
        .width(Length::Fill)
        .on_press(msg)
        .style(move |_theme: &Theme, status: button::Status| {
            let bg = match (is_sel, is_active, status) {
                (true, _, _) => Some(Background::Color(sel_bg)),
                (false, _, button::Status::Hovered | button::Status::Pressed) => {
                    Some(Background::Color(hov_bg))
                }
                (false, true, _) => Some(Background::Color(active_bg)),
                _ => None,
            };
            button::Style {
                background: bg,
                text_color: txt_c,
                border: Border {
                    radius: 2.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        });

    // Right-click → tree-scoped context menu. `mouse_area` wraps the
    // button so left-press still reaches the button's `on_press`; only
    // `on_right_press` is intercepted here.
    let row_element = mouse_area(row_btn).on_right_press(TreeMsg::ContextMenu(path.to_vec()));
    col = col.push(row_element);

    // Expanded children
    if is_expandable && node.expanded {
        if has_kids {
            for (i, child) in node.children.iter().enumerate() {
                let mut child_path = path.to_vec();
                child_path.push(i);
                col = render_node(col, child, depth + 1, &child_path, selected, tokens);
            }
        } else {
            // "(empty)" indicator — matches React's italic muted placeholder
            let empty_pad =
                ((depth + 1) as f32) * INDENT_PER_DEPTH + BASE_PAD_LEFT + CHEVRON_W + ELEM_GAP;
            let muted = theme_ext::text_secondary(tokens);
            let empty_c = Color::from_rgba(muted.r, muted.g, muted.b, 0.3);
            col = col.push(
                container(
                    text("(empty)")
                        .size(11.0)
                        .color(empty_c)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .padding(iced::Padding {
                    top: 2.0,
                    right: 0.0,
                    bottom: 2.0,
                    left: empty_pad,
                }),
            );
        }
    }

    col
}

// ─── State Helpers ────────────────────────────────────────────

/// Get a node by path (immutable).
pub fn get_node<'a>(roots: &'a [TreeNode], path: &[usize]) -> Option<&'a TreeNode> {
    if path.is_empty() {
        return None;
    }
    let mut current = roots.get(path[0])?;
    for &idx in &path[1..] {
        current = current.children.get(idx)?;
    }
    Some(current)
}

/// Toggle expand/collapse at the given path.
pub fn toggle(roots: &mut [TreeNode], path: &[usize]) {
    if let Some(node) = get_node_mut(roots, path) {
        node.expanded = !node.expanded;
    }
}

fn get_node_mut<'a>(roots: &'a mut [TreeNode], path: &[usize]) -> Option<&'a mut TreeNode> {
    if path.is_empty() {
        return None;
    }
    let mut current = roots.get_mut(path[0])?;
    for &idx in &path[1..] {
        current = current.children.get_mut(idx)?;
    }
    Some(current)
}
