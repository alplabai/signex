// ═══════════════════════════════════════════════════════════════
// Gerber RS-274X & Excellon Drill Export
// ═══════════════════════════════════════════════════════════════

import type {
  PcbData,
  PcbLayerId,
  PcbPad,
  PcbVia,
  PcbFootprint,
  PcbGraphic,
  PcbPoint,
} from "@/types/pcb";
import { generateX2Header } from "./pcbGerberX2";

// --- Coordinate Helpers ---

/** Convert mm to Gerber integer (6 decimal places, FSLAX36Y36). */
function mmToGerber(mm: number): number {
  return Math.round(mm * 1_000_000);
}

/** Format a Gerber coordinate value (no leading zeros). */
function fmtCoord(mm: number): string {
  return String(mmToGerber(mm));
}

/** Format X/Y coordinate pair for Gerber. */
function xy(x: number, y: number): string {
  return `X${fmtCoord(x)}Y${fmtCoord(y)}`;
}

/** Format X/Y coordinate pair for Excellon drill (mm, 3.3 format). */
function drillXY(x: number, y: number): string {
  const fmtDrill = (v: number): string => {
    const scaled = Math.round(v * 1000);
    return String(scaled);
  };
  return `X${fmtDrill(x)}Y${fmtDrill(y)}`;
}

/** Round to 3 decimal places for aperture dimensions. */
function round3(v: number): number {
  return Math.round(v * 1000) / 1000;
}

// --- Aperture Management ---

interface Aperture {
  code: number;      // D-code (10+)
  type: "C" | "R";   // Circle or Rectangle
  params: number[];   // [diameter] or [width, height]
}

/** Drawable PcbGraphic variants (excludes text which has no width). */
type DrawableGraphic = Exclude<PcbGraphic, { type: "text" }>;

/** Check if a PcbGraphic is a drawable type (has a width property). */
function isDrawableGraphic(g: PcbGraphic): g is DrawableGraphic {
  return g.type !== "text";
}

/** Get line width from a drawable graphic. */
function graphicWidth(g: DrawableGraphic): number {
  return g.width || 0.127;
}

function apertureKey(type: "C" | "R", params: number[]): string {
  return `${type}:${params.map(round3).join(",")}`;
}

class ApertureTable {
  private map = new Map<string, Aperture>();
  private nextCode = 10;

  /** Get or create an aperture, returns D-code. */
  getOrCreate(type: "C" | "R", params: number[]): number {
    const key = apertureKey(type, params);
    const existing = this.map.get(key);
    if (existing) return existing.code;
    const code = this.nextCode++;
    this.map.set(key, { code, type, params });
    return code;
  }

  /** Emit all aperture definition lines. */
  definitions(): string[] {
    const lines: string[] = [];
    for (const ap of this.map.values()) {
      const dims = ap.params.map(round3);
      if (ap.type === "C") {
        lines.push(`%ADD${ap.code}C,${dims[0]}*%`);
      } else {
        lines.push(`%ADD${ap.code}R,${dims[0]}X${dims[1]}*%`);
      }
    }
    return lines;
  }
}

// --- Pad Helpers ---

/** Compute absolute pad position given footprint transform. */
function absolutePadPosition(pad: PcbPad, fp: PcbFootprint): PcbPoint {
  const rad = (fp.rotation * Math.PI) / 180;
  const cos = Math.cos(rad);
  const sin = Math.sin(rad);
  return {
    x: fp.position.x + pad.position.x * cos - pad.position.y * sin,
    y: fp.position.y + pad.position.x * sin + pad.position.y * cos,
  };
}

/** Check whether a pad is present on a given layer. */
function padOnLayer(pad: PcbPad, layer: PcbLayerId): boolean {
  if (pad.layers.includes(layer)) return true;
  // Wildcard copper layers: "*.Cu" means all copper layers
  if (pad.layers.includes("*.Cu") && (layer as string).endsWith(".Cu")) return true;
  return false;
}

// --- Gerber Generation ---

/**
 * Generate a Gerber RS-274X file for a single PCB layer.
 *
 * Coordinates are in mm. The output uses FSLAX36Y36 format (leading zero
 * omission, 6 integer + 6 decimal implied digits) with metric units.
 */
