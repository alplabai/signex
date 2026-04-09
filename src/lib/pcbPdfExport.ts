/**
 * PCB PDF Export — render PCB layers to PDF for documentation/review.
 */

import { jsPDF } from "jspdf";
import type { PcbData, PcbLayerId } from "@/types/pcb";
import { DEFAULT_LAYER_COLORS } from "@/types/pcb";

export interface PcbPdfOptions {
  layers: PcbLayerId[];
  scale: number; // mm per pixel
  pageSize: "A4" | "A3" | "Letter";
  orientation: "portrait" | "landscape";
  title?: string;
  showGrid?: boolean;
  showRatsnest?: boolean;
}

const PAGE_SIZES: Record<string, [number, number]> = {
  A4: [210, 297],
  A3: [297, 420],
  Letter: [215.9, 279.4],
};

/**
 * Export PCB as a multi-layer PDF document.
 * Each visible layer gets its own page.
 */
export async function exportPcbPdf(data: PcbData, options: PcbPdfOptions): Promise<Blob> {
  const [pageW, pageH] = options.orientation === "landscape"
    ? [PAGE_SIZES[options.pageSize][1], PAGE_SIZES[options.pageSize][0]]
    : PAGE_SIZES[options.pageSize];

  const doc = new jsPDF({
    orientation: options.orientation,
    unit: "mm",
    format: [pageW, pageH],
  });

  // Board bounds
  let bMinX = Infinity, bMinY = Infinity, bMaxX = -Infinity, bMaxY = -Infinity;
  for (const p of data.board.outline) {
    bMinX = Math.min(bMinX, p.x); bMinY = Math.min(bMinY, p.y);
    bMaxX = Math.max(bMaxX, p.x); bMaxY = Math.max(bMaxY, p.y);
  }
  if (!isFinite(bMinX)) { bMinX = 0; bMinY = 0; bMaxX = 100; bMaxY = 80; }

  const boardW = bMaxX - bMinX;
  const boardH = bMaxY - bMinY;
  const margin = 15;
  const availW = pageW - margin * 2;
  const availH = pageH - margin * 2 - 10; // 10mm for title
  const scale = Math.min(availW / boardW, availH / boardH);
  const offsetX = margin + (availW - boardW * scale) / 2;
  const offsetY = margin + 10 + (availH - boardH * scale) / 2;

  const toPageX = (x: number) => offsetX + (x - bMinX) * scale;
  const toPageY = (y: number) => offsetY + (y - bMinY) * scale;

  for (let i = 0; i < options.layers.length; i++) {
    if (i > 0) doc.addPage([pageW, pageH], options.orientation);

    const layer = options.layers[i];
    const color = DEFAULT_LAYER_COLORS[layer] || "#808080";

    // Title
    doc.setFontSize(10);
    doc.setTextColor(0, 0, 0);
    doc.text(`${options.title || "PCB"} — ${layer}`, margin, margin + 5);

    // Board outline
    if (data.board.outline.length >= 3) {
      doc.setDrawColor(0, 0, 0);
      doc.setLineWidth(0.2);
      // Draw board outline as lines
      const pts = data.board.outline.map((p) => ({ x: toPageX(p.x), y: toPageY(p.y) }));
      for (let j = 0; j < pts.length; j++) {
        const k = (j + 1) % pts.length;
        doc.line(pts[j].x, pts[j].y, pts[k].x, pts[k].y);
      }
    }

    // Parse layer color
    const r = parseInt(color.slice(1, 3), 16);
    const g = parseInt(color.slice(3, 5), 16);
    const b = parseInt(color.slice(5, 7), 16);

    // Trace segments on this layer
    doc.setDrawColor(r, g, b);
    for (const seg of data.segments) {
      if (seg.layer !== layer) continue;
      doc.setLineWidth(seg.width * scale);
      doc.line(toPageX(seg.start.x), toPageY(seg.start.y), toPageX(seg.end.x), toPageY(seg.end.y));
    }

    // Pads on this layer
    doc.setFillColor(r, g, b);
    for (const fp of data.footprints) {
      for (const pad of fp.pads) {
        if (!pad.layers.includes(layer) && !pad.layers.includes("*.Cu" as PcbLayerId)) continue;
        const px = toPageX(fp.position.x + pad.position.x);
        const py = toPageY(fp.position.y + pad.position.y);
        const sw = pad.size[0] * scale;
        const sh = pad.size[1] * scale;

        if (pad.shape === "circle") {
          doc.circle(px, py, sw / 2, "F");
        } else {
          doc.rect(px - sw / 2, py - sh / 2, sw, sh, "F");
        }
      }
    }

    // Vias
    if (layer.endsWith(".Cu")) {
      doc.setFillColor(180, 180, 180);
      for (const via of data.vias) {
        const vx = toPageX(via.position.x);
        const vy = toPageY(via.position.y);
        doc.circle(vx, vy, via.diameter * scale / 2, "F");
        doc.setFillColor(255, 255, 255);
        doc.circle(vx, vy, via.drill * scale / 2, "F");
        doc.setFillColor(180, 180, 180);
      }
    }

    // Footprint references on silk/fab
    if (layer === "F.SilkS" || layer === "B.SilkS" || layer === "F.Fab" || layer === "B.Fab") {
      doc.setFontSize(6);
      doc.setTextColor(r, g, b);
      const fpLayer = layer.startsWith("F.") ? "F.Cu" : "B.Cu";
      for (const fp of data.footprints) {
        if (fp.layer !== fpLayer) continue;
        doc.text(fp.reference, toPageX(fp.position.x), toPageY(fp.position.y), { align: "center" });
      }
    }
  }

  return doc.output("blob");
}

