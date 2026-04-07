import { jsPDF } from "jspdf";
import type { SchematicData, Graphic, SchPin, SchPoint, TextPropData } from "@/types";
import { substituteSpecialStrings } from "./specialStrings";

const PAPER: Record<string, [number, number]> = {
  A4: [297, 210], A3: [420, 297], A2: [594, 420], A1: [841, 594], A0: [1189, 841],
  A: [279.4, 215.9], B: [431.8, 279.4], C: [558.8, 431.8], D: [863.6, 558.8],
};

const DARK = {
  bg: "#1a1b2e", paper: "#1e2035", paperBorder: "#2a2d4a",
  grid: "#2d3060", gridMajor: "#3a3f75",
  wire: "#4fc3f7", junction: "#4fc3f7",
  body: "#9fa8da", bodyFill: "#1e2035",
  pin: "#81c784", pinName: "#90a4ae", pinNum: "#607d8b",
  ref: "#e8c66a", val: "#9598b3",
  labelNet: "#81c784", labelGlobal: "#ff8a65", labelHier: "#ba68c8",
  sheet: "#5b8def", sheetText: "#cdd6f4",
  noConnect: "#e8667a", power: "#ef5350",
  bus: "#4a86c8", busEntry: "#4a86c8",
};

const PRINT = {
  bg: "#ffffff", paper: "#ffffff", paperBorder: "#000000",
  grid: "#d0d0d0", gridMajor: "#b0b0b0",
  wire: "#006600", junction: "#006600",
  body: "#000000", bodyFill: "#ffffff",
  pin: "#333333", pinName: "#444444", pinNum: "#555555",
  ref: "#880000", val: "#333333",
  labelNet: "#006600", labelGlobal: "#cc4400", labelHier: "#7700aa",
  sheet: "#003399", sheetText: "#000000",
  noConnect: "#cc0000", power: "#cc0000",
  bus: "#003399", busEntry: "#003399",
};

type Colors = typeof DARK;

function symToSch(
  lx: number, ly: number, sx: number, sy: number,
  rot: number, mx: boolean, my: boolean,
): [number, number] {
  const x = lx, y = -ly;
  const rad = -(rot * Math.PI) / 180;
  const cos = Math.cos(rad), sin = Math.sin(rad);
  let rx = x * cos - y * sin;
  let ry = x * sin + y * cos;
  if (mx) ry = -ry;
  if (my) rx = -rx;
  return [sx + rx, sy + ry];
}

function pinEnd(pin: SchPin): SchPoint {
  const rad = (pin.rotation * Math.PI) / 180;
  return {
    x: pin.position.x + Math.cos(rad) * pin.length,
    y: pin.position.y + Math.sin(rad) * pin.length,
  };
}

function arcCenter(p1: SchPoint, p2: SchPoint, p3: SchPoint): SchPoint | null {
  const d = 2 * (p1.x * (p2.y - p3.y) + p2.x * (p3.y - p1.y) + p3.x * (p1.y - p2.y));
  if (Math.abs(d) < 1e-10) return null;
  const ux = ((p1.x ** 2 + p1.y ** 2) * (p2.y - p3.y) + (p2.x ** 2 + p2.y ** 2) * (p3.y - p1.y) + (p3.x ** 2 + p3.y ** 2) * (p1.y - p2.y)) / d;
  const uy = ((p1.x ** 2 + p1.y ** 2) * (p3.x - p2.x) + (p2.x ** 2 + p2.y ** 2) * (p1.x - p3.x) + (p3.x ** 2 + p3.y ** 2) * (p2.x - p1.x)) / d;
  return { x: ux, y: uy };
}