export function generateGerber(data: PcbData, layer: PcbLayerId): string {
  const lines: string[] = [];
  const apertures = new ApertureTable();

  // --- Pre-scan to build aperture table ---

  // Trace segments on this layer
  const layerSegments = data.segments.filter((s) => s.layer === layer);
  for (const seg of layerSegments) {
    apertures.getOrCreate("C", [seg.width]);
  }

  // Pads on this layer
  const layerPads: { pad: PcbPad; pos: PcbPoint }[] = [];
  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      if (padOnLayer(pad, layer)) {
        const pos = absolutePadPosition(pad, fp);
        layerPads.push({ pad, pos });
        if (pad.shape === "circle") {
          apertures.getOrCreate("C", [pad.size[0]]);
        } else {
          // rect, oval, roundrect, etc. -> rectangular aperture
          apertures.getOrCreate("R", [pad.size[0], pad.size[1]]);
        }
      }
    }
  }

  // Vias on copper layers (flash as circular pads)
  const layerVias: PcbVia[] = [];
  if ((layer as string).endsWith(".Cu")) {
    for (const via of data.vias) {
      // Through vias appear on all copper layers
      if (via.type === "through" || via.layers.includes(layer)) {
        layerVias.push(via);
        apertures.getOrCreate("C", [via.diameter]);
      }
    }
  }

  // Zones on this layer
  const layerZones = data.zones.filter((z) => z.layer === layer);
  for (const zone of layerZones) {
    // Use a thin line to draw zone outlines
    apertures.getOrCreate("C", [zone.minThickness || 0.254]);
  }

  // Footprint graphics on this layer (skip text — not drawable in Gerber)
  const layerGraphics: { g: DrawableGraphic; fp: PcbFootprint }[] = [];
  for (const fp of data.footprints) {
    for (const g of fp.graphics) {
      if (g.layer === layer && isDrawableGraphic(g)) {
        layerGraphics.push({ g, fp });
        apertures.getOrCreate("C", [graphicWidth(g)]);
      }
    }
  }

  // Board-level graphics on this layer (skip text)
  const boardGraphics = data.graphics.filter(
    (g): g is DrawableGraphic => g.layer === layer && isDrawableGraphic(g),
  );
  for (const g of boardGraphics) {
    apertures.getOrCreate("C", [graphicWidth(g)]);
  }

  // --- Gerber X2 Extended Attributes ---
  const x2Header = generateX2Header(data, layer);
  if (x2Header) lines.push(x2Header);

  // --- Header ---

  lines.push("G04 Signex EDA Gerber Export*");
  lines.push(`G04 Layer: ${layer}*`);
  lines.push("%FSLAX36Y36*%");   // Format: leading zero omit, absolute, 3.6
  lines.push("%MOMM*%");          // Metric (millimeters)
  lines.push("%LPD*%");           // Layer polarity: dark

  // Aperture definitions
  for (const def of apertures.definitions()) {
    lines.push(def);
  }

  // --- Draw trace segments ---

  for (const seg of layerSegments) {
    const dc = apertures.getOrCreate("C", [seg.width]);
    lines.push(`D${dc}*`);
    lines.push(`${xy(seg.start.x, seg.start.y)}D02*`);  // Move
    lines.push(`${xy(seg.end.x, seg.end.y)}D01*`);      // Draw
  }

  // --- Flash pads ---

  for (const { pad, pos } of layerPads) {
    let dc: number;
    if (pad.shape === "circle") {
      dc = apertures.getOrCreate("C", [pad.size[0]]);
    } else {
      dc = apertures.getOrCreate("R", [pad.size[0], pad.size[1]]);
    }
    lines.push(`D${dc}*`);
    lines.push(`${xy(pos.x, pos.y)}D03*`);  // Flash
  }

  // --- Flash vias ---

  for (const via of layerVias) {
    const dc = apertures.getOrCreate("C", [via.diameter]);
    lines.push(`D${dc}*`);
    lines.push(`${xy(via.position.x, via.position.y)}D03*`);  // Flash
  }

  // --- Draw zones (outline) ---

  for (const zone of layerZones) {
    const outline = zone.filled && zone.filled.length > 0
      ? zone.filled[0]
      : zone.outline;
    if (outline.length < 2) continue;

    const dc = apertures.getOrCreate("C", [zone.minThickness || 0.254]);
    lines.push(`D${dc}*`);

    // If filled polygons exist, draw them as region fills
    if (zone.filled && zone.filled.length > 0) {
      for (const poly of zone.filled) {
        if (poly.length < 3) continue;
        lines.push("G36*");  // Region fill on
        lines.push(`${xy(poly[0].x, poly[0].y)}D02*`);
        for (let i = 1; i < poly.length; i++) {
          lines.push(`${xy(poly[i].x, poly[i].y)}D01*`);
        }
        lines.push(`${xy(poly[0].x, poly[0].y)}D01*`);  // Close
        lines.push("G37*");  // Region fill off
      }
    } else {
      // No fill data — draw outline only
      lines.push(`${xy(outline[0].x, outline[0].y)}D02*`);
      for (let i = 1; i < outline.length; i++) {
        lines.push(`${xy(outline[i].x, outline[i].y)}D01*`);
      }
      lines.push(`${xy(outline[0].x, outline[0].y)}D01*`);  // Close
    }
  }

  // --- Draw footprint graphics ---

  for (const { g, fp } of layerGraphics) {
    const dc = apertures.getOrCreate("C", [graphicWidth(g)]);
    lines.push(`D${dc}*`);
    emitGraphic(lines, g, fp);
  }

  // --- Draw board-level graphics ---

  for (const g of boardGraphics) {
    const dc = apertures.getOrCreate("C", [graphicWidth(g)]);
    lines.push(`D${dc}*`);
    emitGraphicAbsolute(lines, g);
  }

  // --- Footer ---

  lines.push("M02*");

  return lines.join("\n") + "\n";
}

