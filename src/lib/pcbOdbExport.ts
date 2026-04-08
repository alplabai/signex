// ===============================================================
// ODB++ Export — Directory-based fabrication format
// ===============================================================

import type {
  PcbData,
  PcbLayerId,
  PcbPad,
  PcbFootprint,
  PcbPoint,
  PcbGraphic,
  PcbSegment,
  PcbVia,
  PcbZone,
} from "@/types/pcb";

// --- Coordinate helpers ---

/** Format a number to 6 decimal places for ODB features. */
function fmt(v: number): string {
  return v.toFixed(6);
}

/** Compute absolute pad position given footprint transform. */
function absolutePadPos(pad: PcbPad, fp: PcbFootprint): PcbPoint {
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
  if (pad.layers.includes("*.Cu") && (layer as string).endsWith(".Cu")) return true;
  return false;
}

// --- ODB symbol table ---

/** Manages ODB aperture/symbol definitions (r0, rect0x0, etc.). */
class SymbolTable {
  private map = new Map<string, number>();
  private entries: string[] = [];

  /** Get or create a round symbol, returns symbol index. */
  round(diameter: number): number {
    const key = `r${diameter.toFixed(4)}`;
    return this.getOrCreate(key);
  }

  /** Get or create a rectangular symbol, returns symbol index. */
  rect(width: number, height: number): number {
    const key = `rect${width.toFixed(4)}x${height.toFixed(4)}`;
    return this.getOrCreate(key);
  }

  private getOrCreate(key: string): number {
    const existing = this.map.get(key);
    if (existing !== undefined) return existing;
    const idx = this.entries.length;
    this.entries.push(key);
    this.map.set(key, idx);
    return idx;
  }

  /** Emit the symbol definition block for the features file header. */
  definitions(): string[] {
    return this.entries.map((name, i) => `$${i} ${name}`);
  }
}

// --- Layer mapping ---

interface OdbLayerInfo {
  name: string;
  context: "BOARD";
  type: "SIGNAL" | "SOLDER_MASK" | "SILK_SCREEN" | "SOLDER_PASTE" | "DOCUMENT" | "DRILL";
  polarity: "POSITIVE" | "NEGATIVE";
  pcbLayer: PcbLayerId;
}

/** Map KiCad layer IDs to ODB layer definitions. */
function buildLayerMap(data: PcbData): OdbLayerInfo[] {
  const layers: OdbLayerInfo[] = [];

  // Copper layers in stack order
  const copperLayers = data.board.layers.layers
    .filter((l) => (l.id as string).endsWith(".Cu"));
  for (const cl of copperLayers) {
    layers.push({
      name: cl.id.replace(".", "_"),
      context: "BOARD",
      type: "SIGNAL",
      polarity: "POSITIVE",
      pcbLayer: cl.id,
    });
  }

  // Solder mask
  const maskLayers: { id: PcbLayerId; name: string }[] = [
    { id: "F.Mask", name: "F_Mask" },
    { id: "B.Mask", name: "B_Mask" },
  ];
  for (const ml of maskLayers) {
    if (data.board.layers.layers.some((l) => l.id === ml.id)) {
      layers.push({
        name: ml.name,
        context: "BOARD",
        type: "SOLDER_MASK",
        polarity: "POSITIVE",
        pcbLayer: ml.id,
      });
    }
  }

  // Silkscreen
  const silkLayers: { id: PcbLayerId; name: string }[] = [
    { id: "F.SilkS", name: "F_SilkS" },
    { id: "B.SilkS", name: "B_SilkS" },
  ];
  for (const sl of silkLayers) {
    if (data.board.layers.layers.some((l) => l.id === sl.id)) {
      layers.push({
        name: sl.name,
        context: "BOARD",
        type: "SILK_SCREEN",
        polarity: "POSITIVE",
        pcbLayer: sl.id,
      });
    }
  }

  // Paste
  const pasteLayers: { id: PcbLayerId; name: string }[] = [
    { id: "F.Paste", name: "F_Paste" },
    { id: "B.Paste", name: "B_Paste" },
  ];
  for (const pl of pasteLayers) {
    if (data.board.layers.layers.some((l) => l.id === pl.id)) {
      layers.push({
        name: pl.name,
        context: "BOARD",
        type: "SOLDER_PASTE",
        polarity: "POSITIVE",
        pcbLayer: pl.id,
      });
    }
  }

  return layers;
}

