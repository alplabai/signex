//! PDF /Outlines (bookmark) emission for the schematic exporter.
//!
//! Builds a flat tree of bookmark items in two passes so that each
//! item gets a stable `Ref` before its parent/sibling links need to
//! be written. Pass 1 walks every sheet in the export and collects
//! `PendingBookmark`s gated by the `PdfOptions` toggles. Pass 2
//! assigns sequential `Ref`s and writes the outline dict + every
//! item dict.
//!
//! Layout (Altium parity):
//! ```text
//! Outline root
//! ├── Sheet 1: Power
//! │   ├── Components
//! │   │   ├── R1
//! │   │   └── U1
//! │   └── Nets
//! │       ├── /VCC
//! │       ├── /GND
//! │       └── pin U1.3
//! └── Sheet 2: ...
//! ```
//!
//! With `global_bookmarks = true` the Components / Nets groups are
//! pulled out from under each sheet and aggregated into two
//! top-level groups instead. Per-sheet items still appear as their
//! own top-level entries so navigation by page is preserved.

use pdf_writer::{Finish, Pdf, Ref, TextStr};

use super::PdfOptions;
use super::layout::PageTransform;
use crate::ExportContext;
use signex_types::schematic::LabelType;

/// A bookmark target before any PDF refs have been allocated.
#[derive(Debug, Clone)]
pub(crate) struct PendingBookmark {
    pub title: String,
    pub parent_idx: Option<usize>,
    pub children: Vec<usize>,
    /// Index into the `page_refs` slice at emit time, *not* the raw
    /// sheet index — we only emit bookmarks for pages actually in
    /// the export's page range.
    pub page_idx: usize,
    /// Destination coordinates in PDF user-space points. The PDF
    /// origin is the bottom-left of the page so we keep these
    /// already-flipped values.
    pub x_pt: f32,
    pub y_pt: f32,
}

impl PendingBookmark {
    fn new_root_node(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            parent_idx: None,
            children: Vec::new(),
            // Group nodes don't have a real destination — emit as
            // page 0 with the same coords every viewer falls back to.
            page_idx: 0,
            x_pt: 0.0,
            y_pt: 0.0,
        }
    }
}

