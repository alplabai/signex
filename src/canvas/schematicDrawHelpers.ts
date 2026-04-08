/**
 * schematicDrawHelpers.ts
 *
 * Pure drawing helpers for SchematicRenderer. All functions are standalone
 * (no React hooks, no closure capture) and take only what they need as params.
 *
 * The `ctx` is always assumed to be in world-space (translate/scale applied by
 * the caller before invoking these helpers, restored after).
 */

import type {
  Graphic,
  SchematicData,
  SchPin,
  SchPoint,
  TextPropData,
} from "@/types";
import { substituteSpecialStrings } from "@/lib/specialStrings";

// ---------------------------------------------------------------------------
// Re-exported constants so SchematicRenderer.tsx can import from one place
// ---------------------------------------------------------------------------

export const PAPER: Record<string, [number, number]> = {
  A4: [297, 210], A3: [420, 297], A2: [594, 420], A1: [841, 594], A0: [1189, 841],
  A: [279.4, 215.9], B: [431.8, 279.4], C: [558.8, 431.8], D: [863.6, 558.8],
};

export const C = {
  bg: "#1a1b2e", paper: "#1e2035", paperBorder: "#2a2d4a",
  grid: "#2d3060", gridMajor: "#3a3f75",
  wire: "#4fc3f7", junction: "#4fc3f7",
  body: "#9fa8da", bodyFill: "#1e2035",
  pin: "#81c784", pinName: "#90a4ae", pinNum: "#607d8b",
  ref: "#e8c66a", val: "#9598b3",
  labelNet: "#81c784", labelGlobal: "#ff8a65", labelHier: "#ba68c8",
  sheet: "#5b8def", sheetText: "#cdd6f4",
  noConnect: "#e8667a", power: "#ef5350",
  selection: "#00bcd4", selectionFill: "rgba(0,188,212,0.06)",
  bus: "#4a86c8", busEntry: "#4a86c8",
  handleFill: "#4caf50", handleBorder: "#2e7d32",
};

// ---------------------------------------------------------------------------
// Small utilities
// ---------------------------------------------------------------------------

export const txt = (s: string) => s.replace(/\{slash\}/g, "/");

/** Transform a point from symbol-local (Y-up) to schematic (Y-down) space */
export function symToSch(
  lx: number, ly: number,
  sx: number, sy: number,
  rot: number, mx: boolean, my: boolean,
): [number, number] {
  const x = lx;
  const y = -ly;
  const rad = -(rot * Math.PI) / 180;
  const cos = Math.cos(rad);
  const sin = Math.sin(rad);
  let rx = x * cos - y * sin;
  let ry = x * sin + y * cos;
  if (mx) ry = -ry;
  if (my) rx = -rx;
  return [sx + rx, sy + ry];
}

/** Pin end position in symbol-local space */
export function pinEnd(pin: SchPin): SchPoint {
  const rad = (pin.rotation * Math.PI) / 180;
  return {
    x: pin.position.x + Math.cos(rad) * pin.length,
    y: pin.position.y + Math.sin(rad) * pin.length,
  };
}

export function arcCenter(p1: SchPoint, p2: SchPoint, p3: SchPoint): SchPoint | null {
  const d = 2 * (p1.x * (p2.y - p3.y) + p2.x * (p3.y - p1.y) + p3.x * (p1.y - p2.y));
  if (Math.abs(d) < 1e-10) return null;
  const ux = ((p1.x ** 2 + p1.y ** 2) * (p2.y - p3.y) + (p2.x ** 2 + p2.y ** 2) * (p3.y - p1.y) + (p3.x ** 2 + p3.y ** 2) * (p1.y - p2.y)) / d;
  const uy = ((p1.x ** 2 + p1.y ** 2) * (p3.x - p2.x) + (p2.x ** 2 + p2.y ** 2) * (p1.x - p3.x) + (p3.x ** 2 + p3.y ** 2) * (p2.x - p1.x)) / d;
  return { x: ux, y: uy };
}

