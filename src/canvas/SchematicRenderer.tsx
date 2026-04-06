import { useRef, useEffect, useCallback, useState } from "react";
import { useEditorStore } from "@/stores/editor";
import { useSchematicStore, snapPoint } from "@/stores/schematic";
import { useLayoutStore } from "@/stores/layout";
import { hitTest, boxSelect } from "./hitTest";
import { FindReplace } from "@/components/FindReplace";
import type { Graphic, SchematicData, SchPin, SchPoint, TextPropData } from "@/types";
interface Camera { x: number; y: number; zoom: number }

const PAPER: Record<string, [number, number]> = {
  A4: [297, 210], A3: [420, 297], A2: [594, 420], A1: [841, 594], A0: [1189, 841],
  A: [279.4, 215.9], B: [431.8, 279.4], C: [558.8, 431.8], D: [863.6, 558.8],
};

const C = {
  bg: "#1a1b2e", paper: "#1e2035", paperBorder: "#2a2d4a",
  grid: "#222440", gridMajor: "#2a2d4a",
  wire: "#4fc3f7", junction: "#4fc3f7",
  body: "#9fa8da", bodyFill: "#1e2035",
  pin: "#81c784", pinName: "#90a4ae", pinNum: "#607d8b",
  ref: "#e8c66a", val: "#9598b3",
  labelNet: "#81c784", labelGlobal: "#ff8a65", labelHier: "#ba68c8",
  sheet: "#5b8def", sheetText: "#cdd6f4",
  noConnect: "#e8667a", power: "#ef5350",
};

const txt = (s: string) => s.replace(/\{slash\}/g, "/");

// Transform a point from symbol-local (Y-up) to schematic (Y-down) space
function symToSch(lx: number, ly: number, sx: number, sy: number, rot: number, mx: boolean, my: boolean): [number, number] {
  // 1. Flip Y (symbol Y-up → screen Y-down)
  const x = lx;
  const y = -ly;

  // 2. Rotate (KiCad CW in screen space = negate for math CCW)
  const rad = -(rot * Math.PI) / 180;
  const cos = Math.cos(rad);
  const sin = Math.sin(rad);
  let rx = x * cos - y * sin;
  let ry = x * sin + y * cos;

  // 3. Mirror AFTER rotation (KiCad applies mirror post-rotation)
  if (mx) ry = -ry;
  if (my) rx = -rx;

  return [sx + rx, sy + ry];
}

// Pin end position in symbol-local space
function pinEnd(pin: SchPin): SchPoint {
  const rad = (pin.rotation * Math.PI) / 180;
  return {
    x: pin.position.x + Math.cos(rad) * pin.length,
    y: pin.position.y + Math.sin(rad) * pin.length,
  };
}

const ELECTRICAL_SNAP_RANGE = 2.0; // World units — snap to pins/wire endpoints within this range

/** Find nearest pin endpoint or wire endpoint for electrical snapping */
function findNearestElectricalPoint(
  data: SchematicData, worldX: number, worldY: number
): SchPoint | null {
  let bestDist = ELECTRICAL_SNAP_RANGE;
  let bestPoint: SchPoint | null = null;

  // Check all symbol pin endpoints
  for (const sym of data.symbols) {
    const lib = data.lib_symbols[sym.lib_id];
    if (!lib) continue;
    for (const pin of lib.pins) {
      const [px, py] = symToSch(pin.position.x, pin.position.y,
        sym.position.x, sym.position.y, sym.rotation, sym.mirror_x, sym.mirror_y);
      const d = Math.hypot(worldX - px, worldY - py);
      if (d < bestDist) { bestDist = d; bestPoint = { x: px, y: py }; }
    }
  }

  // Check wire endpoints
  for (const wire of data.wires) {
    for (const pt of [wire.start, wire.end]) {
      const d = Math.hypot(worldX - pt.x, worldY - pt.y);
      if (d < bestDist) { bestDist = d; bestPoint = { x: pt.x, y: pt.y }; }
    }
  }

  return bestPoint;
}