function isCounterClockwise(a1: number, aMid: number, a2: number): boolean {
  const norm = (a: number) => ((a % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI);
  const n1 = norm(a1), nM = norm(aMid), n2 = norm(a2);
  return n1 < n2 ? !(nM >= n1 && nM <= n2) : (nM >= n2 && nM <= n1);
}

const txt = (s: string) => s.replace(/\{slash\}/g, "/");

function drawGraphicTransformed(
  ctx: CanvasRenderingContext2D, g: Graphic, C: Colors,
  sx: number, sy: number, rot: number, mx: boolean, my: boolean,
) {
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

function drawTextProp(
  ctx: CanvasRenderingContext2D, text: string, prop: TextPropData, color: string, bold: boolean,
) {
  ctx.fillStyle = color;
  ctx.font = (bold ? "bold " : "") + prop.font_size + "px sans-serif";
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

function renderSchematicToCanvas(
  data: SchematicData, canvas: HTMLCanvasElement,
  opts: { showGrid: boolean; colorMode: "color" | "monochrome" },
) {
  const ctx = canvas.getContext("2d") as CanvasRenderingContext2D;
  if (!ctx) throw new Error("Failed to create 2D context");
  const C = opts.colorMode === "monochrome" ? PRINT : DARK;
  const [pw, ph] = PAPER[data.paper_size] || PAPER.A4;
  const scale = Math.min(canvas.width / pw, canvas.height / ph);
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = C.bg;
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.save();
  ctx.scale(scale, scale);

  // Paper
  ctx.fillStyle = C.paper;
  ctx.fillRect(0, 0, pw, ph);
  ctx.strokeStyle = C.paperBorder;
  ctx.lineWidth = 0.3;
  ctx.strokeRect(0, 0, pw, ph);
  ctx.lineWidth = 0.15;
  ctx.strokeRect(pw - 100, ph - 30, 100, 30);

  // Title block fields
  {
    const tbx = pw - 100, tby = ph - 30;
    const tb = data.title_block || {};
    ctx.strokeStyle = C.paperBorder;
    ctx.lineWidth = 0.08;
    ctx.beginPath();
    ctx.moveTo(tbx, tby + 10); ctx.lineTo(tbx + 100, tby + 10);
    ctx.moveTo(tbx, tby + 20); ctx.lineTo(tbx + 100, tby + 20);
    ctx.moveTo(tbx + 50, tby); ctx.lineTo(tbx + 50, tby + 10);
    ctx.stroke();
    ctx.fillStyle = C.ref;
    ctx.font = "0.8px sans-serif";
    ctx.textAlign = "left";
    ctx.textBaseline = "top";
    ctx.fillText("Title:", tbx + 1, tby + 1);
    ctx.fillText("Date:", tbx + 51, tby + 1);
    ctx.fillText("Rev:", tbx + 1, tby + 11);
    ctx.fillText("Company:", tbx + 51, tby + 11);
    ctx.fillStyle = C.val;
    ctx.font = "bold 1.5px sans-serif";
    ctx.textBaseline = "middle";
    ctx.fillText(substituteSpecialStrings(tb.title || "", data), tbx + 1, tby + 25);
    ctx.font = "1.2px sans-serif";
    ctx.fillText(tb.title || "", tbx + 8, tby + 5);
    ctx.fillText(tb.date || "", tbx + 58, tby + 5);
    ctx.fillText(tb.rev || "", tbx + 8, tby + 15);
    ctx.fillText(tb.company || "", tbx + 63, tby + 15);
  }

  // Grid
  if (opts.showGrid) {
    const gs = 1.27;
    ctx.globalAlpha = 0.4;
    for (let i = 0; i * gs <= pw; i++) {
      const gx = i * gs; const maj = i % 10 === 0;
      ctx.strokeStyle = maj ? C.gridMajor : C.grid;
      ctx.lineWidth = maj ? 0.06 : 0.02;
      ctx.beginPath(); ctx.moveTo(gx, 0); ctx.lineTo(gx, ph); ctx.stroke();
    }
    for (let i = 0; i * gs <= ph; i++) {
      const gy = i * gs; const maj = i % 10 === 0;
      ctx.strokeStyle = maj ? C.gridMajor : C.grid;
      ctx.lineWidth = maj ? 0.06 : 0.02;
      ctx.beginPath(); ctx.moveTo(0, gy); ctx.lineTo(pw, gy); ctx.stroke();
    }
    ctx.globalAlpha = 1;
  }

  // Wires
  ctx.strokeStyle = C.wire; ctx.lineWidth = 0.15; ctx.lineCap = "round";
  for (const wire of data.wires) {
    ctx.beginPath(); ctx.moveTo(wire.start.x, wire.start.y);
    ctx.lineTo(wire.end.x, wire.end.y); ctx.stroke();
  }
  ctx.lineCap = "butt";

  // Junctions
  ctx.fillStyle = C.junction;
  for (const j of data.junctions) {
    ctx.beginPath(); ctx.arc(j.position.x, j.position.y, 0.3, 0, Math.PI * 2); ctx.fill();
  }

  // No-connects
  ctx.lineWidth = 0.2;
  for (const nc of data.no_connects) {
    ctx.strokeStyle = C.noConnect; ctx.beginPath();
    ctx.moveTo(nc.position.x - 0.7, nc.position.y - 0.7);
    ctx.lineTo(nc.position.x + 0.7, nc.position.y + 0.7);
    ctx.moveTo(nc.position.x + 0.7, nc.position.y - 0.7);
    ctx.lineTo(nc.position.x - 0.7, nc.position.y + 0.7);
    ctx.stroke();
  }

  // No ERC directives
  if (data.no_erc_directives) {
    for (const d of data.no_erc_directives) {
      ctx.strokeStyle = "#66bb6a"; ctx.fillStyle = "rgba(102,187,106,0.15)";
      ctx.lineWidth = 0.15; ctx.beginPath();
      ctx.arc(d.position.x, d.position.y, 0.5, 0, Math.PI * 2);
      ctx.fill(); ctx.stroke();
      ctx.strokeStyle = "#66bb6a"; ctx.lineWidth = 0.15; ctx.beginPath();
      ctx.moveTo(d.position.x - 0.2, d.position.y);
      ctx.lineTo(d.position.x - 0.05, d.position.y + 0.2);
      ctx.lineTo(d.position.x + 0.25, d.position.y - 0.15);
      ctx.stroke();
    }
  }

  // Symbols
  for (const sym of data.symbols) {
    const lib = data.lib_symbols[sym.lib_id];
    if (!lib) continue;
    const sx = sym.position.x, sy = sym.position.y;
    const rot = sym.rotation, mx = sym.mirror_x, my = sym.mirror_y;
    ctx.strokeStyle = sym.is_power ? C.power : C.body;
    for (const g of lib.graphics) drawGraphicTransformed(ctx, g, C, sx, sy, rot, mx, my);

    for (const pin of lib.pins) {
      const [px, py] = symToSch(pin.position.x, pin.position.y, sx, sy, rot, mx, my);
      const pe = pinEnd(pin);
      const [ex, ey] = symToSch(pe.x, pe.y, sx, sy, rot, mx, my);
      ctx.strokeStyle = C.pin; ctx.lineWidth = 0.1;
      ctx.beginPath(); ctx.moveTo(px, py); ctx.lineTo(ex, ey); ctx.stroke();

      if (lib.show_pin_numbers && pin.number_visible && pin.number !== "~") {
        ctx.fillStyle = C.pinNum; ctx.font = "1.0px sans-serif";
        ctx.textAlign = "center"; ctx.textBaseline = "bottom";
        const nmx = (px + ex) / 2, nmy = (py + ey) / 2;
        const dx = ex - px, dy = ey - py, len = Math.hypot(dx, dy) || 1;
        ctx.fillText(txt(pin.number), nmx - dy / len * 0.5, nmy + dx / len * 0.5);
      }
      if (lib.show_pin_names && pin.name_visible && pin.name !== "~") {
        ctx.fillStyle = C.pinName; ctx.font = "0.75px sans-serif";
        const dx = ex - px, dy = ey - py, len = Math.hypot(dx, dy) || 1;
        const nx = ex + (dx / len) * 0.4, ny = ey + (dy / len) * 0.4;
        if (Math.abs(dx) > Math.abs(dy)) {
          ctx.textBaseline = "middle"; ctx.textAlign = dx > 0 ? "left" : "right";
        } else { ctx.textAlign = "center"; ctx.textBaseline = dy > 0 ? "top" : "bottom"; }
        ctx.fillText(txt(pin.name), nx, ny);
      }
    }

    if (!sym.is_power) {
      if (!sym.ref_text.hidden) drawTextProp(ctx, txt(sym.reference), sym.ref_text, C.ref, true);
      if (!sym.val_text.hidden) drawTextProp(ctx, txt(sym.value), sym.val_text, C.val, false);
    } else {
      const pt = sym.val_text.hidden ? sym.ref_text : sym.val_text;
      if (!pt.hidden) {
        const pp = { ...pt, rotation: 0 };
        drawTextProp(ctx, txt(sym.value || sym.reference), pp, C.power, true);
      }
    }
  }

  // Labels
  for (const label of data.labels) {
    const color = label.label_type === "Global" ? C.labelGlobal
      : label.label_type === "Hierarchical" ? C.labelHier : C.labelNet;
    const text = txt(label.text);
    const fs = label.font_size || 1.27;
    const r = label.rotation;
    const lx = label.position.x, ly = label.position.y;

    if (label.label_type === "Global" && label.shape) {
      ctx.font = fs + "px sans-serif";
      const tw = ctx.measureText(text).width;
      const h = fs * 1.4, pad = fs * 0.3, arrowW = h * 0.5;
      const isHoriz = r === 0 || r === 180;
      ctx.strokeStyle = color; ctx.lineWidth = 0.15;
      if (isHoriz) {
        const dir = r === 0 ? 1 : -1;
        const bs = dir > 0 ? arrowW : -arrowW;
        const be = dir > 0 ? arrowW + tw + pad * 2 : -arrowW - tw - pad * 2;
        ctx.beginPath();
        if (label.shape === "input") {
          ctx.moveTo(lx, ly); ctx.lineTo(lx + bs, ly - h / 2);
          ctx.lineTo(lx + be, ly - h / 2); ctx.lineTo(lx + be, ly + h / 2);
          ctx.lineTo(lx + bs, ly + h / 2); ctx.closePath();
        } else if (label.shape === "output" || label.shape === "bidirectional") {
          const tip = dir > 0 ? be + arrowW : be - arrowW;
          ctx.moveTo(lx, ly); ctx.lineTo(lx + bs, ly - h / 2);
          ctx.lineTo(lx + be, ly - h / 2); ctx.lineTo(lx + tip, ly);
          ctx.lineTo(lx + be, ly + h / 2); ctx.lineTo(lx + bs, ly + h / 2); ctx.closePath();
        } else {
          const x1 = Math.min(lx + bs, lx + be);
          ctx.rect(x1, ly - h / 2, Math.abs(be - bs), h);
        }
        ctx.stroke();
        ctx.fillStyle = color; ctx.font = fs + "px sans-serif"; ctx.textBaseline = "middle";
        if (dir > 0) { ctx.textAlign = "left"; ctx.fillText(text, lx + arrowW + pad, ly); }
        else { ctx.textAlign = "right"; ctx.fillText(text, lx - arrowW - pad, ly); }
      } else {
        ctx.save(); ctx.translate(lx, ly);
        ctx.rotate(r === 90 ? -Math.PI / 2 : Math.PI / 2);
        ctx.beginPath(); ctx.moveTo(0, 0);
        ctx.lineTo(arrowW, -h / 2); ctx.lineTo(arrowW + tw + pad * 2, -h / 2);
        ctx.lineTo(arrowW + tw + pad * 2, h / 2); ctx.lineTo(arrowW, h / 2);
        ctx.closePath(); ctx.stroke();
        ctx.fillStyle = color; ctx.font = fs + "px sans-serif";
        ctx.textAlign = "left"; ctx.textBaseline = "middle";
        ctx.fillText(text, arrowW + pad, 0); ctx.restore();
      }
    } else {
      ctx.fillStyle = color; ctx.font = fs + "px sans-serif";
      let rot = r;
      let jh: string = label.justify === "right" ? "right" : "left";
      if (rot === 180) { rot = 0; jh = jh === "left" ? "right" : "left"; }
      if (rot === 270) { rot = 90; jh = jh === "left" ? "right" : "left"; }
      ctx.textAlign = jh as CanvasTextAlign; ctx.textBaseline = "bottom";
      if (rot === 90) {
        ctx.save(); ctx.translate(lx, ly); ctx.rotate(-Math.PI / 2);
        ctx.fillText(text, 0.3, 0); ctx.restore();
      } else { ctx.fillText(text, lx, ly - 0.3); }
    }
  }

  // Child sheets
  for (const sheet of data.child_sheets) {
    const sx = sheet.position.x, sy = sheet.position.y, sw = sheet.size[0], sh = sheet.size[1];
    ctx.fillStyle = "rgba(91,141,239,0.03)"; ctx.fillRect(sx, sy, sw, sh);
    ctx.strokeStyle = C.sheet; ctx.lineWidth = 0.2; ctx.strokeRect(sx, sy, sw, sh);
    ctx.fillStyle = C.sheetText; ctx.font = "bold 1.2px sans-serif";
    ctx.textAlign = "left"; ctx.textBaseline = "bottom";
    ctx.fillText(sheet.name, sx + 0.5, sy - 0.3);
    ctx.fillStyle = C.sheet; ctx.font = "0.8px monospace"; ctx.textBaseline = "top";
    ctx.fillText(sheet.filename, sx + 0.5, sy + 0.5);
    if (sheet.pins && sheet.pins.length > 0) {
      ctx.fillStyle = C.labelHier; ctx.font = "0.9px sans-serif"; ctx.textBaseline = "middle";
      for (const pin of sheet.pins) {
        const px = pin.position.x, py = pin.position.y;
        ctx.beginPath();
        if (px <= sx + 0.1) {
          ctx.moveTo(px, py); ctx.lineTo(px + 0.8, py - 0.4); ctx.lineTo(px + 0.8, py + 0.4);
          ctx.textAlign = "left"; ctx.fillText(pin.name, px + 1.2, py);
        } else {
          ctx.moveTo(px, py); ctx.lineTo(px - 0.8, py - 0.4); ctx.lineTo(px - 0.8, py + 0.4);
          ctx.textAlign = "right"; ctx.fillText(pin.name, px - 1.2, py);
        }
        ctx.closePath(); ctx.fill();
      }
    }
  }

  // Buses
  ctx.lineWidth = 0.4;
  for (const b of data.buses) {
    ctx.strokeStyle = C.bus; ctx.beginPath();
    ctx.moveTo(b.start.x, b.start.y); ctx.lineTo(b.end.x, b.end.y); ctx.stroke();
  }

  // Bus entries
  ctx.lineWidth = 0.2;
  for (const be of data.bus_entries) {
    ctx.strokeStyle = C.busEntry; ctx.beginPath();
    ctx.moveTo(be.position.x, be.position.y);
    ctx.lineTo(be.position.x + be.size[0], be.position.y + be.size[1]); ctx.stroke();
  }

  // Rectangles
  for (const r of data.rectangles) {
    const rx = Math.min(r.start.x, r.end.x), ry = Math.min(r.start.y, r.end.y);
    const rw = Math.abs(r.end.x - r.start.x), rh = Math.abs(r.end.y - r.start.y);
    ctx.strokeStyle = C.sheet; ctx.lineWidth = 0.15;
    if (r.stroke_type === "dash") ctx.setLineDash([1.0, 0.5]);
    else if (r.stroke_type === "dash_dot") ctx.setLineDash([1.0, 0.3, 0.2, 0.3]);
    else if (r.stroke_type === "dot") ctx.setLineDash([0.2, 0.3]);
    else ctx.setLineDash([]);
    ctx.strokeRect(rx, ry, rw, rh); ctx.setLineDash([]);
  }

  // Drawing objects
  for (const d of data.drawings) {
    ctx.strokeStyle = C.body; ctx.lineWidth = Math.max("width" in d ? d.width || 0.15 : 0.15, 0.15);
    if (d.type === "Line") {
      ctx.beginPath(); ctx.moveTo(d.start.x, d.start.y); ctx.lineTo(d.end.x, d.end.y); ctx.stroke();
    } else if (d.type === "Rect") {
      const rx = Math.min(d.start.x, d.end.x), ry = Math.min(d.start.y, d.end.y);
      const rw = Math.abs(d.end.x - d.start.x), rh = Math.abs(d.end.y - d.start.y);
      if (d.fill) { ctx.fillStyle = C.bodyFill; ctx.fillRect(rx, ry, rw, rh); }
      ctx.strokeRect(rx, ry, rw, rh);
    } else if (d.type === "Circle") {
      ctx.beginPath(); ctx.arc(d.center.x, d.center.y, d.radius, 0, Math.PI * 2);
      if (d.fill) { ctx.fillStyle = C.bodyFill; ctx.fill(); } ctx.stroke();
    } else if (d.type === "Arc") {
      ctx.beginPath(); ctx.moveTo(d.start.x, d.start.y);
      ctx.quadraticCurveTo(d.mid.x, d.mid.y, d.end.x, d.end.y); ctx.stroke();
    } else if (d.type === "Polyline") {
      if (d.points.length > 1) {
        ctx.beginPath(); ctx.moveTo(d.points[0].x, d.points[0].y);
        for (let i = 1; i < d.points.length; i++) ctx.lineTo(d.points[i].x, d.points[i].y);
        if (d.fill) { ctx.closePath(); ctx.fillStyle = C.bodyFill; ctx.fill(); }
        ctx.stroke();
      }
    }
  }

  // Text notes
  for (const note of data.text_notes) {
    ctx.fillStyle = C.sheetText; ctx.font = note.font_size + "px sans-serif";
    ctx.textAlign = "left"; ctx.textBaseline = "top";
    if (note.rotation === 90 || note.rotation === 270) {
      ctx.save(); ctx.translate(note.position.x, note.position.y); ctx.rotate(-Math.PI / 2);
      const lines = substituteSpecialStrings(note.text, data).split("\n");
      lines.forEach((line: string, i: number) => { ctx.fillText(line, 0, i * note.font_size * 1.3); });
      ctx.restore();
    } else {
      const lines = substituteSpecialStrings(note.text, data).split("\n");
      lines.forEach((line: string, i: number) => {
        ctx.fillText(line, note.position.x, note.position.y + i * note.font_size * 1.3);
      });
    }
  }

  ctx.restore();
}

export interface PdfExportOptions {
  dpi?: number;
  showGrid?: boolean;
  colorMode?: "color" | "monochrome";
}

export async function exportSchematicPdf(
  data: SchematicData, options: PdfExportOptions = {},
): Promise<Blob> {
  const dpi = options.dpi ?? 300;
  const showGrid = options.showGrid ?? false;
  const colorMode = options.colorMode ?? "color";
  const [pw, ph] = PAPER[data.paper_size] || PAPER.A4;
  const pxW = Math.round(pw * (dpi / 25.4));
  const pxH = Math.round(ph * (dpi / 25.4));
  const canvas = document.createElement("canvas");
  canvas.width = pxW; canvas.height = pxH;
  renderSchematicToCanvas(data, canvas, { showGrid, colorMode });
  const orientation = pw > ph ? "landscape" : "portrait";
  const doc = new jsPDF({ orientation, unit: "mm", format: [pw, ph] });
  const imgData = canvas.toDataURL("image/png");
  doc.addImage(imgData, "PNG", 0, 0, pw, ph);
  return doc.output("blob");
}

export async function exportMultiSheetPdf(
  sheets: SchematicData[],
  options: PdfExportOptions = {},
): Promise<Blob> {
  const dpi = options.dpi ?? 300;
  const showGrid = options.showGrid ?? false;
  const colorMode = options.colorMode ?? "color";

  // Use first sheet's paper size for all pages
  const [pw, ph] = PAPER[sheets[0]?.paper_size] || PAPER.A4;
  const pxW = Math.round(pw * (dpi / 25.4));
  const pxH = Math.round(ph * (dpi / 25.4));
  const orientation = pw > ph ? "landscape" : "portrait";
  const doc = new jsPDF({ orientation, unit: "mm", format: [pw, ph] });

  for (let i = 0; i < sheets.length; i++) {
    if (i > 0) doc.addPage([pw, ph], orientation);
    const canvas = document.createElement("canvas");
    canvas.width = pxW; canvas.height = pxH;
    renderSchematicToCanvas(sheets[i], canvas, { showGrid, colorMode });
    const imgData = canvas.toDataURL("image/png");
    doc.addImage(imgData, "PNG", 0, 0, pw, ph);
  }

  return doc.output("blob");
}

export async function printSchematic(data: SchematicData): Promise<void> {
  const [pw, ph] = PAPER[data.paper_size] || PAPER.A4;
  const pxW = Math.round(pw * (150 / 25.4));
  const pxH = Math.round(ph * (150 / 25.4));
  const canvas = document.createElement("canvas");
  canvas.width = pxW; canvas.height = pxH;
  renderSchematicToCanvas(data, canvas, { showGrid: false, colorMode: "monochrome" });
  const imgData = canvas.toDataURL("image/png");
  const w = window.open("", "_blank");
  if (!w) { alert("Pop-up blocked. Please allow pop-ups for printing."); return; }
  w.document.write(
    "<!DOCTYPE html><html><head><title>Print Schematic</title><style>" +
    "@page { size: " + pw + "mm " + ph + "mm; margin: 0; } " +
    "* { margin: 0; padding: 0; } " +
    "body { display: flex; justify-content: center; align-items: center; } " +
    "img { width: 100vw; height: auto; max-height: 100vh; object-fit: contain; }" +
    "</style></head><body>" +
    "<img src=\"" + imgData + "\" onload=\"window.print();window.close();\" />" +
    "</body></html>"
  );
  w.document.close();
}

export function getPaperSize(paperSize: string): [number, number] {
  return PAPER[paperSize] || PAPER.A4;
}