/** Emit Gerber draw commands for a footprint graphic, applying footprint transform. */
function emitGraphic(lines: string[], g: DrawableGraphic, fp: PcbFootprint): void {
  const transform = (p: PcbPoint): PcbPoint => {
    const rad = (fp.rotation * Math.PI) / 180;
    const cos = Math.cos(rad);
    const sin = Math.sin(rad);
    return {
      x: fp.position.x + p.x * cos - p.y * sin,
      y: fp.position.y + p.x * sin + p.y * cos,
    };
  };

  switch (g.type) {
    case "line": {
      const s = transform(g.start);
      const e = transform(g.end);
      lines.push(`${xy(s.x, s.y)}D02*`);
      lines.push(`${xy(e.x, e.y)}D01*`);
      break;
    }
    case "rect": {
      const s = transform(g.start);
      const e = transform(g.end);
      const tl = { x: s.x, y: s.y };
      const tr = { x: e.x, y: s.y };
      const br = { x: e.x, y: e.y };
      const bl = { x: s.x, y: e.y };
      lines.push(`${xy(tl.x, tl.y)}D02*`);
      lines.push(`${xy(tr.x, tr.y)}D01*`);
      lines.push(`${xy(br.x, br.y)}D01*`);
      lines.push(`${xy(bl.x, bl.y)}D01*`);
      lines.push(`${xy(tl.x, tl.y)}D01*`);
      break;
    }
    case "circle": {
      const c = transform(g.center);
      // Approximate circle with line segments
      emitCircleApprox(lines, c.x, c.y, g.radius);
      break;
    }
    case "arc": {
      const s = transform(g.start);
      const m = transform(g.mid);
      const e = transform(g.end);
      // Approximate arc through 3 points with line segments
      emitArcApprox(lines, s, m, e);
      break;
    }
    case "poly": {
      if (g.points.length < 2) break;
      const pts = g.points.map(transform);
      lines.push(`${xy(pts[0].x, pts[0].y)}D02*`);
      for (let i = 1; i < pts.length; i++) {
        lines.push(`${xy(pts[i].x, pts[i].y)}D01*`);
      }
      if (g.fill) {
        lines.push(`${xy(pts[0].x, pts[0].y)}D01*`);
      }
      break;
    }
  }
}