/// Walk the export context and gather every bookmark the user asked
/// for via `PdfOptions`. Returns an empty vec when no bookmark
/// toggles are on — the caller should skip emitting `/Outlines`
/// entirely in that case.
pub(crate) fn build_bookmarks(
    ctx: &ExportContext,
    opts: &PdfOptions,
    page_sheet_indices: &[usize],
    page_w_mm: f64,
    page_h_mm: f64,
    page_h_pt: f32,
) -> Vec<PendingBookmark> {
    if !any_bookmark_toggle_on(opts) {
        return Vec::new();
    }

    // mm → pt; matches `pdf::MM_TO_PT` (1 pt = 1/72 inch).
    let mm_to_pt = 72.0 / 25.4;

    let mut items: Vec<PendingBookmark> = Vec::new();

    // Top-level group nodes for the global mode. Per-sheet items
    // still appear at the top level so users can jump to a page
    // without diving into a group.
    let global_components_idx = if opts.global_bookmarks && opts.include_component_parameters {
        let i = items.len();
        items.push(PendingBookmark::new_root_node("Components"));
        Some(i)
    } else {
        None
    };
    let global_nets_idx = if opts.global_bookmarks && nets_group_active(opts) {
        let i = items.len();
        items.push(PendingBookmark::new_root_node("Nets"));
        Some(i)
    } else {
        None
    };

    for (page_idx, &sheet_idx) in page_sheet_indices.iter().enumerate() {
        let Some(sheet) = ctx.sheets.get(sheet_idx) else {
            continue;
        };

        let xform = PageTransform::new(
            sheet,
            page_w_mm,
            page_h_mm,
            &opts.margins,
            &opts.scale,
            mm_to_pt,
        );

        // Sheet bookmark — always emitted when any sub-bookmark is
        // active. Variant tag in the title lets readers tell
        // expanded physical structures apart at a glance.
        let sheet_title = build_sheet_title(sheet, opts);
        let sheet_bookmark_idx = items.len();
        items.push(PendingBookmark {
            title: sheet_title,
            parent_idx: None,
            children: Vec::new(),
            page_idx,
            // Destination at page top-left so the outline jump
            // shows the title-block area rather than mid-sheet.
            x_pt: 0.0,
            y_pt: page_h_pt,
        });

        // Per-sheet groups in non-global mode: nest Components / Nets
        // under the sheet so the outline matches Altium's tree.
        let local_components_idx = if !opts.global_bookmarks && opts.include_component_parameters {
            let i = items.len();
            items.push(PendingBookmark {
                title: "Components".to_string(),
                parent_idx: Some(sheet_bookmark_idx),
                children: Vec::new(),
                page_idx,
                x_pt: 0.0,
                y_pt: page_h_pt,
            });
            items[sheet_bookmark_idx].children.push(i);
            Some(i)
        } else {
            None
        };

        let local_nets_idx = if !opts.global_bookmarks && nets_group_active(opts) {
            let i = items.len();
            items.push(PendingBookmark {
                title: "Nets".to_string(),
                parent_idx: Some(sheet_bookmark_idx),
                children: Vec::new(),
                page_idx,
                x_pt: 0.0,
                y_pt: page_h_pt,
            });
            items[sheet_bookmark_idx].children.push(i);
            Some(i)
        } else {
            None
        };

        // Components — every symbol in the sheet, gated by the
        // include_component_parameters toggle.
        if opts.include_component_parameters {
            let parent_idx = if opts.global_bookmarks {
                global_components_idx
            } else {
                local_components_idx
            };
            if let Some(parent_idx) = parent_idx {
                for sym in &sheet.schematic.symbols {
                    if sym.reference.is_empty() {
                        continue;
                    }
                    let item_idx = items.len();
                    items.push(PendingBookmark {
                        title: format_component_title(&sym.reference, &sym.value, opts),
                        parent_idx: Some(parent_idx),
                        children: Vec::new(),
                        page_idx,
                        x_pt: xform.x(sym.position.x),
                        y_pt: xform.pdf_y(sym.position.y, page_h_pt),
                    });
                    items[parent_idx].children.push(item_idx);
                }
            }
        }

        // Nets — net labels (and ports / pins as sub-categories).
        if opts.generate_nets_info {
            let parent_idx = if opts.global_bookmarks {
                global_nets_idx
            } else {
                local_nets_idx
            };
            if let Some(parent_idx) = parent_idx {
                if opts.bookmark_net_labels {
                    for label in &sheet.schematic.labels {
                        if !matches!(label.label_type, LabelType::Net | LabelType::Global) {
                            continue;
                        }
                        if label.text.is_empty() {
                            continue;
                        }
                        let item_idx = items.len();
                        items.push(PendingBookmark {
                            title: format!("/{}", label.text),
                            parent_idx: Some(parent_idx),
                            children: Vec::new(),
                            page_idx,
                            x_pt: xform.x(label.position.x),
                            y_pt: xform.pdf_y(label.position.y, page_h_pt),
                        });
                        items[parent_idx].children.push(item_idx);
                    }
                }
                if opts.bookmark_ports {
                    for label in &sheet.schematic.labels {
                        if !matches!(label.label_type, LabelType::Hierarchical) {
                            continue;
                        }
                        if label.text.is_empty() {
                            continue;
                        }
                        let item_idx = items.len();
                        items.push(PendingBookmark {
                            title: format!("port {}", label.text),
                            parent_idx: Some(parent_idx),
                            children: Vec::new(),
                            page_idx,
                            x_pt: xform.x(label.position.x),
                            y_pt: xform.pdf_y(label.position.y, page_h_pt),
                        });
                        items[parent_idx].children.push(item_idx);
                    }
                }
                if opts.bookmark_pins {
                    // Standard doesn't store per-pin physical positions
                    // separately from the parent symbol — render a
                    // pin entry per symbol-pin pair using the symbol
                    // anchor as the destination. Coarse but matches
                    // Altium's "find this pin" behaviour well enough.
                    for sym in &sheet.schematic.symbols {
                        for (pin_number, _uuid) in &sym.pin_uuids {
                            let item_idx = items.len();
                            items.push(PendingBookmark {
                                title: format!("pin {}.{}", sym.reference, pin_number),
                                parent_idx: Some(parent_idx),
                                children: Vec::new(),
                                page_idx,
                                x_pt: xform.x(sym.position.x),
                                y_pt: xform.pdf_y(sym.position.y, page_h_pt),
                            });
                            items[parent_idx].children.push(item_idx);
                        }
                    }
                }
            }
        }
    }

    items
}