// --- Matrix file ---

function generateMatrix(layerMap: OdbLayerInfo[]): string {
  const lines: string[] = [];

  lines.push("STEP {");
  lines.push("   COL_NAME=pcb");
  lines.push("");

  for (let i = 0; i < layerMap.length; i++) {
    const layer = layerMap[i];
    lines.push(`   ROW=${i}`);
    lines.push(`   CONTEXT=${layer.context}`);
    lines.push(`   TYPE=${layer.type}`);
    lines.push(`   NAME=${layer.name}`);
    lines.push(`   POLARITY=${layer.polarity}`);
    lines.push(`   START_NAME=`);
    lines.push(`   END_NAME=`);
    lines.push(`   OLD_NAME=`);
    lines.push("");
  }

  lines.push("}");
  return lines.join("\n");
}

// --- Misc/info file ---

function generateMiscInfo(data: PcbData): string {
  const lines: string[] = [];
  lines.push("UNITS=MM");
  lines.push(`JOB_NAME=${data.board.generator || "board"}`);
  lines.push("ODB_VERSION_MAJOR=8");
  lines.push("ODB_VERSION_MINOR=1");
  lines.push(`CREATION_DATE=${new Date().toISOString().slice(0, 10)}`);
  lines.push("SAVE_APP=Signex EDA");
  lines.push("SAVE_APP_VERSION=1.0");
  lines.push(`MAX_UID=${data.footprints.length + data.segments.length + data.vias.length}`);
  return lines.join("\n");
}

// --- Board profile ---

function generateProfile(data: PcbData): string {
  const outline = data.board.outline;
  if (outline.length < 3) {
    // Fallback: generate a default 100x80mm board profile
    return [
      "OB 0.000000 0.000000 I",
      "OS 100.000000 0.000000",
      "OS 100.000000 80.000000",
      "OS 0.000000 80.000000",
      "OE",
    ].join("\n");
  }

  const lines: string[] = [];
  // ODB profile: OB = begin outline, OS = outline segment, OE = end outline
  lines.push(`OB ${fmt(outline[0].x)} ${fmt(outline[0].y)} I`);
  for (let i = 1; i < outline.length; i++) {
    lines.push(`OS ${fmt(outline[i].x)} ${fmt(outline[i].y)}`);
  }
  // Close back to start
  lines.push(`OS ${fmt(outline[0].x)} ${fmt(outline[0].y)}`);
  lines.push("OE");

  return lines.join("\n");
}

// --- Per-layer features ---

type DrawableGraphic = Exclude<PcbGraphic, { type: "text" }>;

function isDrawableGraphic(g: PcbGraphic): g is DrawableGraphic {
  return g.type !== "text";
}