/** Emit Gerber draw commands for a board-level graphic (absolute coords). */
function emitGraphicAbsolute(lines: string[], g: DrawableGraphic): void {
  switch (g.type) {
    case "line":
      lines.push(`${xy(g.start.x, g.start.y)}D02*`);
      lines.push(`${xy(g.end.x, g.end.y)}D01*`);
      break;
    case "rect": {
      const s = g.start, e = g.end;
      lines.push(`${xy(s.x, s.y)}D02*`);
      lines.push(`${xy(e.x, s.y)}D01*`);
      lines.push(`${xy(e.x, e.y)}D01*`);
      lines.push(`${xy(s.x, e.y)}D01*`);
      lines.push(`${xy(s.x, s.y)}D01*`);
      break;
    }
    case "circle":
      emitCircleApprox(lines, g.center.x, g.center.y, g.radius);
      break;
    case "arc":
      emitArcApprox(lines, g.start, g.mid, g.end);
      break;
    case "poly": {
      if (g.points.length < 2) break;
      lines.push(`${xy(g.points[0].x, g.points[0].y)}D02*`);
      for (let i = 1; i < g.points.length; i++) {
        lines.push(`${xy(g.points[i].x, g.points[i].y)}D01*`);
      }
      if (g.fill) {
        lines.push(`${xy(g.points[0].x, g.points[0].y)}D01*`);
      }
      break;
    }
  }
}

/** Approximate a circle with 36 line segments. */
function emitCircleApprox(lines: string[], cx: number, cy: number, r: number): void {
  const n = 36;
  const step = (2 * Math.PI) / n;
  const firstX = cx + r;
  const firstY = cy;
  lines.push(`${xy(firstX, firstY)}D02*`);
  for (let i = 1; i <= n; i++) {
    const a = i * step;
    lines.push(`${xy(cx + r * Math.cos(a), cy + r * Math.sin(a))}D01*`);
  }
}

