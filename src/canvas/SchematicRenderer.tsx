import { useRef, useEffect, useCallback } from "react";
import { useEditorStore } from "@/stores/editor";
import type { SchematicData, Graphic, SchPin, SchPoint, TextPropData } from "@/types";

interface Props { data: SchematicData }
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

export function SchematicRenderer({ data }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const camRef = useRef<Camera>({ x: 0, y: 0, zoom: 3 });
  const dragging = useRef(false);
  const lastMouse = useRef({ x: 0, y: 0 });
  const animRef = useRef(0);
  const updateStatusBar = useEditorStore((s) => s.updateStatusBar);

  const s2w = useCallback((sx: number, sy: number) => {
    const c = camRef.current;
    return { x: (sx - c.x) / c.zoom, y: (sy - c.y) / c.zoom };
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

    // Grid (only if zoomed enough)
    const gridSize = 1.27;
    if (gridSize * cam.zoom > 6) {
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
          ctx.font = "0.65px Roboto";
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
        // Power symbol: show value at its property position
        if (!sym.val_text.hidden) {
          drawTextProp(ctx, txt(sym.value || sym.reference), sym.val_text, C.power, true);
        } else if (!sym.ref_text.hidden) {
          drawTextProp(ctx, txt(sym.value || sym.reference), sym.ref_text, C.power, true);
        }
      }
    }

    // Labels
    for (const label of data.labels) {
      const color = label.label_type === "Global" ? C.labelGlobal
        : label.label_type === "Hierarchical" ? C.labelHier : C.labelNet;
      ctx.fillStyle = color;
      ctx.font = label.label_type === "Global" ? "bold 1.0px Roboto" : "0.95px Roboto";

      const text = txt(label.text);
      const r = label.rotation;

      if (r === 0) {
        ctx.textAlign = "left"; ctx.textBaseline = "bottom";
        ctx.fillText(text, label.position.x, label.position.y - 0.3);
      } else if (r === 180) {
        ctx.textAlign = "right"; ctx.textBaseline = "bottom";
        ctx.fillText(text, label.position.x, label.position.y - 0.3);
      } else if (r === 90 || r === 270) {
        ctx.save();
        ctx.translate(label.position.x, label.position.y);
        ctx.rotate(-Math.PI / 2);
        ctx.textAlign = "left"; ctx.textBaseline = "bottom";
        ctx.fillText(text, 0.3, 0);
        ctx.restore();
      } else {
        ctx.textAlign = "left"; ctx.textBaseline = "bottom";
        ctx.fillText(text, label.position.x, label.position.y - 0.3);
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

    ctx.restore();
  }, [data, drawGraphicTransformed, drawTextProp]);

  // Fit to view
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
  }, [data, render, updateStatusBar]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const obs = new ResizeObserver(() => render());
    obs.observe(container);
    return () => obs.disconnect();
  }, [render]);

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

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 2 || e.button === 1) {
      dragging.current = true;
      lastMouse.current = { x: e.clientX, y: e.clientY };
      e.preventDefault();
    }
  }, []);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const r = canvasRef.current?.getBoundingClientRect();
    if (r) {
      const world = s2w(e.clientX - r.left, e.clientY - r.top);
      updateStatusBar({ cursorPosition: { x: Math.round(world.x * 100) / 100, y: Math.round(world.y * 100) / 100 } });
    }
    if (dragging.current) {
      camRef.current.x += e.clientX - lastMouse.current.x;
      camRef.current.y += e.clientY - lastMouse.current.y;
      lastMouse.current = { x: e.clientX, y: e.clientY };
      cancelAnimationFrame(animRef.current);
      animRef.current = requestAnimationFrame(render);
    }
  }, [render, s2w, updateStatusBar]);

  const handleMouseUp = useCallback(() => { dragging.current = false; }, []);

  // Keyboard: Home = fit
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Home") {
        const container = containerRef.current;
        if (!container) return;
        const rect = container.getBoundingClientRect();
        const [pw, ph] = PAPER[data.paper_size] || PAPER.A4;
        const pad = 40;
        const zoom = Math.min((rect.width - pad * 2) / pw, (rect.height - pad * 2) / ph);
        camRef.current = { zoom, x: (rect.width - pw * zoom) / 2, y: (rect.height - ph * zoom) / 2 };
        updateStatusBar({ zoom: Math.round(zoom * 100 / 3) });
        render();
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
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onContextMenu={(e) => e.preventDefault()}
      />
      <div className="absolute top-2 left-2 text-[10px] text-text-muted/40 bg-bg-primary/60 px-2 py-1 rounded pointer-events-none">
        {data.symbols.filter(s => !s.is_power).length} components | {data.wires.length} wires | {data.labels.length} labels | {data.paper_size}
      </div>
    </div>
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