function generateLayerFeatures(
  data: PcbData,
  layer: PcbLayerId,
): string {
  const symbols = new SymbolTable();
  const featureLines: string[] = [];

  // --- Trace segments ---
  const layerSegments = data.segments.filter((s: PcbSegment) => s.layer === layer);
  for (const seg of layerSegments) {
    const sym = symbols.round(seg.width);
    featureLines.push(
      `L ${fmt(seg.start.x)} ${fmt(seg.start.y)} ${fmt(seg.end.x)} ${fmt(seg.end.y)} ${sym} P 0`
    );
  }

  // --- Pads ---
  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      if (!padOnLayer(pad, layer)) continue;
      const pos = absolutePadPos(pad, fp);
      let sym: number;
      if (pad.shape === "circle") {
        sym = symbols.round(pad.size[0]);
      } else {
        sym = symbols.rect(pad.size[0], pad.size[1]);
      }
      featureLines.push(
        `P ${fmt(pos.x)} ${fmt(pos.y)} ${sym} P 0 0 0`
      );
    }
  }

  // --- Vias (on copper layers) ---
  if ((layer as string).endsWith(".Cu")) {
    for (const via of data.vias as PcbVia[]) {
      if (via.type === "through" || via.layers.includes(layer)) {
        const sym = symbols.round(via.diameter);
        featureLines.push(
          `P ${fmt(via.position.x)} ${fmt(via.position.y)} ${sym} P 0 0 0`
        );
      }
    }
  }

  // --- Zones (draw outlines as line segments) ---
  const layerZones = data.zones.filter((z: PcbZone) => z.layer === layer);
  for (const zone of layerZones) {
    const polys = zone.filled && zone.filled.length > 0 ? zone.filled : [zone.outline];
    const sym = symbols.round(zone.minThickness || 0.254);
    for (const poly of polys) {
      if (poly.length < 2) continue;
      for (let i = 0; i < poly.length; i++) {
        const p1 = poly[i];
        const p2 = poly[(i + 1) % poly.length];
        featureLines.push(
          `L ${fmt(p1.x)} ${fmt(p1.y)} ${fmt(p2.x)} ${fmt(p2.y)} ${sym} P 0`
        );
      }
    }
  }

  // --- Footprint graphics ---
  for (const fp of data.footprints) {
    for (const g of fp.graphics) {
      if (g.layer !== layer || !isDrawableGraphic(g)) continue;
      const sym = symbols.round(g.width || 0.127);
      emitGraphicFeatures(featureLines, g, fp, sym);
    }
  }

  // --- Board-level graphics ---
  for (const g of data.graphics) {
    if (g.layer !== layer || !isDrawableGraphic(g)) continue;
    const sym = symbols.round(g.width || 0.127);
    emitAbsoluteGraphicFeatures(featureLines, g, sym);
  }

  // --- Assemble output ---
  const header: string[] = [];
  header.push("#");
  header.push("# Signex EDA ODB++ Feature File");
  header.push(`# Layer: ${layer}`);
  header.push("#");
  header.push("UNITS=MM");
  header.push("");

  // Symbol definitions
  const symDefs = symbols.definitions();
  for (const sd of symDefs) {
    header.push(sd);
  }

  header.push("");
  header.push("# Features");

  return [...header, ...featureLines].join("\n");
}

/** Emit ODB feature lines for a footprint graphic, applying footprint transform. */
function emitGraphicFeatures(
  lines: string[],
  g: DrawableGraphic,
  fp: PcbFootprint,
  sym: number,
): void {
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
      lines.push(`L ${fmt(s.x)} ${fmt(s.y)} ${fmt(e.x)} ${fmt(e.y)} ${sym} P 0`);
      break;
    }
    case "rect": {
      const s = transform(g.start);
      const e = transform(g.end);
      const tl = s;
      const tr = { x: e.x, y: s.y };
      const br = e;
      const bl = { x: s.x, y: e.y };
      lines.push(`L ${fmt(tl.x)} ${fmt(tl.y)} ${fmt(tr.x)} ${fmt(tr.y)} ${sym} P 0`);
      lines.push(`L ${fmt(tr.x)} ${fmt(tr.y)} ${fmt(br.x)} ${fmt(br.y)} ${sym} P 0`);
      lines.push(`L ${fmt(br.x)} ${fmt(br.y)} ${fmt(bl.x)} ${fmt(bl.y)} ${sym} P 0`);
      lines.push(`L ${fmt(bl.x)} ${fmt(bl.y)} ${fmt(tl.x)} ${fmt(tl.y)} ${sym} P 0`);
      break;
    }
    case "circle": {
      const c = transform(g.center);
      emitCircleFeatures(lines, c.x, c.y, g.radius, sym);
      break;
    }
    case "arc": {
      const s = transform(g.start);
      const m = transform(g.mid);
      const e = transform(g.end);
      emitArcFeatures(lines, s, m, e, sym);
      break;
    }
    case "poly": {
      if (g.points.length < 2) break;
      const pts = g.points.map(transform);
      for (let i = 0; i < pts.length - 1; i++) {
        lines.push(
          `L ${fmt(pts[i].x)} ${fmt(pts[i].y)} ${fmt(pts[i + 1].x)} ${fmt(pts[i + 1].y)} ${sym} P 0`
        );
      }
      if (g.fill && pts.length >= 3) {
        lines.push(
          `L ${fmt(pts[pts.length - 1].x)} ${fmt(pts[pts.length - 1].y)} ${fmt(pts[0].x)} ${fmt(pts[0].y)} ${sym} P 0`
        );
      }
      break;
    }
  }
}

