import { useRef, useEffect, useCallback, useState } from "react";
import { useLibraryEditorStore } from "@/stores/libraryEditor";
import { useEditorStore } from "@/stores/editor";
import type { LibSymbol, SchPin, Graphic, SchPoint } from "@/types";
import {
  MousePointer2, Move, Pin, Square, Minus, Circle, Spline, Type, Hexagon,
  AlignLeft, X as XIcon, ChevronDown,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { LibEditMode } from "@/stores/libraryEditor";

const GRID_SIZE = 1.27; // mm
const PIN_HIT_RADIUS = 1.0;
const GRAPHIC_HIT_TOLERANCE = 0.5;

const COLORS = {
  bg: "#1a1b2e",
  grid: "#2d3060",
  gridMajor: "#3a3f75",
  origin: "#e8667a",
  body: "#9fa8da",
  bodyFill: "#1e2035",
  pin: "#81c784",
  pinName: "#90a4ae",
  pinNum: "#607d8b",
  selected: "#f9e04b",
  cursor: "rgba(137, 180, 250, 0.5)",
};

function pinEnd(pin: SchPin): SchPoint {
  const rad = (pin.rotation * Math.PI) / 180;
  return {
    x: pin.position.x + Math.cos(rad) * pin.length,
    y: pin.position.y + Math.sin(rad) * pin.length,
  };
}

export function LibraryEditorCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // View state (not in store — local to canvas)
  const viewRef = useRef({ offsetX: 0, offsetY: 0, zoom: 20 }); // zoom = pixels per mm
  const isPanningRef = useRef(false);
  const panStartRef = useRef({ x: 0, y: 0, ox: 0, oy: 0 });
  const cursorWorldRef = useRef<SchPoint>({ x: 0, y: 0 });
  const rafRef = useRef(0);

  const symbol = useLibraryEditorStore((s) => s.symbol);
  const selectedItem = useLibraryEditorStore((s) => s.selectedItem);
  const editMode = useLibraryEditorStore((s) => s.editMode);
  const gridVisible = useEditorStore((s) => s.gridVisible);
  const snapEnabled = useEditorStore((s) => s.statusBar.snapEnabled);

  const snap = useCallback(
    (v: number) => (snapEnabled ? Math.round(v / GRID_SIZE) * GRID_SIZE : v),
    [snapEnabled]
  );

  const screenToWorld = useCallback((sx: number, sy: number): SchPoint => {
    const v = viewRef.current;
    return {
      x: (sx - v.offsetX) / v.zoom,
      y: (sy - v.offsetY) / v.zoom,
    };
  }, []);

  // Render
  const render = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const v = viewRef.current;
    const w = canvas.width;
    const h = canvas.height;

    const dpr = window.devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w / dpr, h / dpr);
    ctx.fillStyle = COLORS.bg;
    ctx.fillRect(0, 0, w / dpr, h / dpr);

    ctx.save();
    ctx.translate(v.offsetX, v.offsetY);
    ctx.scale(v.zoom, v.zoom);

    // Grid
    if (gridVisible) {
      const tl = screenToWorld(0, 0);
      const br = screenToWorld(w / dpr, h / dpr);
      const gs = GRID_SIZE;
      const startX = Math.floor(tl.x / gs) * gs;
      const startY = Math.floor(tl.y / gs) * gs;
      ctx.globalAlpha = 0.4;

      // Pass 1: minor grid lines
      ctx.strokeStyle = COLORS.grid;
      ctx.lineWidth = 0.02;
      ctx.beginPath();
      for (let x = startX; x <= br.x; x += gs) {
        if (Math.round(x / gs) % 10 === 0) continue;
        ctx.moveTo(x, tl.y); ctx.lineTo(x, br.y);
      }
      for (let y = startY; y <= br.y; y += gs) {
        if (Math.round(y / gs) % 10 === 0) continue;
        ctx.moveTo(tl.x, y); ctx.lineTo(br.x, y);
      }
      ctx.stroke();

      // Pass 2: major grid lines
      ctx.strokeStyle = COLORS.gridMajor;
      ctx.lineWidth = 0.06;
      ctx.beginPath();
      for (let x = startX; x <= br.x; x += gs) {
        if (Math.round(x / gs) % 10 !== 0) continue;
        ctx.moveTo(x, tl.y); ctx.lineTo(x, br.y);
      }
      for (let y = startY; y <= br.y; y += gs) {
        if (Math.round(y / gs) % 10 !== 0) continue;
        ctx.moveTo(tl.x, y); ctx.lineTo(br.x, y);
      }
      ctx.stroke();

      ctx.globalAlpha = 1;
    }

    // Origin cross
    ctx.strokeStyle = COLORS.origin;
    ctx.lineWidth = 0.1;
    ctx.beginPath();
    ctx.moveTo(-3, 0); ctx.lineTo(3, 0);
    ctx.moveTo(0, -3); ctx.lineTo(0, 3);
    ctx.stroke();

    if (symbol) {
      renderSymbol(ctx, symbol, selectedItem);
    }

    // Cursor crosshair
    const cw = cursorWorldRef.current;
    ctx.strokeStyle = COLORS.cursor;
    ctx.lineWidth = 0.05;
    ctx.setLineDash([0.3, 0.3]);
    ctx.beginPath();
    ctx.moveTo(cw.x - 2, cw.y); ctx.lineTo(cw.x + 2, cw.y);
    ctx.moveTo(cw.x, cw.y - 2); ctx.lineTo(cw.x, cw.y + 2);
    ctx.stroke();
    ctx.setLineDash([]);

    ctx.restore();
  }, [symbol, selectedItem, gridVisible, screenToWorld]);

  // Resize
  useEffect(() => {
    const container = containerRef.current;
    const canvas = canvasRef.current;
    if (!container || !canvas) return;

    const resize = () => {
      const dpr = window.devicePixelRatio || 1;
      const rect = container.getBoundingClientRect();
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      canvas.style.width = rect.width + "px";
      canvas.style.height = rect.height + "px";
      // DPR scaling is applied in render() via setTransform — not here to avoid accumulation

      // Center origin
      viewRef.current.offsetX = rect.width / 2;
      viewRef.current.offsetY = rect.height / 2;
      render();
    };

    const ro = new ResizeObserver(resize);
    ro.observe(container);
    resize();
    return () => ro.disconnect();
  }, [render]);

  // Demand-driven render — re-render once whenever dependencies change
  useEffect(() => {
    cancelAnimationFrame(rafRef.current);
    rafRef.current = requestAnimationFrame(render);
    return () => cancelAnimationFrame(rafRef.current);
  }, [render]);

  // Mouse handlers
  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;

      if (e.button === 1 || (e.button === 0 && e.altKey)) {
        // Pan
        isPanningRef.current = true;
        panStartRef.current = { x: e.clientX, y: e.clientY, ox: viewRef.current.offsetX, oy: viewRef.current.offsetY };
        return;
      }

      if (e.button !== 0) return;
      const world = screenToWorld(sx, sy);
      const sw = { x: snap(world.x), y: snap(world.y) };

      const store = useLibraryEditorStore.getState();

      if (store.editMode === "select") {
        const sym = store.symbol;
        if (!sym) return;
        // Hit test pins
        for (let i = 0; i < sym.pins.length; i++) {
          const pin = sym.pins[i];
          const pe = pinEnd(pin);
          if (dist(world, pin.position) < PIN_HIT_RADIUS || dist(world, pe) < PIN_HIT_RADIUS) {
            store.setSelectedItem({ type: "pin", index: i });
            cancelAnimationFrame(rafRef.current);
            rafRef.current = requestAnimationFrame(render);
            return;
          }
        }
        // Hit test graphics
        for (let i = 0; i < sym.graphics.length; i++) {
          if (hitTestGraphic(world, sym.graphics[i])) {
            store.setSelectedItem({ type: "graphic", index: i });
            cancelAnimationFrame(rafRef.current);
            rafRef.current = requestAnimationFrame(render);
            return;
          }
        }
        store.setSelectedItem(null);
        cancelAnimationFrame(rafRef.current);
        rafRef.current = requestAnimationFrame(render);
      } else if (store.editMode === "addPin") {
        // Auto-increment: find next unused pin number
        const usedNums = new Set(symbol ? symbol.pins.map((p) => parseInt(p.number, 10)).filter((n) => !isNaN(n)) : []);
        let nextNum = 1;
        while (usedNums.has(nextNum)) nextNum++;
        store.addPin({
          pin_type: "passive",
          shape: "line",
          position: sw,
          rotation: 180,
          length: 2.54,
          name: `Pin${nextNum}`,
          number: String(nextNum),
          name_visible: true,
          number_visible: true,
        });
        // Stay in addPin mode for rapid placement (right-click or Escape to exit)
        cancelAnimationFrame(rafRef.current);
        rafRef.current = requestAnimationFrame(render);
      } else if (store.editMode === "addRect") {
        store.addGraphic({
          type: "Rectangle",
          start: { x: sw.x - 2.54, y: sw.y - 2.54 },
          end: { x: sw.x + 2.54, y: sw.y + 2.54 },
          width: 0.254,
          fill_type: "none",
        });
        store.setEditMode("select");
      } else if (store.editMode === "addCircle") {
        store.addGraphic({
          type: "Circle",
          center: sw,
          radius: 2.54,
          width: 0.254,
          fill_type: "none",
        });
        store.setEditMode("select");
      } else if (store.editMode === "addPolyline") {
        store.addGraphic({
          type: "Polyline",
          points: [
            { x: sw.x - 2.54, y: sw.y },
            { x: sw.x, y: sw.y - 2.54 },
            { x: sw.x + 2.54, y: sw.y },
          ],
          width: 0.254,
          fill_type: "none",
        });
        store.setEditMode("select");
      } else if (store.editMode === "addArc") {
        store.addGraphic({
          type: "Arc",
          start: { x: sw.x - 2.54, y: sw.y },
          mid: { x: sw.x, y: sw.y - 2.54 },
          end: { x: sw.x + 2.54, y: sw.y },
          width: 0.254,
          fill_type: "none",
        });
        store.setEditMode("select");
      } else if (store.editMode === "addText") {
        const text = prompt("Enter text:");
        if (text) {
          store.addGraphic({
            type: "Text",
            text,
            position: sw,
            rotation: 0,
            font_size: 1.27,
            bold: false,
            italic: false,
            justify_h: "left",
            justify_v: "center",
          });
        }
        store.setEditMode("select");
      } else if (store.editMode === "addEllipse") {
        // Ellipse as a circle — KiCad symbol format uses circles
        store.addGraphic({
          type: "Circle",
          center: sw,
          radius: 2.54,
          width: 0.254,
          fill_type: "none",
        });
        store.setEditMode("select");
      } else if (store.editMode === "addPolygon") {
        store.addGraphic({
          type: "Polyline",
          points: [
            { x: sw.x, y: sw.y - 2.54 },
            { x: sw.x + 2.2, y: sw.y + 1.27 },
            { x: sw.x - 2.2, y: sw.y + 1.27 },
            { x: sw.x, y: sw.y - 2.54 }, // closed
          ],
          width: 0.254,
          fill_type: "outline",
        });
        store.setEditMode("select");
      }
    },
    [symbol, snap, screenToWorld]
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (isPanningRef.current) {
        const dx = e.clientX - panStartRef.current.x;
        const dy = e.clientY - panStartRef.current.y;
        viewRef.current.offsetX = panStartRef.current.ox + dx;
        viewRef.current.offsetY = panStartRef.current.oy + dy;
        cancelAnimationFrame(rafRef.current);
        rafRef.current = requestAnimationFrame(render);
        return;
      }
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;
      const world = screenToWorld(sx, sy);
      cursorWorldRef.current = { x: snap(world.x), y: snap(world.y) };
      cancelAnimationFrame(rafRef.current);
      rafRef.current = requestAnimationFrame(render);
    },
    [snap, screenToWorld, render]
  );

  const handleMouseUp = useCallback(() => {
    isPanningRef.current = false;
  }, []);

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault();
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;
      const v = viewRef.current;
      const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
      const newZoom = Math.max(2, Math.min(200, v.zoom * factor));
      v.offsetX = sx - (sx - v.offsetX) * (newZoom / v.zoom);
      v.offsetY = sy - (sy - v.offsetY) * (newZoom / v.zoom);
      v.zoom = newZoom;
      cancelAnimationFrame(rafRef.current);
      rafRef.current = requestAnimationFrame(render);
    },
    [render]
  );

  // Keyboard
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const store = useLibraryEditorStore.getState();
      if (!store.active) return;

      if (e.key === "Delete" || e.key === "Backspace") {
        if (store.selectedItem?.type === "pin") store.removePin(store.selectedItem.index);
        else if (store.selectedItem?.type === "graphic") store.removeGraphic(store.selectedItem.index);
      }
      if (e.ctrlKey && e.key === "z") { e.preventDefault(); store.undo(); }
      if (e.ctrlKey && e.key === "y") { e.preventDefault(); store.redo(); }
      if (e.key === "Escape") { store.setEditMode("select"); store.setSelectedItem(null); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  return (
    <div ref={containerRef} className="w-full h-full relative">
      <canvas
        ref={canvasRef}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onWheel={handleWheel}
        className="absolute inset-0"
        style={{ cursor: editMode === "select" ? "default" : "crosshair" }}
      />
      {/* Floating Active Bar (Altium-style) */}
      <LibActiveBar editMode={editMode} />
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════
// ACTIVE BAR — floating toolbar on canvas (Altium-style)
// ═══════════════════════════════════════════════════════════════

function LibActiveBar({ editMode }: { editMode: LibEditMode }) {
  const setEditMode = useLibraryEditorStore(s => s.setEditMode);
  const [openMenu, setOpenMenu] = useState<string | null>(null);

  const close = () => setOpenMenu(null);

  return (
    <>
      {openMenu && <div className="absolute inset-0 z-30" onClick={close} />}
      <div className="absolute top-3 left-1/2 -translate-x-1/2 z-40 flex items-center gap-px bg-bg-secondary/95 border border-border-subtle rounded-lg shadow-lg px-1 py-0.5">
        {/* Filter */}
        <ABBtn icon={<svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"><path d="M3 4a1 1 0 011-1h16a1 1 0 01.8 1.6L14 14v5a1 1 0 01-.55.9l-4 2A1 1 0 018 21v-7L1.2 4.6A1 1 0 012 3h1z"/></svg>}
          title="Filter" />

        {/* Move — with dropdown */}
        <ABBtn icon={<Move size={14} />} title="Move" hasMenu
          menuOpen={openMenu === "move"} onMenuToggle={() => setOpenMenu(openMenu === "move" ? null : "move")}
          menu={<>
            <MenuItem label="Move" onClick={close} />
            <MenuItem label="Rotate Selection" onClick={() => { close(); }} />
            <MenuItem label="Bring To Front" onClick={close} />
            <MenuItem label="Send To Back" onClick={close} />
          </>} />

        {/* Selection — with dropdown */}
        <ABBtn icon={<MousePointer2 size={14} />} title="Select"
          active={editMode === "select"}
          onClick={() => setEditMode("select")}
          hasMenu menuOpen={openMenu === "sel"} onMenuToggle={() => setOpenMenu(openMenu === "sel" ? null : "sel")}
          menu={<>
            <MenuItem label="Lasso Select" onClick={close} />
            <MenuItem label="Inside Area" onClick={close} />
            <MenuItem label="Touching Rectangle" onClick={close} />
            <MenuItem label="All" onClick={close} />
            <MenuItem label="Toggle Selection" onClick={close} />
          </>} />

        {/* Place Component */}
        <ABBtn icon={<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>}
          title="Place Component" />

        {/* Place Pin */}
        <ABBtn icon={<Pin size={14} />} title="Place Pin"
          active={editMode === "addPin"}
          onClick={() => setEditMode("addPin")} />

        {/* Align — with dropdown */}
        <ABBtn icon={<AlignLeft size={14} />} title="Align" hasMenu
          menuOpen={openMenu === "align"} onMenuToggle={() => setOpenMenu(openMenu === "align" ? null : "align")}
          menu={<>
            <MenuItem label="Align Left" onClick={close} />
            <MenuItem label="Align Right" onClick={close} />
            <MenuItem label="Align Horizontal Centers" onClick={close} />
            <MenuItem label="Distribute Horizontally" onClick={close} />
            <MenuSep />
            <MenuItem label="Align Top" onClick={close} />
            <MenuItem label="Align Bottom" onClick={close} />
            <MenuItem label="Align Vertical Centers" onClick={close} />
            <MenuItem label="Distribute Vertically" onClick={close} />
            <MenuSep />
            <MenuItem label="Align To Grid" onClick={close} />
          </>} />

        {/* No Connect */}
        <ABBtn icon={<XIcon size={14} />} title="No Connect" />

        {/* Draw — with dropdown */}
        <ABBtn icon={<Minus size={14} />} title="Draw" hasMenu
          menuOpen={openMenu === "draw"} onMenuToggle={() => setOpenMenu(openMenu === "draw" ? null : "draw")}
          menu={<>
            <MenuItem label="Line" icon={<Minus size={12} />} onClick={() => { setEditMode("addPolyline"); close(); }} />
            <MenuItem label="Arc" icon={<Spline size={12} />} onClick={() => { setEditMode("addArc"); close(); }} />
            <MenuItem label="Full Circle" icon={<Circle size={12} />} onClick={() => { setEditMode("addCircle"); close(); }} />
            <MenuItem label="Ellipse" icon={<Circle size={12} />} onClick={() => { setEditMode("addEllipse"); close(); }} />
            <MenuItem label="Rectangle" icon={<Square size={12} />} onClick={() => { setEditMode("addRect"); close(); }} />
            <MenuItem label="Polygon" icon={<Hexagon size={12} />} onClick={() => { setEditMode("addPolygon"); close(); }} />
          </>} />

        {/* Text — with dropdown */}
        <ABBtn icon={<Type size={14} />} title="Text" hasMenu
          menuOpen={openMenu === "text"} onMenuToggle={() => setOpenMenu(openMenu === "text" ? null : "text")}
          menu={<>
            <MenuItem label="Text String" icon={<Type size={12} />} onClick={() => { setEditMode("addText"); close(); }} />
          </>} />
      </div>
    </>
  );
}

function ABBtn({ icon, title, active, onClick, hasMenu, menuOpen, onMenuToggle, menu }: {
  icon: React.ReactNode; title: string; active?: boolean; onClick?: () => void;
  hasMenu?: boolean; menuOpen?: boolean; onMenuToggle?: () => void; menu?: React.ReactNode;
}) {
  return (
    <div className="relative">
      <div className="flex items-center">
        <button title={title} onClick={onClick}
          className={cn("p-1.5 rounded-l transition-colors",
            active ? "bg-accent/20 text-accent" : "text-text-secondary hover:bg-bg-hover hover:text-text-primary",
            !hasMenu && "rounded-r"
          )}>
          {icon}
        </button>
        {hasMenu && (
          <button onClick={onMenuToggle}
            className={cn("px-0.5 py-1.5 rounded-r transition-colors border-l border-border-subtle/30",
              menuOpen ? "bg-accent/20 text-accent" : "text-text-muted/40 hover:bg-bg-hover hover:text-text-primary"
            )}>
            <ChevronDown size={8} />
          </button>
        )}
      </div>
      {menuOpen && menu && (
        <div className="absolute top-full left-0 mt-1 bg-bg-secondary border border-border-subtle rounded shadow-xl py-1 min-w-[160px] z-50">
          {menu}
        </div>
      )}
    </div>
  );
}

function MenuItem({ label, icon, onClick }: { label: string; icon?: React.ReactNode; onClick?: () => void }) {
  return (
    <button onClick={onClick}
      className="flex items-center gap-2 w-full px-3 py-1 text-[11px] text-text-secondary hover:bg-bg-hover hover:text-text-primary text-left">
      {icon && <span className="w-4 shrink-0">{icon}</span>}
      {label}
    </button>
  );
}

function MenuSep() {
  return <div className="my-1 border-t border-border-subtle" />;
}

// ═══════════════════════════════════════════════════════════════
// RENDERING
// ═══════════════════════════════════════════════════════════════

function renderSymbol(
  ctx: CanvasRenderingContext2D,
  sym: LibSymbol,
  sel: ReturnType<typeof useLibraryEditorStore.getState>["selectedItem"]
) {
  // Graphics
  for (let i = 0; i < sym.graphics.length; i++) {
    const g = sym.graphics[i];
    const isSelected = sel?.type === "graphic" && sel.index === i;
    ctx.strokeStyle = isSelected ? COLORS.selected : COLORS.body;
    const gWidth = "width" in g ? g.width : 0;
    ctx.lineWidth = Math.max(gWidth || 0.1, 0.1);

    switch (g.type) {
      case "Polyline": {
        if (g.points.length < 2) break;
        ctx.beginPath();
        ctx.moveTo(g.points[0].x, g.points[0].y);
        for (let j = 1; j < g.points.length; j++) ctx.lineTo(g.points[j].x, g.points[j].y);
        if (g.fill_type === "background") {
          ctx.fillStyle = COLORS.bodyFill;
          ctx.fill();
        } else if (g.fill_type === "outline") {
          ctx.fillStyle = COLORS.body;
          ctx.fill();
        }
        ctx.stroke();
        break;
      }
      case "Rectangle": {
        const rx = Math.min(g.start.x, g.end.x), ry = Math.min(g.start.y, g.end.y);
        const rw = Math.abs(g.end.x - g.start.x), rh = Math.abs(g.end.y - g.start.y);
        if (g.fill_type === "background") {
          ctx.fillStyle = COLORS.bodyFill;
          ctx.fillRect(rx, ry, rw, rh);
        } else if (g.fill_type === "outline") {
          ctx.fillStyle = COLORS.body;
          ctx.fillRect(rx, ry, rw, rh);
        }
        ctx.strokeRect(rx, ry, rw, rh);
        break;
      }
      case "Circle": {
        ctx.beginPath();
        ctx.arc(g.center.x, g.center.y, g.radius, 0, Math.PI * 2);
        if (g.fill_type === "background") {
          ctx.fillStyle = COLORS.bodyFill;
          ctx.fill();
        } else if (g.fill_type === "outline") {
          ctx.fillStyle = COLORS.body;
          ctx.fill();
        }
        ctx.stroke();
        break;
      }
      case "Arc": {
        ctx.beginPath();
        ctx.moveTo(g.start.x, g.start.y);
        ctx.quadraticCurveTo(g.mid.x, g.mid.y, g.end.x, g.end.y);
        ctx.stroke();
        break;
      }
      case "Text": {
        ctx.fillStyle = isSelected ? COLORS.selected : COLORS.body;
        const fs = g.font_size || 1.27;
        ctx.font = `${g.bold ? "bold " : ""}${g.italic ? "italic " : ""}${fs}px sans-serif`;
        ctx.textBaseline = "top";
        ctx.textAlign = g.justify_h === "right" ? "right" : g.justify_h === "center" ? "center" : "left";
        ctx.save();
        ctx.translate(g.position.x, g.position.y);
        if (g.rotation) ctx.rotate((g.rotation * Math.PI) / 180);
        ctx.fillText(g.text, 0, 0);
        ctx.restore();
        break;
      }
      case "TextBox": {
        const rx = g.position.x, ry = g.position.y;
        const rw = Math.abs(g.size.x), rh = Math.abs(g.size.y);
        ctx.strokeStyle = isSelected ? COLORS.selected : COLORS.body;
        ctx.lineWidth = 0.1;
        ctx.strokeRect(rx, ry, rw, rh);
        ctx.fillStyle = isSelected ? COLORS.selected : COLORS.body;
        ctx.font = `${g.font_size || 1.0}px sans-serif`;
        ctx.textBaseline = "top";
        ctx.textAlign = "left";
        ctx.fillText(g.text, rx + 0.3, ry + 0.3);
        break;
      }
    }
  }

  // Pins
  for (let i = 0; i < sym.pins.length; i++) {
    const pin = sym.pins[i];
    const pe = pinEnd(pin);
    const isSelected = sel?.type === "pin" && sel.index === i;

    // Pin line
    ctx.strokeStyle = isSelected ? COLORS.selected : COLORS.pin;
    ctx.lineWidth = 0.1;
    ctx.beginPath();
    ctx.moveTo(pin.position.x, pin.position.y);
    ctx.lineTo(pe.x, pe.y);
    ctx.stroke();

    // Pin endpoint dot (connection point)
    ctx.fillStyle = isSelected ? COLORS.selected : COLORS.pin;
    ctx.beginPath();
    ctx.arc(pe.x, pe.y, 0.2, 0, Math.PI * 2);
    ctx.fill();

    // Pin number
    if (sym.show_pin_numbers && pin.number_visible && pin.number !== "~") {
      ctx.fillStyle = isSelected ? COLORS.selected : COLORS.pinNum;
      ctx.font = "1.0px sans-serif";
      ctx.textAlign = "center";
      ctx.textBaseline = "bottom";
      const mx = (pin.position.x + pe.x) / 2;
      const my = (pin.position.y + pe.y) / 2;
      ctx.fillText(pin.number, mx, my - 0.3);
    }

    // Pin name
    if (sym.show_pin_names && pin.name_visible && pin.name !== "~") {
      ctx.fillStyle = isSelected ? COLORS.selected : COLORS.pinName;
      ctx.font = "0.75px sans-serif";
      const dx = pe.x - pin.position.x;
      const dy = pe.y - pin.position.y;
      const len = Math.hypot(dx, dy) || 1;
      const nx = pe.x + (dx / len) * 0.4;
      const ny = pe.y + (dy / len) * 0.4;
      if (Math.abs(dx) > Math.abs(dy)) {
        ctx.textBaseline = "middle";
        ctx.textAlign = dx > 0 ? "left" : "right";
      } else {
        ctx.textAlign = "center";
        ctx.textBaseline = dy > 0 ? "top" : "bottom";
      }
      ctx.fillText(pin.name, nx, ny);
    }
  }
}

// ═══════════════════════════════════════════════════════════════
// HIT TESTING
// ═══════════════════════════════════════════════════════════════

function dist(a: SchPoint, b: SchPoint): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function pointToSegmentDist(p: SchPoint, a: SchPoint, b: SchPoint): number {
  const dx = b.x - a.x, dy = b.y - a.y;
  const lenSq = dx * dx + dy * dy;
  if (lenSq === 0) return dist(p, a);
  let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));
  return dist(p, { x: a.x + t * dx, y: a.y + t * dy });
}

function hitTestGraphic(point: SchPoint, g: Graphic): boolean {
  const tol = GRAPHIC_HIT_TOLERANCE;
  switch (g.type) {
    case "Rectangle": {
      const rx = Math.min(g.start.x, g.end.x) - tol;
      const ry = Math.min(g.start.y, g.end.y) - tol;
      const rw = Math.abs(g.end.x - g.start.x) + tol * 2;
      const rh = Math.abs(g.end.y - g.start.y) + tol * 2;
      if (point.x < rx || point.x > rx + rw || point.y < ry || point.y > ry + rh) return false;
      // Check if near edges
      const ix = Math.min(g.start.x, g.end.x);
      const iy = Math.min(g.start.y, g.end.y);
      const ex = Math.max(g.start.x, g.end.x);
      const ey = Math.max(g.start.y, g.end.y);
      return (
        Math.abs(point.x - ix) < tol || Math.abs(point.x - ex) < tol ||
        Math.abs(point.y - iy) < tol || Math.abs(point.y - ey) < tol ||
        g.fill_type !== "none"
      );
    }
    case "Circle": {
      const d = dist(point, g.center);
      return g.fill_type !== "none" ? d <= g.radius + tol : Math.abs(d - g.radius) < tol;
    }
    case "Polyline": {
      for (let i = 0; i < g.points.length - 1; i++) {
        if (pointToSegmentDist(point, g.points[i], g.points[i + 1]) < tol) return true;
      }
      return false;
    }
    case "Arc": {
      return (
        pointToSegmentDist(point, g.start, g.mid) < tol ||
        pointToSegmentDist(point, g.mid, g.end) < tol
      );
    }
    case "Text": {
      return dist(point, g.position) < tol * 4;
    }
    case "TextBox": {
      return (
        point.x >= g.position.x - tol &&
        point.x <= g.position.x + Math.abs(g.size.x) + tol &&
        point.y >= g.position.y - tol &&
        point.y <= g.position.y + Math.abs(g.size.y) + tol
      );
    }
  }
}