export function isCounterClockwise(a1: number, aMid: number, a2: number): boolean {
  const norm = (a: number) => ((a % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  const n1 = norm(a1), nM = norm(aMid), n2 = norm(a2);
  return n1 < n2 ? !(nM >= n1 && nM <= n2) : (nM >= n2 && nM <= n1);
}

// ---------------------------------------------------------------------------
// Electrical snap
// ---------------------------------------------------------------------------

const ELECTRICAL_SNAP_RANGE = 2.0;

export function findNearestElectricalPoint(
  data: SchematicData, worldX: number, worldY: number,
): SchPoint | null {
  let bestDist = ELECTRICAL_SNAP_RANGE;
  let bestPoint: SchPoint | null = null;

  for (const sym of data.symbols) {
    const lib = data.lib_symbols[sym.lib_id];
    if (!lib) continue;
    for (const pin of lib.pins) {
      const pe = pinEnd(pin);
      const [px, py] = symToSch(pe.x, pe.y,
        sym.position.x, sym.position.y, sym.rotation, sym.mirror_x, sym.mirror_y);
      const d = Math.hypot(worldX - px, worldY - py);
      if (d < bestDist) { bestDist = d; bestPoint = { x: px, y: py }; }
    }
  }

  for (const wire of data.wires) {
    for (const pt of [wire.start, wire.end]) {
      const d = Math.hypot(worldX - pt.x, worldY - pt.y);
      if (d < bestDist) { bestDist = d; bestPoint = { x: pt.x, y: pt.y }; }
    }
  }

  return bestPoint;
}

// ---------------------------------------------------------------------------
// SelectionFilter / AutoFocus helper
// ---------------------------------------------------------------------------

export type SelectionFilter = Record<string, { visible?: boolean } | undefined>;

export function makeAlphaFor(
  sf: SelectionFilter,
  autoFocus: string[] | null,
) {
  const hasFocus = autoFocus !== null && autoFocus.length > 0;
  return (uuid: string, filterKey: string): number => {
    if (!sf[filterKey]?.visible) return 0.12;
    if (hasFocus && !autoFocus!.includes(uuid)) return 0.15;
    return 1;
  };
}

// ---------------------------------------------------------------------------
// Section 1 – Paper / background
// ---------------------------------------------------------------------------

export function drawPaper(
  ctx: CanvasRenderingContext2D,
  pw: number,
  ph: number,
): void {
  ctx.fillStyle = C.paper;
  ctx.fillRect(0, 0, pw, ph);
  ctx.strokeStyle = C.paperBorder;
  ctx.lineWidth = 0.3;
  ctx.strokeRect(0, 0, pw, ph);

  const zoneMargin = 5;
  const cols = Math.max(4, Math.floor(pw / 50));
  const rows = Math.max(2, Math.floor(ph / 50));
  const colW = (pw - zoneMargin * 2) / cols;
  const rowH = (ph - zoneMargin * 2) / rows;

  ctx.strokeStyle = C.paperBorder;
  ctx.lineWidth = 0.1;
  ctx.fillStyle = "#4a4d6a";
  ctx.font = "2px Roboto";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";

  for (let c = 0; c < cols; c++) {
    const x = zoneMargin + c * colW;
    ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, zoneMargin); ctx.stroke();
    ctx.fillText(String(c + 1), x + colW / 2, zoneMargin / 2);
    ctx.beginPath(); ctx.moveTo(x, ph - zoneMargin); ctx.lineTo(x, ph); ctx.stroke();
    ctx.fillText(String(c + 1), x + colW / 2, ph - zoneMargin / 2);
  }
  ctx.beginPath(); ctx.moveTo(pw - zoneMargin, 0); ctx.lineTo(pw - zoneMargin, zoneMargin); ctx.stroke();
  ctx.beginPath(); ctx.moveTo(pw - zoneMargin, ph - zoneMargin); ctx.lineTo(pw - zoneMargin, ph); ctx.stroke();

  for (let r = 0; r < rows; r++) {
    const y = zoneMargin + r * rowH;
    const letter = String.fromCharCode(65 + r);
    ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(zoneMargin, y); ctx.stroke();
    ctx.fillText(letter, zoneMargin / 2, y + rowH / 2);
    ctx.beginPath(); ctx.moveTo(pw - zoneMargin, y); ctx.lineTo(pw, y); ctx.stroke();
    ctx.fillText(letter, pw - zoneMargin / 2, y + rowH / 2);
  }
  ctx.beginPath(); ctx.moveTo(0, ph - zoneMargin); ctx.lineTo(zoneMargin, ph - zoneMargin); ctx.stroke();
  ctx.beginPath(); ctx.moveTo(pw - zoneMargin, ph - zoneMargin); ctx.lineTo(pw, ph - zoneMargin); ctx.stroke();

  ctx.strokeStyle = C.paperBorder;
  ctx.lineWidth = 0.15;
  ctx.strokeRect(zoneMargin, zoneMargin, pw - zoneMargin * 2, ph - zoneMargin * 2);
}

// ---------------------------------------------------------------------------
// Section 2 – Title block
// ---------------------------------------------------------------------------

export function drawTitleBlock(
  ctx: CanvasRenderingContext2D,
  pw: number,
  ph: number,
  titleBlock: Record<string, string>,
): void {
  const zoneMargin = 5;
  ctx.lineWidth = 0.15;
  ctx.strokeStyle = C.paperBorder;
  ctx.strokeRect(pw - 100, ph - 30, 100 - zoneMargin, 30 - zoneMargin);

  const tbx = pw - 100, tby = ph - 30;
  const tb = titleBlock || {};

  ctx.strokeStyle = C.paperBorder;
  ctx.lineWidth = 0.08;
  ctx.beginPath();
  ctx.moveTo(tbx, tby + 10); ctx.lineTo(tbx + 100, tby + 10);
  ctx.moveTo(tbx, tby + 20); ctx.lineTo(tbx + 100, tby + 20);
  ctx.moveTo(tbx + 50, tby); ctx.lineTo(tbx + 50, tby + 10);
  ctx.stroke();

  ctx.fillStyle = C.ref;
  ctx.font = "0.8px Roboto";
  ctx.textAlign = "left";
  ctx.textBaseline = "top";
  ctx.fillText("Title:", tbx + 1, tby + 1);
  ctx.fillText("Date:", tbx + 51, tby + 1);
  ctx.fillText("Rev:", tbx + 1, tby + 11);
  ctx.fillText("Company:", tbx + 51, tby + 11);

  ctx.fillStyle = C.val;
  ctx.font = "1.2px Roboto";
  ctx.textBaseline = "middle";
  ctx.fillText(tb.title || "", tbx + 8, tby + 5);
  ctx.fillText(tb.date || "", tbx + 58, tby + 5);
  ctx.fillText(tb.rev || "", tbx + 8, tby + 15);
  ctx.fillText(tb.company || "", tbx + 63, tby + 15);

  ctx.font = "bold 1.5px Roboto";
  ctx.fillText(tb.title || "", tbx + 2, tby + 25);
}

// ---------------------------------------------------------------------------
// Section 3 – Grid
// ---------------------------------------------------------------------------

export function drawGrid(
  ctx: CanvasRenderingContext2D,
  cam: { x: number; y: number; zoom: number },
  gridSize: number,
  w: number,
  h: number,
): void {
  if (gridSize * cam.zoom <= 2) return;

  const dotSize = gridSize * 0.04;
  const majDotSize = gridSize * 0.08;
  const gStartX = Math.floor(-cam.x / cam.zoom / gridSize) * gridSize;
  const gStartY = Math.floor(-cam.y / cam.zoom / gridSize) * gridSize;
  const gEndX = gStartX + w / cam.zoom + gridSize;
  const gEndY = gStartY + h / cam.zoom + gridSize;

  ctx.globalAlpha = 0.5;
  for (let gx = gStartX; gx <= gEndX; gx += gridSize) {
    const ix = Math.round(gx / gridSize);
    const majX = ix % 10 === 0;
    for (let gy = gStartY; gy <= gEndY; gy += gridSize) {
      const iy = Math.round(gy / gridSize);
      const majY = iy % 10 === 0;
      const maj = majX && majY;
      ctx.fillStyle = maj ? C.gridMajor : C.grid;
      const r = maj ? majDotSize : dotSize;
      ctx.fillRect(gx - r, gy - r, r * 2, r * 2);
    }
  }
  ctx.globalAlpha = 1;
}

// ---------------------------------------------------------------------------
// Section 4 – Wires
// ---------------------------------------------------------------------------

export function drawWires(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  alphaFor: (uuid: string, filterKey: string) => number,
): void {
  ctx.strokeStyle = C.wire;
  ctx.lineWidth = 0.2;
  ctx.lineCap = "round";
  for (const wire of data.wires) {
    ctx.globalAlpha = alphaFor(wire.uuid, "wires");
    ctx.beginPath();
    ctx.moveTo(wire.start.x, wire.start.y);
    ctx.lineTo(wire.end.x, wire.end.y);
    ctx.stroke();
  }
  ctx.globalAlpha = 1;
  ctx.lineCap = "butt";
}

// ---------------------------------------------------------------------------
// Section 5 – Junctions
// ---------------------------------------------------------------------------

export function drawJunctions(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  sf: SelectionFilter,
): void {
  ctx.fillStyle = C.junction;
  ctx.globalAlpha = sf.junctions?.visible === false ? 0.12 : 1;
  for (const j of data.junctions) {
    ctx.beginPath();
    ctx.arc(j.position.x, j.position.y, 0.3, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.globalAlpha = 1;
}

// ---------------------------------------------------------------------------
// Section 6 – No-connects + No-ERC + Parameter Sets + Diff Pair + Blankets +
//              Compile Masks + Notes (all the "annotation" overlay objects)
// ---------------------------------------------------------------------------

export function drawNoConnects(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
  sf: SelectionFilter,
): void {
  ctx.lineWidth = 0.2;
  ctx.globalAlpha = sf.noConnects?.visible === false ? 0.12 : 1;
  for (const nc of data.no_connects) {
    const sel = selectedIds.has(nc.uuid);
    ctx.strokeStyle = sel ? C.selection : C.noConnect;
    ctx.beginPath();
    ctx.moveTo(nc.position.x - 0.7, nc.position.y - 0.7); ctx.lineTo(nc.position.x + 0.7, nc.position.y + 0.7);
    ctx.moveTo(nc.position.x + 0.7, nc.position.y - 0.7); ctx.lineTo(nc.position.x - 0.7, nc.position.y + 0.7);
    ctx.stroke();
  }
  ctx.globalAlpha = 1;
}

export function drawNoErcDirectives(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
): void {
  if (!data.no_erc_directives) return;
  for (const d of data.no_erc_directives) {
    const sel = selectedIds.has(d.uuid);
    ctx.strokeStyle = sel ? C.selection : "#66bb6a";
    ctx.fillStyle = sel ? C.selectionFill : "rgba(102,187,106,0.15)";
    ctx.lineWidth = 0.15;
    ctx.beginPath();
    ctx.arc(d.position.x, d.position.y, 0.5, 0, Math.PI * 2);
    ctx.fill(); ctx.stroke();
    ctx.strokeStyle = sel ? C.selection : "#66bb6a";
    ctx.lineWidth = 0.15;
    ctx.beginPath();
    ctx.moveTo(d.position.x - 0.2, d.position.y);
    ctx.lineTo(d.position.x - 0.05, d.position.y + 0.2);
    ctx.lineTo(d.position.x + 0.25, d.position.y - 0.15);
    ctx.stroke();
  }
}

export function drawParameterSets(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
): void {
  if (!data.parameter_sets) return;
  for (const ps of data.parameter_sets) {
    const sel = selectedIds.has(ps.uuid);
    const px = ps.position.x, py = ps.position.y;
    ctx.strokeStyle = sel ? C.selection : "#ab47bc";
    ctx.fillStyle = sel ? C.selectionFill : "rgba(171,71,188,0.12)";
    ctx.lineWidth = 0.15;
    ctx.fillRect(px - 0.8, py - 0.6, 1.6, 1.2);
    ctx.strokeRect(px - 0.8, py - 0.6, 1.6, 1.2);
    ctx.beginPath();
    ctx.moveTo(px - 0.8, py - 0.15); ctx.lineTo(px + 0.8, py - 0.15);
    ctx.moveTo(px - 0.8, py + 0.3); ctx.lineTo(px + 0.8, py + 0.3);
    ctx.stroke();
    ctx.fillStyle = sel ? C.selection : "#ab47bc";
    ctx.beginPath(); ctx.arc(px - 0.4, py - 0.38, 0.08, 0, Math.PI * 2); ctx.fill();
    ctx.beginPath(); ctx.arc(px - 0.4, py + 0.08, 0.08, 0, Math.PI * 2); ctx.fill();
    ctx.beginPath(); ctx.arc(px - 0.4, py + 0.52, 0.08, 0, Math.PI * 2); ctx.fill();
    if (ps.parameters.length > 0) {
      ctx.fillStyle = sel ? C.selection : "#ce93d8";
      ctx.font = `${0.5}px sans-serif`;
      ctx.textAlign = "left"; ctx.textBaseline = "top";
      ctx.fillText(ps.parameters[0].key + "=" + ps.parameters[0].value, px + 1.0, py - 0.4);
    }
  }
}

export function drawDiffPairDirectives(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
): void {
  if (!data.diff_pair_directives) return;
  for (const dp of data.diff_pair_directives) {
    const sel = selectedIds.has(dp.uuid);
    const px = dp.position.x, py = dp.position.y;
    ctx.strokeStyle = sel ? C.selection : "#42a5f5";
    ctx.lineWidth = 0.15;
    ctx.beginPath();
    ctx.moveTo(px - 0.8, py - 0.25); ctx.lineTo(px + 0.8, py - 0.25);
    ctx.moveTo(px - 0.8, py + 0.25); ctx.lineTo(px + 0.8, py + 0.25);
    ctx.stroke();
    ctx.fillStyle = sel ? C.selection : "#42a5f5";
    ctx.font = `bold ${0.45}px sans-serif`;
    ctx.textAlign = "center"; ctx.textBaseline = "middle";
    ctx.fillText("+", px + 1.1, py - 0.25);
    ctx.fillText("\u2013", px + 1.1, py + 0.25);
    ctx.font = `${0.4}px sans-serif`;
    ctx.textAlign = "left"; ctx.textBaseline = "top";
    ctx.fillText(dp.positiveNet, px - 0.8, py - 0.8);
    ctx.fillText(dp.negativeNet, px - 0.8, py + 0.5);
  }
}

export function drawBlankets(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
): void {
  if (!data.blankets) return;
  for (const bl of data.blankets) {
    const sel = selectedIds.has(bl.uuid);
    if (bl.points.length < 2) continue;
    ctx.strokeStyle = sel ? C.selection : "#ff9800";
    ctx.fillStyle = sel ? C.selectionFill : "rgba(255,152,0,0.06)";
    ctx.lineWidth = 0.15;
    ctx.setLineDash([0.4, 0.25]);
    ctx.beginPath();
    ctx.moveTo(bl.points[0].x, bl.points[0].y);
    for (let i = 1; i < bl.points.length; i++) ctx.lineTo(bl.points[i].x, bl.points[i].y);
    ctx.closePath();
    ctx.fill(); ctx.stroke();
    ctx.setLineDash([]);
    if (bl.parameters.length > 0) {
      ctx.fillStyle = sel ? C.selection : "#ffb74d";
      ctx.font = `${0.5}px sans-serif`;
      ctx.textAlign = "left"; ctx.textBaseline = "bottom";
      ctx.fillText(bl.parameters.map(p => p.key + "=" + p.value).join(", "), bl.points[0].x + 0.3, bl.points[0].y - 0.2);
    }
  }
}

export function drawCompileMasks(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
): void {
  if (!data.compile_masks) return;
  for (const cm of data.compile_masks) {
    const sel = selectedIds.has(cm.uuid);
    const px = cm.position.x, py = cm.position.y;
    const cw = cm.size[0], ch = cm.size[1];
    ctx.strokeStyle = sel ? C.selection : "#78909c";
    ctx.fillStyle = sel ? C.selectionFill : "rgba(120,144,156,0.08)";
    ctx.lineWidth = 0.15;
    ctx.fillRect(px, py, cw, ch);
    ctx.strokeRect(px, py, cw, ch);
    ctx.save();
    ctx.beginPath();
    ctx.rect(px, py, cw, ch);
    ctx.clip();
    ctx.strokeStyle = sel ? C.selection : "rgba(120,144,156,0.25)";
    ctx.lineWidth = 0.08;
    for (let d = -ch; d < cw + ch; d += 0.8) {
      ctx.beginPath();
      ctx.moveTo(px + d, py);
      ctx.lineTo(px + d - ch, py + ch);
      ctx.stroke();
    }
    ctx.restore();
    ctx.fillStyle = sel ? C.selection : "#90a4ae";
    ctx.font = `${0.5}px sans-serif`;
    ctx.textAlign = "center"; ctx.textBaseline = "middle";
    ctx.fillText("Compile Mask", px + cw / 2, py + ch / 2);
  }
}

export function drawNotes(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
): void {
  if (!data.notes) return;
  for (const n of data.notes) {
    const sel = selectedIds.has(n.uuid);
    const px = n.position.x, py = n.position.y;
    const nw = n.size[0], nh = n.size[1];
    ctx.fillStyle = sel ? C.selectionFill : "rgba(255,193,7,0.15)";
    ctx.strokeStyle = sel ? C.selection : "#ffc107";
    ctx.lineWidth = 0.15;
    ctx.beginPath();
    ctx.moveTo(px, py + nh);
    ctx.lineTo(px - 0.5, py + nh + 0.8);
    ctx.lineTo(px + 0.8, py + nh);
    ctx.closePath();
    ctx.fill(); ctx.stroke();
    ctx.fillRect(px, py, nw, nh);
    ctx.strokeRect(px, py, nw, nh);
    ctx.fillStyle = sel ? C.selection : "#ffca28";
    ctx.font = `${0.55}px sans-serif`;
    ctx.textAlign = "left"; ctx.textBaseline = "top";
    const words = n.text.split(" ");
    let line = "";
    let ty = py + 0.3;
    for (const word of words) {
      const test = line + (line ? " " : "") + word;
      if (ctx.measureText(test).width > nw - 0.6 && line) {
        ctx.fillText(line, px + 0.3, ty);
        line = word;
        ty += 0.65;
      } else {
        line = test;
      }
    }
    if (line) ctx.fillText(line, px + 0.3, ty);
  }
}

// ---------------------------------------------------------------------------
// Section 7 – Graphic primitive (used by symbols and drawGraphicTransformed)
// ---------------------------------------------------------------------------

export function drawGraphicTransformed(
  ctx: CanvasRenderingContext2D,
  g: Graphic,
  sx: number, sy: number,
  rot: number, mx: boolean, my: boolean,
): void {
  const t = (lx: number, ly: number) => symToSch(lx, ly, sx, sy, rot, mx, my);
  ctx.lineWidth = Math.max(g.width || 0.1, 0.1);

  switch (g.type) {
    case "Polyline": {
      if (g.points.length < 2) break;
      ctx.beginPath();
      const [x0, y0] = t(g.points[0].x, g.points[0].y);
      ctx.moveTo(x0, y0);
      for (let i = 1; i < g.points.length; i++) {
        const [xi, yi] = t(g.points[i].x, g.points[i].y);
        ctx.lineTo(xi, yi);
      }
      if (g.fill) { ctx.fillStyle = C.body; ctx.globalAlpha = 0.15; ctx.fill(); ctx.globalAlpha = 1; }
      ctx.stroke();
      break;
    }
    case "Rectangle": {
      const [x1, y1] = t(g.start.x, g.start.y);
      const [x2, y2] = t(g.end.x, g.end.y);
      const rx = Math.min(x1, x2), ry = Math.min(y1, y2);
      const rw = Math.abs(x2 - x1), rh = Math.abs(y2 - y1);
      ctx.fillStyle = C.bodyFill;
      ctx.fillRect(rx, ry, rw, rh);
      ctx.strokeRect(rx, ry, rw, rh);
      break;
    }
    case "Circle": {
      const [cx, cy] = t(g.center.x, g.center.y);
      ctx.beginPath();
      ctx.arc(cx, cy, g.radius, 0, Math.PI * 2);
      if (g.fill) { ctx.fillStyle = C.bodyFill; ctx.fill(); }
      ctx.stroke();
      break;
    }
    case "Arc": {
      const [sx1, sy1] = t(g.start.x, g.start.y);
      const [mx1, my1] = t(g.mid.x, g.mid.y);
      const [ex1, ey1] = t(g.end.x, g.end.y);
      const center = arcCenter({ x: sx1, y: sy1 }, { x: mx1, y: my1 }, { x: ex1, y: ey1 });
      if (center) {
        const r = Math.hypot(sx1 - center.x, sy1 - center.y);
        const a1 = Math.atan2(sy1 - center.y, sx1 - center.x);
        const a2 = Math.atan2(ey1 - center.y, ex1 - center.x);
        const aM = Math.atan2(my1 - center.y, mx1 - center.x);
        ctx.beginPath();
        ctx.arc(center.x, center.y, r, a1, a2, isCounterClockwise(a1, aM, a2));
        ctx.stroke();
      }
      break;
    }
  }
}

// ---------------------------------------------------------------------------
// Section 8 – Text property (KiCad-normalized rotation + justify)
// ---------------------------------------------------------------------------

export function drawTextProp(
  ctx: CanvasRenderingContext2D,
  text: string,
  prop: TextPropData,
  color: string,
  bold: boolean,
): void {
  ctx.fillStyle = color;
  ctx.font = `${bold ? "bold " : ""}${prop.font_size}px Roboto`;

  let jh = prop.justify_h;
  let jv = prop.justify_v;
  let rot = prop.rotation;

  if (rot === 180) {
    rot = 0;
    jh = jh === "left" ? "right" : jh === "right" ? "left" : jh;
    jv = jv === "top" ? "bottom" : jv === "bottom" ? "top" : jv;
  } else if (rot === 270) {
    rot = 90;
    jh = jh === "left" ? "right" : jh === "right" ? "left" : jh;
    jv = jv === "top" ? "bottom" : jv === "bottom" ? "top" : jv;
  }

  ctx.textAlign = jh === "left" ? "left" : jh === "right" ? "right" : "center";
  ctx.textBaseline = jv === "top" ? "top" : jv === "bottom" ? "bottom" : "middle";

  if (rot === 90) {
    ctx.save();
    ctx.translate(prop.position.x, prop.position.y);
    ctx.rotate(-Math.PI / 2);
    ctx.fillText(text, 0, 0);
    ctx.restore();
  } else {
    ctx.fillText(text, prop.position.x, prop.position.y);
  }
}

// ---------------------------------------------------------------------------
// Section 9 – Symbols
// ---------------------------------------------------------------------------

export function drawSymbols(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  _selectedIds: Set<string>,
  alphaFor: (uuid: string, filterKey: string) => number,
  inPlaceEditUuid: string | null,
  inPlaceEditField: string | null,
): void {
  for (const sym of data.symbols) {
    const lib = data.lib_symbols[sym.lib_id];
    if (!lib) continue;
    ctx.globalAlpha = alphaFor(sym.uuid, sym.is_power ? "powerPorts" : "components");

    const sx = sym.position.x, sy = sym.position.y;
    const rot = sym.rotation, mx = sym.mirror_x, my = sym.mirror_y;

    ctx.strokeStyle = sym.is_power ? C.power : C.body;
    for (const g of lib.graphics) {
      drawGraphicTransformed(ctx, g, sx, sy, rot, mx, my);
    }

    for (const pin of lib.pins) {
      if (pin.hidden) continue;
      const [px, py] = symToSch(pin.position.x, pin.position.y, sx, sy, rot, mx, my);
      const pe = pinEnd(pin);
      const [ex, ey] = symToSch(pe.x, pe.y, sx, sy, rot, mx, my);

      ctx.strokeStyle = C.pin;
      ctx.lineWidth = 0.1;
      ctx.beginPath();
      ctx.moveTo(px, py);
      ctx.lineTo(ex, ey);
      ctx.stroke();

      if (lib.show_pin_numbers && pin.number_visible && pin.number !== "~") {
        ctx.fillStyle = C.pinNum;
        ctx.font = "1.0px Roboto";
        ctx.textAlign = "center";
        ctx.textBaseline = "bottom";
        const nmx = (px + ex) / 2, nmy = (py + ey) / 2;
        const dx = ex - px, dy = ey - py;
        const len = Math.hypot(dx, dy) || 1;
        ctx.fillText(txt(pin.number), nmx - dy / len * 0.5, nmy + dx / len * 0.5);
      }

      if (lib.show_pin_names && pin.name_visible && pin.name !== "~") {
        ctx.fillStyle = C.pinName;
        ctx.font = "0.75px Roboto";
        const dx = ex - px, dy = ey - py;
        const len = Math.hypot(dx, dy) || 1;
        const offset = 0.4;
        const nx = ex + (dx / len) * offset;
        const ny = ey + (dy / len) * offset;
        if (Math.abs(dx) > Math.abs(dy)) {
          ctx.textBaseline = "middle";
          ctx.textAlign = dx > 0 ? "left" : "right";
        } else {
          ctx.textAlign = "center";
          ctx.textBaseline = dy > 0 ? "top" : "bottom";
        }
        ctx.fillText(txt(pin.name), nx, ny);
      }
    }

    if (!sym.is_power) {
      if (!sym.ref_text.hidden && !(inPlaceEditUuid === sym.uuid && inPlaceEditField === "reference")) {
        drawTextProp(ctx, txt(sym.reference), sym.ref_text, C.ref, true);
      }
      if (!sym.val_text.hidden && !(inPlaceEditUuid === sym.uuid && inPlaceEditField === "value")) {
        drawTextProp(ctx, txt(sym.value), sym.val_text, C.val, false);
      }
    } else {
      const powerText = sym.val_text.hidden ? sym.ref_text : sym.val_text;
      if (!powerText.hidden) {
        const powerProp = { ...powerText, rotation: 0 };
        drawTextProp(ctx, txt(sym.value || sym.reference), powerProp, C.power, true);
      }
    }
  }
  ctx.globalAlpha = 1;
}

// ---------------------------------------------------------------------------
// Section 10 – Labels (net, global, hierarchical, power port)
// ---------------------------------------------------------------------------

export function drawLabels(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  _selectedIds: Set<string>,
  alphaFor: (uuid: string, filterKey: string) => number,
  inPlaceEditUuid: string | null,
): void {
  for (const label of data.labels) {
    ctx.globalAlpha = alphaFor(label.uuid, label.label_type === "Power" ? "powerPorts" : "labels");
    if (inPlaceEditUuid === label.uuid) continue;

    const color = label.label_type === "Global" ? C.labelGlobal
      : label.label_type === "Hierarchical" ? C.labelHier : C.labelNet;
    const text = txt(label.text);
    const fs = label.font_size || 1.27;
    const r = label.rotation;
    const lx = label.position.x, ly = label.position.y;

    if (label.label_type === "Power") {
      const pColor = C.power;
      const lw = 0.15;
      const stemLen = 2.0;
      const symSize = 1.2;

      let style = label.shape || "input";
      if (style === "input") {
        const lower = label.text.toLowerCase();
        if (lower.includes("gnd") || lower.includes("vss") || lower.includes("ground")) {
          style = "power_ground";
        } else {
          style = "bar";
        }
      }

      const isGround = style.includes("ground") || style === "earth_ground";
      const dir = isGround ? 1 : -1;

      ctx.save();
      ctx.translate(lx, ly);
      const rotRad = -(r * Math.PI) / 180;
      ctx.rotate(rotRad);

      ctx.strokeStyle = pColor;
      ctx.fillStyle = pColor;
      ctx.lineWidth = lw;
      ctx.lineCap = "round";

      ctx.beginPath();
      ctx.moveTo(0, 0);
      ctx.lineTo(0, dir * stemLen);
      ctx.stroke();

      const sy = dir * stemLen;

      if (style === "bar") {
        ctx.lineWidth = 0.2;
        ctx.beginPath();
        ctx.moveTo(-symSize, sy); ctx.lineTo(symSize, sy);
        ctx.stroke();
      } else if (style === "arrow") {
        ctx.lineWidth = 0.18;
        ctx.beginPath();
        ctx.moveTo(0, sy - 0.6); ctx.lineTo(-symSize * 0.5, sy + 0.2);
        ctx.moveTo(0, sy - 0.6); ctx.lineTo(symSize * 0.5, sy + 0.2);
        ctx.moveTo(0, sy - 0.6); ctx.lineTo(0, sy);
        ctx.stroke();
      } else if (style === "power_ground") {
        ctx.lineWidth = 0.15;
        ctx.beginPath();
        ctx.moveTo(-symSize, sy); ctx.lineTo(symSize, sy);
        ctx.moveTo(-symSize * 0.65, sy + dir * 0.4); ctx.lineTo(symSize * 0.65, sy + dir * 0.4);
        ctx.moveTo(-symSize * 0.3, sy + dir * 0.8); ctx.lineTo(symSize * 0.3, sy + dir * 0.8);
        ctx.stroke();
      } else if (style === "signal_ground") {
        ctx.lineWidth = 0.15;
        ctx.beginPath();
        ctx.moveTo(-symSize, sy); ctx.lineTo(symSize, sy); ctx.lineTo(0, sy + dir * symSize);
        ctx.closePath();
        ctx.stroke();
      } else if (style === "earth_ground") {
        ctx.lineWidth = 0.15;
        ctx.beginPath();
        ctx.moveTo(-symSize, sy); ctx.lineTo(symSize, sy);
        ctx.stroke();
        for (let i = -3; i <= 3; i++) {
          const sxi = i * (symSize / 3);
          ctx.beginPath();
          ctx.moveTo(sxi, sy); ctx.lineTo(sxi - dir * 0.4, sy + dir * 0.6);
          ctx.stroke();
        }
      } else if (style === "circle") {
        ctx.lineWidth = 0.15;
        ctx.beginPath();
        ctx.arc(0, sy + dir * (-symSize * 0.4), symSize * 0.4, 0, Math.PI * 2);
        ctx.stroke();
      } else if (style === "wave") {
        ctx.lineWidth = 0.15;
        ctx.beginPath();
        ctx.arc(-symSize * 0.35, sy, symSize * 0.35, Math.PI, 0);
        ctx.arc(symSize * 0.35, sy, symSize * 0.35, Math.PI, 0, true);
        ctx.stroke();
      } else {
        ctx.lineWidth = 0.2;
        ctx.beginPath();
        ctx.moveTo(-symSize, sy); ctx.lineTo(symSize, sy);
        ctx.stroke();
      }

      ctx.restore();
      ctx.save();
      ctx.translate(lx, ly);
      ctx.fillStyle = pColor;
      ctx.font = `${fs}px Roboto`;
      ctx.textAlign = "center";

      const norm = ((r % 360) + 360) % 360;
      if (norm === 0) {
        if (isGround) {
          ctx.textBaseline = "top";
          ctx.fillText(text, 0, stemLen + 1.2);
        } else {
          ctx.textBaseline = "bottom";
          ctx.fillText(text, 0, -(stemLen + 0.4));
        }
      } else if (norm === 180) {
        if (isGround) {
          ctx.textBaseline = "bottom";
          ctx.fillText(text, 0, -(stemLen + 1.2));
        } else {
          ctx.textBaseline = "top";
          ctx.fillText(text, 0, stemLen + 0.4);
        }
      } else {
        ctx.textBaseline = isGround ? "top" : "bottom";
        ctx.fillText(text, 0, isGround ? (stemLen + 1.2) : -(stemLen + 0.4));
      }
      ctx.restore();
      continue;
    }

    if (label.label_type === "Global" && label.shape) {
      ctx.font = `${fs}px Roboto`;
      const tw = ctx.measureText(text).width;
      const h = fs * 1.4;
      const pad = fs * 0.3;
      const arrowW = h * 0.5;

      const isHoriz = r === 0 || r === 180;
      const connRight = r === 0;

      ctx.strokeStyle = color;
      ctx.lineWidth = 0.15;

      if (isHoriz) {
        const dir = connRight ? 1 : -1;
        const bodyStart = dir > 0 ? arrowW : -arrowW;
        const bodyEnd = dir > 0 ? arrowW + tw + pad * 2 : -arrowW - tw - pad * 2;

        ctx.beginPath();
        if (label.shape === "input") {
          ctx.moveTo(lx, ly);
          ctx.lineTo(lx + bodyStart, ly - h / 2);
          ctx.lineTo(lx + bodyEnd, ly - h / 2);
          ctx.lineTo(lx + bodyEnd, ly + h / 2);
          ctx.lineTo(lx + bodyStart, ly + h / 2);
          ctx.closePath();
        } else if (label.shape === "output") {
          const tip = dir > 0 ? bodyEnd + arrowW : bodyEnd - arrowW;
          ctx.moveTo(lx, ly);
          ctx.lineTo(lx + bodyStart, ly - h / 2);
          ctx.lineTo(lx + bodyEnd, ly - h / 2);
          ctx.lineTo(lx + tip, ly);
          ctx.lineTo(lx + bodyEnd, ly + h / 2);
          ctx.lineTo(lx + bodyStart, ly + h / 2);
          ctx.closePath();
        } else if (label.shape === "bidirectional") {
          const tip = dir > 0 ? bodyEnd + arrowW : bodyEnd - arrowW;
          ctx.moveTo(lx, ly);
          ctx.lineTo(lx + bodyStart, ly - h / 2);
          ctx.lineTo(lx + bodyEnd, ly - h / 2);
          ctx.lineTo(lx + tip, ly);
          ctx.lineTo(lx + bodyEnd, ly + h / 2);
          ctx.lineTo(lx + bodyStart, ly + h / 2);
          ctx.closePath();
        } else {
          const x1 = Math.min(lx + bodyStart, lx + bodyEnd);
          ctx.rect(x1, ly - h / 2, Math.abs(bodyEnd - bodyStart), h);
        }
        ctx.stroke();

        ctx.fillStyle = color;
        ctx.font = `${fs}px Roboto`;
        ctx.textBaseline = "middle";
        const textYOff = fs * 0.1;
        if (dir > 0) {
          ctx.textAlign = "left";
          ctx.fillText(text, lx + arrowW + pad, ly + textYOff);
        } else {
          ctx.textAlign = "right";
          ctx.fillText(text, lx - arrowW - pad, ly + textYOff);
        }
      } else {
        ctx.save();
        ctx.translate(lx, ly);
        const rotAngle = r === 90 ? -Math.PI / 2 : Math.PI / 2;
        ctx.rotate(rotAngle);
        ctx.beginPath();
        ctx.moveTo(0, 0);
        ctx.lineTo(arrowW, -h / 2);
        ctx.lineTo(arrowW + tw + pad * 2, -h / 2);
        ctx.lineTo(arrowW + tw + pad * 2, h / 2);
        ctx.lineTo(arrowW, h / 2);
        ctx.closePath();
        ctx.stroke();
        ctx.fillStyle = color;
        ctx.font = `${fs}px Roboto`;
        ctx.textAlign = "left";
        ctx.textBaseline = "middle";
        ctx.fillText(text, arrowW + pad, 0);
        ctx.restore();
      }
    } else {
      ctx.fillStyle = color;
      ctx.font = `${fs}px Roboto`;

      let labelRot = r;
      let jh = label.justify === "right" ? "right" : "left";
      if (labelRot === 180) { labelRot = 0; jh = jh === "left" ? "right" : "left"; }
      if (labelRot === 270) { labelRot = 90; jh = jh === "left" ? "right" : "left"; }

      ctx.textAlign = jh as CanvasTextAlign;
      ctx.textBaseline = "bottom";

      if (labelRot === 90) {
        ctx.save();
        ctx.translate(lx, ly);
        ctx.rotate(-Math.PI / 2);
        ctx.fillText(text, 0.3, 0);
        ctx.restore();
      } else {
        ctx.fillText(text, lx, ly - 0.3);
      }
    }
  }
  ctx.globalAlpha = 1;
}

// ---------------------------------------------------------------------------
// Section 11 – Child sheets
// ---------------------------------------------------------------------------

export function drawChildSheets(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
  sf: SelectionFilter,
): void {
  ctx.globalAlpha = sf.sheetSymbols?.visible === false ? 0.12 : 1;
  for (const sheet of data.child_sheets) {
    const isSel = selectedIds.has(sheet.uuid);
    const sx = sheet.position.x, sy = sheet.position.y, sw = sheet.size[0], sh = sheet.size[1];
    ctx.fillStyle = isSel ? "rgba(91,141,239,0.08)" : "rgba(91,141,239,0.03)";
    ctx.fillRect(sx, sy, sw, sh);
    ctx.strokeStyle = isSel ? C.selection : C.sheet;
    ctx.lineWidth = isSel ? 0.25 : 0.2;
    ctx.setLineDash(isSel ? [0.4, 0.25] : []);
    ctx.strokeRect(sx, sy, sw, sh);
    ctx.setLineDash([]);
    ctx.fillStyle = C.sheetText;
    ctx.font = "bold 1.2px Roboto"; ctx.textAlign = "left"; ctx.textBaseline = "bottom";
    ctx.fillText(sheet.name, sx + 0.5, sy - 0.3);
    ctx.fillStyle = C.sheet;
    ctx.font = "0.8px Roboto Mono"; ctx.textBaseline = "top";
    ctx.fillText(sheet.filename, sx + 0.5, sy + 0.5);
    if (sheet.pins && sheet.pins.length > 0) {
      ctx.fillStyle = C.labelHier;
      ctx.font = "0.9px Roboto";
      ctx.textBaseline = "middle";
      for (const pin of sheet.pins) {
        const px = pin.position.x, py = pin.position.y;
        ctx.beginPath();
        const isLeft = px <= sx + 0.1;
        if (isLeft) {
          ctx.moveTo(px, py); ctx.lineTo(px + 0.8, py - 0.4); ctx.lineTo(px + 0.8, py + 0.4);
          ctx.textAlign = "left";
          ctx.fillText(pin.name, px + 1.2, py);
        } else {
          ctx.moveTo(px, py); ctx.lineTo(px - 0.8, py - 0.4); ctx.lineTo(px - 0.8, py + 0.4);
          ctx.textAlign = "right";
          ctx.fillText(pin.name, px - 1.2, py);
        }
        ctx.closePath();
        ctx.fill();
      }
    }
  }
  ctx.globalAlpha = 1;
}

// ---------------------------------------------------------------------------
// Section 12 – Buses + bus entries + top-level rectangles
// ---------------------------------------------------------------------------

export function drawBuses(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
  sf: SelectionFilter,
): void {
  ctx.lineWidth = 0.4;
  ctx.globalAlpha = sf.buses?.visible === false ? 0.12 : 1;
  for (const b of data.buses) {
    ctx.strokeStyle = selectedIds.has(b.uuid) ? C.selection : C.bus;
    ctx.beginPath();
    ctx.moveTo(b.start.x, b.start.y);
    ctx.lineTo(b.end.x, b.end.y);
    ctx.stroke();
  }
  ctx.globalAlpha = 1;
  ctx.lineWidth = 0.2;
  for (const be of data.bus_entries) {
    ctx.strokeStyle = selectedIds.has(be.uuid) ? C.selection : C.busEntry;
    ctx.beginPath();
    ctx.moveTo(be.position.x, be.position.y);
    ctx.lineTo(be.position.x + be.size[0], be.position.y + be.size[1]);
    ctx.stroke();
  }
  ctx.globalAlpha = 1;
}

export function drawTopLevelRectangles(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
): void {
  for (const r of data.rectangles) {
    const rx = Math.min(r.start.x, r.end.x);
    const ry = Math.min(r.start.y, r.end.y);
    const rw = Math.abs(r.end.x - r.start.x);
    const rh = Math.abs(r.end.y - r.start.y);
    ctx.strokeStyle = C.sheet;
    ctx.lineWidth = 0.15;
    if (r.stroke_type === "dash") ctx.setLineDash([1.0, 0.5]);
    else if (r.stroke_type === "dash_dot") ctx.setLineDash([1.0, 0.3, 0.2, 0.3]);
    else if (r.stroke_type === "dot") ctx.setLineDash([0.2, 0.3]);
    else ctx.setLineDash([]);
    ctx.strokeRect(rx, ry, rw, rh);
    ctx.setLineDash([]);
  }
}

// ---------------------------------------------------------------------------
// Section 13 – Drawing objects (user-drawn)
// ---------------------------------------------------------------------------

function applyLineStyle(ctx: CanvasRenderingContext2D, ls?: string): void {
  if (ls === "dash") ctx.setLineDash([1.0, 0.5]);
  else if (ls === "dot") ctx.setLineDash([0.2, 0.3]);
  else if (ls === "dash_dot") ctx.setLineDash([1.0, 0.3, 0.2, 0.3]);
  else ctx.setLineDash([]);
}

function drawArrow(ctx: CanvasRenderingContext2D, x: number, y: number, angle: number, style?: string): void {
  if (!style || style === "none") return;
  const sz = 0.8;
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(angle);
  ctx.beginPath();
  if (style === "open") {
    ctx.moveTo(-sz, -sz * 0.5); ctx.lineTo(0, 0); ctx.lineTo(-sz, sz * 0.5);
    ctx.stroke();
  } else if (style === "closed") {
    ctx.moveTo(0, 0); ctx.lineTo(-sz, -sz * 0.5); ctx.lineTo(-sz, sz * 0.5); ctx.closePath();
    ctx.fill(); ctx.stroke();
  } else if (style === "diamond") {
    ctx.moveTo(0, 0); ctx.lineTo(-sz * 0.5, -sz * 0.4); ctx.lineTo(-sz, 0); ctx.lineTo(-sz * 0.5, sz * 0.4); ctx.closePath();
    ctx.fill(); ctx.stroke();
  }
  ctx.restore();
}

export function drawDrawings(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
  sf: SelectionFilter,
  imageCache: Map<string, HTMLImageElement>,
  maxImageCache: number,
): void {
  ctx.globalAlpha = sf.drawings?.visible === false ? 0.12 : 1;
  for (const d of data.drawings) {
    const sel = selectedIds.has(d.uuid);
    const strokeColor = sel ? C.selection : ("color" in d && d.color) || C.body;
    ctx.strokeStyle = strokeColor;
    ctx.fillStyle = strokeColor;
    ctx.lineWidth = Math.max("width" in d ? d.width || 0.15 : 0.15, 0.15);
    applyLineStyle(ctx, "lineStyle" in d ? d.lineStyle : undefined);
    if (d.type === "Line") {
      ctx.beginPath(); ctx.moveTo(d.start.x, d.start.y); ctx.lineTo(d.end.x, d.end.y); ctx.stroke();
      const angle = Math.atan2(d.end.y - d.start.y, d.end.x - d.start.x);
      drawArrow(ctx, d.end.x, d.end.y, angle, d.arrowEnd);
      drawArrow(ctx, d.start.x, d.start.y, angle + Math.PI, d.arrowStart);
    } else if (d.type === "Rect") {
      const rx = Math.min(d.start.x, d.end.x), ry = Math.min(d.start.y, d.end.y);
      const rw = Math.abs(d.end.x - d.start.x), rh = Math.abs(d.end.y - d.start.y);
      if (d.fill) { ctx.fillStyle = sel ? C.selectionFill : d.fillColor || C.bodyFill; ctx.fillRect(rx, ry, rw, rh); }
      ctx.strokeRect(rx, ry, rw, rh);
    } else if (d.type === "Circle") {
      ctx.beginPath(); ctx.arc(d.center.x, d.center.y, d.radius, 0, Math.PI * 2);
      if (d.fill) { ctx.fillStyle = sel ? C.selectionFill : d.fillColor || C.bodyFill; ctx.fill(); }
      ctx.stroke();
    } else if (d.type === "Arc") {
      ctx.beginPath(); ctx.moveTo(d.start.x, d.start.y);
      ctx.quadraticCurveTo(d.mid.x, d.mid.y, d.end.x, d.end.y);
      ctx.stroke();
    } else if (d.type === "Polyline") {
      if (d.points.length > 1) {
        ctx.beginPath(); ctx.moveTo(d.points[0].x, d.points[0].y);
        for (let i = 1; i < d.points.length; i++) ctx.lineTo(d.points[i].x, d.points[i].y);
        if (d.fill) { ctx.closePath(); ctx.fillStyle = sel ? C.selectionFill : d.fillColor || C.bodyFill; ctx.fill(); }
        ctx.stroke();
        if (d.points.length >= 2) {
          const p0 = d.points[0], p1 = d.points[1];
          drawArrow(ctx, p0.x, p0.y, Math.atan2(p0.y - p1.y, p0.x - p1.x), d.arrowStart);
          const pn = d.points[d.points.length - 1], pn1 = d.points[d.points.length - 2];
          drawArrow(ctx, pn.x, pn.y, Math.atan2(pn.y - pn1.y, pn.x - pn1.x), d.arrowEnd);
        }
      }
    } else if (d.type === "Ellipse") {
      ctx.beginPath();
      ctx.ellipse(d.center.x, d.center.y, d.radiusX, d.radiusY, 0, 0, Math.PI * 2);
      if (d.fill) { ctx.fillStyle = sel ? C.selectionFill : d.fillColor || C.bodyFill; ctx.fill(); }
      ctx.stroke();
    } else if (d.type === "RoundRect") {
      const rx = Math.min(d.start.x, d.end.x), ry = Math.min(d.start.y, d.end.y);
      const rw = Math.abs(d.end.x - d.start.x), rh = Math.abs(d.end.y - d.start.y);
      const cr = Math.min(d.cornerRadius, rw / 2, rh / 2);
      ctx.beginPath();
      ctx.moveTo(rx + cr, ry);
      ctx.lineTo(rx + rw - cr, ry); ctx.arcTo(rx + rw, ry, rx + rw, ry + cr, cr);
      ctx.lineTo(rx + rw, ry + rh - cr); ctx.arcTo(rx + rw, ry + rh, rx + rw - cr, ry + rh, cr);
      ctx.lineTo(rx + cr, ry + rh); ctx.arcTo(rx, ry + rh, rx, ry + rh - cr, cr);
      ctx.lineTo(rx, ry + cr); ctx.arcTo(rx, ry, rx + cr, ry, cr);
      ctx.closePath();
      if (d.fill) { ctx.fillStyle = sel ? C.selectionFill : d.fillColor || C.bodyFill; ctx.fill(); }
      ctx.stroke();
    } else if (d.type === "TextFrame") {
      const rx = Math.min(d.start.x, d.end.x), ry = Math.min(d.start.y, d.end.y);
      const rw = Math.abs(d.end.x - d.start.x), rh = Math.abs(d.end.y - d.start.y);
      if (d.fill) { ctx.fillStyle = sel ? C.selectionFill : d.fillColor || C.bodyFill; ctx.fillRect(rx, ry, rw, rh); }
      ctx.strokeRect(rx, ry, rw, rh);
      ctx.fillStyle = sel ? C.selection : d.color || C.sheetText;
      ctx.font = `${d.fontSize || 1.27}px Roboto`;
      ctx.textAlign = "left"; ctx.textBaseline = "top";
      const padding = 0.5;
      const lines = substituteSpecialStrings(d.text, data).split("\n");
      lines.forEach((line, i) => {
        ctx.fillText(line, rx + padding, ry + padding + i * (d.fontSize || 1.27) * 1.3, rw - padding * 2);
      });
    } else if (d.type === "Polygon") {
      if (d.points.length >= 3) {
        ctx.beginPath();
        ctx.moveTo(d.points[0].x, d.points[0].y);
        for (let i = 1; i < d.points.length; i++) ctx.lineTo(d.points[i].x, d.points[i].y);
        ctx.closePath();
        ctx.fillStyle = sel ? C.selectionFill : d.fillColor || C.bodyFill;
        ctx.fill();
        ctx.stroke();
      }
    } else if (d.type === "Image") {
      const rx = Math.min(d.start.x, d.end.x), ry = Math.min(d.start.y, d.end.y);
      const rw = Math.abs(d.end.x - d.start.x), rh = Math.abs(d.end.y - d.start.y);
      let img = imageCache.get(d.uuid);
      if (!img && d.dataUrl) {
        if (imageCache.size >= maxImageCache) {
          const firstKey = imageCache.keys().next().value;
          if (firstKey !== undefined) imageCache.delete(firstKey);
        }
        img = new Image();
        img.src = d.dataUrl;
        imageCache.set(d.uuid, img);
      }
      if (img?.complete) {
        ctx.drawImage(img, rx, ry, rw, rh);
      }
      if (sel) { ctx.strokeStyle = C.selection; ctx.setLineDash([0.3, 0.2]); ctx.strokeRect(rx, ry, rw, rh); ctx.setLineDash([]); }
    }
    ctx.setLineDash([]);
  }
  ctx.globalAlpha = 1;
}

// ---------------------------------------------------------------------------
// Section 14 – Text notes
// ---------------------------------------------------------------------------

export function drawTextNotes(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
  sf: SelectionFilter,
): void {
  ctx.globalAlpha = sf.textNotes?.visible === false ? 0.12 : 1;
  for (const note of data.text_notes) {
    ctx.fillStyle = selectedIds.has(note.uuid) ? C.selection : C.sheetText;
    ctx.font = `${note.font_size}px Roboto`;
    ctx.textAlign = "left";
    ctx.textBaseline = "top";
    const noteText = substituteSpecialStrings(note.text, data);
    if (note.rotation === 90 || note.rotation === 270) {
      ctx.save();
      ctx.translate(note.position.x, note.position.y);
      ctx.rotate(-Math.PI / 2);
      const lines = noteText.split("\n");
      lines.forEach((line, i) => {
        ctx.fillText(line, 0, i * note.font_size * 1.3);
      });
      ctx.restore();
    } else {
      const lines = noteText.split("\n");
      lines.forEach((line, i) => {
        ctx.fillText(line, note.position.x, note.position.y + i * note.font_size * 1.3);
      });
    }
  }
  ctx.globalAlpha = 1;
}

// ---------------------------------------------------------------------------
// Section 15 – Selection overlays
// ---------------------------------------------------------------------------

export function drawSelectionOverlay(
  ctx: CanvasRenderingContext2D,
  data: SchematicData,
  selectedIds: Set<string>,
  zoom: number,
): void {
  if (selectedIds.size === 0) return;

  const hs = 0.35 / zoom * 3;

  const drawSelBox = (bx: number, by: number, bw: number, bh: number) => {
    ctx.strokeStyle = "#66bb6a";
    ctx.lineWidth = 0.08;
    ctx.setLineDash([0.3, 0.2]);
    ctx.strokeRect(bx, by, bw, bh);
    ctx.setLineDash([]);
  };

  const drawLineHandles = (x1: number, y1: number, x2: number, y2: number) => {
    ctx.strokeStyle = C.selection;
    ctx.lineWidth = 0.2;
    ctx.setLineDash([0.4, 0.25]);
    ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke();
    ctx.setLineDash([]);
    ctx.fillStyle = C.handleFill;
    ctx.strokeStyle = C.handleBorder;
    ctx.lineWidth = 0.08;
    for (const [cx, cy] of [[x1, y1], [x2, y2]] as [number, number][]) {
      ctx.fillRect(cx - hs / 2, cy - hs / 2, hs, hs);
      ctx.strokeRect(cx - hs / 2, cy - hs / 2, hs, hs);
    }
  };

  const drawPointHandle = (px: number, py: number) => {
    ctx.fillStyle = C.handleFill;
    ctx.strokeStyle = C.handleBorder;
    ctx.lineWidth = 0.08;
    ctx.fillRect(px - hs / 2, py - hs / 2, hs, hs);
    ctx.strokeRect(px - hs / 2, py - hs / 2, hs, hs);
  };

  for (const sym of data.symbols) {
    if (!selectedIds.has(sym.uuid)) continue;
    const lib = data.lib_symbols[sym.lib_id];
    if (!lib) continue;
    let lMinX = Infinity, lMaxX = -Infinity, lMinY = Infinity, lMaxY = -Infinity;
    for (const g of lib.graphics) {
      if (g.type === "Rectangle") {
        lMinX = Math.min(lMinX, g.start.x, g.end.x); lMaxX = Math.max(lMaxX, g.start.x, g.end.x);
        lMinY = Math.min(lMinY, g.start.y, g.end.y); lMaxY = Math.max(lMaxY, g.start.y, g.end.y);
      } else if (g.type === "Polyline") {
        for (const p of g.points) { lMinX = Math.min(lMinX, p.x); lMaxX = Math.max(lMaxX, p.x); lMinY = Math.min(lMinY, p.y); lMaxY = Math.max(lMaxY, p.y); }
      } else if (g.type === "Circle") {
        lMinX = Math.min(lMinX, g.center.x - g.radius); lMaxX = Math.max(lMaxX, g.center.x + g.radius);
        lMinY = Math.min(lMinY, g.center.y - g.radius); lMaxY = Math.max(lMaxY, g.center.y + g.radius);
      }
    }
    if (!isFinite(lMinX)) { lMinX = -2; lMaxX = 2; lMinY = -2; lMaxY = 2; }
    const pad = 0.5;
    const tc = [
      symToSch(lMinX - pad, lMinY - pad, sym.position.x, sym.position.y, sym.rotation, sym.mirror_x, sym.mirror_y),
      symToSch(lMaxX + pad, lMinY - pad, sym.position.x, sym.position.y, sym.rotation, sym.mirror_x, sym.mirror_y),
      symToSch(lMaxX + pad, lMaxY + pad, sym.position.x, sym.position.y, sym.rotation, sym.mirror_x, sym.mirror_y),
      symToSch(lMinX - pad, lMaxY + pad, sym.position.x, sym.position.y, sym.rotation, sym.mirror_x, sym.mirror_y),
    ];
    const bx = Math.min(...tc.map(c => c[0]));
    const by = Math.min(...tc.map(c => c[1]));
    const bw = Math.max(...tc.map(c => c[0])) - bx;
    const bh = Math.max(...tc.map(c => c[1])) - by;
    drawSelBox(bx, by, bw, bh);
  }

  for (const wire of data.wires) {
    if (!selectedIds.has(wire.uuid)) continue;
    drawLineHandles(wire.start.x, wire.start.y, wire.end.x, wire.end.y);
  }

  for (const bus of data.buses) {
    if (!selectedIds.has(bus.uuid)) continue;
    drawLineHandles(bus.start.x, bus.start.y, bus.end.x, bus.end.y);
  }

  for (const label of data.labels) {
    if (!selectedIds.has(label.uuid)) continue;
    if (label.label_type === "Power") {
      const stemLen = 2.0, symSize = 1.2;
      const fs = label.font_size || 1.27;
      const style = label.shape || "bar";
      const isGnd = style.includes("ground") || style === "earth_ground";
      const textW = label.text.length * fs * 0.55;
      const halfW = Math.max(symSize, textW / 2) + 0.1;
      if (isGnd) {
        const top = label.position.y - 0.1;
        const bottom = label.position.y + stemLen + 1.2 + fs;
        drawSelBox(label.position.x - halfW, top, halfW * 2, bottom - top);
      } else {
        const top = label.position.y - stemLen - 0.3 - fs;
        const bottom = label.position.y + 0.1;
        drawSelBox(label.position.x - halfW, top, halfW * 2, bottom - top);
      }
    } else if ((label.label_type === "Global" || label.label_type === "Hierarchical") && label.shape) {
      const fs = label.font_size || 1.27;
      ctx.font = `${fs}px Roboto`;
      const tw = ctx.measureText(txt(label.text)).width;
      const h = fs * 1.4;
      const pad = fs * 0.3;
      const arrowW = h * 0.5;
      const r = label.rotation;
      const isHoriz = r === 0 || r === 180;
      if (isHoriz) {
        const connRight = r === 0;
        const totalLen = arrowW + tw + pad * 2 + (label.shape === "output" || label.shape === "bidirectional" ? arrowW : 0);
        if (connRight) {
          drawSelBox(label.position.x, label.position.y - h / 2, totalLen, h);
        } else {
          drawSelBox(label.position.x - totalLen, label.position.y - h / 2, totalLen, h);
        }
      } else {
        const totalLen = arrowW + tw + pad * 2;
        if (r === 90) {
          drawSelBox(label.position.x - h / 2, label.position.y - totalLen, h, totalLen);
        } else {
          drawSelBox(label.position.x - h / 2, label.position.y, h, totalLen);
        }
      }
    } else {
      const tw = label.text.length * label.font_size * 0.65;
      const th = label.font_size * 1.4;
      drawSelBox(label.position.x - 0.3, label.position.y - th, tw + 0.6, th + 0.3);
    }
  }

  for (const j of data.junctions) {
    if (!selectedIds.has(j.uuid)) continue;
    drawPointHandle(j.position.x, j.position.y);
  }

  for (const nc of data.no_connects) {
    if (!selectedIds.has(nc.uuid)) continue;
    drawSelBox(nc.position.x - 1, nc.position.y - 1, 2, 2);
  }

  for (const note of data.text_notes) {
    if (!selectedIds.has(note.uuid)) continue;
    const tw = note.text.length * note.font_size * 0.6;
    const th = note.font_size * 1.4;
    drawSelBox(note.position.x - 0.3, note.position.y - 0.3, tw + 0.6, th + 0.6);
  }

  for (const be of data.bus_entries) {
    if (!selectedIds.has(be.uuid)) continue;
    drawPointHandle(be.position.x, be.position.y);
  }
}

// ---------------------------------------------------------------------------
// Section 16 – Wire drawing preview
// ---------------------------------------------------------------------------

export interface WireDrawingState {
  active: boolean;
  points: SchPoint[];
  routingMode?: string;
}

export function drawWirePreview(
  ctx: CanvasRenderingContext2D,
  wireDrawing: WireDrawingState,
  wireCursor: SchPoint,
  isBusDrawing: boolean,
  data: SchematicData,
  findNearestElectricalPoint: (data: SchematicData, wx: number, wy: number) => SchPoint | null,
): void {
  if (!wireDrawing.active || wireDrawing.points.length === 0) return;

  ctx.strokeStyle = isBusDrawing ? C.bus : "#4fc3f7";
  ctx.lineWidth = isBusDrawing ? 0.4 : 0.15;
  ctx.setLineDash([]);
  if (wireDrawing.points.length > 1) {
    ctx.beginPath();
    ctx.moveTo(wireDrawing.points[0].x, wireDrawing.points[0].y);
    for (let i = 1; i < wireDrawing.points.length; i++) {
      ctx.lineTo(wireDrawing.points[i].x, wireDrawing.points[i].y);
    }
    ctx.stroke();
  }

  const last = wireDrawing.points[wireDrawing.points.length - 1];
  const cur = wireCursor;
  ctx.strokeStyle = "#80deea";
  ctx.lineWidth = 0.15;
  ctx.setLineDash([0.3, 0.2]);
  ctx.beginPath();
  ctx.moveTo(last.x, last.y);

  const rMode = wireDrawing.routingMode;
  if (rMode === "manhattan") {
    ctx.lineTo(cur.x, last.y);
    ctx.lineTo(cur.x, cur.y);
  } else if (rMode === "diagonal") {
    const dx = cur.x - last.x, dy = cur.y - last.y;
    const diag = Math.min(Math.abs(dx), Math.abs(dy));
    const mx = last.x + Math.sign(dx) * diag;
    const my = last.y + Math.sign(dy) * diag;
    ctx.lineTo(mx, my);
    ctx.lineTo(cur.x, cur.y);
  } else {
    ctx.lineTo(cur.x, cur.y);
  }
  ctx.stroke();
  ctx.setLineDash([]);

  ctx.strokeStyle = "#80deea";
  ctx.lineWidth = 0.08;
  ctx.beginPath();
  ctx.moveTo(cur.x - 3, cur.y); ctx.lineTo(cur.x + 3, cur.y);
  ctx.moveTo(cur.x, cur.y - 3); ctx.lineTo(cur.x, cur.y + 3);
  ctx.stroke();

  ctx.fillStyle = "#80deea";
  ctx.beginPath();
  ctx.arc(last.x, last.y, 0.3, 0, Math.PI * 2);
  ctx.fill();

  const eSnap = findNearestElectricalPoint(data, cur.x, cur.y);
  if (eSnap && Math.hypot(cur.x - eSnap.x, cur.y - eSnap.y) < 0.1) {
    ctx.strokeStyle = "#ff4444";
    ctx.lineWidth = 0.15;
    ctx.beginPath();
    ctx.moveTo(cur.x - 0.6, cur.y - 0.6); ctx.lineTo(cur.x + 0.6, cur.y + 0.6);
    ctx.moveTo(cur.x + 0.6, cur.y - 0.6); ctx.lineTo(cur.x - 0.6, cur.y + 0.6);
    ctx.stroke();
  }
}

// ---------------------------------------------------------------------------
// Section 17 – Placement preview (ghost symbol)
// ---------------------------------------------------------------------------

export interface PlacingSymbolState {
  lib: { graphics: Graphic[]; pins: SchPin[] };
  rotation: number;
  mirrorX: boolean;
  mirrorY: boolean;
}

export function drawPlacementPreview(
  ctx: CanvasRenderingContext2D,
  placing: PlacingSymbolState,
  cursor: SchPoint,
): void {
  const rot = placing.rotation;
  const mx = placing.mirrorX;
  const my = placing.mirrorY;

  ctx.globalAlpha = 0.5;
  ctx.strokeStyle = "#4fc3f7";
  for (const g of placing.lib.graphics) {
    drawGraphicTransformed(ctx, g, cursor.x, cursor.y, rot, mx, my);
  }

  for (const pin of placing.lib.pins) {
    const [px, py] = symToSch(pin.position.x, pin.position.y, cursor.x, cursor.y, rot, mx, my);
    const pe = pinEnd(pin);
    const [ex, ey] = symToSch(pe.x, pe.y, cursor.x, cursor.y, rot, mx, my);
    ctx.strokeStyle = "#81c784";
    ctx.lineWidth = 0.1;
    ctx.beginPath();
    ctx.moveTo(px, py);
    ctx.lineTo(ex, ey);
    ctx.stroke();
  }
  ctx.globalAlpha = 1;

  ctx.strokeStyle = "#4fc3f7";
  ctx.lineWidth = 0.08;
  ctx.beginPath();
  ctx.moveTo(cursor.x - 3, cursor.y); ctx.lineTo(cursor.x + 3, cursor.y);
  ctx.moveTo(cursor.x, cursor.y - 3); ctx.lineTo(cursor.x, cursor.y + 3);
  ctx.stroke();
}

// ---------------------------------------------------------------------------
// Section 18 – Net label / power port / no-connect placement preview
// ---------------------------------------------------------------------------

export function drawEditModePreviews(
  ctx: CanvasRenderingContext2D,
  editMode: string,
  cursor: SchPoint,
  powerPreset: { net: string; style: string },
): void {
  ctx.globalAlpha = 0.6;

  if (editMode === "placeLabel") {
    const labelText = "NET?";
    const fs = 1.27;
    ctx.font = `${fs}px Roboto`;
    const tw = ctx.measureText(labelText).width;
    ctx.fillStyle = C.labelNet;
    ctx.textAlign = "left";
    ctx.textBaseline = "bottom";
    ctx.fillText(labelText, cursor.x, cursor.y - 0.3);
    ctx.strokeStyle = C.labelNet;
    ctx.lineWidth = 0.08;
    ctx.beginPath();
    ctx.moveTo(cursor.x, cursor.y - fs - 0.2);
    ctx.lineTo(cursor.x + tw, cursor.y - fs - 0.2);
    ctx.stroke();
    ctx.fillStyle = C.labelNet;
    ctx.beginPath();
    ctx.arc(cursor.x, cursor.y, 0.15, 0, Math.PI * 2);
    ctx.fill();

  } else if (editMode === "placePower") {
    const stemLen = 2.0, symSize = 1.2;
    const isGnd = powerPreset.style.includes("ground");
    const dir = isGnd ? 1 : -1;
    ctx.strokeStyle = C.power;
    ctx.lineWidth = 0.12;
    ctx.lineCap = "round";
    ctx.beginPath();
    ctx.moveTo(cursor.x, cursor.y);
    ctx.lineTo(cursor.x, cursor.y + dir * stemLen);
    ctx.stroke();
    const sy = cursor.y + dir * stemLen;
    if (powerPreset.style === "bar") {
      ctx.lineWidth = 0.18;
      ctx.beginPath();
      ctx.moveTo(cursor.x - symSize, sy); ctx.lineTo(cursor.x + symSize, sy);
      ctx.stroke();
    } else if (isGnd) {
      ctx.lineWidth = 0.12;
      ctx.beginPath();
      ctx.moveTo(cursor.x - symSize, sy); ctx.lineTo(cursor.x + symSize, sy);
      ctx.moveTo(cursor.x - symSize * 0.65, sy + dir * 0.4); ctx.lineTo(cursor.x + symSize * 0.65, sy + dir * 0.4);
      ctx.moveTo(cursor.x - symSize * 0.3, sy + dir * 0.8); ctx.lineTo(cursor.x + symSize * 0.3, sy + dir * 0.8);
      ctx.stroke();
    }
    ctx.fillStyle = C.power;
    ctx.font = "1.27px Roboto";
    ctx.textAlign = "center";
    ctx.textBaseline = isGnd ? "top" : "bottom";
    ctx.fillText(powerPreset.net, cursor.x, isGnd ? sy + 1.2 : sy - 0.4);

  } else if (editMode === "placeNoConnect") {
    ctx.strokeStyle = C.noConnect;
    ctx.lineWidth = 0.15;
    ctx.beginPath();
    ctx.moveTo(cursor.x - 0.7, cursor.y - 0.7); ctx.lineTo(cursor.x + 0.7, cursor.y + 0.7);
    ctx.moveTo(cursor.x + 0.7, cursor.y - 0.7); ctx.lineTo(cursor.x - 0.7, cursor.y + 0.7);
    ctx.stroke();

  } else if (editMode === "placePort") {
    const portText = "PORT?";
    const fs = 1.27;
    const h = fs * 1.4;
    const pad = fs * 0.3;
    const arrowW = h * 0.5;
    ctx.font = `${fs}px Roboto`;
    const tw = ctx.measureText(portText).width;
    ctx.strokeStyle = C.labelHier;
    ctx.lineWidth = 0.12;
    ctx.beginPath();
    ctx.moveTo(cursor.x, cursor.y);
    ctx.lineTo(cursor.x + arrowW, cursor.y - h / 2);
    ctx.lineTo(cursor.x + arrowW + tw + pad * 2, cursor.y - h / 2);
    ctx.lineTo(cursor.x + arrowW + tw + pad * 2, cursor.y + h / 2);
    ctx.lineTo(cursor.x + arrowW, cursor.y + h / 2);
    ctx.closePath();
    ctx.stroke();
    ctx.lineWidth = 0.1;
    ctx.beginPath();
    ctx.moveTo(cursor.x, cursor.y); ctx.lineTo(cursor.x - 1, cursor.y);
    ctx.stroke();
    ctx.fillStyle = C.labelHier;
    ctx.textAlign = "left";
    ctx.textBaseline = "middle";
    ctx.fillText(portText, cursor.x + arrowW + pad, cursor.y);
  }

  ctx.globalAlpha = 1;
}

// ---------------------------------------------------------------------------
// Section 19 – Drawing tool ghost previews (line, rect, circle, polyline)
// ---------------------------------------------------------------------------

export function drawDrawingToolPreviews(
  ctx: CanvasRenderingContext2D,
  editMode: string,
  cursor: SchPoint,
  drawStart: SchPoint | null,
  polyPoints: SchPoint[],
): void {
  if (drawStart && (editMode === "drawLine" || editMode === "drawRect" || editMode === "drawCircle")) {
    ctx.strokeStyle = "#80deea";
    ctx.lineWidth = 0.1;
    ctx.setLineDash([0.3, 0.2]);
    ctx.globalAlpha = 0.6;
    if (editMode === "drawLine") {
      ctx.beginPath(); ctx.moveTo(drawStart.x, drawStart.y); ctx.lineTo(cursor.x, cursor.y); ctx.stroke();
    } else if (editMode === "drawRect") {
      ctx.strokeRect(Math.min(drawStart.x, cursor.x), Math.min(drawStart.y, cursor.y), Math.abs(cursor.x - drawStart.x), Math.abs(cursor.y - drawStart.y));
    } else if (editMode === "drawCircle") {
      const radius = Math.hypot(cursor.x - drawStart.x, cursor.y - drawStart.y);
      ctx.beginPath(); ctx.arc(drawStart.x, drawStart.y, radius, 0, Math.PI * 2); ctx.stroke();
    }
    ctx.setLineDash([]);
    ctx.globalAlpha = 1;
  }
  if (editMode === "drawPolyline" && polyPoints.length > 0) {
    ctx.strokeStyle = "#80deea";
    ctx.lineWidth = 0.1;
    ctx.setLineDash([0.3, 0.2]);
    ctx.globalAlpha = 0.6;
    ctx.beginPath();
    ctx.moveTo(polyPoints[0].x, polyPoints[0].y);
    for (let i = 1; i < polyPoints.length; i++) {
      ctx.lineTo(polyPoints[i].x, polyPoints[i].y);
    }
    ctx.lineTo(cursor.x, cursor.y);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.globalAlpha = 1;
  }
}

// ---------------------------------------------------------------------------
// Section 20 – ERC markers
// ---------------------------------------------------------------------------

export interface ErcMarker {
  position: SchPoint;
  severity: "error" | "warning";
}

export function drawErcMarkers(
  ctx: CanvasRenderingContext2D,
  ercMarkers: ErcMarker[],
  show: boolean,
): void {
  if (!show || ercMarkers.length === 0) return;
  for (const marker of ercMarkers) {
    const mx = marker.position.x, my = marker.position.y;
    const r = 0.6;
    ctx.beginPath();
    ctx.arc(mx, my, r, 0, Math.PI * 2);
    ctx.fillStyle = marker.severity === "error" ? "rgba(239,83,80,0.3)" : "rgba(255,183,77,0.3)";
    ctx.fill();
    ctx.strokeStyle = marker.severity === "error" ? "#ef5350" : "#ffb74d";
    ctx.lineWidth = 0.12;
    ctx.stroke();
    ctx.fillStyle = marker.severity === "error" ? "#ef5350" : "#ffb74d";
    ctx.font = "bold 0.8px Roboto";
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(marker.severity === "error" ? "X" : "!", mx, my);
  }
}

// ---------------------------------------------------------------------------
// Section 21 – Drag-box selection rectangle
// ---------------------------------------------------------------------------

export function drawDragBox(
  ctx: CanvasRenderingContext2D,
  selecting: boolean,
  selectionMode: string,
  lassoPoints: { x: number; y: number }[],
  selectStart: SchPoint,
  selectEnd: SchPoint,
): void {
  if (!selecting) return;

  if (selectionMode === "lasso" && lassoPoints.length > 1) {
    ctx.strokeStyle = "#00bfff";
    ctx.fillStyle = "rgba(0,191,255,0.08)";
    ctx.lineWidth = 0.2;
    ctx.setLineDash([]);
    ctx.beginPath();
    ctx.moveTo(lassoPoints[0].x, lassoPoints[0].y);
    for (let i = 1; i < lassoPoints.length; i++) {
      ctx.lineTo(lassoPoints[i].x, lassoPoints[i].y);
    }
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
  } else {
    const s = selectStart;
    const e = selectEnd;
    const rx = Math.min(s.x, e.x), ry = Math.min(s.y, e.y);
    const rw = Math.abs(e.x - s.x), rh = Math.abs(e.y - s.y);
    const mode = selectionMode;
    const crossing = mode === "touchingRect" || (mode === "box" && e.x < s.x);
    const isOutside = mode === "outsideArea";
    ctx.strokeStyle = isOutside ? "#ff6b6b" : crossing ? "#4fc3f7" : "#00bfff";
    ctx.fillStyle = isOutside ? "rgba(255,107,107,0.08)" : crossing ? "rgba(79,195,247,0.08)" : "rgba(0,191,255,0.08)";
    ctx.lineWidth = 0.2;
    if (crossing || isOutside) ctx.setLineDash([0.5, 0.3]);
    else ctx.setLineDash([]);
    ctx.fillRect(rx, ry, rw, rh);
    ctx.strokeRect(rx, ry, rw, rh);
    ctx.setLineDash([]);
  }
}