/** Emit ODB feature lines for a board-level graphic (absolute coords). */
function emitAbsoluteGraphicFeatures(
  lines: string[],
  g: DrawableGraphic,
  sym: number,
): void {
  switch (g.type) {
    case "line":
      lines.push(`L ${fmt(g.start.x)} ${fmt(g.start.y)} ${fmt(g.end.x)} ${fmt(g.end.y)} ${sym} P 0`);
      break;
    case "rect": {
      const s = g.start, e = g.end;
      lines.push(`L ${fmt(s.x)} ${fmt(s.y)} ${fmt(e.x)} ${fmt(s.y)} ${sym} P 0`);
      lines.push(`L ${fmt(e.x)} ${fmt(s.y)} ${fmt(e.x)} ${fmt(e.y)} ${sym} P 0`);
      lines.push(`L ${fmt(e.x)} ${fmt(e.y)} ${fmt(s.x)} ${fmt(e.y)} ${sym} P 0`);
      lines.push(`L ${fmt(s.x)} ${fmt(e.y)} ${fmt(s.x)} ${fmt(s.y)} ${sym} P 0`);
      break;
    }
    case "circle":
      emitCircleFeatures(lines, g.center.x, g.center.y, g.radius, sym);
      break;
    case "arc":
      emitArcFeatures(lines, g.start, g.mid, g.end, sym);
      break;
    case "poly":
      if (g.points.length < 2) break;
      for (let i = 0; i < g.points.length - 1; i++) {
        lines.push(
          `L ${fmt(g.points[i].x)} ${fmt(g.points[i].y)} ${fmt(g.points[i + 1].x)} ${fmt(g.points[i + 1].y)} ${sym} P 0`
        );
      }
      if (g.fill && g.points.length >= 3) {
        const last = g.points[g.points.length - 1];
        lines.push(
          `L ${fmt(last.x)} ${fmt(last.y)} ${fmt(g.points[0].x)} ${fmt(g.points[0].y)} ${sym} P 0`
        );
      }
      break;
  }
}

/** Approximate a circle with ODB arc feature. */
function emitCircleFeatures(
  lines: string[],
  cx: number,
  cy: number,
  r: number,
  sym: number,
): void {
  // ODB arc: A xs ys xe ye xc yc sym P cw
  // Full circle: start = end = (cx+r, cy), center = (cx, cy), clockwise
  lines.push(
    `A ${fmt(cx + r)} ${fmt(cy)} ${fmt(cx + r)} ${fmt(cy)} ${fmt(cx)} ${fmt(cy)} ${sym} P Y`
  );
}