export function SchematicRenderer() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const camRef = useRef<Camera>({ x: 0, y: 0, zoom: 3 });
  const dragging = useRef(false);
  const moving = useRef(false);
  const selecting = useRef(false); // Drag-box selection active
  const selectStart = useRef({ x: 0, y: 0 }); // Drag-box start (world coords)
  const selectEnd = useRef({ x: 0, y: 0 }); // Drag-box end (world coords)
  const moveStart = useRef({ x: 0, y: 0 });
  const lastMouse = useRef({ x: 0, y: 0 });
  const animRef = useRef(0);
  const wireCursorRef = useRef<SchPoint>({ x: 0, y: 0 }); // Ref for live wire cursor — no Zustand churn
  const placeCursorRef = useRef<SchPoint>({ x: 0, y: 0 }); // Ref for placement cursor
  const updateStatusBar = useEditorStore((s) => s.updateStatusBar);

  // Schematic store
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);
  const editMode = useSchematicStore((s) => s.editMode);
  const wireDrawing = useSchematicStore((s) => s.wireDrawing);
  const placingSymbol = useSchematicStore((s) => s.placingSymbol);

  // Find/Replace state
  const [findOpen, setFindOpen] = useState(false);
  const [findShowReplace, setFindShowReplace] = useState(false);

  // In-place text editing state
  const [inPlaceEdit, setInPlaceEdit] = useState<{
    uuid: string; field: string; value: string;
    screenX: number; screenY: number;
  } | null>(null);

  const s2w = useCallback((sx: number, sy: number) => {
    const c = camRef.current;
    return { x: (sx - c.x) / c.zoom, y: (sy - c.y) / c.zoom };
  }, []);

  // World to screen
  const w2s = useCallback((wx: number, wy: number) => {
    const c = camRef.current;
    return { x: wx * c.zoom + c.x, y: wy * c.zoom + c.y };
  }, []);

  const drawGraphicTransformed = useCallback((
    ctx: CanvasRenderingContext2D, g: Graphic,
    sx: number, sy: number, rot: number, mx: boolean, my: boolean
  ) => {
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
        // Always fill rectangles with paper bg to make body opaque
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
  }, []);

  // Draw text using exact KiCad property position, rotation, justify, and font size.
  // KiCad normalizes text to always be readable (never upside-down).
  const drawTextProp = useCallback((
    ctx: CanvasRenderingContext2D, text: string, prop: TextPropData, color: string, bold: boolean
  ) => {
    ctx.fillStyle = color;
    ctx.font = `${bold ? "bold " : ""}${prop.font_size}px Roboto`;

    let jh = prop.justify_h;
    let jv = prop.justify_v;
    let rot = prop.rotation;

    // KiCad keeps text readable: normalize 180° → 0° and 270° → 90° with flipped justify
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

  }, []);

  const render = useCallback(() => {
    const canvas = canvasRef.current;
    const container = containerRef.current;
    if (!canvas || !container) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = container.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    canvas.style.width = `${rect.width}px`;
    canvas.style.height = `${rect.height}px`;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(dpr, dpr);
    const cam = camRef.current;
    const w = rect.width, h = rect.height;

    ctx.fillStyle = C.bg;
    ctx.fillRect(0, 0, w, h);

    if (!data) return;
    const [pw, ph] = PAPER[data.paper_size] || PAPER.A4;

    ctx.save();
    ctx.translate(cam.x, cam.y);
    ctx.scale(cam.zoom, cam.zoom);

    // Paper
    ctx.fillStyle = C.paper;
    ctx.fillRect(0, 0, pw, ph);
    ctx.strokeStyle = C.paperBorder;
    ctx.lineWidth = 0.3;
    ctx.strokeRect(0, 0, pw, ph);
    ctx.lineWidth = 0.15;
    ctx.strokeRect(pw - 100, ph - 30, 100, 30);

    // Grid (only if zoomed enough and visible)
    const editorState = useEditorStore.getState();
    const gridSize = editorState.statusBar.gridSize;
    if (editorState.gridVisible && gridSize * cam.zoom > 6) {
      ctx.globalAlpha = 0.4;
      for (let gx = 0; gx <= pw; gx += gridSize) {
        const maj = Math.abs(gx % (gridSize * 10)) < 0.01;
        ctx.strokeStyle = maj ? C.gridMajor : C.grid;
        ctx.lineWidth = maj ? 0.06 : 0.02;
        ctx.beginPath(); ctx.moveTo(gx, 0); ctx.lineTo(gx, ph); ctx.stroke();
      }
      for (let gy = 0; gy <= ph; gy += gridSize) {
        const maj = Math.abs(gy % (gridSize * 10)) < 0.01;
        ctx.strokeStyle = maj ? C.gridMajor : C.grid;
        ctx.lineWidth = maj ? 0.06 : 0.02;
        ctx.beginPath(); ctx.moveTo(0, gy); ctx.lineTo(pw, gy); ctx.stroke();
      }
      ctx.globalAlpha = 1;
    }

    // Wires
    ctx.strokeStyle = C.wire;
    ctx.lineWidth = 0.15;
    ctx.lineCap = "round";
    for (const wire of data.wires) {
      ctx.beginPath();
      ctx.moveTo(wire.start.x, wire.start.y);
      ctx.lineTo(wire.end.x, wire.end.y);
      ctx.stroke();
    }

    // Junctions
    ctx.fillStyle = C.junction;
    for (const j of data.junctions) {
      ctx.beginPath();
      ctx.arc(j.position.x, j.position.y, 0.3, 0, Math.PI * 2);
      ctx.fill();
    }

    // No-connects
    ctx.strokeStyle = C.noConnect;
    ctx.lineWidth = 0.2;
    for (const nc of data.no_connects) {
      ctx.beginPath();
      ctx.moveTo(nc.x - 0.7, nc.y - 0.7); ctx.lineTo(nc.x + 0.7, nc.y + 0.7);
      ctx.moveTo(nc.x + 0.7, nc.y - 0.7); ctx.lineTo(nc.x - 0.7, nc.y + 0.7);
      ctx.stroke();
    }

    // --- Symbols ---
    for (const sym of data.symbols) {
      const lib = data.lib_symbols[sym.lib_id];
      if (!lib) continue;

      const sx = sym.position.x, sy = sym.position.y;
      const rot = sym.rotation, mx = sym.mirror_x, my = sym.mirror_y;

      // Draw graphics (transformed from symbol-local Y-up to screen Y-down)
      ctx.strokeStyle = sym.is_power ? C.power : C.body;
      for (const g of lib.graphics) {
        drawGraphicTransformed(ctx, g, sx, sy, rot, mx, my);
      }

      // Draw pins — all in screen space, no canvas transform needed
      for (const pin of lib.pins) {
        const [px, py] = symToSch(pin.position.x, pin.position.y, sx, sy, rot, mx, my);
        const pe = pinEnd(pin);
        const [ex, ey] = symToSch(pe.x, pe.y, sx, sy, rot, mx, my);

        // Pin line
        ctx.strokeStyle = C.pin;
        ctx.lineWidth = 0.1;
        ctx.beginPath();
        ctx.moveTo(px, py);
        ctx.lineTo(ex, ey);
        ctx.stroke();

        // Pin number (midpoint of pin line) — respect lib symbol visibility
        if (lib.show_pin_numbers && pin.number_visible && pin.number !== "~") {
          ctx.fillStyle = C.pinNum;
          ctx.font = "1.0px Roboto";
          ctx.textAlign = "center";
          ctx.textBaseline = "bottom";
          const nmx = (px + ex) / 2, nmy = (py + ey) / 2;
          // Offset perpendicular to pin direction
          const dx = ex - px, dy = ey - py;
          const len = Math.hypot(dx, dy) || 1;
          ctx.fillText(txt(pin.number), nmx - dy / len * 0.5, nmy + dx / len * 0.5);
        }

        // Pin name (at inner end, toward body) — respect lib symbol visibility
        if (lib.show_pin_names && pin.name_visible && pin.name !== "~") {
          ctx.fillStyle = C.pinName;
          ctx.font = "0.75px Roboto";
          const dx = ex - px, dy = ey - py;
          const len = Math.hypot(dx, dy) || 1;
          // Name is drawn at pin end (inside body), along pin direction
          const offset = 0.4;
          const nx = ex + (dx / len) * offset;
          const ny = ey + (dy / len) * offset;

          // Determine text alignment based on pin direction
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

      // Reference & value text at their exact KiCad positions with justify
      if (!sym.is_power) {
        if (!sym.ref_text.hidden) {
          drawTextProp(ctx, txt(sym.reference), sym.ref_text, C.ref, true);
        }
        if (!sym.val_text.hidden) {
          drawTextProp(ctx, txt(sym.value), sym.val_text, C.val, false);
        }
      } else {
        // Power symbol: render value text ALWAYS horizontal (KiCad behavior)
        const powerText = sym.val_text.hidden ? sym.ref_text : sym.val_text;
        if (!powerText.hidden) {
          const powerProp = { ...powerText, rotation: 0 }; // Force horizontal
          drawTextProp(ctx, txt(sym.value || sym.reference), powerProp, C.power, true);
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

      // Draw global label shape (flag/arrow)
      if (label.label_type === "Global" && label.shape) {
        ctx.font = `${fs}px Roboto`;
        const tw = ctx.measureText(text).width;
        const h = fs * 1.4;
        const pad = fs * 0.3;
        const arrowW = h * 0.5;

        // Determine shape direction based on rotation:
        // 0° = connection LEFT, text right. 180° = connection RIGHT, text left.
        // 90° = connection TOP. 270° = connection BOTTOM.
        const isHoriz = r === 0 || r === 180;
        const connRight = r === 0;   // connection point is on the left, shape extends right

        ctx.strokeStyle = color;
        ctx.lineWidth = 0.15;

        if (isHoriz) {
          // Draw shape horizontally — text always reads L→R
          const dir = connRight ? 1 : -1; // shape extends in this direction from connection
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

          // Text — always L→R
          ctx.fillStyle = color;
          ctx.font = `${fs}px Roboto`;
          ctx.textBaseline = "middle";
          if (dir > 0) {
            ctx.textAlign = "left";
            ctx.fillText(text, lx + arrowW + pad, ly);
          } else {
            ctx.textAlign = "right";
            ctx.fillText(text, lx - arrowW - pad, ly);
          }
        } else {
          // Vertical labels (90°, 270°) — draw rotated but text still readable
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
        // Net label or label without shape — simple text with overline
        ctx.fillStyle = color;
        ctx.font = `${fs}px Roboto`;

        // Normalize rotation for readability
        let rot = r;
        let jh = label.justify === "right" ? "right" : "left";
        if (rot === 180) { rot = 0; jh = jh === "left" ? "right" : "left"; }
        if (rot === 270) { rot = 90; jh = jh === "left" ? "right" : "left"; }

        ctx.textAlign = jh as CanvasTextAlign;
        ctx.textBaseline = "bottom";

        if (rot === 90) {
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

    // Child sheets
    ctx.setLineDash([0.5, 0.3]);
    for (const sheet of data.child_sheets) {
      ctx.strokeStyle = C.sheet; ctx.lineWidth = 0.2;
      ctx.strokeRect(sheet.position.x, sheet.position.y, sheet.size[0], sheet.size[1]);
      ctx.fillStyle = C.sheetText;
      ctx.font = "bold 1.2px Roboto"; ctx.textAlign = "left"; ctx.textBaseline = "bottom";
      ctx.fillText(sheet.name, sheet.position.x + 0.5, sheet.position.y - 0.3);
      ctx.fillStyle = C.sheet;
      ctx.font = "0.8px Roboto Mono"; ctx.textBaseline = "top";
      ctx.fillText(sheet.filename, sheet.position.x + 0.5, sheet.position.y + 0.5);
    }
    ctx.setLineDash([]);

    // Top-level rectangles (dashed section boxes)
    for (const r of data.rectangles) {
      const rx = Math.min(r.start.x, r.end.x);
      const ry = Math.min(r.start.y, r.end.y);
      const rw = Math.abs(r.end.x - r.start.x);
      const rh = Math.abs(r.end.y - r.start.y);
      ctx.strokeStyle = C.sheet;
      ctx.lineWidth = 0.15;
      if (r.stroke_type === "dash") {
        ctx.setLineDash([1.0, 0.5]);
      } else if (r.stroke_type === "dash_dot") {
        ctx.setLineDash([1.0, 0.3, 0.2, 0.3]);
      } else if (r.stroke_type === "dot") {
        ctx.setLineDash([0.2, 0.3]);
      } else {
        ctx.setLineDash([]);
      }
      ctx.strokeRect(rx, ry, rw, rh);
      ctx.setLineDash([]);
    }

    // Text notes
    for (const note of data.text_notes) {
      ctx.fillStyle = C.sheetText;
      ctx.font = `${note.font_size}px Roboto`;
      ctx.textAlign = "left";
      ctx.textBaseline = "top";
      if (note.rotation === 90 || note.rotation === 270) {
        ctx.save();
        ctx.translate(note.position.x, note.position.y);
        ctx.rotate(-Math.PI / 2);
        // Handle multiline
        const lines = note.text.split("\n");
        lines.forEach((line, i) => {
          ctx.fillText(line, 0, i * note.font_size * 1.3);
        });
        ctx.restore();
      } else {
        const lines = note.text.split("\n");
        lines.forEach((line, i) => {
          ctx.fillText(line, note.position.x, note.position.y + i * note.font_size * 1.3);
        });
      }
    }

    // --- Selection highlights (already in world-space transform) ---
    if (selectedIds.size > 0) {
      ctx.strokeStyle = "#00bfff";
      ctx.lineWidth = 0.3;
      ctx.setLineDash([0.5, 0.3]);

      for (const sym of data.symbols) {
        if (!selectedIds.has(sym.uuid)) continue;
        const lib = data.lib_symbols[sym.lib_id];
        if (!lib) continue;
        // Calculate tight bounding box from body graphics only
        let minX = -2, maxX = 2, minY = -2, maxY = 2;
        for (const g of lib.graphics) {
          if (g.type === "Rectangle") {
            minX = Math.min(minX, g.start.x, g.end.x);
            maxX = Math.max(maxX, g.start.x, g.end.x);
            minY = Math.min(minY, g.start.y, g.end.y);
            maxY = Math.max(maxY, g.start.y, g.end.y);
          } else if (g.type === "Polyline") {
            for (const p of g.points) {
              minX = Math.min(minX, p.x); maxX = Math.max(maxX, p.x);
              minY = Math.min(minY, p.y); maxY = Math.max(maxY, p.y);
            }
          } else if (g.type === "Circle") {
            minX = Math.min(minX, g.center.x - g.radius);
            maxX = Math.max(maxX, g.center.x + g.radius);
            minY = Math.min(minY, g.center.y - g.radius);
            maxY = Math.max(maxY, g.center.y + g.radius);
          }
        }
        // Transform bounds to screen space
        const pad = 1;
        const corners = [
          symToSch(minX - pad, minY - pad, sym.position.x, sym.position.y, sym.rotation, sym.mirror_x, sym.mirror_y),
          symToSch(maxX + pad, minY - pad, sym.position.x, sym.position.y, sym.rotation, sym.mirror_x, sym.mirror_y),
          symToSch(maxX + pad, maxY + pad, sym.position.x, sym.position.y, sym.rotation, sym.mirror_x, sym.mirror_y),
          symToSch(minX - pad, maxY + pad, sym.position.x, sym.position.y, sym.rotation, sym.mirror_x, sym.mirror_y),
        ];
        const bx = Math.min(...corners.map(c => c[0]));
        const by = Math.min(...corners.map(c => c[1]));
        const bw = Math.max(...corners.map(c => c[0])) - bx;
        const bh = Math.max(...corners.map(c => c[1])) - by;
        ctx.strokeRect(bx, by, bw, bh);
      }

      for (const wire of data.wires) {
        if (!selectedIds.has(wire.uuid)) continue;
        ctx.beginPath();
        ctx.moveTo(wire.start.x, wire.start.y);
        ctx.lineTo(wire.end.x, wire.end.y);
        ctx.stroke();
      }

      for (const label of data.labels) {
        if (!selectedIds.has(label.uuid)) continue;
        ctx.strokeRect(label.position.x - 1, label.position.y - 1.5, 10, 2);
      }
      ctx.setLineDash([]);
    }

    // --- Wire drawing preview with live Manhattan routing ---
    if (wireDrawing.active && wireDrawing.points.length > 0) {
      // Draw placed segments (solid)
      ctx.strokeStyle = "#4fc3f7";
      ctx.lineWidth = 0.15;
      ctx.setLineDash([]);
      if (wireDrawing.points.length > 1) {
        ctx.beginPath();
        ctx.moveTo(wireDrawing.points[0].x, wireDrawing.points[0].y);
        for (let i = 1; i < wireDrawing.points.length; i++) {
          ctx.lineTo(wireDrawing.points[i].x, wireDrawing.points[i].y);
        }
        ctx.stroke();
      }

      // Draw live preview from last placed point to cursor
      const last = wireDrawing.points[wireDrawing.points.length - 1];
      const cur = wireCursorRef.current;
      ctx.strokeStyle = "#80deea";
      ctx.lineWidth = 0.15;
      ctx.setLineDash([0.3, 0.2]);
      ctx.beginPath();
      ctx.moveTo(last.x, last.y);

      const rMode = useSchematicStore.getState().wireDrawing.routingMode;
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

      // Cursor crosshair
      ctx.strokeStyle = "#80deea";
      ctx.lineWidth = 0.08;
      ctx.beginPath();
      ctx.moveTo(cur.x - 3, cur.y); ctx.lineTo(cur.x + 3, cur.y);
      ctx.moveTo(cur.x, cur.y - 3); ctx.lineTo(cur.x, cur.y + 3);
      ctx.stroke();

      // Dot at connection point
      ctx.fillStyle = "#80deea";
      ctx.beginPath();
      ctx.arc(last.x, last.y, 0.3, 0, Math.PI * 2);
      ctx.fill();

      // Electrical snap indicator — red cross when snapped to a pin/endpoint
      if (data) {
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
    }

    // --- Placement preview (ghost symbol following cursor) ---
    const placing = useSchematicStore.getState().placingSymbol;
    if (placing) {
      const cur = placeCursorRef.current;
      const rot = placing.rotation;
      const mx = placing.mirrorX;
      const my = placing.mirrorY;

      ctx.globalAlpha = 0.5;
      ctx.strokeStyle = "#4fc3f7";
      for (const g of placing.lib.graphics) {
        drawGraphicTransformed(ctx, g, cur.x, cur.y, rot, mx, my);
      }

      // Draw pins
      for (const pin of placing.lib.pins) {
        const [px, py] = symToSch(pin.position.x, pin.position.y, cur.x, cur.y, rot, mx, my);
        const pe = pinEnd(pin);
        const [ex, ey] = symToSch(pe.x, pe.y, cur.x, cur.y, rot, mx, my);
        ctx.strokeStyle = "#81c784";
        ctx.lineWidth = 0.1;
        ctx.beginPath();
        ctx.moveTo(px, py);
        ctx.lineTo(ex, ey);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;

      // Crosshair at placement point
      ctx.strokeStyle = "#4fc3f7";
      ctx.lineWidth = 0.08;
      ctx.beginPath();
      ctx.moveTo(cur.x - 3, cur.y); ctx.lineTo(cur.x + 3, cur.y);
      ctx.moveTo(cur.x, cur.y - 3); ctx.lineTo(cur.x, cur.y + 3);
      ctx.stroke();
    }

    // --- Drag-box selection rectangle ---
    if (selecting.current) {
      const s = selectStart.current;
      const e = selectEnd.current;
      const rx = Math.min(s.x, e.x), ry = Math.min(s.y, e.y);
      const rw = Math.abs(e.x - s.x), rh = Math.abs(e.y - s.y);
      // Left-to-right = inside selection (solid), right-to-left = crossing (dashed)
      const crossing = e.x < s.x;
      ctx.strokeStyle = crossing ? "#4fc3f7" : "#00bfff";
      ctx.fillStyle = crossing ? "rgba(79,195,247,0.08)" : "rgba(0,191,255,0.08)";
      ctx.lineWidth = 0.2;
      if (crossing) ctx.setLineDash([0.5, 0.3]);
      else ctx.setLineDash([]);
      ctx.fillRect(rx, ry, rw, rh);
      ctx.strokeRect(rx, ry, rw, rh);
      ctx.setLineDash([]);
    }

    ctx.restore(); // End world-space transform
  }, [data, drawGraphicTransformed, drawTextProp, selectedIds, wireDrawing, placingSymbol]);

  // Fit to view — only when data changes (new sheet loaded), NOT on selection/edit
  const dataUuid = data?.uuid;
  useEffect(() => {
    const container = containerRef.current;
    if (!container || !data) return;
    const rect = container.getBoundingClientRect();
    const [pw, ph] = PAPER[data.paper_size] || PAPER.A4;
    const pad = 40;
    const zoom = Math.min((rect.width - pad * 2) / pw, (rect.height - pad * 2) / ph);
    camRef.current = { zoom, x: (rect.width - pw * zoom) / 2, y: (rect.height - ph * zoom) / 2 };
    updateStatusBar({ zoom: Math.round(zoom * 100 / 3) });
    render();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dataUuid]); // Only re-fit when a different schematic is loaded

  // Re-render when data, selection, or wire drawing changes
  useEffect(() => {
    cancelAnimationFrame(animRef.current);
    animRef.current = requestAnimationFrame(render);
  }, [render]);

  // ResizeObserver — mount once, call latest render via ref to avoid recreation
  const renderRef = useRef(render);
  renderRef.current = render;
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const obs = new ResizeObserver(() => {
      cancelAnimationFrame(animRef.current);
      animRef.current = requestAnimationFrame(() => renderRef.current());
    });
    obs.observe(container);
    return () => obs.disconnect();
  }, []); // Mount once — renderRef always has latest

  // --- Altium mouse: scroll=zoom, right-drag=pan ---
  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const cam = camRef.current;
    const f = e.deltaY > 0 ? 0.9 : 1.1;
    const r = canvasRef.current?.getBoundingClientRect();
    if (!r) return;
    const mx = e.clientX - r.left, my = e.clientY - r.top;
    const nz = Math.min(200, Math.max(0.3, cam.zoom * f));
    cam.x = mx - (mx - cam.x) * (nz / cam.zoom);
    cam.y = my - (my - cam.y) * (nz / cam.zoom);
    cam.zoom = nz;
    updateStatusBar({ zoom: Math.round(nz * 100 / 3) });
    cancelAnimationFrame(animRef.current);
    animRef.current = requestAnimationFrame(render);
  }, [render, updateStatusBar]);

  // --- Mouse handlers: pan (right-drag) + select/move/wire (left-click) ---

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    // Right/middle button = pan
    if (e.button === 2 || e.button === 1) {
      dragging.current = true;
      lastMouse.current = { x: e.clientX, y: e.clientY };
      e.preventDefault();
      return;
    }

    // Left button = select, move, or wire
    if (e.button === 0 && data) {
      const r = canvasRef.current?.getBoundingClientRect();
      if (!r) return;
      const world = s2w(e.clientX - r.left, e.clientY - r.top);
      const store = useSchematicStore.getState();

      if (store.editMode === "drawWire") {
        // Electrical snap on click: prefer pin/wire endpoints
        const eSnap = findNearestElectricalPoint(data, world.x, world.y);
        const wirePos = eSnap || world;
        if (store.wireDrawing.active) {
          store.addWirePoint(wirePos);
        } else {
          store.startWire(wirePos);
        }
        return;
      }

      if (store.editMode === "placeSymbol" && store.placingSymbol) {
        store.placeSymbolAt(world);
        return;
      }

      if (store.editMode === "placeLabel") {
        const name = prompt("Net label name:");
        if (name) store.placeNetLabel(world, name);
        return;
      }

      if (store.editMode === "placePower") {
        const name = prompt("Power net name (e.g. VCC, GND):");
        if (name) store.placePowerPort(world, name, "input");
        return;
      }

      if (store.editMode === "placeNoConnect") {
        store.placeNoConnect(world);
        return;
      }

      // Select mode: hit test
      const hit = hitTest(data, world.x, world.y);
      if (hit) {
        if (e.shiftKey) {
          store.toggleSelect(hit.uuid);
        } else if (!store.selectedIds.has(hit.uuid)) {
          store.select(hit.uuid);
        }
        // Start move drag
        moving.current = true;
        moveStart.current = { x: world.x, y: world.y };
        // Push undo before move
        store.pushUndo();
      } else {
        // Start drag-box selection
        if (!e.shiftKey) store.deselectAll();
        selecting.current = true;
        selectStart.current = { x: world.x, y: world.y };
        selectEnd.current = { x: world.x, y: world.y };
      }
    }
  }, [data, s2w]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const r = canvasRef.current?.getBoundingClientRect();
    if (!r) return;
    const world = s2w(e.clientX - r.left, e.clientY - r.top);
    updateStatusBar({
      cursorPosition: { x: Math.round(world.x * 100) / 100, y: Math.round(world.y * 100) / 100 },
    });

    // Pan
    if (dragging.current) {
      camRef.current.x += e.clientX - lastMouse.current.x;
      camRef.current.y += e.clientY - lastMouse.current.y;
      lastMouse.current = { x: e.clientX, y: e.clientY };
      cancelAnimationFrame(animRef.current);
      animRef.current = requestAnimationFrame(render);
      return;
    }

    // Update wire/placement cursor for live preview (ref + rAF, no Zustand state churn)
    if (data) {
      const store = useSchematicStore.getState();
      if (store.wireDrawing.active) {
        // Electrical grid snap: prefer snapping to nearby pins/wire endpoints
        const eSnap = findNearestElectricalPoint(data, world.x, world.y);
        wireCursorRef.current = eSnap || snapPoint(world);
        cancelAnimationFrame(animRef.current);
        animRef.current = requestAnimationFrame(render);
      }
      if (store.editMode === "placeSymbol" && store.placingSymbol) {
        placeCursorRef.current = snapPoint(world);
        cancelAnimationFrame(animRef.current);
        animRef.current = requestAnimationFrame(render);
      }
    }

    // Drag-box selection update
    if (selecting.current) {
      selectEnd.current = { x: world.x, y: world.y };
      cancelAnimationFrame(animRef.current);
      animRef.current = requestAnimationFrame(render);
      return;
    }

    // Move selected elements
    if (moving.current && data) {
      const store = useSchematicStore.getState();
      if (store.selectedIds.size > 0) {
        const snapped = snapPoint(world);
        const startSnapped = snapPoint(moveStart.current);
        const dx = snapped.x - startSnapped.x;
        const dy = snapped.y - startSnapped.y;
        if (dx !== 0 || dy !== 0) {
          store.moveElements([...store.selectedIds], dx, dy);
          moveStart.current = { x: snapped.x, y: snapped.y };
        }
      }
    }
  }, [render, s2w, updateStatusBar, data]);

  const handleMouseUp = useCallback(() => {
    // Finalize drag-box selection
    if (selecting.current && data) {
      selecting.current = false;
      const s = selectStart.current, e = selectEnd.current;
      // Only if dragged more than a tiny amount (avoid accidental micro-drags)
      if (Math.abs(e.x - s.x) > 0.5 || Math.abs(e.y - s.y) > 0.5) {
        const uuids = boxSelect(data, s.x, s.y, e.x, e.y);
        if (uuids.length > 0) {
          useSchematicStore.getState().selectMultiple(uuids);
        }
      }
      cancelAnimationFrame(animRef.current);
      animRef.current = requestAnimationFrame(render);
    }
    dragging.current = false;
    moving.current = false;
  }, [data, render]);

  const handleDblClick = useCallback((e: React.MouseEvent) => {
    const store = useSchematicStore.getState();
    if (store.editMode === "drawWire" && store.wireDrawing.active) {
      store.finishWire();
      return;
    }

    // Double-click = in-place edit (Altium behavior)
    if (data) {
      const r = canvasRef.current?.getBoundingClientRect();
      if (!r) return;
      const world = s2w(e.clientX - r.left, e.clientY - r.top);
      const hit = hitTest(data, world.x, world.y);
      if (hit) {
        // Select it first
        if (!store.selectedIds.has(hit.uuid)) store.select(hit.uuid);

        const sym = data.symbols.find(s => s.uuid === hit.uuid);
        if (sym) {
          const sp = w2s(sym.position.x, sym.position.y);
          setInPlaceEdit({ uuid: hit.uuid, field: "reference", value: sym.reference, screenX: sp.x, screenY: sp.y - 20 });
          return;
        }
        const lbl = data.labels.find(l => l.uuid === hit.uuid);
        if (lbl) {
          const sp = w2s(lbl.position.x, lbl.position.y);
          setInPlaceEdit({ uuid: hit.uuid, field: "text", value: lbl.text, screenX: sp.x, screenY: sp.y - 10 });
        }
      }
    }
  }, [data, s2w, w2s]);

  // --- Keyboard shortcuts ---
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!data) return;
      const store = useSchematicStore.getState();

      // Don't handle if typing in an input
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      switch (e.key) {
        case "Home": {
          const container = containerRef.current;
          if (!container) return;
          const rect = container.getBoundingClientRect();
          const [pw, ph] = PAPER[data.paper_size] || PAPER.A4;
          const pad = 40;
          const zoom = Math.min((rect.width - pad * 2) / pw, (rect.height - pad * 2) / ph);
          camRef.current = { zoom, x: (rect.width - pw * zoom) / 2, y: (rect.height - ph * zoom) / 2 };
          updateStatusBar({ zoom: Math.round(zoom * 100 / 3) });
          render();
          break;
        }
        case "Escape":
          if (store.wireDrawing.active) {
            store.cancelWire();
          } else if (store.placingSymbol) {
            store.cancelPlacement();
          } else {
            store.deselectAll();
            store.setEditMode("select");
          }
          break;
        case "F2": {
          // In-place text editing for selected component or label
          if (store.selectedIds.size !== 1 || !data) break;
          const selId = [...store.selectedIds][0];
          const sym = data.symbols.find(s => s.uuid === selId);
          if (sym) {
            const sp = w2s(sym.position.x, sym.position.y);
            setInPlaceEdit({ uuid: selId, field: "reference", value: sym.reference, screenX: sp.x, screenY: sp.y - 20 });
            break;
          }
          const lbl = data.labels.find(l => l.uuid === selId);
          if (lbl) {
            const sp = w2s(lbl.position.x, lbl.position.y);
            setInPlaceEdit({ uuid: selId, field: "text", value: lbl.text, screenX: sp.x, screenY: sp.y - 10 });
          }
          break;
        }
        case "Tab":
          // During placement: open properties for the component being placed
          if (store.placingSymbol) {
            e.preventDefault();
            // Focus the properties panel by toggling right panel open
            const layout = useLayoutStore.getState();
            if (layout.rightCollapsed) layout.toggleRight();
          }
          break;
        case "w":
        case "W":
          if (!e.ctrlKey) store.setEditMode("drawWire");
          break;
        case "l":
        case "L":
          if (!e.ctrlKey) store.setEditMode("placeLabel");
          break;
        case "Delete":
          store.deleteSelected();
          break;
        case "Backspace":
          if (store.wireDrawing.active) {
            e.preventDefault();
            store.removeLastWirePoint();
          } else {
            store.deleteSelected();
          }
          break;
        case " ":
          if (e.shiftKey && store.wireDrawing.active) {
            e.preventDefault();
            store.cycleWireRouting();
          }
          break;
        case "a":
          if (e.ctrlKey) {
            e.preventDefault();
            store.selectAll();
          }
          break;
        case "f":
          if (e.ctrlKey) {
            e.preventDefault();
            setFindShowReplace(false);
            setFindOpen(true);
          }
          break;
        case "h":
          if (e.ctrlKey) {
            e.preventDefault();
            setFindShowReplace(true);
            setFindOpen(true);
          }
          break;
        case "q":
          if (e.ctrlKey) {
            e.preventDefault();
            const editor = useEditorStore.getState();
            const nextUnits = editor.statusBar.units === "mm" ? "mil" : editor.statusBar.units === "mil" ? "inch" : "mm";
            editor.updateStatusBar({ units: nextUnits as "mm" | "mil" | "inch" });
          }
          break;
        case "ArrowUp":
        case "ArrowDown":
        case "ArrowLeft":
        case "ArrowRight": {
          if (!e.ctrlKey || store.selectedIds.size === 0) break;
          e.preventDefault();
          const grid = useEditorStore.getState().statusBar.gridSize;
          const step = e.shiftKey ? grid * 10 : grid;
          const ddx = e.key === "ArrowRight" ? step : e.key === "ArrowLeft" ? -step : 0;
          const ddy = e.key === "ArrowDown" ? step : e.key === "ArrowUp" ? -step : 0;
          if (ddx !== 0 || ddy !== 0) {
            store.pushUndo();
            store.moveElements([...store.selectedIds], ddx, ddy);
          }
          break;
        }
        case "r":
        case "R":
          if (!e.ctrlKey) {
            if (store.placingSymbol) store.rotatePlacement();
            else store.rotateSelected();
          }
          break;
        case "x":
        case "X":
          if (!e.ctrlKey) {
            if (store.placingSymbol) store.mirrorPlacementX();
            else if (store.selectedIds.size > 0) store.mirrorSelectedX();
          }
          break;
        case "y":
        case "Y":
          if (e.ctrlKey) {
            e.preventDefault(); store.redo();
          } else {
            if (store.placingSymbol) store.mirrorPlacementY();
            else if (store.selectedIds.size > 0) store.mirrorSelectedY();
          }
          break;
        case "d":
          if (e.ctrlKey) {
            e.preventDefault();
            store.duplicateSelected();
          }
          break;
        case "g":
          if (e.shiftKey) {
            useEditorStore.getState().toggleSnap();
          } else {
            useEditorStore.getState().toggleGrid();
          }
          break;
        case "z":
          if (e.ctrlKey) { e.preventDefault(); store.undo(); }
          break;
        case "Z":
          if (e.ctrlKey && e.shiftKey) { e.preventDefault(); store.redo(); }
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [data, render, updateStatusBar]);

  return (
    <div ref={containerRef} className="w-full h-full overflow-hidden relative">
      <canvas
        ref={canvasRef}
        className="absolute inset-0"
        style={{ cursor: editMode !== "select" ? "crosshair" : "default" }}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onDoubleClick={handleDblClick}
        onContextMenu={(e) => e.preventDefault()}
      />

      {/* In-place text editor overlay */}
      {inPlaceEdit && (
        <input
          autoFocus
          value={inPlaceEdit.value}
          onChange={(e) => setInPlaceEdit({ ...inPlaceEdit, value: e.target.value })}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              const store = useSchematicStore.getState();
              const data = store.data;
              if (data) {
                const sym = data.symbols.find(s => s.uuid === inPlaceEdit.uuid);
                if (sym) store.updateSymbolProp(inPlaceEdit.uuid, inPlaceEdit.field, inPlaceEdit.value);
                const lbl = data.labels.find(l => l.uuid === inPlaceEdit.uuid);
                if (lbl) store.updateLabelProp(inPlaceEdit.uuid, inPlaceEdit.field, inPlaceEdit.value);
              }
              setInPlaceEdit(null);
            }
            if (e.key === "Escape") setInPlaceEdit(null);
            e.stopPropagation();
          }}
          onBlur={() => {
            const store = useSchematicStore.getState();
            const data = store.data;
            if (data) {
              const sym = data.symbols.find(s => s.uuid === inPlaceEdit.uuid);
              if (sym) store.updateSymbolProp(inPlaceEdit.uuid, inPlaceEdit.field, inPlaceEdit.value);
              const lbl = data.labels.find(l => l.uuid === inPlaceEdit.uuid);
              if (lbl) store.updateLabelProp(inPlaceEdit.uuid, inPlaceEdit.field, inPlaceEdit.value);
            }
            setInPlaceEdit(null);
          }}
          className="absolute z-40 bg-bg-primary border border-accent rounded px-2 py-0.5 text-[12px] font-mono text-text-primary outline-none shadow-lg"
          style={{ left: inPlaceEdit.screenX, top: inPlaceEdit.screenY, minWidth: 80 }}
        />
      )}

      {/* Find/Replace */}
      <FindReplace open={findOpen} onClose={() => setFindOpen(false)} showReplace={findShowReplace} />

      {/* Altium-style Active Bar — floating canvas toolbar */}
      <div className="absolute top-3 right-3 flex items-center gap-0.5 bg-bg-surface/90 backdrop-blur-sm border border-border-subtle rounded-lg px-1.5 py-1 shadow-lg shadow-black/30">
        <CanvasBtn
          active={editMode === "select"}
          label="Select (Esc)"
          onClick={() => useSchematicStore.getState().setEditMode("select")}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M3 3l7.07 16.97 2.51-7.39 7.39-2.51L3 3z"/></svg>
        </CanvasBtn>
        <CanvasBtn
          active={editMode === "drawWire"}
          label="Wire (W)"
          onClick={() => useSchematicStore.getState().setEditMode("drawWire")}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M4 12h8v-8"/></svg>
        </CanvasBtn>
        <CanvasBtn
          active={editMode === "placeSymbol"}
          label="Component (P)"
          onClick={() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "p" }))}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="4" y="4" width="16" height="16" rx="2"/><circle cx="12" cy="12" r="3"/></svg>
        </CanvasBtn>
        <CanvasBtn
          active={editMode === "placeLabel"}
          label="Net Label (L)"
          onClick={() => useSchematicStore.getState().setEditMode("placeLabel")}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M4 7h11l5 5-5 5H4V7z"/></svg>
        </CanvasBtn>
        <CanvasBtn
          active={editMode === "placePower"}
          label="Power Port"
          onClick={() => useSchematicStore.getState().setEditMode("placePower")}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M12 2v10"/><path d="M5 12h14"/></svg>
        </CanvasBtn>
        <CanvasBtn
          active={editMode === "placeNoConnect"}
          label="No Connect"
          onClick={() => useSchematicStore.getState().setEditMode("placeNoConnect")}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M6 6l12 12"/><path d="M18 6L6 18"/></svg>
        </CanvasBtn>
        <div className="w-px h-4 bg-border-subtle mx-0.5" />
        <CanvasBtn label="Rotate (R)" onClick={() => {
          const s = useSchematicStore.getState();
          if (s.placingSymbol) s.rotatePlacement(); else s.rotateSelected();
        }}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21 12a9 9 0 11-6.219-8.56"/><polyline points="21 3 21 9 15 9"/></svg>
        </CanvasBtn>
        <CanvasBtn label="Delete (Del)" onClick={() => useSchematicStore.getState().deleteSelected()}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6"/><path d="M10 11v6"/><path d="M14 11v6"/></svg>
        </CanvasBtn>
        <div className="w-px h-4 bg-border-subtle mx-0.5" />
        <CanvasBtn label="Fit View (Home)" onClick={() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Home" }))}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M15 3h6v6"/><path d="M9 21H3v-6"/><path d="M21 3l-7 7"/><path d="M3 21l7-7"/></svg>
        </CanvasBtn>
      </div>

      {/* Info overlay */}
      <div className="absolute top-2 left-2 text-[10px] text-text-muted/40 bg-bg-primary/60 px-2 py-1 rounded pointer-events-none flex gap-3">
        <span>{data?.symbols.filter(s => !s.is_power).length ?? 0} components | {data?.wires.length ?? 0} wires | {data?.labels.length ?? 0} labels</span>
        {selectedIds.size > 0 && <span className="text-accent">{selectedIds.size} selected</span>}
        {editMode !== "select" && <span className="text-warning uppercase">{editMode}</span>}
        {wireDrawing.active && <span className="text-info">Wire: {wireDrawing.routingMode} ({wireDrawing.points.length} pts) Shift+Space:mode Backspace:undo</span>}
        {placingSymbol && <span className="text-info">Placing {placingSymbol.meta.symbol_id} (R:rotate X:mirrorX Y:mirrorY)</span>}
      </div>
    </div>
  );
}

function CanvasBtn({ children, label, active, onClick }: {
  children: React.ReactNode; label: string; active?: boolean; onClick: () => void;
}) {
  return (
    <button
      title={label}
      onClick={onClick}
      className={`p-1.5 rounded transition-colors ${
        active ? "bg-accent/25 text-accent" : "text-text-muted/60 hover:bg-bg-hover hover:text-text-primary"
      }`}
    >
      {children}
    </button>
  );
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