/**
 * Layer set management — save/load visibility configurations.
 */
export interface LayerSet {
  name: string;
  visibleLayers: string[];
}

const PRESET_LAYER_SETS: LayerSet[] = [
  { name: "All Layers", visibleLayers: ["F.Cu", "B.Cu", "In1.Cu", "In2.Cu", "F.SilkS", "B.SilkS", "F.Mask", "B.Mask", "Edge.Cuts", "F.Fab", "B.Fab", "F.CrtYd", "B.CrtYd"] },
  { name: "Top Only", visibleLayers: ["F.Cu", "F.SilkS", "F.Mask", "Edge.Cuts", "F.Fab", "F.CrtYd"] },
  { name: "Bottom Only", visibleLayers: ["B.Cu", "B.SilkS", "B.Mask", "Edge.Cuts", "B.Fab", "B.CrtYd"] },
  { name: "Copper Only", visibleLayers: ["F.Cu", "B.Cu", "In1.Cu", "In2.Cu", "In3.Cu", "In4.Cu", "Edge.Cuts"] },
  { name: "Assembly Top", visibleLayers: ["F.Fab", "F.CrtYd", "F.SilkS", "Edge.Cuts"] },
  { name: "Assembly Bottom", visibleLayers: ["B.Fab", "B.CrtYd", "B.SilkS", "Edge.Cuts"] },
  { name: "Manufacturing", visibleLayers: ["F.Cu", "B.Cu", "F.Mask", "B.Mask", "F.Paste", "B.Paste", "Edge.Cuts"] },
];

export function getPresetLayerSets(): LayerSet[] {
  return PRESET_LAYER_SETS;
}

export function getCustomLayerSets(): LayerSet[] {
  try {
    const stored = localStorage.getItem("signex-pcb-layer-sets");
    return stored ? JSON.parse(stored) : [];
  } catch {
    return [];
  }
}

export function saveCustomLayerSet(set: LayerSet) {
  const existing = getCustomLayerSets();
  const updated = [...existing.filter((s) => s.name !== set.name), set];
  localStorage.setItem("signex-pcb-layer-sets", JSON.stringify(updated));
}

export function deleteCustomLayerSet(name: string) {
  const existing = getCustomLayerSets();
  localStorage.setItem("signex-pcb-layer-sets", JSON.stringify(existing.filter((s) => s.name !== name)));
}