/** Approximate an arc through 3 points with ODB arc feature. */
function emitArcFeatures(
  lines: string[],
  start: PcbPoint,
  mid: PcbPoint,
  end: PcbPoint,
  sym: number,
): void {
  // Find circle center from 3 points
  const d = 2 * (start.x * (mid.y - end.y) + mid.x * (end.y - start.y) + end.x * (start.y - mid.y));
  if (Math.abs(d) < 1e-10) {
    // Degenerate arc: emit as line
    lines.push(`L ${fmt(start.x)} ${fmt(start.y)} ${fmt(end.x)} ${fmt(end.y)} ${sym} P 0`);
    return;
  }
  const cx = ((start.x ** 2 + start.y ** 2) * (mid.y - end.y) +
    (mid.x ** 2 + mid.y ** 2) * (end.y - start.y) +
    (end.x ** 2 + end.y ** 2) * (start.y - mid.y)) / d;
  const cy = ((start.x ** 2 + start.y ** 2) * (end.x - mid.x) +
    (mid.x ** 2 + mid.y ** 2) * (start.x - end.x) +
    (end.x ** 2 + end.y ** 2) * (mid.x - start.x)) / d;

  // Determine CW/CCW direction
  const a1 = Math.atan2(start.y - cy, start.x - cx);
  const aMid = Math.atan2(mid.y - cy, mid.x - cx);
  const a2 = Math.atan2(end.y - cy, end.x - cx);
  const normalize = (a: number) => ((a % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  const n1 = normalize(a1), nM = normalize(aMid), n2 = normalize(a2);
  const ccw = n1 < n2 ? (nM >= n1 && nM <= n2) : !(nM >= n2 && nM <= n1);
  const cw = ccw ? "N" : "Y";

  lines.push(
    `A ${fmt(start.x)} ${fmt(start.y)} ${fmt(end.x)} ${fmt(end.y)} ${fmt(cx)} ${fmt(cy)} ${sym} P ${cw}`
  );
}

// --- Component list ---

function generateComponentList(data: PcbData, side: "top" | "bottom"): string {
  const layer = side === "top" ? "F.Cu" : "B.Cu";
  const lines: string[] = [];

  lines.push("#");
  lines.push(`# Component list — ${side}`);
  lines.push("#");
  lines.push("UNITS=MM");
  lines.push("");

  const footprints = data.footprints.filter((fp) => fp.layer === layer);
  for (const fp of footprints) {
    lines.push(`CMP ${fp.pads.length} ${fp.position.x.toFixed(4)} ${fp.position.y.toFixed(4)} ${fp.rotation.toFixed(1)} N ;0;0`);
    const escPrp = (s: string) => s.replace(/'/g, "\\'");
    lines.push(`PRP REF '${escPrp(fp.reference)}'`);
    lines.push(`PRP VAL '${escPrp(fp.value)}'`);
    lines.push(`PRP PKG '${escPrp(fp.footprintId)}'`);
    // Pad records
    for (const pad of fp.pads) {
      const pos = absolutePadPos(pad, fp);
      const netName = pad.net?.name || "";
      lines.push(`TOP ${pad.number} ${pos.x.toFixed(4)} ${pos.y.toFixed(4)} ${pad.type === "smd" ? "S" : "T"} ${netName}`);
    }
    lines.push("");
  }

  return lines.join("\n");
}

// --- Public API ---

/**
 * Generate ODB++ directory structure as an array of {filename, content} entries.
 *
 * ODB++ is a directory-based fabrication format. Each entry represents a file
 * in the ODB++ archive structure. The files can be packed into a .tgz or
 * written to a directory tree.
 */
export function generateOdbPlusPlus(data: PcbData): { filename: string; content: string }[] {
  const files: { filename: string; content: string }[] = [];
  const layerMap = buildLayerMap(data);

  // 1. Matrix — layer stack definition
  files.push({
    filename: "matrix/matrix",
    content: generateMatrix(layerMap),
  });

  // 2. Misc/info — job information
  files.push({
    filename: "misc/info",
    content: generateMiscInfo(data),
  });

  // 3. Board profile
  files.push({
    filename: "steps/pcb/profile",
    content: generateProfile(data),
  });

  // 4. Per-layer features
  for (const layer of layerMap) {
    files.push({
      filename: `steps/pcb/layers/${layer.name}/features`,
      content: generateLayerFeatures(data, layer.pcbLayer),
    });
  }

  // 5. Component lists (top and bottom)
  files.push({
    filename: "steps/pcb/layers/comp_+_top/components",
    content: generateComponentList(data, "top"),
  });
  files.push({
    filename: "steps/pcb/layers/comp_+_bot/components",
    content: generateComponentList(data, "bottom"),
  });

  // 6. Net list
  files.push({
    filename: "steps/pcb/netlists/cadnet/netlist",
    content: generateNetlist(data),
  });

  return files;
}

/** Generate ODB++ netlist file. */
function generateNetlist(data: PcbData): string {
  const lines: string[] = [];
  lines.push("#");
  lines.push("# ODB++ Netlist");
  lines.push("#");
  lines.push("H optimize n");
  lines.push("");

  for (const net of data.nets) {
    if (net.number === 0) continue;
    lines.push(`$${net.number} ${net.name}`);

    // List pads connected to this net
    for (const fp of data.footprints) {
      for (const pad of fp.pads) {
        if (pad.net?.number === net.number) {
          lines.push(`  ${fp.reference} ${pad.number}`);
        }
      }
    }

    lines.push("");
  }

  return lines.join("\n");
}