/** Approximate an arc through 3 points with line segments. */
function emitArcApprox(lines: string[], start: PcbPoint, mid: PcbPoint, end: PcbPoint): void {
  // Find circle center from 3 points
  const d = 2 * (start.x * (mid.y - end.y) + mid.x * (end.y - start.y) + end.x * (start.y - mid.y));
  if (Math.abs(d) < 1e-10) {
    // Degenerate: draw straight line
    lines.push(`${xy(start.x, start.y)}D02*`);
    lines.push(`${xy(end.x, end.y)}D01*`);
    return;
  }
  const ux = ((start.x ** 2 + start.y ** 2) * (mid.y - end.y) +
    (mid.x ** 2 + mid.y ** 2) * (end.y - start.y) +
    (end.x ** 2 + end.y ** 2) * (start.y - mid.y)) / d;
  const uy = ((start.x ** 2 + start.y ** 2) * (end.x - mid.x) +
    (mid.x ** 2 + mid.y ** 2) * (start.x - end.x) +
    (end.x ** 2 + end.y ** 2) * (mid.x - start.x)) / d;
  const r = Math.hypot(start.x - ux, start.y - uy);

  const a1 = Math.atan2(start.y - uy, start.x - ux);
  const aMid = Math.atan2(mid.y - uy, mid.x - ux);
  const a2 = Math.atan2(end.y - uy, end.x - ux);

  // Determine sweep direction
  const normalize = (a: number) => ((a % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  const n1 = normalize(a1), nM = normalize(aMid), n2 = normalize(a2);
  const ccw = n1 < n2 ? (nM >= n1 && nM <= n2) : !(nM >= n2 && nM <= n1);

  // Draw with line segments
  const n = 36;
  let sweep = ccw ? normalize(a2 - a1) : -normalize(a1 - a2);
  if (Math.abs(sweep) < 1e-6) sweep = ccw ? 2 * Math.PI : -2 * Math.PI;

  lines.push(`${xy(start.x, start.y)}D02*`);
  for (let i = 1; i <= n; i++) {
    const t = i / n;
    const a = a1 + sweep * t;
    lines.push(`${xy(ux + r * Math.cos(a), uy + r * Math.sin(a))}D01*`);
  }
}

// --- Excellon Drill File ---

interface DrillHole {
  x: number;
  y: number;
  diameter: number;
}

/**
 * Generate an Excellon drill file from PCB data.
 *
 * Collects all through-hole pad drills and via drills, groups them by
 * diameter, and outputs in Excellon 2 format with metric coordinates.
 */
export function generateDrillFile(data: PcbData): string {
  const holes: DrillHole[] = [];

  // Collect via drill holes
  for (const via of data.vias) {
    holes.push({
      x: via.position.x,
      y: via.position.y,
      diameter: via.drill,
    });
  }

  // Collect through-hole pad drill holes
  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      if ((pad.type === "thru_hole" || pad.type === "np_thru_hole") && pad.drill) {
        const pos = absolutePadPosition(pad, fp);
        holes.push({
          x: pos.x,
          y: pos.y,
          diameter: pad.drill.diameter,
        });
      }
    }
  }

  if (holes.length === 0) {
    return [
      "M48",
      "; Signex EDA Drill Export",
      "METRIC,TZ",
      "%",
      "M30",
    ].join("\n") + "\n";
  }

  // Group by unique drill diameter
  const diameterSet = new Set<number>();
  for (const h of holes) {
    diameterSet.add(round3(h.diameter));
  }
  const diameters = Array.from(diameterSet).sort((a, b) => a - b);

  // Assign tool numbers
  const toolMap = new Map<number, number>();
  diameters.forEach((d, i) => {
    toolMap.set(d, i + 1);
  });

  const lines: string[] = [];

  // --- Header ---
  lines.push("M48");
  lines.push("; Signex EDA Drill Export");
  lines.push("METRIC,TZ");
  lines.push("FMAT,2");

  // Tool definitions
  for (const d of diameters) {
    const toolNum = toolMap.get(d)!;
    const fmtDiam = d.toFixed(3);
    lines.push(`T${String(toolNum).padStart(2, "0")}C${fmtDiam}`);
  }

  lines.push("%");  // End of header

  // --- Drill hits ---
  for (const d of diameters) {
    const toolNum = toolMap.get(d)!;
    lines.push(`T${String(toolNum).padStart(2, "0")}`);

    const matchingHoles = holes.filter((h) => round3(h.diameter) === d);
    for (const h of matchingHoles) {
      lines.push(drillXY(h.x, h.y));
    }
  }

  // --- Footer ---
  lines.push("M30");

  return lines.join("\n") + "\n";
}

// --- Full Fabrication Set ---

/** Layer-to-filename mapping for Gerber output. */
const GERBER_LAYERS: { layer: PcbLayerId; suffix: string }[] = [
  { layer: "F.Cu", suffix: "-F_Cu.gbr" },
  { layer: "B.Cu", suffix: "-B_Cu.gbr" },
  { layer: "F.SilkS", suffix: "-F_SilkS.gbr" },
  { layer: "B.SilkS", suffix: "-B_SilkS.gbr" },
  { layer: "F.Mask", suffix: "-F_Mask.gbr" },
  { layer: "B.Mask", suffix: "-B_Mask.gbr" },
  { layer: "Edge.Cuts", suffix: "-Edge_Cuts.gbr" },
];

/**
 * Generate a complete Gerber fabrication file set.
 *
 * Returns an array of { filename, content } objects for:
 * - Copper layers (F.Cu, B.Cu)
 * - Silkscreen layers (F.SilkS, B.SilkS)
 * - Solder mask layers (F.Mask, B.Mask)
 * - Board outline (Edge.Cuts)
 * - Drill file (Excellon)
 *
 * Filename prefix is derived from the board generator or defaults to "board".
 */
export function exportGerberSet(data: PcbData): { filename: string; content: string }[] {
  const rawPrefix = data.board.generator || "board";
  const prefix = rawPrefix.replace(/[/\\:*?"<>|]/g, "_").slice(0, 64);
  const files: { filename: string; content: string }[] = [];

  for (const { layer, suffix } of GERBER_LAYERS) {
    const content = generateGerber(data, layer);
    files.push({
      filename: `${prefix}${suffix}`,
      content,
    });
  }

  // Drill file
  files.push({
    filename: `${prefix}.drl`,
    content: generateDrillFile(data),
  });

  return files;
}