/// Emit `/Outlines` and every outline item into `pdf`. `root_id`
/// must already be referenced from the catalog dict via
/// `catalog.outlines(root_id)` before calling this. `bookmark_id_base`
/// is the Ref number assigned to `bookmarks[0]`; subsequent items
/// land at `bookmark_id_base + i`.
pub(crate) fn emit_bookmarks(
    pdf: &mut Pdf,
    bookmarks: &[PendingBookmark],
    root_id: Ref,
    bookmark_id_base: i32,
    page_refs: &[Ref],
    opts: &PdfOptions,
) {
    if bookmarks.is_empty() {
        return;
    }

    let bookmark_id = |i: usize| Ref::new(bookmark_id_base + i as i32);

    // Top-level items are everything without a parent.
    let top_level: Vec<usize> = (0..bookmarks.len())
        .filter(|i| bookmarks[*i].parent_idx.is_none())
        .collect();

    // Outline root dict.
    {
        let mut outline = pdf.outline(root_id);
        if let Some(&first) = top_level.first() {
            outline.first(bookmark_id(first));
        }
        if let Some(&last) = top_level.last() {
            outline.last(bookmark_id(last));
        }
        // /Count = total visible items so the bookmarks panel opens
        // fully expanded by default, matching common EDA exporters.
        outline.count(bookmarks.len() as i32);
        outline.finish();
    }

    // Map bookmark_zoom (0.0 = Far, 1.0 = Close) to a PDF /XYZ zoom
    // factor. Default 0.5 → 100 % which is roughly "actual size".
    let zoom = bookmark_zoom_factor(opts.bookmark_zoom);

    for (idx, item) in bookmarks.iter().enumerate() {
        let self_id = bookmark_id(idx);
        let mut outline_item = pdf.outline_item(self_id);
        outline_item.title(TextStr(&item.title));

        // Parent: explicit child link → parent bookmark; top-level
        // items point at the outline root.
        if let Some(p) = item.parent_idx {
            outline_item.parent(bookmark_id(p));
        } else {
            outline_item.parent(root_id);
        }

        // Sibling chain on this level. For top-level items the
        // siblings come from `top_level`; for children of a parent
        // bookmark they come from `parent.children`.
        let siblings: &[usize] = match item.parent_idx {
            Some(p) => &bookmarks[p].children,
            None => &top_level,
        };
        if let Some(pos) = siblings.iter().position(|&i| i == idx) {
            if pos > 0 {
                outline_item.prev(bookmark_id(siblings[pos - 1]));
            }
            if pos + 1 < siblings.len() {
                outline_item.next(bookmark_id(siblings[pos + 1]));
            }
        }

        // Children chain. /Count >= 0 → tree opens expanded.
        if !item.children.is_empty() {
            outline_item.first(bookmark_id(*item.children.first().unwrap()));
            outline_item.last(bookmark_id(*item.children.last().unwrap()));
            outline_item.count(item.children.len() as i32);
        }

        // Destination: /XYZ on the target page at the recorded
        // coords with the resolved zoom factor. Group nodes (no
        // recorded coords) still need a /Dest so PDF readers don't
        // grey them out — point them at page 0 at the same zoom.
        if let Some(&page_ref) = page_refs.get(item.page_idx) {
            outline_item
                .dest()
                .page(page_ref)
                .xyz(item.x_pt, item.y_pt, Some(zoom));
        }

        outline_item.finish();
    }
}

/// Returns true when at least one toggle that produces a bookmark
/// is enabled.
fn any_bookmark_toggle_on(opts: &PdfOptions) -> bool {
    opts.include_component_parameters || opts.generate_nets_info
}

/// Returns true when the Nets group has any reason to exist.
fn nets_group_active(opts: &PdfOptions) -> bool {
    opts.generate_nets_info
        && (opts.bookmark_net_labels || opts.bookmark_pins || opts.bookmark_ports)
}

fn build_sheet_title(sheet: &crate::SheetSnapshot, opts: &PdfOptions) -> String {
    let mut title = if opts.physical_sheet_number {
        format!("Sheet {}: {}", sheet.sheet_number, sheet.sheet_name)
    } else {
        sheet.sheet_name.clone()
    };
    if opts.use_physical_structure {
        if let Some(variant) = opts.variant.as_deref() {
            if !variant.is_empty() {
                title.push_str(&format!(" [{variant}]"));
            }
        }
    }
    title
}

fn format_component_title(reference: &str, value: &str, opts: &PdfOptions) -> String {
    if !opts.physical_designators || value.is_empty() {
        reference.to_string()
    } else {
        format!("{reference} ({value})")
    }
}

fn bookmark_zoom_factor(slider: f32) -> f32 {
    // Altium's "Far ... Close" slider; clamp to [0, 1] and stretch
    // into PDF zoom space. 0.0 → 25 % (very far), 0.5 → 100 %
    // (default), 1.0 → 400 % (close-up).
    let s = slider.clamp(0.0, 1.0);
    if s <= 0.5 {
        // 0.0 → 0.25, 0.5 → 1.0
        0.25 + s * (1.0 - 0.25) / 0.5
    } else {
        // 0.5 → 1.0, 1.0 → 4.0
        1.0 + (s - 0.5) * (4.0 - 1.0) / 0.5
    }
}

#[cfg(test)]
mod tests;
