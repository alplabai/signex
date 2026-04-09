import { useRef, useEffect, useCallback, useState } from "react";
import { useEditorStore } from "@/stores/editor";
import { useSchematicStore, snapPoint } from "@/stores/schematic";
import { useLayoutStore } from "@/stores/layout";
import { useProjectStore } from "@/stores/project";
import { hitTest, boxSelect, lassoSelect, outsideBoxSelect, lineSelect, connectionSelect } from "./hitTest";
import { FindReplace } from "@/components/FindReplace";
import { ContextMenu, type ContextMenuItem } from "@/components/ContextMenu";
import { resolveNets } from "@/lib/netResolver";
import type { Graphic, SchPoint, TextPropData } from "@/types";
import { substituteSpecialStrings } from "@/lib/specialStrings";
import { useThemeStore } from "@/stores/theme";
import {
  PAPER, C, txt,
  symToSch, pinEnd, findNearestElectricalPoint,
} from "./schematicDrawHelpers";
interface Camera { x: number; y: number; zoom: number }
const IMAGE_CACHE = new Map<string, HTMLImageElement>();
const MAX_IMAGE_CACHE = 100;

export function SchematicRenderer() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const ctxRef = useRef<CanvasRenderingContext2D | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const camRef = useRef<Camera>({ x: 0, y: 0, zoom: 3 });
  const dragging = useRef(false);
  const moving = useRef(false);
  const selecting = useRef(false); // Drag-box selection active
  const selectStart = useRef({ x: 0, y: 0 }); // Drag-box start (world coords)
  const selectEnd = useRef({ x: 0, y: 0 }); // Drag-box end (world coords)
  const moveNoRubber = useRef(false); // Ctrl held = move without rubber-banding
  const moveStart = useRef({ x: 0, y: 0 });
  const lastMouse = useRef({ x: 0, y: 0 });
  const autoPanRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const autoPanDir = useRef({ dx: 0, dy: 0 });
  const animRef = useRef(0);
  const wireCursorRef = useRef<SchPoint>({ x: 0, y: 0 }); // Ref for live wire cursor — no Zustand churn
  const placeCursorRef = useRef<SchPoint>({ x: 0, y: 0 }); // Ref for placement cursor
  const draggingEndpoint = useRef<{ uuid: string; endpoint: "start" | "end" } | null>(null);
  const drawStart = useRef<SchPoint | null>(null);
  const drawMid = useRef<SchPoint | null>(null); // For 3-click arc
  const polyPoints = useRef<SchPoint[]>([]); // For polyline accumulation
  // Canvas theme colors — synced to module-level C object each frame
  const canvasColors = useThemeStore((s) => s.getActiveTheme().tokens.canvas);
  const canvasColorsRef = useRef(canvasColors);
  useEffect(() => { canvasColorsRef.current = canvasColors; Object.assign(C, canvasColors); }, [canvasColors]);
  const updateStatusBar = useEditorStore((s) => s.updateStatusBar);
  const gridVisible = useEditorStore((s) => s.gridVisible);
  const gridSize = useEditorStore((s) => s.statusBar.gridSize);
  const selectionFilter = useEditorStore((s) => s.selectionFilter);
  const setFilterItem = useEditorStore((s) => s.setFilterItem);

  // Schematic store
  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);
  const editMode = useSchematicStore((s) => s.editMode);
  const wireDrawing = useSchematicStore((s) => s.wireDrawing);
  const placingSymbol = useSchematicStore((s) => s.placingSymbol);

  // Find/Replace state
  const [findOpen, setFindOpen] = useState(false);
  const [activeBarMenu, setActiveBarMenu] = useState<string | null>(null);
  const selectionModeRef = useRef<"box" | "lasso" | "insideArea" | "outsideArea" | "touchingRect" | "touchingLine">("box");
  const lassoPoints = useRef<{ x: number; y: number }[]>([]);
  const powerPreset = useRef<{ net: string; style: string }>({ net: "VCC", style: "bar" });
  // Track last-used tool per Active Bar group (Altium: icon changes to last used)
  const [lastTool, setLastTool] = useState<Record<string, string>>({
    wire: "drawWire",
    text: "placeText",
    draw: "drawLine",
    power: "placePower",
  });
  const [findShowReplace, setFindShowReplace] = useState(false);

  // Context menu state
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);

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

  // Prevent double-commit: Enter commits + blur fires again
  const committedRef = useRef(false);
  useEffect(() => { committedRef.current = false; }, [inPlaceEdit]);

  const drawGraphicTransformed = useCallback((
    ctx: CanvasRenderingContext2D, g: Graphic,
    sx: number, sy: number, rot: number, mx: boolean, my: boolean,
    mode: 'full' | 'fillOnly' | 'strokeOnly' = 'full'
  ) => {
    const t = (lx: number, ly: number) => symToSch(lx, ly, sx, sy, rot, mx, my);
    const doFill   = mode !== 'strokeOnly';
    const doStroke = mode !== 'fillOnly';

    const gWidth = "width" in g ? (g.width as number) : 0;
    ctx.lineWidth = Math.max(gWidth || 0.1, 0.1);

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
        if (doFill && g.fill_type === "background") { ctx.fillStyle = C.bodyFill; ctx.fill(); }
        else if (doFill && g.fill_type === "outline") { ctx.fillStyle = C.body; ctx.fill(); }
        if (doStroke) ctx.stroke();
        break;
      }
      case "Rectangle": {
        const [x1, y1] = t(g.start.x, g.start.y);
        const [x2, y2] = t(g.end.x, g.end.y);
        const rx = Math.min(x1, x2), ry = Math.min(y1, y2);
        const rw = Math.abs(x2 - x1), rh = Math.abs(y2 - y1);
        if (doFill && g.fill_type === "background") { ctx.fillStyle = C.bodyFill; ctx.fillRect(rx, ry, rw, rh); }
        else if (doFill && g.fill_type === "outline") { ctx.fillStyle = C.body; ctx.fillRect(rx, ry, rw, rh); }
        if (doStroke) ctx.strokeRect(rx, ry, rw, rh);
        break;
      }
      case "Circle": {
        const [cx, cy] = t(g.center.x, g.center.y);
        ctx.beginPath();
        ctx.arc(cx, cy, g.radius, 0, Math.PI * 2);
        if (doFill && g.fill_type === "background") { ctx.fillStyle = C.bodyFill; ctx.fill(); }
        else if (doFill && g.fill_type === "outline") { ctx.fillStyle = C.body; ctx.fill(); }
        if (doStroke) ctx.stroke();
        break;
      }
      case "Arc": {
        const [sx1, sy1] = t(g.start.x, g.start.y);
        const [mx1, my1] = t(g.mid.x, g.mid.y);
        const [ex1, ey1] = t(g.end.x, g.end.y);
        const center = arcCenter({ x: sx1, y: sy1 }, { x: mx1, y: my1 }, { x: ex1, y: ey1 });
        if (center && doStroke) {
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
    const newW = Math.round(rect.width * dpr);
    const newH = Math.round(rect.height * dpr);
    // Only resize when dimensions change (avoids clearing context state every frame)
    if (canvas.width !== newW || canvas.height !== newH) {
      canvas.width = newW;
      canvas.height = newH;
      canvas.style.width = `${rect.width}px`;
      canvas.style.height = `${rect.height}px`;
    }

    if (!ctxRef.current) ctxRef.current = canvas.getContext("2d");
    const ctx = ctxRef.current;
    if (!ctx) return;
    // Reset transform and clear entire canvas
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const cam = camRef.current;
    const w = rect.width, h = rect.height;

    ctx.fillStyle = C.bg;
    ctx.fillRect(0, 0, w, h);

    if (!data) return;
    const [pw, ph] = PAPER[data.paper_size] || PAPER.A4;

    ctx.save();
    ctx.translate(cam.x, cam.y);
    ctx.scale(cam.zoom, cam.zoom);

    // Paper with Altium-style zone markers
    ctx.fillStyle = C.paper;
    ctx.fillRect(0, 0, pw, ph);
    ctx.strokeStyle = C.paperBorder;
    ctx.lineWidth = 0.3;
    ctx.strokeRect(0, 0, pw, ph);

    // Zone markers (A-H columns, 1-N rows)
    const zoneMargin = 5; // margin width for zone labels
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

    // Column markers (top and bottom)
    for (let c = 0; c < cols; c++) {
      const x = zoneMargin + c * colW;
      // Top tick
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, zoneMargin); ctx.stroke();
      ctx.fillText(String(c + 1), x + colW / 2, zoneMargin / 2);
      // Bottom tick
      ctx.beginPath(); ctx.moveTo(x, ph - zoneMargin); ctx.lineTo(x, ph); ctx.stroke();
      ctx.fillText(String(c + 1), x + colW / 2, ph - zoneMargin / 2);
    }
    // Last column tick
    ctx.beginPath(); ctx.moveTo(pw - zoneMargin, 0); ctx.lineTo(pw - zoneMargin, zoneMargin); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(pw - zoneMargin, ph - zoneMargin); ctx.lineTo(pw - zoneMargin, ph); ctx.stroke();

    // Row markers (left and right)
    for (let r = 0; r < rows; r++) {
      const y = zoneMargin + r * rowH;
      const letter = String.fromCharCode(65 + r); // A, B, C...
      // Left tick
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(zoneMargin, y); ctx.stroke();
      ctx.fillText(letter, zoneMargin / 2, y + rowH / 2);
      // Right tick
      ctx.beginPath(); ctx.moveTo(pw - zoneMargin, y); ctx.lineTo(pw, y); ctx.stroke();
      ctx.fillText(letter, pw - zoneMargin / 2, y + rowH / 2);
    }
    // Last row tick
    ctx.beginPath(); ctx.moveTo(0, ph - zoneMargin); ctx.lineTo(zoneMargin, ph - zoneMargin); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(pw - zoneMargin, ph - zoneMargin); ctx.lineTo(pw, ph - zoneMargin); ctx.stroke();

    // Inner border (working area)
    ctx.strokeStyle = C.paperBorder;
    ctx.lineWidth = 0.15;
    ctx.strokeRect(zoneMargin, zoneMargin, pw - zoneMargin * 2, ph - zoneMargin * 2);

    // Title block
    ctx.lineWidth = 0.15;
    ctx.strokeRect(pw - 100, ph - 30, 100 - zoneMargin, 30 - zoneMargin);

    // Title block fields inside the border box
    {
      const tbx = pw - 100, tby = ph - 30;
      const tb = data.title_block || {};
      // Internal grid lines
      ctx.strokeStyle = C.paperBorder;
      ctx.lineWidth = 0.08;
      ctx.beginPath();
      ctx.moveTo(tbx, tby + 10); ctx.lineTo(tbx + 100, tby + 10); // row separator
      ctx.moveTo(tbx, tby + 20); ctx.lineTo(tbx + 100, tby + 20); // row separator
      ctx.moveTo(tbx + 50, tby); ctx.lineTo(tbx + 50, tby + 10); // col separator top row
      ctx.stroke();
      // Labels (small gray text)
      ctx.fillStyle = C.ref;
      ctx.font = "0.8px Roboto";
      ctx.textAlign = "left";
      ctx.textBaseline = "top";
      ctx.fillText("Title:", tbx + 1, tby + 1);
      ctx.fillText("Date:", tbx + 51, tby + 1);
      ctx.fillText("Rev:", tbx + 1, tby + 11);
      ctx.fillText("Company:", tbx + 51, tby + 11);
      // Values
      ctx.fillStyle = C.val;
      ctx.font = "1.2px Roboto";
      ctx.textBaseline = "middle";
      ctx.fillText(tb.title || "", tbx + 8, tby + 5);
      ctx.fillText(tb.date || "", tbx + 58, tby + 5);
      ctx.fillText(tb.rev || "", tbx + 8, tby + 15);
      ctx.fillText(tb.company || "", tbx + 63, tby + 15);
      // Large title at bottom row
      ctx.font = "bold 1.5px Roboto";
      ctx.fillText(tb.title || "", tbx + 2, tby + 25);
    }

    // Grid (Altium-style dots at intersections)
    if (gridVisible && gridSize * cam.zoom > 2) {
      const dotSize = gridSize * 0.04;
      const majDotSize = gridSize * 0.08;
      // Calculate visible grid range from camera
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

    // AutoFocus: dim non-focused elements
    const autoFocus = useEditorStore.getState().autoFocusUuids;
    const hasFocus = autoFocus !== null && autoFocus.length > 0;
    const sf = useEditorStore.getState().selectionFilter;
    const alphaFor = (uuid: string, filterKey: string) => {
      if (!sf[filterKey]?.visible) return 0.12;
      if (hasFocus && !autoFocus!.includes(uuid)) return 0.15;
      return 1;
    };

    // Wires (Altium-style: slightly thicker for visibility)
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

    // Junctions
    ctx.fillStyle = C.junction;
    ctx.globalAlpha = sf.junctions?.visible === false ? 0.12 : 1;
    for (const j of data.junctions) {
      ctx.beginPath();
      ctx.arc(j.position.x, j.position.y, 0.3, 0, Math.PI * 2);
      ctx.fill();
    }

    // No-connects
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

    // No ERC directives (green circle with check)
    if (data.no_erc_directives) {
      for (const d of data.no_erc_directives) {
        const sel = selectedIds.has(d.uuid);
        ctx.strokeStyle = sel ? C.selection : "#66bb6a";
        ctx.fillStyle = sel ? C.selectionFill : "rgba(102,187,106,0.15)";
        ctx.lineWidth = 0.15;
        ctx.beginPath();
        ctx.arc(d.position.x, d.position.y, 0.5, 0, Math.PI * 2);
        ctx.fill(); ctx.stroke();
        // Checkmark
        ctx.strokeStyle = sel ? C.selection : "#66bb6a";
        ctx.lineWidth = 0.15;
        ctx.beginPath();
        ctx.moveTo(d.position.x - 0.2, d.position.y);
        ctx.lineTo(d.position.x - 0.05, d.position.y + 0.2);
        ctx.lineTo(d.position.x + 0.25, d.position.y - 0.15);
        ctx.stroke();
      }
    }

    // Parameter Sets (purple/magenta table icon)
    if (data.parameter_sets) {
      for (const ps of data.parameter_sets) {
        const sel = selectedIds.has(ps.uuid);
        const px = ps.position.x, py = ps.position.y;
        ctx.strokeStyle = sel ? C.selection : "#ab47bc";
        ctx.fillStyle = sel ? C.selectionFill : "rgba(171,71,188,0.12)";
        ctx.lineWidth = 0.15;
        // Table icon: rectangle with horizontal lines
        ctx.fillRect(px - 0.8, py - 0.6, 1.6, 1.2);
        ctx.strokeRect(px - 0.8, py - 0.6, 1.6, 1.2);
        ctx.beginPath();
        ctx.moveTo(px - 0.8, py - 0.15); ctx.lineTo(px + 0.8, py - 0.15);
        ctx.moveTo(px - 0.8, py + 0.3); ctx.lineTo(px + 0.8, py + 0.3);
        ctx.stroke();
        // Small dots
        ctx.fillStyle = sel ? C.selection : "#ab47bc";
        ctx.beginPath(); ctx.arc(px - 0.4, py - 0.38, 0.08, 0, Math.PI * 2); ctx.fill();
        ctx.beginPath(); ctx.arc(px - 0.4, py + 0.08, 0.08, 0, Math.PI * 2); ctx.fill();
        ctx.beginPath(); ctx.arc(px - 0.4, py + 0.52, 0.08, 0, Math.PI * 2); ctx.fill();
        // Label
        if (ps.parameters.length > 0) {
          ctx.fillStyle = sel ? C.selection : "#ce93d8";
          ctx.font = `${0.5}px sans-serif`;
          ctx.textAlign = "left"; ctx.textBaseline = "top";
          ctx.fillText(ps.parameters[0].key + "=" + ps.parameters[0].value, px + 1.0, py - 0.4);
        }
      }
    }

    // Differential Pair Directives (blue parallel lines with +/-)
    if (data.diff_pair_directives) {
      for (const dp of data.diff_pair_directives) {
        const sel = selectedIds.has(dp.uuid);
        const px = dp.position.x, py = dp.position.y;
        ctx.strokeStyle = sel ? C.selection : "#42a5f5";
        ctx.lineWidth = 0.15;
        // Two parallel lines
        ctx.beginPath();
        ctx.moveTo(px - 0.8, py - 0.25); ctx.lineTo(px + 0.8, py - 0.25);
        ctx.moveTo(px - 0.8, py + 0.25); ctx.lineTo(px + 0.8, py + 0.25);
        ctx.stroke();
        // + and - labels
        ctx.fillStyle = sel ? C.selection : "#42a5f5";
        ctx.font = `bold ${0.45}px sans-serif`;
        ctx.textAlign = "center"; ctx.textBaseline = "middle";
        ctx.fillText("+", px + 1.1, py - 0.25);
        ctx.fillText("\u2013", px + 1.1, py + 0.25);
        // Net names
        ctx.font = `${0.4}px sans-serif`;
        ctx.textAlign = "left"; ctx.textBaseline = "top";
        ctx.fillText(dp.positiveNet, px - 0.8, py - 0.8);
        ctx.fillText(dp.negativeNet, px - 0.8, py + 0.5);
      }
    }

    // Blankets (dashed orange polygon)
    if (data.blankets) {
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
        // Parameters label
        if (bl.parameters.length > 0) {
          ctx.fillStyle = sel ? C.selection : "#ffb74d";
          ctx.font = `${0.5}px sans-serif`;
          ctx.textAlign = "left"; ctx.textBaseline = "bottom";
          ctx.fillText(bl.parameters.map(p => p.key + "=" + p.value).join(", "), bl.points[0].x + 0.3, bl.points[0].y - 0.2);
        }
      }
    }

    // Compile Masks (hatched gray rectangle)
    if (data.compile_masks) {
      for (const cm of data.compile_masks) {
        const sel = selectedIds.has(cm.uuid);
        const px = cm.position.x, py = cm.position.y;
        const w = cm.size[0], h = cm.size[1];
        ctx.strokeStyle = sel ? C.selection : "#78909c";
        ctx.fillStyle = sel ? C.selectionFill : "rgba(120,144,156,0.08)";
        ctx.lineWidth = 0.15;
        ctx.fillRect(px, py, w, h);
        ctx.strokeRect(px, py, w, h);
        // Hatching lines (diagonal)
        ctx.save();
        ctx.beginPath();
        ctx.rect(px, py, w, h);
        ctx.clip();
        ctx.strokeStyle = sel ? C.selection : "rgba(120,144,156,0.25)";
        ctx.lineWidth = 0.08;
        for (let d = -h; d < w + h; d += 0.8) {
          ctx.beginPath();
          ctx.moveTo(px + d, py);
          ctx.lineTo(px + d - h, py + h);
          ctx.stroke();
        }
        ctx.restore();
        // Label
        ctx.fillStyle = sel ? C.selection : "#90a4ae";
        ctx.font = `${0.5}px sans-serif`;
        ctx.textAlign = "center"; ctx.textBaseline = "middle";
        ctx.fillText("Compile Mask", px + w / 2, py + h / 2);
      }
    }

    // Notes (yellow/amber rectangle with text and pointer triangle)
    if (data.notes) {
      for (const n of data.notes) {
        const sel = selectedIds.has(n.uuid);
        const px = n.position.x, py = n.position.y;
        const w = n.size[0], h = n.size[1];
        // Pointer triangle (bottom-left)
        ctx.fillStyle = sel ? C.selectionFill : "rgba(255,193,7,0.15)";
        ctx.strokeStyle = sel ? C.selection : "#ffc107";
        ctx.lineWidth = 0.15;
        ctx.beginPath();
        ctx.moveTo(px, py + h);
        ctx.lineTo(px - 0.5, py + h + 0.8);
        ctx.lineTo(px + 0.8, py + h);
        ctx.closePath();
        ctx.fill(); ctx.stroke();
        // Body rectangle
        ctx.fillRect(px, py, w, h);
        ctx.strokeRect(px, py, w, h);
        // Text
        ctx.fillStyle = sel ? C.selection : "#ffca28";
        ctx.font = `${0.55}px sans-serif`;
        ctx.textAlign = "left"; ctx.textBaseline = "top";
        // Word-wrap text
        const words = n.text.split(" ");
        let line = "";
        let ty = py + 0.3;
        for (const word of words) {
          const test = line + (line ? " " : "") + word;
          if (ctx.measureText(test).width > w - 0.6 && line) {
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

    // --- Symbols ---
    for (const sym of data.symbols) {
      const lib = data.lib_symbols[sym.lib_id];
      if (!lib) continue;
      ctx.globalAlpha = alphaFor(sym.uuid, sym.is_power ? "powerPorts" : "components");

      const sx = sym.position.x, sy = sym.position.y;
      const rot = sym.rotation, mx = sym.mirror_x, my = sym.mirror_y;

      const bodyStroke = sym.is_power ? C.power : C.body;

      // Phase 1: Pin lines (stubs) drawn first, under the body fill
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
      }

      // Phase 2: Body fill on top of pin stubs — covers inner portions of stubs
      ctx.strokeStyle = bodyStroke;
      for (const g of lib.graphics) {
        drawGraphicTransformed(ctx, g, sx, sy, rot, mx, my, 'fillOnly');
      }

      // Phase 3: Body stroke (outline) — clean border on top of fill
      for (const g of lib.graphics) {
        drawGraphicTransformed(ctx, g, sx, sy, rot, mx, my, 'strokeOnly');
      }

      // Phase 4: Pin numbers + names on top of the body
      const nameOffset = lib.pin_name_offset > 0 ? lib.pin_name_offset : 0.4;
      for (const pin of lib.pins) {
        if (pin.hidden) continue;
        const [px, py] = symToSch(pin.position.x, pin.position.y, sx, sy, rot, mx, my);
        const pe = pinEnd(pin);
        const [ex, ey] = symToSch(pe.x, pe.y, sx, sy, rot, mx, my);
        const dx = ex - px, dy = ey - py;
        const len = Math.hypot(dx, dy) || 1;

        // Pin number (midpoint, offset toward the "upper" side of the stub)
        if (lib.show_pin_numbers && pin.number_visible && pin.number !== "~") {
          ctx.fillStyle = C.pinNum;
          ctx.font = "1.0px Roboto";
          const nmx = (px + ex) / 2, nmy = (py + ey) / 2;
          if (Math.abs(dx) >= Math.abs(dy)) {
            // Horizontal stub — number above the line
            ctx.textAlign = "center";
            ctx.textBaseline = "bottom";
            ctx.fillText(txt(pin.number), nmx, nmy - 0.3);
          } else {
            // Vertical stub — number to the left of the line
            ctx.textAlign = "right";
            ctx.textBaseline = "middle";
            ctx.fillText(txt(pin.number), nmx - 0.3, nmy);
          }
        }

        // Pin name (inner end toward body, using lib pin_name_offset)
        if (lib.show_pin_names && pin.name_visible && pin.name !== "~") {
          ctx.fillStyle = C.pinName;
          ctx.font = "0.75px Roboto";
          const nx = ex + (dx / len) * nameOffset;
          const ny = ey + (dy / len) * nameOffset;
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

      // Reference & value text — hide while being edited inline
      const editUuid = inPlaceEdit?.uuid ?? null;
      if (!sym.is_power) {
        if (!sym.ref_text.hidden && !(editUuid === sym.uuid && inPlaceEdit?.field === "reference")) {
          drawTextProp(ctx, txt(sym.reference), sym.ref_text, C.ref, true);
        }
        if (!sym.val_text.hidden && !(editUuid === sym.uuid && inPlaceEdit?.field === "value")) {
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
    ctx.globalAlpha = 1;

    // Labels
    for (const label of data.labels) {
      ctx.globalAlpha = alphaFor(label.uuid, label.label_type === "Power" ? "powerPorts" : "labels");
      // Hide label text while being edited inline
      if (inPlaceEdit?.uuid === label.uuid) continue;
      const color = label.label_type === "Global" ? C.labelGlobal
        : label.label_type === "Hierarchical" ? C.labelHier : C.labelNet;
      const text = txt(label.text);
      const fs = label.font_size || 1.27;
      const r = label.rotation;
      const lx = label.position.x, ly = label.position.y;

      // ── Altium-style power port symbols ──
      if (label.label_type === "Power") {
        const pColor = C.power; // red
        const lw = 0.15;
        const stemLen = 2.0;  // connection stem length
        const symSize = 1.2;  // symbol head size

        // Determine power style from shape or auto-detect from name
        let style = label.shape || "input";
        if (style === "input") {
          // Auto-detect: GND-like names get ground symbol, others get bar
          const lower = label.text.toLowerCase();
          if (lower.includes("gnd") || lower.includes("vss") || lower.includes("ground")) {
            style = "power_ground";
          } else {
            style = "bar";
          }
        }

        const isGround = style.includes("ground") || style === "earth_ground";
        // Ground: connection at top, symbol below. Others: connection at bottom, symbol above.
        const dir = isGround ? 1 : -1; // 1 = symbol below origin, -1 = symbol above

        ctx.save();
        ctx.translate(lx, ly);
        const rotRad = -(r * Math.PI) / 180;
        ctx.rotate(rotRad);

        ctx.strokeStyle = pColor;
        ctx.fillStyle = pColor;
        ctx.lineWidth = lw;
        ctx.lineCap = "round";

        // Draw stem from origin
        ctx.beginPath();
        ctx.moveTo(0, 0);
        ctx.lineTo(0, dir * stemLen);
        ctx.stroke();

        // Symbol head position
        const sy = dir * stemLen;

        if (style === "bar") {
          ctx.lineWidth = 0.2;
          ctx.beginPath();
          ctx.moveTo(-symSize, sy);
          ctx.lineTo(symSize, sy);
          ctx.stroke();
        } else if (style === "arrow") {
          ctx.lineWidth = 0.18;
          ctx.beginPath();
          ctx.moveTo(0, sy - 0.6);
          ctx.lineTo(-symSize * 0.5, sy + 0.2);
          ctx.moveTo(0, sy - 0.6);
          ctx.lineTo(symSize * 0.5, sy + 0.2);
          ctx.moveTo(0, sy - 0.6);
          ctx.lineTo(0, sy);
          ctx.stroke();
        } else if (style === "power_ground") {
          ctx.lineWidth = 0.15;
          ctx.beginPath();
          ctx.moveTo(-symSize, sy);
          ctx.lineTo(symSize, sy);
          ctx.moveTo(-symSize * 0.65, sy + dir * 0.4);
          ctx.lineTo(symSize * 0.65, sy + dir * 0.4);
          ctx.moveTo(-symSize * 0.3, sy + dir * 0.8);
          ctx.lineTo(symSize * 0.3, sy + dir * 0.8);
          ctx.stroke();
        } else if (style === "signal_ground") {
          ctx.lineWidth = 0.15;
          ctx.beginPath();
          ctx.moveTo(-symSize, sy);
          ctx.lineTo(symSize, sy);
          ctx.lineTo(0, sy + dir * symSize);
          ctx.closePath();
          ctx.stroke();
        } else if (style === "earth_ground") {
          ctx.lineWidth = 0.15;
          ctx.beginPath();
          ctx.moveTo(-symSize, sy);
          ctx.lineTo(symSize, sy);
          ctx.stroke();
          for (let i = -3; i <= 3; i++) {
            const sx = i * (symSize / 3);
            ctx.beginPath();
            ctx.moveTo(sx, sy);
            ctx.lineTo(sx - dir * 0.4, sy + dir * 0.6);
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
          ctx.moveTo(-symSize, sy);
          ctx.lineTo(symSize, sy);
          ctx.stroke();
        }

        // Net name text — centered, on the opposite side from the symbol
        ctx.restore();
        ctx.save();
        ctx.translate(lx, ly);
        ctx.fillStyle = pColor;
        ctx.font = `${fs}px Roboto`;
        ctx.textAlign = "center";

        const norm = ((r % 360) + 360) % 360;
        if (norm === 0) {
          if (isGround) {
            // Ground: text below the ground lines
            ctx.textBaseline = "top";
            ctx.fillText(text, 0, stemLen + 1.2);
          } else {
            // Bar/arrow: text above the symbol
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

        continue; // Skip normal label rendering
      }

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
          const textYOff = fs * 0.1; // Offset to visually center uppercase text in flag
          if (dir > 0) {
            ctx.textAlign = "left";
            ctx.fillText(text, lx + arrowW + pad, ly + textYOff);
          } else {
            ctx.textAlign = "right";
            ctx.fillText(text, lx - arrowW - pad, ly + textYOff);
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

    // Reset alpha before child sheets
    ctx.globalAlpha = 1;
    // Child sheets
    ctx.globalAlpha = sf.sheetSymbols?.visible === false ? 0.12 : 1;
    for (const sheet of data.child_sheets) {
      const isSel = selectedIds.has(sheet.uuid);
      const sx = sheet.position.x, sy = sheet.position.y, sw = sheet.size[0], sh = sheet.size[1];
      // Fill background
      ctx.fillStyle = isSel ? "rgba(91,141,239,0.08)" : "rgba(91,141,239,0.03)";
      ctx.fillRect(sx, sy, sw, sh);
      // Border
      ctx.strokeStyle = isSel ? C.selection : C.sheet;
      ctx.lineWidth = isSel ? 0.25 : 0.2;
      ctx.setLineDash(isSel ? [0.4, 0.25] : []);
      ctx.strokeRect(sx, sy, sw, sh);
      ctx.setLineDash([]);
      // Sheet name (above)
      ctx.fillStyle = C.sheetText;
      ctx.font = "bold 1.2px Roboto"; ctx.textAlign = "left"; ctx.textBaseline = "bottom";
      ctx.fillText(sheet.name, sx + 0.5, sy - 0.3);
      // Filename (inside top)
      ctx.fillStyle = C.sheet;
      ctx.font = "0.8px Roboto Mono"; ctx.textBaseline = "top";
      ctx.fillText(sheet.filename, sx + 0.5, sy + 0.5);
      // Sheet pins (small arrows on edges)
      if (sheet.pins && sheet.pins.length > 0) {
        ctx.fillStyle = C.labelHier;
        ctx.font = "0.9px Roboto";
        ctx.textBaseline = "middle";
        for (const pin of sheet.pins) {
          const px = pin.position.x, py = pin.position.y;
          // Draw small triangle indicator
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

    // Buses (thicker blue lines)
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
    // Bus entries (short diagonal lines)
    ctx.lineWidth = 0.2;
    for (const be of data.bus_entries) {
      ctx.strokeStyle = selectedIds.has(be.uuid) ? C.selection : C.busEntry;
      ctx.beginPath();
      ctx.moveTo(be.position.x, be.position.y);
      ctx.lineTo(be.position.x + be.size[0], be.position.y + be.size[1]);
      ctx.stroke();
    }

    ctx.globalAlpha = 1;
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

    // Drawing objects (user-drawn lines, rects, circles, arcs, polylines)
    // Helper: apply line style dash pattern
    const applyLineStyle = (ls?: string) => {
      if (ls === "dash") ctx.setLineDash([1.0, 0.5]);
      else if (ls === "dot") ctx.setLineDash([0.2, 0.3]);
      else if (ls === "dash_dot") ctx.setLineDash([1.0, 0.3, 0.2, 0.3]);
      else ctx.setLineDash([]);
    };
    // Helper: draw arrow at point in direction angle
    const drawArrow = (x: number, y: number, angle: number, style?: string) => {
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
    };

    ctx.globalAlpha = sf.drawings?.visible === false ? 0.12 : 1;
    for (const d of data.drawings) {
      const sel = selectedIds.has(d.uuid);
      const strokeColor = sel ? C.selection : ("color" in d && d.color) || C.body;
      ctx.strokeStyle = strokeColor;
      ctx.fillStyle = strokeColor;
      ctx.lineWidth = Math.max("width" in d ? d.width || 0.15 : 0.15, 0.15);
      applyLineStyle("lineStyle" in d ? d.lineStyle : undefined);
      if (d.type === "Line") {
        ctx.beginPath(); ctx.moveTo(d.start.x, d.start.y); ctx.lineTo(d.end.x, d.end.y); ctx.stroke();
        const angle = Math.atan2(d.end.y - d.start.y, d.end.x - d.start.x);
        drawArrow(d.end.x, d.end.y, angle, d.arrowEnd);
        drawArrow(d.start.x, d.start.y, angle + Math.PI, d.arrowStart);
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
          // Arrows on polyline endpoints
          if (d.points.length >= 2) {
            const p0 = d.points[0], p1 = d.points[1];
            drawArrow(p0.x, p0.y, Math.atan2(p0.y - p1.y, p0.x - p1.x), d.arrowStart);
            const pn = d.points[d.points.length - 1], pn1 = d.points[d.points.length - 2];
            drawArrow(pn.x, pn.y, Math.atan2(pn.y - pn1.y, pn.x - pn1.x), d.arrowEnd);
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
        // Image rendering uses cached HTMLImageElement
        let img = IMAGE_CACHE.get(d.uuid);
        if (!img && d.dataUrl) {
          if (IMAGE_CACHE.size >= MAX_IMAGE_CACHE) {
            const firstKey = IMAGE_CACHE.keys().next().value;
            if (firstKey !== undefined) IMAGE_CACHE.delete(firstKey);
          }
          img = new Image();
          img.src = d.dataUrl;
          IMAGE_CACHE.set(d.uuid, img);
        }
        if (img?.complete) {
          ctx.drawImage(img, rx, ry, rw, rh);
        }
        if (sel) { ctx.strokeStyle = C.selection; ctx.setLineDash([0.3, 0.2]); ctx.strokeRect(rx, ry, rw, rh); ctx.setLineDash([]); }
      }
      ctx.setLineDash([]);
    }

    // Text notes (with special string substitution)
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

    // --- Altium-style selection highlights with corner handles ---
    if (selectedIds.size > 0) {
      const hs = 0.35 / camRef.current.zoom * 3; // Handle size in world units (constant screen size)

      // Helper: draw selection box with dashed outline + filled bg + corner handles
      const drawSelBox = (bx: number, by: number, bw: number, bh: number) => {
        // Altium-style: thin green dashed outline, no fill
        ctx.strokeStyle = "#66bb6a";
        ctx.lineWidth = 0.08;
        ctx.setLineDash([0.3, 0.2]);
        ctx.strokeRect(bx, by, bw, bh);
        ctx.setLineDash([]);
      };

      // Helper: draw endpoint handles for wires/buses
      const drawLineHandles = (x1: number, y1: number, x2: number, y2: number) => {
        ctx.strokeStyle = C.selection;
        ctx.lineWidth = 0.2;
        ctx.setLineDash([0.4, 0.25]);
        ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(x2, y2); ctx.stroke();
        ctx.setLineDash([]);
        ctx.fillStyle = C.handleFill;
        ctx.strokeStyle = C.handleBorder;
        ctx.lineWidth = 0.08;
        for (const [cx, cy] of [[x1, y1], [x2, y2]]) {
          ctx.fillRect(cx - hs / 2, cy - hs / 2, hs, hs);
          ctx.strokeRect(cx - hs / 2, cy - hs / 2, hs, hs);
        }
      };

      // Helper: draw point handle for junctions/no-connects
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
          // Power port: selection box covers symbol + stem + text
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
          // Port flag shape: tight selection matching the rendered flag
          const fs = label.font_size || 1.27;
          ctx.font = `${fs}px Roboto`;
          const tw = ctx.measureText(txt(label.text)).width; // Use txt() to convert {slash} to /
          const h = fs * 1.4;
          const pad = fs * 0.3;
          const arrowW = h * 0.5;
          const r = label.rotation;
          const isHoriz = r === 0 || r === 180;

          if (isHoriz) {
            const connRight = r === 0;
            // Total shape length: arrow + text body (+ possible output arrow)
            const totalLen = arrowW + tw + pad * 2 + (label.shape === "output" || label.shape === "bidirectional" ? arrowW : 0);
            if (connRight) {
              // Connection on left, flag extends right
              drawSelBox(label.position.x, label.position.y - h / 2, totalLen, h);
            } else {
              // Connection on right, flag extends left
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
          // Net label: selection box around text
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

    // --- Wire/Bus drawing preview with live Manhattan routing ---
    const isBusDrawing = useSchematicStore.getState().editMode === "drawBus";
    if (wireDrawing.active && wireDrawing.points.length > 0) {
      // Draw placed segments (solid)
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

    // --- Net Label / Power Port / No Connect placement preview ---
    const editMode2 = useSchematicStore.getState().editMode;
    const paused = useEditorStore.getState().placementPaused;
    if (!placing && editMode2 !== "select" && !paused) {
      const cur = placeCursorRef.current;
      ctx.globalAlpha = 0.6;

      if (editMode2 === "placeLabel") {
        // Net label preview — simple text with overline (NOT a flag/port shape)
        const labelText = "NET?";
        const fs = 1.27;
        ctx.font = `${fs}px Roboto`;
        const tw = ctx.measureText(labelText).width;

        // Text
        ctx.fillStyle = C.labelNet;
        ctx.textAlign = "left";
        ctx.textBaseline = "bottom";
        ctx.fillText(labelText, cur.x, cur.y - 0.3);

        // Overline
        ctx.strokeStyle = C.labelNet;
        ctx.lineWidth = 0.08;
        ctx.beginPath();
        ctx.moveTo(cur.x, cur.y - fs - 0.2);
        ctx.lineTo(cur.x + tw, cur.y - fs - 0.2);
        ctx.stroke();

        // Connection point indicator (small dot)
        ctx.fillStyle = C.labelNet;
        ctx.beginPath();
        ctx.arc(cur.x, cur.y, 0.15, 0, Math.PI * 2);
        ctx.fill();

      } else if (editMode2 === "placePower") {
        // Power port preview
        const preset = powerPreset.current;
        const stemLen = 2.0, symSize = 1.2;
        const isGnd = preset.style.includes("ground");
        const dir = isGnd ? 1 : -1;

        ctx.strokeStyle = C.power;
        ctx.lineWidth = 0.12;
        ctx.lineCap = "round";

        // Stem
        ctx.beginPath();
        ctx.moveTo(cur.x, cur.y);
        ctx.lineTo(cur.x, cur.y + dir * stemLen);
        ctx.stroke();

        // Symbol
        const sy = cur.y + dir * stemLen;
        if (preset.style === "bar") {
          ctx.lineWidth = 0.18;
          ctx.beginPath();
          ctx.moveTo(cur.x - symSize, sy);
          ctx.lineTo(cur.x + symSize, sy);
          ctx.stroke();
        } else if (isGnd) {
          ctx.lineWidth = 0.12;
          ctx.beginPath();
          ctx.moveTo(cur.x - symSize, sy);
          ctx.lineTo(cur.x + symSize, sy);
          ctx.moveTo(cur.x - symSize * 0.65, sy + dir * 0.4);
          ctx.lineTo(cur.x + symSize * 0.65, sy + dir * 0.4);
          ctx.moveTo(cur.x - symSize * 0.3, sy + dir * 0.8);
          ctx.lineTo(cur.x + symSize * 0.3, sy + dir * 0.8);
          ctx.stroke();
        }

        // Name
        ctx.fillStyle = C.power;
        ctx.font = "1.27px Roboto";
        ctx.textAlign = "center";
        ctx.textBaseline = isGnd ? "top" : "bottom";
        ctx.fillText(preset.net, cur.x, isGnd ? sy + 1.2 : sy - 0.4);

      } else if (editMode2 === "placeNoConnect") {
        // X mark preview
        ctx.strokeStyle = C.noConnect;
        ctx.lineWidth = 0.15;
        ctx.beginPath();
        ctx.moveTo(cur.x - 0.7, cur.y - 0.7);
        ctx.lineTo(cur.x + 0.7, cur.y + 0.7);
        ctx.moveTo(cur.x + 0.7, cur.y - 0.7);
        ctx.lineTo(cur.x - 0.7, cur.y + 0.7);
        ctx.stroke();

      } else if (editMode2 === "placePort") {
        // Port preview — flag shape with "PORT?" text
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
        ctx.moveTo(cur.x, cur.y);
        ctx.lineTo(cur.x + arrowW, cur.y - h / 2);
        ctx.lineTo(cur.x + arrowW + tw + pad * 2, cur.y - h / 2);
        ctx.lineTo(cur.x + arrowW + tw + pad * 2, cur.y + h / 2);
        ctx.lineTo(cur.x + arrowW, cur.y + h / 2);
        ctx.closePath();
        ctx.stroke();

        // Connection stub
        ctx.lineWidth = 0.1;
        ctx.beginPath();
        ctx.moveTo(cur.x, cur.y);
        ctx.lineTo(cur.x - 1, cur.y);
        ctx.stroke();

        // Text
        ctx.fillStyle = C.labelHier;
        ctx.textAlign = "left";
        ctx.textBaseline = "middle";
        ctx.fillText(portText, cur.x + arrowW + pad, cur.y);
      }

      ctx.globalAlpha = 1;
    }

    // --- Drawing tool ghost previews ---
    const curEdit = useSchematicStore.getState().editMode;
    const cur = placeCursorRef.current;
    if (drawStart.current && (curEdit === "drawLine" || curEdit === "drawRect" || curEdit === "drawCircle")) {
      ctx.strokeStyle = "#80deea";
      ctx.lineWidth = 0.1;
      ctx.setLineDash([0.3, 0.2]);
      ctx.globalAlpha = 0.6;
      const ds = drawStart.current;
      if (curEdit === "drawLine") {
        ctx.beginPath(); ctx.moveTo(ds.x, ds.y); ctx.lineTo(cur.x, cur.y); ctx.stroke();
      } else if (curEdit === "drawRect") {
        ctx.strokeRect(Math.min(ds.x, cur.x), Math.min(ds.y, cur.y), Math.abs(cur.x - ds.x), Math.abs(cur.y - ds.y));
      } else if (curEdit === "drawCircle") {
        const radius = Math.hypot(cur.x - ds.x, cur.y - ds.y);
        ctx.beginPath(); ctx.arc(ds.x, ds.y, radius, 0, Math.PI * 2); ctx.stroke();
      }
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
    }
    if (curEdit === "drawPolyline" && polyPoints.current.length > 0) {
      ctx.strokeStyle = "#80deea";
      ctx.lineWidth = 0.1;
      ctx.setLineDash([0.3, 0.2]);
      ctx.globalAlpha = 0.6;
      ctx.beginPath();
      ctx.moveTo(polyPoints.current[0].x, polyPoints.current[0].y);
      for (let i = 1; i < polyPoints.current.length; i++) {
        ctx.lineTo(polyPoints.current[i].x, polyPoints.current[i].y);
      }
      ctx.lineTo(cur.x, cur.y);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
    }

    // --- ERC markers ---
    const ercMarkers = useEditorStore.getState().ercMarkers;
    const showErc = useEditorStore.getState().showErcMarkers;
    if (showErc && ercMarkers.length > 0) {
      for (const marker of ercMarkers) {
        const mx = marker.position.x, my = marker.position.y;
        const r = 0.6;
        // Draw marker circle
        ctx.beginPath();
        ctx.arc(mx, my, r, 0, Math.PI * 2);
        ctx.fillStyle = marker.severity === "error" ? "rgba(239,83,80,0.3)" : "rgba(255,183,77,0.3)";
        ctx.fill();
        ctx.strokeStyle = marker.severity === "error" ? "#ef5350" : "#ffb74d";
        ctx.lineWidth = 0.12;
        ctx.stroke();
        // Draw icon (! for warning, X for error)
        ctx.fillStyle = marker.severity === "error" ? "#ef5350" : "#ffb74d";
        ctx.font = "bold 0.8px Roboto";
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(marker.severity === "error" ? "X" : "!", mx, my);
      }
    }

    // --- Drag-box selection rectangle ---
    if (selecting.current) {
      if (selectionModeRef.current === "lasso" && lassoPoints.current.length > 1) {
        // Lasso: draw freeform polygon
        ctx.strokeStyle = "#00bfff";
        ctx.fillStyle = "rgba(0,191,255,0.08)";
        ctx.lineWidth = 0.2;
        ctx.setLineDash([]);
        ctx.beginPath();
        ctx.moveTo(lassoPoints.current[0].x, lassoPoints.current[0].y);
        for (let i = 1; i < lassoPoints.current.length; i++) {
          ctx.lineTo(lassoPoints.current[i].x, lassoPoints.current[i].y);
        }
        ctx.closePath();
        ctx.fill();
        ctx.stroke();
      } else {
        // Box selection (all rect-based modes)
        const s = selectStart.current;
        const e = selectEnd.current;
        const rx = Math.min(s.x, e.x), ry = Math.min(s.y, e.y);
        const rw = Math.abs(e.x - s.x), rh = Math.abs(e.y - s.y);
        const mode = selectionModeRef.current;
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

    ctx.restore(); // End world-space transform
  }, [data, drawGraphicTransformed, drawTextProp, selectedIds, wireDrawing, placingSymbol, gridVisible, gridSize, inPlaceEdit]);

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
    return () => cancelAnimationFrame(animRef.current);
  }, [render]);

  // Cross-probe zoom: when a panel requests zoom-to-object
  const zoomToRequest = useSchematicStore((s) => s.zoomToRequest);
  useEffect(() => {
    if (!zoomToRequest || !containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    const zoom = zoomToRequest.zoom ?? 6;
    camRef.current = {
      zoom,
      x: rect.width / 2 - zoomToRequest.x * zoom,
      y: rect.height / 2 - zoomToRequest.y * zoom,
    };
    useSchematicStore.getState().clearZoomRequest();
    render();
  }, [zoomToRequest, render]);

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

  // Cleanup autoPan interval on unmount
  useEffect(() => {
    return () => {
      if (autoPanRef.current) {
        clearInterval(autoPanRef.current);
        autoPanRef.current = null;
      }
    };
  }, []);

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
    // Right-click: finish wire, show context menu, or pan
    if (e.button === 2) {
      const store = useSchematicStore.getState();
      if (store.wireDrawing.active) {
        e.preventDefault();
        if (store.editMode === "drawBus") store.finishBus();
        else store.finishWire();
        return;
      }
      // Polyline: finish on right-click
      if (store.editMode === "drawPolyline" && polyPoints.current.length >= 2) {
        e.preventDefault();
        store.addDrawing({ type: "Polyline", uuid: crypto.randomUUID(), points: [...polyPoints.current], width: 0.15, fill: false });
        polyPoints.current = [];
        store.setEditMode("select");
        return;
      }
      // Any placement mode: cancel and return to select
      if (store.editMode !== "select") {
        e.preventDefault();
        drawStart.current = null;
        drawMid.current = null;
        polyPoints.current = [];
        store.setEditMode("select");
        return;
      }
      // Select mode: right-click-drag = pan, right-click (no drag) = context menu
      // Start pan immediately, show menu on mouseup if no drag occurred
      e.preventDefault();
      dragging.current = true;
      (dragging as any).totalPanDist = 0;
      lastMouse.current = { x: e.clientX, y: e.clientY };
      const r2 = canvasRef.current?.getBoundingClientRect();
      const world2 = r2 ? s2w(e.clientX - r2.left, e.clientY - r2.top) : { x: 0, y: 0 };
      // Store position relative to canvas container for correct menu placement
      const containerRect = canvasRef.current?.parentElement?.getBoundingClientRect();
      const menuX = containerRect ? e.clientX - containerRect.left : e.clientX;
      const menuY = containerRect ? e.clientY - containerRect.top : e.clientY;
      (dragging as any).rightClickPos = { x: menuX, y: menuY, worldX: world2.x, worldY: world2.y };
      return;
    }
    // Middle button = pan
    if (e.button === 1) {
      dragging.current = true;
      lastMouse.current = { x: e.clientX, y: e.clientY };
      e.preventDefault();
      return;
    }

    // Left button = select, move, or wire
    if (ctxMenu) setCtxMenu(null); // Dismiss context menu on left click
    if (e.button === 0 && data) {
      const r = canvasRef.current?.getBoundingClientRect();
      if (!r) return;
      const world = s2w(e.clientX - r.left, e.clientY - r.top);
      const store = useSchematicStore.getState();

      if (store.editMode === "drawWire") {
        // Electrical snap on click: prefer pin/wire endpoints, else grid snap
        const eSnap = findNearestElectricalPoint(data, world.x, world.y);
        const wirePos = eSnap || snapPoint(world);
        if (store.wireDrawing.active) {
          store.addWirePoint(wirePos);
        } else {
          store.startWire(wirePos);
        }
        // Keep cursor ref in sync — same snapped position, no jump
        wireCursorRef.current = wirePos;
        return;
      }

      // If placement is paused (Tab pressed), clicking on canvas resumes
      if (useEditorStore.getState().placementPaused) {
        useEditorStore.getState().setPlacementPaused(false);
        return;
      }

      if (store.editMode === "placeSymbol" && store.placingSymbol) {
        store.placeSymbolAt(world);
        return;
      }

      if (store.editMode === "placeLabel") {
        const eSnap = findNearestElectricalPoint(data, world.x, world.y);
        const pos = eSnap || world;
        const sp = w2s(pos.x, pos.y);
        // Place label and immediately enter in-place edit for naming
        store.placeNetLabel(pos, "NET?");
        // Find the just-placed label (last one) and start editing
        const newData = useSchematicStore.getState().data;
        if (newData && newData.labels.length > 0) {
          const newLabel = newData.labels[newData.labels.length - 1];
          store.select(newLabel.uuid);
          setInPlaceEdit({ uuid: newLabel.uuid, field: "text", value: "NET?", screenX: sp.x, screenY: sp.y });
        }
        return;
      }

      if (store.editMode === "placePower") {
        const eSnap = findNearestElectricalPoint(data, world.x, world.y);
        const pos = eSnap || world;
        const sp = w2s(pos.x, pos.y);
        const preset = powerPreset.current;
        store.placePowerPort(pos, preset.net, preset.style);
        const newData = useSchematicStore.getState().data;
        if (newData && newData.labels.length > 0) {
          const newLabel = newData.labels[newData.labels.length - 1];
          store.select(newLabel.uuid);
          setInPlaceEdit({ uuid: newLabel.uuid, field: "text", value: preset.net, screenX: sp.x, screenY: sp.y });
        }
        return;
      }

      if (store.editMode === "placeNoConnect") {
        store.placeNoConnect(world);
        return;
      }

      if (store.editMode === "placeNoErc") {
        const eSnap = findNearestElectricalPoint(data, world.x, world.y);
        store.placeNoErcDirective(eSnap || world);
        return;
      }

      if (store.editMode === "placeParameterSet") {
        store.placeParameterSet(world);
        return;
      }

      if (store.editMode === "placeDifferentialPair") {
        store.placeDifferentialPairDirective(world);
        return;
      }

      if (store.editMode === "placeBlanket") {
        store.placeBlanket(world);
        return;
      }

      if (store.editMode === "placeCompileMask") {
        store.placeCompileMask(world);
        return;
      }

      if (store.editMode === "placeTextFrame") {
        store.placeTextFrame(world);
        return;
      }

      if (store.editMode === "placeNote") {
        store.placeNote(world);
        return;
      }

      if (store.editMode === "placePort") {
        const eSnap = findNearestElectricalPoint(data, world.x, world.y);
        const pos = eSnap || world;
        const sp = w2s(pos.x, pos.y);
        store.placePort(pos, "PORT?", "bidirectional");
        const newData = useSchematicStore.getState().data;
        if (newData && newData.labels.length > 0) {
          const newLabel = newData.labels[newData.labels.length - 1];
          store.select(newLabel.uuid);
          setInPlaceEdit({ uuid: newLabel.uuid, field: "text", value: "PORT?", screenX: sp.x, screenY: sp.y });
        }
        return;
      }

      if (store.editMode === "placeText") {
        store.addTextNote(world, "Text");
        const newData = useSchematicStore.getState().data;
        if (newData && newData.text_notes.length > 0) {
          const newNote = newData.text_notes[newData.text_notes.length - 1];
          store.select(newNote.uuid);
          // Open properties panel for editing via Tab
          const layout = useLayoutStore.getState();
          if (layout.rightCollapsed) layout.toggleRight();
        }
        return;
      }

      if (store.editMode === "drawLine") {
        const snapped = snapPoint(world);
        if (!drawStart.current) {
          drawStart.current = snapped; // Reuse measure refs for line start/end
        } else {
          store.addDrawing({ type: "Line", uuid: crypto.randomUUID(), start: drawStart.current, end: snapped, width: 0.15 });
          drawStart.current = null;
        }
        cancelAnimationFrame(animRef.current);
        animRef.current = requestAnimationFrame(render);
        return;
      }

      if (store.editMode === "drawRect") {
        const snapped = snapPoint(world);
        if (!drawStart.current) {
          drawStart.current = snapped;
        } else {
          store.addDrawing({ type: "Rect", uuid: crypto.randomUUID(), start: drawStart.current, end: snapped, width: 0.15, fill: false });
          drawStart.current = null;
        }
        cancelAnimationFrame(animRef.current);
        animRef.current = requestAnimationFrame(render);
        return;
      }

      // Circle: 1st click = center, 2nd click = edge (radius)
      if (store.editMode === "drawCircle") {
        const snapped = snapPoint(world);
        if (!drawStart.current) {
          drawStart.current = snapped;
        } else {
          const radius = Math.hypot(snapped.x - drawStart.current.x, snapped.y - drawStart.current.y);
          if (radius > 0.1) {
            store.addDrawing({ type: "Circle", uuid: crypto.randomUUID(), center: drawStart.current, radius, width: 0.15, fill: false });
          }
          drawStart.current = null;
        }
        cancelAnimationFrame(animRef.current);
        animRef.current = requestAnimationFrame(render);
        return;
      }

      // Polyline: accumulate points; double-click or right-click finishes
      if (store.editMode === "drawPolyline") {
        const snapped = snapPoint(world);
        polyPoints.current.push(snapped);
        cancelAnimationFrame(animRef.current);
        animRef.current = requestAnimationFrame(render);
        return;
      }

      // Sheet symbol placement
      if (store.editMode === "placeSheetSymbol") {
        const snapped = snapPoint(world);
        store.placeSheetSymbol(snapped, "Sheet", "sheet.kicad_sch");
        store.setEditMode("select");
        cancelAnimationFrame(animRef.current);
        animRef.current = requestAnimationFrame(render);
        return;
      }

      // Bus entry placement
      if (store.editMode === "placeBusEntry") {
        const snapped = snapPoint(world);
        store.placeBusEntry(snapped);
        cancelAnimationFrame(animRef.current);
        animRef.current = requestAnimationFrame(render);
        return;
      }

      if (store.editMode === "drawBus") {
        // Bus drawing uses the same wire drawing state machine
        const eSnap = findNearestElectricalPoint(data, world.x, world.y);
        const busPos = eSnap || snapPoint(world);
        if (store.wireDrawing.active) {
          store.addWirePoint(busPos);
        } else {
          store.startWire(busPos);
        }
        wireCursorRef.current = busPos;
        return;
      }

      // Alt+Click = select entire net (Altium behavior)
      if (e.altKey) {
        const hit = hitTest(data, world.x, world.y, 2.0, useEditorStore.getState().selectionFilter);
        if (hit) {
          const nets = resolveNets(data);
          // Find which net this element belongs to
          const net = nets.find(n =>
            n.wireUuids.includes(hit.uuid) || n.labelUuids.includes(hit.uuid) || n.junctionUuids.includes(hit.uuid) ||
            n.pins.some(p => p.symbolUuid === hit.uuid)
          );
          if (net) {
            const allUuids = [...net.wireUuids, ...net.labelUuids, ...net.junctionUuids];
            store.selectMultiple(allUuids);
          } else {
            store.select(hit.uuid);
          }
        }
        return;
      }

      // Select mode: hit test
      const hit = hitTest(data, world.x, world.y, 2.0, useEditorStore.getState().selectionFilter);
      if (hit) {
        // Wire endpoint drag — special handling for dragging wire vertices
        if (hit.type === "wireEndpoint" && hit.endpoint) {
          store.select(hit.uuid);
          draggingEndpoint.current = { uuid: hit.uuid, endpoint: hit.endpoint };
          store.pushUndo();
          return;
        }
        // Wire body drag → move the whole wire segment
        if (e.shiftKey) {
          store.toggleSelect(hit.uuid);
        } else if (!store.selectedIds.has(hit.uuid)) {
          store.select(hit.uuid);
        }
        // Start move drag (Ctrl = move without rubber-banding, like Altium)
        moving.current = true;
        moveNoRubber.current = e.ctrlKey;
        moveStart.current = { x: world.x, y: world.y };
        // Push undo before move
        store.pushUndo();
      } else {
        // Start drag-box or lasso selection — also clear AutoFocus
        if (!e.shiftKey) { store.deselectAll(); useEditorStore.getState().setAutoFocus(null); }
        selecting.current = true;
        selectStart.current = { x: world.x, y: world.y };
        selectEnd.current = { x: world.x, y: world.y };
        if (selectionModeRef.current === "lasso") {
          lassoPoints.current = [{ x: world.x, y: world.y }];
        }
        // Global listeners so selection works even when mouse leaves canvas
        const globalMove = (ev: MouseEvent) => {
          const cr = canvasRef.current?.getBoundingClientRect();
          if (!cr) return;
          const world2 = s2w(ev.clientX - cr.left, ev.clientY - cr.top);
          selectEnd.current = { x: world2.x, y: world2.y };
          if (selectionModeRef.current === "lasso") {
            lassoPoints.current.push({ x: world2.x, y: world2.y });
          }
          cancelAnimationFrame(animRef.current);
          animRef.current = requestAnimationFrame(render);
        };
        const globalUp = () => {
          handleMouseUp();
          window.removeEventListener("mousemove", globalMove);
          window.removeEventListener("mouseup", globalUp);
        };
        window.addEventListener("mousemove", globalMove);
        window.addEventListener("mouseup", globalUp);
      }
    }
  }, [data, s2w, w2s, ctxMenu]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    const r = canvasRef.current?.getBoundingClientRect();
    if (!r) return;
    const world = s2w(e.clientX - r.left, e.clientY - r.top);
    updateStatusBar({
      cursorPosition: { x: Math.round(world.x * 100) / 100, y: Math.round(world.y * 100) / 100 },
    });

    // Pan
    if (dragging.current) {
      const dx = e.clientX - lastMouse.current.x;
      const dy = e.clientY - lastMouse.current.y;
      camRef.current.x += dx;
      camRef.current.y += dy;
      (dragging as any).totalPanDist = ((dragging as any).totalPanDist || 0) + Math.abs(dx) + Math.abs(dy);
      lastMouse.current = { x: e.clientX, y: e.clientY };
      cancelAnimationFrame(animRef.current);
      animRef.current = requestAnimationFrame(render);
      return;
    }

    // Wire endpoint dragging
    if (draggingEndpoint.current && data) {
      const eSnap = findNearestElectricalPoint(data, world.x, world.y);
      const pos = eSnap || snapPoint(world);
      useSchematicStore.getState().moveWireEndpoint(draggingEndpoint.current.uuid, draggingEndpoint.current.endpoint, pos);
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
      // Update cursor for drawing tool ghost previews
      const drawModes = ["drawLine", "drawRect", "drawCircle", "drawPolyline", "placeBusEntry", "placeSheetSymbol", "placeLabel", "placePower", "placeNoConnect", "placePort", "placeText", "placeNoErc", "placeParameterSet", "placeDifferentialPair", "placeBlanket", "placeCompileMask", "placeTextFrame", "placeNote"];
      if (drawModes.includes(store.editMode)) {
        // Use electrical snap for placement modes that snap to pins/wires
        const eSnapModes = ["placeLabel", "placePower", "placePort", "placeNoConnect", "placeNoErc"];
        const eSnap = eSnapModes.includes(store.editMode) && data ? findNearestElectricalPoint(data, world.x, world.y) : null;
        placeCursorRef.current = eSnap || snapPoint(world);
        cancelAnimationFrame(animRef.current);
        animRef.current = requestAnimationFrame(render);
      }
    }

    // Drag-box selection update
    if (selecting.current) {
      selectEnd.current = { x: world.x, y: world.y };
      if (selectionModeRef.current === "lasso") {
        lassoPoints.current.push({ x: world.x, y: world.y });
      }
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
          store.moveElements([...store.selectedIds], dx, dy, moveNoRubber.current);
          moveStart.current = { x: snapped.x, y: snapped.y };
        }
      }
    }

    // Auto-pan: when cursor is near edge during active modes (NOT when paused)
    const store2 = useSchematicStore.getState();
    const isActive = !useEditorStore.getState().placementPaused && (store2.wireDrawing.active || store2.editMode !== "select" || selecting.current || moving.current);
    if (isActive) {
      const sx = e.clientX - r.left, sy = e.clientY - r.top;
      const edge = 50; // pixels from edge
      const speed = 6; // pixels per frame
      let pdx = 0, pdy = 0;
      if (sx < edge) pdx = speed * (1 - sx / edge);
      else if (sx > r.width - edge) pdx = -speed * (1 - (r.width - sx) / edge);
      if (sy < edge) pdy = speed * (1 - sy / edge);
      else if (sy > r.height - edge) pdy = -speed * (1 - (r.height - sy) / edge);
      // Also handle mouse outside canvas (negative sx/sy or beyond width/height)
      if (sx < 0) pdx = speed * 1.5;
      if (sx > r.width) pdx = -speed * 1.5;
      if (sy < 0) pdy = speed * 1.5;
      if (sy > r.height) pdy = -speed * 1.5;

      autoPanDir.current = { dx: pdx, dy: pdy };

      if (pdx !== 0 || pdy !== 0) {
        camRef.current.x += pdx;
        camRef.current.y += pdy;
        // Update selection end point to follow pan
        if (selecting.current) {
          const world = s2w(e.clientX - r.left, e.clientY - r.top);
          selectEnd.current = { x: world.x, y: world.y };
          if (selectionModeRef.current === "lasso") {
            lassoPoints.current.push({ x: world.x, y: world.y });
          }
        }
        cancelAnimationFrame(animRef.current);
        animRef.current = requestAnimationFrame(render);

        // Start interval for continued panning when mouse stays at edge
        if (!autoPanRef.current) {
          autoPanRef.current = setInterval(() => {
            const { dx, dy } = autoPanDir.current;
            if (dx === 0 && dy === 0) {
              if (autoPanRef.current) { clearInterval(autoPanRef.current); autoPanRef.current = null; }
              return;
            }
            camRef.current.x += dx;
            camRef.current.y += dy;
            cancelAnimationFrame(animRef.current);
            animRef.current = requestAnimationFrame(render);
          }, 16);
        }
      } else {
        autoPanDir.current = { dx: 0, dy: 0 };
        if (autoPanRef.current) { clearInterval(autoPanRef.current); autoPanRef.current = null; }
      }
    } else {
      if (autoPanRef.current) { clearInterval(autoPanRef.current); autoPanRef.current = null; }
    }
  }, [render, s2w, updateStatusBar, data]);

  const handleMouseUp = useCallback(() => {
    // Finalize selection
    if (selecting.current && data) {
      selecting.current = false;
      const s = selectStart.current, e = selectEnd.current;
      const filter = useEditorStore.getState().selectionFilter;
      const mode = selectionModeRef.current;
      let uuids: string[] = [];

      if (mode === "lasso" && lassoPoints.current.length > 2) {
        uuids = lassoSelect(data, lassoPoints.current, filter);
        lassoPoints.current = [];
      } else if (mode === "outsideArea" && (Math.abs(e.x - s.x) > 0.5 || Math.abs(e.y - s.y) > 0.5)) {
        uuids = outsideBoxSelect(data, s.x, s.y, e.x, e.y, filter);
      } else if (mode === "touchingRect" && (Math.abs(e.x - s.x) > 0.5 || Math.abs(e.y - s.y) > 0.5)) {
        // Force crossing mode (R→L behavior)
        uuids = boxSelect(data, e.x, e.y, s.x, s.y, filter);
      } else if (mode === "insideArea" && (Math.abs(e.x - s.x) > 0.5 || Math.abs(e.y - s.y) > 0.5)) {
        // Force inside mode (L→R behavior)
        uuids = boxSelect(data, s.x, s.y, e.x, e.y, filter);
      } else if (mode === "touchingLine") {
        if (Math.abs(e.x - s.x) > 0.5 || Math.abs(e.y - s.y) > 0.5) {
          uuids = lineSelect(data, s, e, 1.0, filter);
        }
      } else if (Math.abs(e.x - s.x) > 0.5 || Math.abs(e.y - s.y) > 0.5) {
        // Default box mode
        uuids = boxSelect(data, s.x, s.y, e.x, e.y, filter);
      }

      if (uuids.length > 0) {
        useSchematicStore.getState().selectMultiple(uuids);
      }
      cancelAnimationFrame(animRef.current);
      animRef.current = requestAnimationFrame(render);
    }
    // Right-click release without drag = show Altium-style context menu
    if (dragging.current && (dragging as any).rightClickPos) {
      const rcp = (dragging as any).rightClickPos;
      const movedDist = (dragging as any).totalPanDist || 0;
      if (movedDist < 5 && data) {
        const world = { x: rcp.worldX, y: rcp.worldY };
        const hit = hitTest(data, world.x, world.y, 2.0, useEditorStore.getState().selectionFilter);
        const store = useSchematicStore.getState();

        // If right-clicked on an unselected object, select it
        if (hit && !store.selectedIds.has(hit.uuid)) {
          store.select(hit.uuid);
        }

        const s = useSchematicStore.getState;
        const sel = s().selectedIds;
        const sep: ContextMenuItem = { separator: true, label: "", action: () => {} };
        const items: ContextMenuItem[] = [];
        const hasSel = sel.size > 0;

        // Top section: Find
        items.push({ label: "Find Similar Objects...", shortcut: "Shift+F", action: () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "F", shiftKey: true })) });
        items.push({ label: "Find Text...", shortcut: "Ctrl+F", action: () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "f", ctrlKey: true })) });
        items.push({ label: "Clear Filter", shortcut: "Shift+C", action: () => useEditorStore.getState().resetFilter() });
        items.push(sep);

        // Place submenu
        items.push({ label: "Place", action: () => {}, children: [
          { label: "Wire", shortcut: "Ctrl+W", action: () => s().setEditMode("drawWire") },
          { label: "Bus", action: () => s().setEditMode("drawBus") },
          { label: "Bus Entry", action: () => s().setEditMode("placeBusEntry") },
          { label: "Net Label", action: () => s().setEditMode("placeLabel") },
          { label: "Power Port", action: () => s().setEditMode("placePower") },
          { label: "Port", action: () => s().setEditMode("placePort") },
          { label: "No Connect", action: () => s().setEditMode("placeNoConnect") },
          sep,
          { label: "Part...", shortcut: "P", action: () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "p" })) },
          { label: "Sheet Symbol", action: () => s().setEditMode("placeSheetSymbol") },
          { label: "Sheet Entry", action: () => {} },
          sep,
          { label: "Text String", action: () => s().setEditMode("placeText") },
          { label: "Text Frame", action: () => s().setEditMode("placeTextFrame") },
          { label: "Note", action: () => s().setEditMode("placeNote") },
          sep,
          { label: "Drawing Tools", action: () => {}, children: [
            { label: "Line", action: () => s().setEditMode("drawLine") },
            { label: "Rectangle", action: () => s().setEditMode("drawRect") },
            { label: "Circle", action: () => s().setEditMode("drawCircle") },
            { label: "Polyline", action: () => s().setEditMode("drawPolyline") },
          ]},
        ]});

        // Part Actions (when component selected)
        if (hasSel) {
          items.push({ label: "Part Actions", action: () => {}, children: [
            { label: "Rotate", shortcut: "Space", action: () => s().rotateSelected() },
            { label: "Flip Horizontal", shortcut: "X", action: () => s().mirrorSelectedY() },
            { label: "Flip Vertical", shortcut: "Y", action: () => s().mirrorSelectedX() },
            sep,
            { label: "Bring to Front", action: () => s().bringToFront() },
            { label: "Send to Back", action: () => s().sendToBack() },
          ]});
        }

        // Sheet Actions
        items.push({ label: "Sheet Actions", action: () => {}, children: [
          { label: "Create Sheet Symbol From Sheet", action: () => {} },
          { label: "Create Component From Sheet", action: () => {} },
        ]});

        // References (when selected)
        if (hasSel) {
          items.push({ label: "References", action: () => {} });
        }

        // Align (when multiple selected)
        if (sel.size > 1) {
          items.push({ label: "Align", action: () => {}, children: [
            { label: "Align Left", shortcut: "Shift+Ctrl+L", action: () => s().alignSelected("left") },
            { label: "Align Right", shortcut: "Shift+Ctrl+R", action: () => s().alignSelected("right") },
            { label: "Align Top", shortcut: "Shift+Ctrl+T", action: () => s().alignSelected("top") },
            { label: "Align Bottom", shortcut: "Shift+Ctrl+B", action: () => s().alignSelected("bottom") },
            sep,
            { label: "Distribute Horizontally", shortcut: "Shift+Ctrl+H", action: () => s().distributeSelected("horizontal") },
            { label: "Distribute Vertically", shortcut: "Shift+Ctrl+V", action: () => s().distributeSelected("vertical") },
            sep,
            { label: "Align to Grid", shortcut: "Shift+Ctrl+D", action: () => s().alignSelectionToGrid() },
          ]});
        }

        // Cross Probe
        items.push({ label: "Cross Probe", action: () => {} });
        items.push(sep);

        // Clipboard
        items.push({ label: "Cut", shortcut: "Ctrl+X", action: () => { s().copySelected(); s().deleteSelected(); }, disabled: !hasSel });
        items.push({ label: "Copy", shortcut: "Ctrl+C", action: () => s().copySelected(), disabled: !hasSel });
        items.push({ label: "Paste", shortcut: "Ctrl+V", action: () => s().pasteClipboard(world), disabled: !s().clipboard });
        if (hasSel) {
          items.push({ label: "Delete", shortcut: "Del", action: () => s().deleteSelected() });
        }
        items.push(sep);

        // Break wire
        if (hit && hit.type === "wire" && sel.size === 1) {
          items.push({ label: "Break Wire", action: () => s().breakWireAt(hit.uuid, { x: world.x, y: world.y }) });
          items.push(sep);
        }

        // Bottom section
        items.push({ label: "Preferences...", action: () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "," })) });
        if (hasSel) {
          items.push({ label: "Properties...", shortcut: "F11", action: () => {
            useLayoutStore.getState().setDockActiveTab("right", "properties");
            if (useLayoutStore.getState().rightCollapsed) useLayoutStore.getState().toggleRight();
          }});
        }

        setCtxMenu({ x: rcp.x, y: rcp.y, items });
      }
      (dragging as any).rightClickPos = null;
    }
    dragging.current = false;
    moving.current = false;
    draggingEndpoint.current = null;
    // Stop auto-pan
    autoPanDir.current = { dx: 0, dy: 0 };
    if (autoPanRef.current) { clearInterval(autoPanRef.current); autoPanRef.current = null; }
  }, [data, render]);

  const handleDblClick = useCallback((e: React.MouseEvent) => {
    const store = useSchematicStore.getState();
    if (store.editMode === "drawWire" && store.wireDrawing.active) {
      store.finishWire();
      return;
    }
    // Double-click finishes polyline
    if (store.editMode === "drawPolyline" && polyPoints.current.length >= 2) {
      store.addDrawing({ type: "Polyline", uuid: crypto.randomUUID(), points: [...polyPoints.current], width: 0.15, fill: false });
      polyPoints.current = [];
      store.setEditMode("select");
      return;
    }

    // Ctrl+Double-Click on child sheet = navigate to that sheet
    if (e.ctrlKey && data) {
      const r = canvasRef.current?.getBoundingClientRect();
      if (r) {
        const world = s2w(e.clientX - r.left, e.clientY - r.top);
        const hit = hitTest(data, world.x, world.y, 2.0, useEditorStore.getState().selectionFilter);
        if (hit?.type === "childSheet") {
          const sheet = data.child_sheets.find((s) => s.uuid === hit.uuid);
          if (sheet) {
            // Open the child sheet tab
            const project = useProjectStore.getState().project;
            if (project) {
              const { openTab } = useProjectStore.getState();
              openTab({
                id: `sch-${project.path}:${sheet.filename}`,
                name: sheet.name,
                type: "schematic",
                path: project.path,
                dirty: false,
              });
            }
          }
          return;
        }
      }
    }

    // Double-click = in-place edit (Altium behavior)
    // Only triggers on TEXT elements: labels, net labels, text notes, symbol ref/value text
    // NEVER triggers on component body
    if (data) {
      const r = canvasRef.current?.getBoundingClientRect();
      if (!r) return;
      const world = s2w(e.clientX - r.left, e.clientY - r.top);

      // Check labels — use hitTest for proper zone matching, position edit at text location
      {
        const hit = hitTest(data, world.x, world.y, 2.0, useEditorStore.getState().selectionFilter);
        if (hit && hit.type === "label") {
          const label = data.labels.find(l => l.uuid === hit.uuid);
          if (label) {
            store.select(label.uuid);
            const textPos = getLabelTextWorldPos(label);
            const sp = w2s(textPos.x, textPos.y);
            setInPlaceEdit({ uuid: label.uuid, field: "text", value: label.text, screenX: sp.x, screenY: sp.y });
            return;
          }
        }
      }

      // Check text notes
      for (const note of data.text_notes) {
        if (Math.hypot(world.x - note.position.x, world.y - note.position.y) < 3) {
          store.select(note.uuid);
          const sp = w2s(note.position.x, note.position.y);
          setInPlaceEdit({ uuid: note.uuid, field: "text", value: note.text, screenX: sp.x, screenY: sp.y });
          return;
        }
      }

      // Check symbol text fields (ref/value) — must click NEAR the text position, not body
      for (const sym of data.symbols) {
        if (sym.is_power) continue;
        // Check reference text position (tight radius)
        if (!sym.ref_text.hidden) {
          const d = Math.hypot(world.x - sym.ref_text.position.x, world.y - sym.ref_text.position.y);
          if (d < 1.5) {
            store.select(sym.uuid);
            const sp = w2s(sym.ref_text.position.x, sym.ref_text.position.y);
            setInPlaceEdit({ uuid: sym.uuid, field: "reference", value: sym.reference, screenX: sp.x, screenY: sp.y });
            return;
          }
        }
        // Check value text position (tight radius)
        if (!sym.val_text.hidden) {
          const d = Math.hypot(world.x - sym.val_text.position.x, world.y - sym.val_text.position.y);
          if (d < 1.5) {
            store.select(sym.uuid);
            const sp = w2s(sym.val_text.position.x, sym.val_text.position.y);
            setInPlaceEdit({ uuid: sym.uuid, field: "value", value: sym.value, screenX: sp.x, screenY: sp.y });
            return;
          }
        }
      }
    }
  }, [data, s2w, w2s]);

  // --- Keyboard shortcuts ---
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!data) return;
      const store = useSchematicStore.getState();

      // Don't handle if typing in an input or contenteditable element
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement) return;
      if (document.activeElement?.getAttribute("contenteditable")) return;

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
        case "Tab": {
          // Altium: Tab opens properties panel (during placement or with selection)
          e.preventDefault();
          const layout = useLayoutStore.getState();
          layout.setDockActiveTab("right", "properties");
          if (layout.rightCollapsed) layout.toggleRight();
          break;
        }
        case "Escape":
          useEditorStore.getState().setPlacementPaused(false);
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
            setInPlaceEdit({ uuid: selId, field: "reference", value: sym.reference, screenX: sp.x, screenY: sp.y });
            break;
          }
          const lbl = data.labels.find(l => l.uuid === selId);
          if (lbl) {
            const textPos = getLabelTextWorldPos(lbl);
            const sp = w2s(textPos.x, textPos.y);
            setInPlaceEdit({ uuid: selId, field: "text", value: lbl.text, screenX: sp.x, screenY: sp.y });
          }
          break;
        }
        case "w":
        case "W":
          if (e.ctrlKey) { e.preventDefault(); store.setEditMode("drawWire"); }
          else if (!e.ctrlKey) store.setEditMode("drawWire");
          break;
        case "b":
        case "B":
          if (!e.ctrlKey) store.setEditMode("drawBus");
          break;
        case "t":
        case "T":
          if (!e.ctrlKey) store.setEditMode("placeText");
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
          e.preventDefault();
          if (e.shiftKey && store.wireDrawing.active) {
            store.cycleWireRouting();
          } else if (store.placingSymbol) {
            store.rotatePlacement(); // Space = rotate during placement (Altium)
          } else if (store.selectedIds.size > 0) {
            store.rotateSelected(); // Space = rotate selected (Altium)
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
        case "F":
          if (e.shiftKey && !e.ctrlKey) {
            // Shift+F = Find Similar Objects
            store.findSimilar();
          }
          break;
        case "h":
          if (e.ctrlKey) {
            e.preventDefault();
            setFindShowReplace(true);
            setFindOpen(true);
          }
          break;
        case "F5":
          // Toggle Net Color Override
          {
            useEditorStore.getState().toggleNetColors();
            const editor = useEditorStore.getState(); // Read AFTER toggle
            if (editor.netColorOverride && data) {
              // Build net colors on enable
              const nets = resolveNets(data);
              const palette = ["#ff6b6b","#51cf66","#339af0","#fcc419","#cc5de8","#20c997","#ff922b","#845ef7","#f06595","#22b8cf","#94d82d","#fd7e14"];
              const colors: Record<string, string> = {};
              nets.forEach((net: { wireUuids: string[]; labelUuids: string[]; junctionUuids: string[] }, i: number) => {
                const color = palette[i % palette.length];
                for (const uuid of [...net.wireUuids, ...net.labelUuids, ...net.junctionUuids]) {
                  colors[uuid] = color;
                }
              });
              editor.setNetColors(colors);
            }
            cancelAnimationFrame(animRef.current);
            animRef.current = requestAnimationFrame(render);
          }
          break;
        case "d":
        case "D":
          if (e.ctrlKey && e.shiftKey) {
            e.preventDefault();
            store.alignSelectionToGrid();
          } else if (e.ctrlKey) {
            e.preventDefault();
            store.duplicateSelected();
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
          if (e.ctrlKey) {
            // Ctrl+X = Cut
            e.preventDefault();
            store.copySelected();
            store.deleteSelected();
          } else {
            // X = horizontal flip (mirror around Y axis) — Altium convention
            if (store.placingSymbol) store.mirrorPlacementY();
            else if (store.selectedIds.size > 0) store.mirrorSelectedY();
          }
          break;
        case "c":
        case "C":
          if (e.ctrlKey) { e.preventDefault(); store.copySelected(); }
          break;
        case "v":
        case "V":
          if (e.ctrlKey && e.shiftKey) {
            e.preventDefault(); store.smartPaste({ x: 2.54, y: 2.54 });
          } else if (e.ctrlKey) {
            e.preventDefault(); store.pasteClipboard({ x: 2.54, y: 2.54 });
          }
          break;
        case "y":
        case "Y":
          if (e.ctrlKey) {
            e.preventDefault(); store.redo();
          } else {
            // Y = vertical flip (mirror around X axis) — Altium convention
            if (store.placingSymbol) store.mirrorPlacementX();
            else if (store.selectedIds.size > 0) store.mirrorSelectedX();
          }
          break;
        case "g":
          if (e.shiftKey && e.ctrlKey) {
            // Shift+Ctrl+G = toggle grid visibility (Altium)
            useEditorStore.getState().toggleGrid();
          } else if (e.shiftKey) {
            // Shift+G = cycle grid backward
            const ed = useEditorStore.getState();
            const presets = [0.635, 1.27, 2.54, 5.08, 10.16];
            const idx = presets.indexOf(ed.statusBar.gridSize);
            const prev = presets[idx > 0 ? idx - 1 : presets.length - 1];
            ed.setGridSize(prev);
          } else {
            // G = cycle grid forward (Altium)
            const ed = useEditorStore.getState();
            const presets = [0.635, 1.27, 2.54, 5.08, 10.16];
            const idx = presets.indexOf(ed.statusBar.gridSize);
            const next = presets[(idx + 1) % presets.length];
            ed.setGridSize(next);
          }
          break;
        case "z":
          if (e.ctrlKey) { e.preventDefault(); store.undo(); }
          break;
        case "Z":
          if (e.ctrlKey && e.shiftKey) { e.preventDefault(); store.redo(); }
          break;
      }
      // Selection memory: Ctrl+1-8 store, Alt+1-8 recall
      const num = parseInt(e.key, 10);
      if (num >= 1 && num <= 8) {
        if (e.ctrlKey && !e.altKey) { e.preventDefault(); store.storeSelection(num); }
        else if (e.altKey && !e.ctrlKey) { e.preventDefault(); store.recallSelection(num); }
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
        onMouseLeave={() => { /* Don't cancel selection on leave — auto-pan handles edge panning */ }}
        onDoubleClick={handleDblClick}
        onContextMenu={(e) => e.preventDefault()}
        onDragOver={(e) => { e.preventDefault(); e.dataTransfer.dropEffect = "copy"; }}
        onDrop={async (e) => {
          e.preventDefault();
          const json = e.dataTransfer.getData("application/signex-symbol");
          if (!json) return;
          try {
            const result = JSON.parse(json) as import("@/types").SymbolSearchResult;
            const lib = (await import("@tauri-apps/api/core")).invoke;
            // Find library path from the result
            const libs = await lib<import("@/types").LibraryInfo[]>("list_libraries");
            const libInfo = libs.find((l) => l.name === result.library);
            if (!libInfo) return;
            const sym = await lib<import("@/types").LibSymbol>("get_symbol", { libraryPath: libInfo.path, symbolId: result.symbol_id });
            const r = canvasRef.current?.getBoundingClientRect();
            if (!r) return;
            const world = s2w(e.clientX - r.left, e.clientY - r.top);
            const snapped = snapPoint(world);
            useSchematicStore.getState().startPlacement(sym, result);
            useSchematicStore.getState().placeSymbolAt(snapped);
          } catch (err) { console.error("Drop failed:", err); }
        }}
      />

      {/* In-place text editor overlay */}
      {inPlaceEdit && (
        <input
          autoFocus
          value={txt(inPlaceEdit.value)}
          onChange={(e) => {
            // Convert / back to {slash} for KiCad compat, keep rest as-is
            const raw = e.target.value.replace(/\//g, "{slash}");
            setInPlaceEdit({ ...inPlaceEdit, value: raw });
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              committedRef.current = true;
              const store = useSchematicStore.getState();
              const d = store.data;
              if (d) {
                if (d.symbols.find(s => s.uuid === inPlaceEdit.uuid)) store.updateSymbolProp(inPlaceEdit.uuid, inPlaceEdit.field, inPlaceEdit.value);
                if (d.labels.find(l => l.uuid === inPlaceEdit.uuid)) store.updateLabelProp(inPlaceEdit.uuid, inPlaceEdit.field, inPlaceEdit.value);
                if (d.text_notes.find(t => t.uuid === inPlaceEdit.uuid)) store.updateTextNoteProp(inPlaceEdit.uuid, inPlaceEdit.field, inPlaceEdit.value);
              }
              setInPlaceEdit(null);
            }
            if (e.key === "Escape") setInPlaceEdit(null);
            e.stopPropagation();
          }}
          onBlur={() => {
            if (committedRef.current) { committedRef.current = false; return; }
            const store = useSchematicStore.getState();
            const d = store.data;
            if (d) {
              if (d.symbols.find(s => s.uuid === inPlaceEdit.uuid)) store.updateSymbolProp(inPlaceEdit.uuid, inPlaceEdit.field, inPlaceEdit.value);
              if (d.labels.find(l => l.uuid === inPlaceEdit.uuid)) store.updateLabelProp(inPlaceEdit.uuid, inPlaceEdit.field, inPlaceEdit.value);
              if (d.text_notes.find(t => t.uuid === inPlaceEdit.uuid)) store.updateTextNoteProp(inPlaceEdit.uuid, inPlaceEdit.field, inPlaceEdit.value);
            }
            setInPlaceEdit(null);
          }}
          className="absolute z-40 bg-bg-primary/90 border border-accent rounded px-1 py-0 outline-none caret-accent shadow-lg"
          style={{
            left: inPlaceEdit.screenX,
            top: inPlaceEdit.screenY,
            fontSize: `${Math.max(10, camRef.current.zoom * 1.27)}px`,
            fontFamily: "Roboto, sans-serif",
            minWidth: Math.max(60, inPlaceEdit.value.length * Math.max(7, camRef.current.zoom * 0.8)),
            color: "#e8c66a",
            lineHeight: 1,
            transform: "translateY(-50%)",
          }}
        />
      )}

      {/* Context menu */}
      {ctxMenu && <ContextMenu x={ctxMenu.x} y={ctxMenu.y} items={ctxMenu.items} onClose={() => setCtxMenu(null)} />}

      {/* Find/Replace */}
      <FindReplace open={findOpen} onClose={() => setFindOpen(false)} showReplace={findShowReplace} />

      {/* ═══ Altium-style Active Bar ═══ */}
      {activeBarMenu && <div className="absolute inset-0 z-30" onClick={() => setActiveBarMenu(null)} />}
      <div className="absolute top-2 left-1/2 -translate-x-1/2 z-30">
        <div className="flex items-center bg-[#2a2d3d] border border-[#3d4054] rounded shadow-xl shadow-black/50 px-0.5 py-0.5 gap-px">

          {/* Filter button (funnel) */}
          <ActiveBarBtn icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M3 4a1 1 0 011-1h16a1 1 0 01.8 1.6L14 14v5a1 1 0 01-.55.9l-4 2A1 1 0 018 21v-7L1.2 4.6A1 1 0 012 3h1z"/></svg>}
            label="Selection Filter" highlighted
            onClick={() => setActiveBarMenu(activeBarMenu === "filter" ? null : "filter")}
            menuOpen={activeBarMenu === "filter"}
            menu={
              <div className="p-2 min-w-[280px]">
                <button onClick={() => {
                    const keys = ["components","wires","buses","sheetSymbols","sheetEntries","labels","parameters","powerPorts","junctions","textNotes","drawings","noConnects"];
                    const allOn = keys.every(k => selectionFilter[k]?.selectable !== false);
                    if (allOn) keys.forEach(k => setFilterItem(k, "selectable", false));
                    else useEditorStore.getState().resetFilter();
                  }}
                    className="px-3 py-1 mb-2 rounded border border-[#3d4054] text-[10px] text-text-secondary hover:bg-accent/15 hover:text-accent transition-colors">
                    {(() => { const keys = ["components","wires","buses","sheetSymbols","sheetEntries","labels","parameters","powerPorts","junctions","textNotes","drawings","noConnects"];
                      return keys.every(k => selectionFilter[k]?.selectable !== false) ? "All - Off" : "All - On"; })()}
                </button>
                <div className="flex flex-wrap gap-1">
                  {[
                    { label: "Components", key: "components" },
                    { label: "Wires", key: "wires" },
                    { label: "Buses", key: "buses" },
                    { label: "Sheet Symbols", key: "sheetSymbols" },
                    { label: "Sheet Entries", key: "sheetEntries" },
                    { label: "Net Labels", key: "labels" },
                    { label: "Parameters", key: "parameters" },
                    { label: "Ports", key: "junctions" },
                    { label: "Power Ports", key: "powerPorts" },
                    { label: "Texts", key: "textNotes" },
                    { label: "Drawing Objects", key: "drawings" },
                    { label: "Other", key: "noConnects" },
                  ].map((item, i) => {
                    const on = selectionFilter[item.key]?.selectable !== false;
                    return (
                      <button key={`${item.key}-${i}`}
                        onClick={() => setFilterItem(item.key, "selectable", !on)}
                        className={`px-2.5 py-1 rounded border text-[10px] font-medium transition-colors ${
                          on ? "bg-accent/15 text-accent border-accent/40"
                             : "bg-transparent text-text-muted/40 border-[#3d4054] hover:text-text-muted/60"
                        }`}>{item.label}</button>
                    );
                  })}
                </div>
              </div>
            } />
          <div className="w-px h-5 bg-[#3d4054]" />

          {/* Select */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2"><path d="M3 3l7.07 16.97 2.51-7.39 7.39-2.51L3 3z"/></svg>}
            label="Select" active={editMode === "select"}
            onClick={() => { useSchematicStore.getState().setEditMode("select"); setActiveBarMenu(null); }}
            menuOpen={activeBarMenu === "select"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "select" ? null : "select")}
            menu={
              <div className="py-1 min-w-[180px]">
                <DropdownItem label="Lasso Select" onClick={() => { selectionModeRef.current = "lasso"; useSchematicStore.getState().setEditMode("select"); setActiveBarMenu(null); }} />
                <DropdownItem label="Inside Area" onClick={() => { selectionModeRef.current = "insideArea"; useSchematicStore.getState().setEditMode("select"); setActiveBarMenu(null); }} />
                <DropdownItem label="Outside Area" onClick={() => { selectionModeRef.current = "outsideArea"; useSchematicStore.getState().setEditMode("select"); setActiveBarMenu(null); }} />
                <DropdownItem label="Touching Rectangle" onClick={() => { selectionModeRef.current = "touchingRect"; useSchematicStore.getState().setEditMode("select"); setActiveBarMenu(null); }} />
                <DropdownItem label="Touching Line" onClick={() => { selectionModeRef.current = "touchingLine"; useSchematicStore.getState().setEditMode("select"); setActiveBarMenu(null); }} />
                <DropdownItem label="All" onClick={() => { useSchematicStore.getState().selectAll(); setActiveBarMenu(null); }} />
                <DropdownItem label="Connection" onClick={() => {
                  const store = useSchematicStore.getState();
                  const data = store.data;
                  if (!data || store.selectedIds.size === 0) { setActiveBarMenu(null); return; }
                  const firstId = [...store.selectedIds][0];
                  const uuids = connectionSelect(data, firstId, useEditorStore.getState().selectionFilter);
                  if (uuids.length > 0) store.selectMultiple(uuids);
                  setActiveBarMenu(null);
                }} />
                <DropdownItem label="Toggle Selection" onClick={() => { useSchematicStore.getState().invertSelection(); setActiveBarMenu(null); }} />
              </div>
            } />

          {/* Move/Transform */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M5 9l-3 3 3 3"/><path d="M9 5l3-3 3 3"/><path d="M15 19l3 3-3 3" /><path d="M19 9l3 3-3 3"/><path d="M2 12h20"/><path d="M12 2v20"/></svg>}
            label="Move"
            onClick={() => setActiveBarMenu(null)}
            menuOpen={activeBarMenu === "move"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "move" ? null : "move")}
            menu={
              <div className="py-1 min-w-[220px]">
                <DropdownItem label="Drag" onClick={() => { useSchematicStore.getState().setEditMode("select"); setActiveBarMenu(null); }} />
                <DropdownItem label="Move" onClick={() => { useSchematicStore.getState().setEditMode("select"); setActiveBarMenu(null); }} />
                <DropdownItem label="Move Selection by X, Y..." onClick={() => {
                  const input = prompt("Enter offset as X,Y (e.g. 2.54,0):");
                  if (input) {
                    const parts = input.split(",").map(s => parseFloat(s.trim()));
                    if (parts.length === 2 && !isNaN(parts[0]) && !isNaN(parts[1])) {
                      useSchematicStore.getState().moveSelectionByXY(parts[0], parts[1]);
                    }
                  }
                  setActiveBarMenu(null);
                }} />
                <div className="h-px bg-[#3d4054] my-1" />
                <DropdownItem label="Move To Front" onClick={() => setActiveBarMenu(null)} />
                <DropdownItem label="Rotate Selection" onClick={() => { useSchematicStore.getState().rotateSelected(); setActiveBarMenu(null); }} />
                <DropdownItem label="Rotate Selection Clockwise" onClick={() => { useSchematicStore.getState().rotateSelected(); setActiveBarMenu(null); }} />
                <div className="h-px bg-[#3d4054] my-1" />
                <DropdownItem label="Flip Selected Symbols Along X" onClick={() => { useSchematicStore.getState().mirrorSelectedX(); setActiveBarMenu(null); }} />
                <DropdownItem label="Flip Selected Symbols Along Y" onClick={() => { useSchematicStore.getState().mirrorSelectedY(); setActiveBarMenu(null); }} />
              </div>
            } />
          <div className="w-px h-5 bg-[#3d4054]" />

          {/* Wiring — icon tracks last-used tool */}
          <ActiveBarBtn
            icon={lastTool.wire === "drawBus"
              ? <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round"><path d="M4 12h16"/></svg>
              : lastTool.wire === "placeLabel"
              ? <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M4 7h11l5 5-5 5H4V7z"/></svg>
              : <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M4 12h8v-8"/></svg>}
            label={lastTool.wire === "drawBus" ? "Bus" : lastTool.wire === "placeLabel" ? "Net Label" : "Wire"}
            active={editMode === "drawWire" || editMode === "drawBus" || editMode === "placeLabel" || editMode === "placeBusEntry"}
            onClick={() => { useSchematicStore.getState().setEditMode(lastTool.wire as any); setActiveBarMenu(null); }}
            menuOpen={activeBarMenu === "wire"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "wire" ? null : "wire")}
            menu={
              <div className="py-1 min-w-[140px]">
                <DropdownItem label="Wire" icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M4 12h8v-8"/></svg>}
                  onClick={() => { setLastTool(t => ({...t, wire: "drawWire"})); useSchematicStore.getState().setEditMode("drawWire"); setActiveBarMenu(null); }} />
                <DropdownItem label="Bus" icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round"><path d="M4 12h16"/></svg>}
                  onClick={() => { setLastTool(t => ({...t, wire: "drawBus"})); useSchematicStore.getState().setEditMode("drawBus"); setActiveBarMenu(null); }} />
                <DropdownItem label="Bus Entry" onClick={() => { setLastTool(t => ({...t, wire: "placeBusEntry"})); useSchematicStore.getState().setEditMode("placeBusEntry"); setActiveBarMenu(null); }} />
                <DropdownItem label="Net Label" icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M4 7h11l5 5-5 5H4V7z"/></svg>}
                  onClick={() => { setLastTool(t => ({...t, wire: "placeLabel"})); useSchematicStore.getState().setEditMode("placeLabel"); setActiveBarMenu(null); }} />
              </div>
            } />

          {/* Power Port */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M12 4v10"/><path d="M6 4h12"/></svg>}
            label="Power Port" active={editMode === "placePower"}
            onClick={() => { useSchematicStore.getState().setEditMode("placePower"); setActiveBarMenu(null); }}
            menuOpen={activeBarMenu === "power"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "power" ? null : "power")}
            menu={
              <div className="py-1 min-w-[200px]">
                {[
                  { label: "Place GND power port", style: "power_ground", net: "GND" },
                  { label: "Place VCC power port", style: "bar", net: "VCC" },
                  { label: "Place +12 power port", style: "bar", net: "+12V" },
                  { label: "Place +5 power port", style: "bar", net: "+5V" },
                  { label: "Place -5 power port", style: "bar", net: "-5V" },
                  { label: "Place +3.3V power port", style: "bar", net: "+3.3V" },
                ].map(m => <DropdownItem key={m.label} label={m.label}
                  onClick={() => { powerPreset.current = { net: m.net, style: m.style }; useSchematicStore.getState().setEditMode("placePower"); setActiveBarMenu(null); }} />)}
                <div className="h-px bg-[#3d4054] my-1" />
                {[
                  { label: "Place Arrow style power port", style: "arrow", net: "VCC" },
                  { label: "Place Wave style power port", style: "wave", net: "VAC" },
                  { label: "Place Bar style power port", style: "bar", net: "VCC" },
                  { label: "Place Circle style power port", style: "circle", net: "VCC" },
                  { label: "Place Signal Ground power port", style: "signal_ground", net: "GND" },
                  { label: "Place Earth power port", style: "earth_ground", net: "GND" },
                ].map(m => <DropdownItem key={m.label} label={m.label}
                  onClick={() => { powerPreset.current = { net: m.net, style: m.style }; useSchematicStore.getState().setEditMode("placePower"); setActiveBarMenu(null); }} />)}
              </div>
            } />

          {/* Harness */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M4 8h6l4 4h6"/><path d="M4 16h6l4-4"/><path d="M10 8v8"/></svg>}
            label="Harness"
            onClick={() => setActiveBarMenu(null)}
            menuOpen={activeBarMenu === "harness"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "harness" ? null : "harness")}
            menu={
              <div className="py-1 min-w-[180px]">
                <DropdownItem label="Signal Harness" onClick={() => setActiveBarMenu(null)} disabled />
                <DropdownItem label="Harness Connector" onClick={() => setActiveBarMenu(null)} disabled />
                <DropdownItem label="Harness Entry" onClick={() => setActiveBarMenu(null)} disabled />
              </div>
            } />

          {/* No Connect */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round"><path d="M6 6l12 12"/><path d="M18 6L6 18"/></svg>}
            label="No Connect" active={editMode === "placeNoConnect"}
            onClick={() => { useSchematicStore.getState().setEditMode("placeNoConnect"); setActiveBarMenu(null); }} />

          {/* Port */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M4 7h11l5 5-5 5H4V7z"/><line x1="4" y1="12" x2="1" y2="12"/></svg>}
            label="Port" active={editMode === "placePort"}
            onClick={() => { useSchematicStore.getState().setEditMode("placePort"); setActiveBarMenu(null); }}
            menuOpen={activeBarMenu === "port"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "port" ? null : "port")}
            menu={
              <div className="py-1 min-w-[180px]">
                <DropdownItem label="Port" onClick={() => { useSchematicStore.getState().setEditMode("placePort"); setActiveBarMenu(null); }} />
                <DropdownItem label="Off Sheet Connector" disabled onClick={() => setActiveBarMenu(null)} />
              </div>
            } />
          <div className="w-px h-5 bg-[#3d4054]" />

          {/* Component */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="4" y="4" width="16" height="16" rx="2"/><circle cx="12" cy="12" r="3"/></svg>}
            label="Place Component" active={editMode === "placeSymbol"}
            onClick={() => { window.dispatchEvent(new KeyboardEvent("keydown", { key: "p" })); setActiveBarMenu(null); }} />

          {/* Sheet Symbol */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="3" width="18" height="18" rx="1"/><path d="M3 8h18"/><path d="M7 12h4"/><path d="M7 16h4"/></svg>}
            label="Sheet Symbol" active={editMode === "placeSheetSymbol"}
            onClick={() => { useSchematicStore.getState().setEditMode("placeSheetSymbol"); setActiveBarMenu(null); }}
            menuOpen={activeBarMenu === "sheet"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "sheet" ? null : "sheet")}
            menu={
              <div className="py-1 min-w-[200px]">
                <DropdownItem label="Sheet Symbol" onClick={() => { useSchematicStore.getState().setEditMode("placeSheetSymbol"); setActiveBarMenu(null); }} />
                <DropdownItem label="Sheet Entry" onClick={() => setActiveBarMenu(null)} />
                <DropdownItem label="Device Sheet Symbol" onClick={() => setActiveBarMenu(null)} disabled />
                <div className="h-px bg-[#3d4054] my-1" />
                <DropdownItem label="Reuse Block..." onClick={() => setActiveBarMenu(null)} disabled />
              </div>
            } />

          {/* Parameter / Directives */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M4 6h16"/><path d="M4 12h10"/><path d="M4 18h6"/><circle cx="20" cy="12" r="2" fill="currentColor" stroke="none"/></svg>}
            label="Directives" active={editMode === "placeNoErc"}
            onClick={() => { useSchematicStore.getState().setEditMode("placeNoErc"); setActiveBarMenu(null); }}
            menuOpen={activeBarMenu === "directives"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "directives" ? null : "directives")}
            menu={
              <div className="py-1 min-w-[180px]">
                <DropdownItem label="Parameter Set" onClick={() => { useSchematicStore.getState().setEditMode("placeParameterSet"); setActiveBarMenu(null); }} />
                <DropdownItem label="Generic No ERC" onClick={() => { useSchematicStore.getState().setEditMode("placeNoErc"); setActiveBarMenu(null); }} />
                <DropdownItem label="Differential Pair" onClick={() => { useSchematicStore.getState().setEditMode("placeDifferentialPair"); setActiveBarMenu(null); }} />
                <DropdownItem label="Blanket" onClick={() => { useSchematicStore.getState().setEditMode("placeBlanket"); setActiveBarMenu(null); }} />
                <DropdownItem label="Compile Mask" onClick={() => { useSchematicStore.getState().setEditMode("placeCompileMask"); setActiveBarMenu(null); }} />
              </div>
            } />

          {/* Text */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M4 7V4h16v3"/><path d="M12 4v16"/><path d="M8 20h8"/></svg>}
            label="Text" active={editMode === "placeText"}
            onClick={() => { useSchematicStore.getState().setEditMode("placeText"); setActiveBarMenu(null); }}
            menuOpen={activeBarMenu === "text"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "text" ? null : "text")}
            menu={
              <div className="py-1 min-w-[140px]">
                <DropdownItem label="Text String" icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M4 7V4h16v3"/><path d="M12 4v16"/></svg>}
                  onClick={() => { useSchematicStore.getState().setEditMode("placeText"); setActiveBarMenu(null); }} />
                <DropdownItem label="Text Frame" onClick={() => { useSchematicStore.getState().setEditMode("placeTextFrame"); setActiveBarMenu(null); }} />
                <DropdownItem label="Note" onClick={() => { useSchematicStore.getState().setEditMode("placeNote"); setActiveBarMenu(null); }} />
              </div>
            } />
          <div className="w-px h-5 bg-[#3d4054]" />

          {/* Drawing tools — icon tracks last-used tool */}
          <ActiveBarBtn
            icon={lastTool.draw === "drawRect"
              ? <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="3" width="18" height="18"/></svg>
              : lastTool.draw === "drawCircle"
              ? <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="9"/></svg>
              : lastTool.draw === "drawPolyline"
              ? <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M4 20l6-10 4 6 6-12"/></svg>
              : <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M4 20L20 4"/></svg>}
            label={lastTool.draw === "drawRect" ? "Rectangle" : lastTool.draw === "drawCircle" ? "Circle" : lastTool.draw === "drawPolyline" ? "Polyline" : "Line"}
            active={editMode === "drawLine" || editMode === "drawRect" || editMode === "drawCircle" || editMode === "drawPolyline"}
            onClick={() => { useSchematicStore.getState().setEditMode(lastTool.draw as any); setActiveBarMenu(null); }}
            menuOpen={activeBarMenu === "draw"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "draw" ? null : "draw")}
            menu={
              <div className="py-1 min-w-[130px]">
                <DropdownItem label="Line" onClick={() => { setLastTool(t => ({...t, draw: "drawLine"})); useSchematicStore.getState().setEditMode("drawLine"); setActiveBarMenu(null); }} />
                <DropdownItem label="Rectangle" onClick={() => { setLastTool(t => ({...t, draw: "drawRect"})); useSchematicStore.getState().setEditMode("drawRect"); setActiveBarMenu(null); }} />
                <DropdownItem label="Circle" onClick={() => { setLastTool(t => ({...t, draw: "drawCircle"})); useSchematicStore.getState().setEditMode("drawCircle"); setActiveBarMenu(null); }} />
                <DropdownItem label="Polyline" onClick={() => { setLastTool(t => ({...t, draw: "drawPolyline"})); useSchematicStore.getState().setEditMode("drawPolyline"); setActiveBarMenu(null); }} />
              </div>
            } />

          {/* Net Colors */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5"><rect x="2" y="2" width="9" height="9" rx="1" fill="#3b82f6" stroke="#3b82f6"/><rect x="13" y="2" width="9" height="9" rx="1" fill="#ef4444" stroke="#ef4444"/><rect x="2" y="13" width="9" height="9" rx="1" fill="#22c55e" stroke="#22c55e"/><rect x="13" y="13" width="9" height="9" rx="1" fill="#eab308" stroke="#eab308"/></svg>}
            label="Net Colors" active={useEditorStore.getState().netColorOverride}
            onClick={() => { useEditorStore.getState().toggleNetColors(); setActiveBarMenu(null); }}
            menuOpen={activeBarMenu === "netcolors"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "netcolors" ? null : "netcolors")}
            menu={
              <div className="py-1 min-w-[180px]">
                {[
                  { label: "Blue", color: "#3b82f6" },
                  { label: "Light Green", color: "#4ade80" },
                  { label: "Light Blue", color: "#38bdf8" },
                  { label: "Red", color: "#ef4444" },
                  { label: "Fuchsia", color: "#d946ef" },
                  { label: "Yellow", color: "#facc15" },
                  { label: "Dark Green", color: "#16a34a" },
                ].map(c => (
                  <button key={c.label} onClick={() => {
                    const store = useEditorStore.getState();
                    const schStore = useSchematicStore.getState();
                    const sel = [...schStore.selectedIds];
                    if (sel.length > 0 && schStore.data) {
                      const colors = { ...store.netColors };
                      for (const id of sel) {
                        const lbl = schStore.data.labels.find(l => l.uuid === id);
                        if (lbl) colors[lbl.text] = c.color;
                      }
                      store.setNetColors(colors);
                      if (!store.netColorOverride) store.toggleNetColors();
                    }
                    setActiveBarMenu(null);
                  }}
                    className="flex items-center gap-2 w-full px-3 py-1.5 text-[11px] text-text-secondary hover:bg-[#3d4054] hover:text-text-primary transition-colors text-left">
                    <span className="w-3 h-3 rounded-sm border border-white/20 shrink-0" style={{ backgroundColor: c.color }} />
                    {c.label}
                  </button>
                ))}
                <button onClick={() => {
                    /* Custom color — prompt user */
                    const hex = prompt("Enter hex color (e.g. #ff8800):");
                    if (hex && /^#[0-9a-fA-F]{6}$/.test(hex)) {
                      const store = useEditorStore.getState();
                      const schStore = useSchematicStore.getState();
                      const sel = [...schStore.selectedIds];
                      if (sel.length > 0 && schStore.data) {
                        const colors = { ...store.netColors };
                        for (const id of sel) {
                          const lbl = schStore.data.labels.find(l => l.uuid === id);
                          if (lbl) colors[lbl.text] = hex;
                        }
                        store.setNetColors(colors);
                        if (!store.netColorOverride) store.toggleNetColors();
                      }
                    }
                    setActiveBarMenu(null);
                  }}
                  className="flex items-center gap-2 w-full px-3 py-1.5 text-[11px] text-text-secondary hover:bg-[#3d4054] hover:text-text-primary transition-colors text-left">
                  <span className="w-3 h-3 rounded-sm border border-dashed border-white/40 shrink-0 bg-gradient-to-br from-red-500 via-green-500 to-blue-500" />
                  Custom...
                </button>
                <div className="h-px bg-[#3d4054] my-1" />
                <DropdownItem label="Clear Net Color" onClick={() => {
                  const store = useEditorStore.getState();
                  const schStore = useSchematicStore.getState();
                  const sel = [...schStore.selectedIds];
                  if (sel.length > 0 && schStore.data) {
                    const colors = { ...store.netColors };
                    for (const id of sel) {
                      const lbl = schStore.data.labels.find(l => l.uuid === id);
                      if (lbl) delete colors[lbl.text];
                    }
                    store.setNetColors(colors);
                  }
                  setActiveBarMenu(null);
                }} />
                <DropdownItem label="Clear All Net Colors" onClick={() => {
                  useEditorStore.getState().setNetColors({});
                  setActiveBarMenu(null);
                }} />
              </div>
            } />
          <div className="w-px h-5 bg-[#3d4054]" />

          {/* Align */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M4 4v16"/><path d="M8 8h10"/><path d="M8 12h6"/><path d="M8 16h8"/></svg>}
            label="Align"
            onClick={() => { useSchematicStore.getState().alignSelectionToGrid(); setActiveBarMenu(null); }}
            menuOpen={activeBarMenu === "align"}
            onMenuToggle={() => setActiveBarMenu(activeBarMenu === "align" ? null : "align")}
            menu={
              <div className="py-1 min-w-[200px]">
                <DropdownItem label="Align Left" onClick={() => { useSchematicStore.getState().alignSelected("left"); setActiveBarMenu(null); }} />
                <DropdownItem label="Align Right" onClick={() => { useSchematicStore.getState().alignSelected("right"); setActiveBarMenu(null); }} />
                <DropdownItem label="Align Horizontal Centers" onClick={() => { useSchematicStore.getState().alignSelected("left"); setActiveBarMenu(null); }} />
                <DropdownItem label="Distribute Horizontally" onClick={() => { useSchematicStore.getState().distributeSelected("horizontal"); setActiveBarMenu(null); }} />
                <div className="h-px bg-[#3d4054] my-1" />
                <DropdownItem label="Align Top" onClick={() => { useSchematicStore.getState().alignSelected("top"); setActiveBarMenu(null); }} />
                <DropdownItem label="Align Bottom" onClick={() => { useSchematicStore.getState().alignSelected("bottom"); setActiveBarMenu(null); }} />
                <DropdownItem label="Align Vertical Centers" onClick={() => { useSchematicStore.getState().alignSelected("top"); setActiveBarMenu(null); }} />
                <DropdownItem label="Distribute Vertically" onClick={() => { useSchematicStore.getState().distributeSelected("vertical"); setActiveBarMenu(null); }} />
                <div className="h-px bg-[#3d4054] my-1" />
                <DropdownItem label="Align To Grid" onClick={() => { useSchematicStore.getState().alignSelectionToGrid(); setActiveBarMenu(null); }} />
              </div>
            } />

          {/* Rotate */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M21 12a9 9 0 11-6.219-8.56"/><polyline points="21 3 21 9 15 9"/></svg>}
            label="Rotate (Space)"
            onClick={() => { const s = useSchematicStore.getState(); if (s.placingSymbol) s.rotatePlacement(); else s.rotateSelected(); }} />

          {/* Fit view */}
          <ActiveBarBtn
            icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M15 3h6v6"/><path d="M9 21H3v-6"/><path d="M21 3l-7 7"/><path d="M3 21l7-7"/></svg>}
            label="Fit View (Home)"
            onClick={() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Home" }))} />
        </div>
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

// ── Active Bar multi-function button (left click = action, right click = dropdown) ──
function ActiveBarBtn({ icon, label, active, highlighted, onClick, menu, menuOpen, onMenuToggle }: {
  icon: React.ReactNode; label: string; active?: boolean; highlighted?: boolean;
  onClick: () => void; menu?: React.ReactNode; menuOpen?: boolean; onMenuToggle?: () => void;
}) {
  const hasMenu = !!menu;
  return (
    <div className="relative">
      <button
        title={label}
        onClick={onClick}
        onContextMenu={(e) => { e.preventDefault(); if (onMenuToggle) onMenuToggle(); else if (hasMenu) onClick(); }}
        className={`p-1.5 rounded transition-colors flex items-center gap-0 ${
          active ? "bg-accent/25 text-accent"
          : highlighted ? "text-accent hover:bg-accent/15"
          : "text-text-muted/60 hover:bg-[#3d4054] hover:text-text-primary"
        }`}
      >
        {icon}
        {hasMenu && (
          <svg width="6" height="6" viewBox="0 0 6 6" className="ml-px opacity-40"><path d="M1 2l2 2 2-2" fill="none" stroke="currentColor" strokeWidth="1.2"/></svg>
        )}
      </button>
      {menuOpen && menu && (
        <div className="absolute top-full left-0 mt-1 bg-[#2a2d3d] border border-[#3d4054] rounded shadow-xl shadow-black/60 z-50">
          {menu}
        </div>
      )}
    </div>
  );
}

function DropdownItem({ label, icon, onClick, disabled }: { label: string; icon?: React.ReactNode; onClick: () => void; disabled?: boolean }) {
  return (
    <button
      onClick={disabled ? undefined : onClick}
      disabled={disabled}
      className={`flex items-center gap-2 w-full px-3 py-1.5 text-[11px] transition-colors text-left ${disabled ? "text-text-secondary/40 cursor-default" : "text-text-secondary hover:bg-[#3d4054] hover:text-text-primary"}`}
    >
      {icon && <span className="w-4 shrink-0 flex justify-center">{icon}</span>}
      {label}
    </button>
  );
}

/** Calculate the world-space position where the label TEXT starts rendering.
 *  This is used for inline editing to overlay the input exactly on the text. */
function getLabelTextWorldPos(label: { position: SchPoint; label_type: string; shape?: string; font_size?: number; rotation: number; text: string }): SchPoint {
  const fs = label.font_size || 1.27;
  const lx = label.position.x, ly = label.position.y;

  if (label.label_type === "Power") {
    const stemLen = 2.0;
    const style = label.shape || "bar";
    const isGnd = style.includes("ground") || style === "earth_ground";
    // Text is centered, return center position
    if (isGnd) return { x: lx, y: ly + stemLen + 1.2 + fs * 0.5 };
    return { x: lx, y: ly - stemLen - 0.4 - fs * 0.5 };
  }

  if ((label.label_type === "Global" || label.label_type === "Hierarchical") && label.shape) {
    const h = fs * 1.4;
    const pad = fs * 0.3;
    const arrowW = h * 0.5;
    const r = label.rotation;
    const tw = label.text.replace(/\{slash\}/g, "/").length * fs * 0.6; // Approximate text width

    if (r === 0 || r === 180) {
      const connRight = r === 0;
      if (connRight) {
        // Connection on left, text goes right — return left edge of text
        return { x: lx + arrowW + pad, y: ly };
      } else {
        // Connection on right, text goes left — return left edge of text
        return { x: lx - arrowW - pad - tw, y: ly };
      }
    }
    return { x: lx, y: ly };
  }

  // Net labels: text renders at position with small offset
  return { x: lx, y: ly - 0.3 };
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
