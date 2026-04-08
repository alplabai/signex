import { useRef, useEffect, useCallback, useState } from "react";
import { useFootprintEditorStore } from "@/stores/footprintEditor";
import type { FootprintData, FpEditMode } from "@/stores/footprintEditor";
import type { PcbGraphic } from "@/types/pcb";
import { DEFAULT_LAYER_COLORS } from "@/types/pcb";
import {
  MousePointer2, Move, Square, Minus, Circle, Spline, Type,
  AlignLeft, ChevronDown, ChevronRight, CircleDot,
} from "lucide-react";
import { cn } from "@/lib/utils";

const GRID_MM = 0.1; // 0.1mm grid for PCB
const PAD_HIT_RADIUS = 0.3;

const COLORS = {
  bg: "#1a1b26",
  grid: "#252535",
  gridMajor: "#303050",
  origin: "#e8667a",
  selected: "#f9e04b",
  cursor: "rgba(137, 180, 250, 0.4)",
  padSmd: "#cc3333",
  padTh: "#cc9933",
  drill: "#1a1b26",
};

function layerColor(layer: string): string {
  return DEFAULT_LAYER_COLORS[layer] || "#666666";
}

export function FootprintEditorCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef({ ox: 0, oy: 0, zoom: 80 }); // pixels per mm
  const isPanningRef = useRef(false);
  const panStartRef = useRef({ x: 0, y: 0, ox: 0, oy: 0 });
  const cursorRef = useRef({ x: 0, y: 0 });
  const rafRef = useRef(0);

  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number } | null>(null);

  const footprint = useFootprintEditorStore(s => s.footprint);
  const selectedItem = useFootprintEditorStore(s => s.selectedItem);
  const editMode = useFootprintEditorStore(s => s.editMode);
  useFootprintEditorStore(s => s.activeLayer); // subscribe for re-render

  const snap = useCallback((v: number) => Math.round(v / GRID_MM) * GRID_MM, []);

  const screenToWorld = useCallback((sx: number, sy: number) => {
    const v = viewRef.current;
    return { x: (sx - v.ox) / v.zoom, y: (sy - v.oy) / v.zoom };
  }, []);

  const render = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const dpr = window.devicePixelRatio || 1;
    const v = viewRef.current;
    const w = canvas.width / dpr, h = canvas.height / dpr;

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    ctx.fillStyle = COLORS.bg;
    ctx.fillRect(0, 0, w, h);

    ctx.save();
    ctx.translate(v.ox, v.oy);
    ctx.scale(v.zoom, v.zoom);

    // Grid
    const tl = screenToWorld(0, 0);
    const br = screenToWorld(w, h);
    const gs = GRID_MM;
    const majorEvery = 10;
    ctx.globalAlpha = 0.3;

    // Minor dots
    ctx.fillStyle = COLORS.grid;
    const startX = Math.floor(tl.x / gs) * gs;
    const startY = Math.floor(tl.y / gs) * gs;
    if (v.zoom > 30) {
      for (let x = startX; x <= br.x; x += gs) {
        for (let y = startY; y <= br.y; y += gs) {
          ctx.fillRect(x - 0.01, y - 0.01, 0.02, 0.02);
        }
      }
    }

    // Major grid lines
    const majorGs = gs * majorEvery;
    ctx.strokeStyle = COLORS.gridMajor;
    ctx.lineWidth = 0.01;
    ctx.beginPath();
    const smx = Math.floor(tl.x / majorGs) * majorGs;
    const smy = Math.floor(tl.y / majorGs) * majorGs;
    for (let x = smx; x <= br.x; x += majorGs) { ctx.moveTo(x, tl.y); ctx.lineTo(x, br.y); }
    for (let y = smy; y <= br.y; y += majorGs) { ctx.moveTo(tl.x, y); ctx.lineTo(br.x, y); }
    ctx.stroke();
    ctx.globalAlpha = 1;

    // Origin cross
    ctx.strokeStyle = COLORS.origin;
    ctx.lineWidth = 0.02;
    ctx.beginPath();
    ctx.moveTo(-1, 0); ctx.lineTo(1, 0);
    ctx.moveTo(0, -1); ctx.lineTo(0, 1);
    ctx.stroke();

    if (footprint) {
      renderFootprint(ctx, footprint, selectedItem, v.zoom);
    }

    // Cursor crosshair
    const cw = cursorRef.current;
    ctx.strokeStyle = COLORS.cursor;
    ctx.lineWidth = 0.02;
    ctx.setLineDash([0.1, 0.1]);
    ctx.beginPath();
    ctx.moveTo(cw.x - 1, cw.y); ctx.lineTo(cw.x + 1, cw.y);
    ctx.moveTo(cw.x, cw.y - 1); ctx.lineTo(cw.x, cw.y + 1);
    ctx.stroke();
    ctx.setLineDash([]);

    ctx.restore();
  }, [footprint, selectedItem, screenToWorld]);

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
      viewRef.current.ox = rect.width / 2;
      viewRef.current.oy = rect.height / 2;
      render();
    };
    const ro = new ResizeObserver(resize);
    ro.observe(container);
    resize();
    return () => ro.disconnect();
  }, [render]);

  useEffect(() => {
    cancelAnimationFrame(rafRef.current);
    rafRef.current = requestAnimationFrame(render);
    return () => cancelAnimationFrame(rafRef.current);
  }, [render]);

  // Mouse handlers
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const sx = e.clientX - rect.left, sy = e.clientY - rect.top;

    if (e.button === 1 || (e.button === 0 && e.altKey)) {
      isPanningRef.current = true;
      panStartRef.current = { x: e.clientX, y: e.clientY, ox: viewRef.current.ox, oy: viewRef.current.oy };
      return;
    }
    if (e.button !== 0) return;

    const world = screenToWorld(sx, sy);
    const sw = { x: snap(world.x), y: snap(world.y) };
    const store = useFootprintEditorStore.getState();
    const fp = store.footprint;

    if (store.editMode === "select" && fp) {
      // Hit test pads
      for (let i = 0; i < fp.pads.length; i++) {
        const p = fp.pads[i];
        const dx = world.x - p.position.x, dy = world.y - p.position.y;
        if (Math.abs(dx) < p.size[0] / 2 + PAD_HIT_RADIUS && Math.abs(dy) < p.size[1] / 2 + PAD_HIT_RADIUS) {
          store.setSelectedItem({ type: "pad", index: i });
          return;
        }
      }
      // Hit test graphics
      for (let i = 0; i < fp.graphics.length; i++) {
        if (hitTestGraphic(world, fp.graphics[i])) {
          store.setSelectedItem({ type: "graphic", index: i });
          return;
        }
      }
      store.setSelectedItem(null);
    } else if (store.editMode === "addPadSmd") {
      const usedNums = new Set(fp ? fp.pads.map(p => parseInt(p.number)).filter(n => !isNaN(n)) : []);
      let next = 1;
      while (usedNums.has(next)) next++;
      store.addPad({
        uuid: crypto.randomUUID(),
        number: String(next),
        type: "smd",
        shape: "roundrect",
        position: sw,
        size: [1.0, 1.2],
        layers: ["F.Cu", "F.Paste", "F.Mask"],
        roundrectRatio: 0.25,
      });
    } else if (store.editMode === "addPadTh") {
      const usedNums = new Set(fp ? fp.pads.map(p => parseInt(p.number)).filter(n => !isNaN(n)) : []);
      let next = 1;
      while (usedNums.has(next)) next++;
      store.addPad({
        uuid: crypto.randomUUID(),
        number: String(next),
        type: "thru_hole",
        shape: "circle",
        position: sw,
        size: [1.7, 1.7],
        drill: { diameter: 1.0 },
        layers: ["*.Cu", "*.Mask"],
      });
    } else if (store.editMode === "addLine") {
      store.addGraphic({ type: "line", start: sw, end: { x: sw.x + 1, y: sw.y }, layer: store.activeLayer, width: 0.12 });
      store.setEditMode("select");
    } else if (store.editMode === "addRect") {
      store.addGraphic({ type: "rect", start: { x: sw.x - 1, y: sw.y - 1 }, end: { x: sw.x + 1, y: sw.y + 1 }, layer: store.activeLayer, width: 0.12 });
      store.setEditMode("select");
    } else if (store.editMode === "addCircle") {
      store.addGraphic({ type: "circle", center: sw, radius: 0.5, layer: store.activeLayer, width: 0.12 });
      store.setEditMode("select");
    } else if (store.editMode === "addArc") {
      store.addGraphic({ type: "arc", start: { x: sw.x - 1, y: sw.y }, mid: { x: sw.x, y: sw.y - 1 }, end: { x: sw.x + 1, y: sw.y }, layer: store.activeLayer, width: 0.12 });
      store.setEditMode("select");
    } else if (store.editMode === "addText") {
      const text = prompt("Enter text:");
      if (text) {
        store.addGraphic({ type: "text", text, position: sw, layer: store.activeLayer, fontSize: 1.0, rotation: 0 });
      }
      store.setEditMode("select");
    }
  }, [footprint, snap, screenToWorld]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (isPanningRef.current) {
      viewRef.current.ox = panStartRef.current.ox + e.clientX - panStartRef.current.x;
      viewRef.current.oy = panStartRef.current.oy + e.clientY - panStartRef.current.y;
      cancelAnimationFrame(rafRef.current);
      rafRef.current = requestAnimationFrame(render);
      return;
    }
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const world = screenToWorld(e.clientX - rect.left, e.clientY - rect.top);
    cursorRef.current = { x: snap(world.x), y: snap(world.y) };
    cancelAnimationFrame(rafRef.current);
    rafRef.current = requestAnimationFrame(render);
  }, [snap, screenToWorld, render]);

  const handleMouseUp = useCallback(() => { isPanningRef.current = false; }, []);

  // Native wheel listener (non-passive) to prevent page scroll during zoom
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const handler = (e: WheelEvent) => {
      e.preventDefault();
      const rect = canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left, sy = e.clientY - rect.top;
      const v = viewRef.current;
      const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
      const newZoom = Math.max(5, Math.min(1000, v.zoom * factor));
      v.ox = sx - (sx - v.ox) * (newZoom / v.zoom);
      v.oy = sy - (sy - v.oy) * (newZoom / v.zoom);
      v.zoom = newZoom;
      cancelAnimationFrame(rafRef.current);
      rafRef.current = requestAnimationFrame(render);
    };
    canvas.addEventListener("wheel", handler, { passive: false });
    return () => canvas.removeEventListener("wheel", handler);
  }, [render]);

  // Keyboard
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const store = useFootprintEditorStore.getState();
      if (!store.active) return;
      if (e.key === "Delete" || e.key === "Backspace") {
        if (store.selectedItem?.type === "pad") store.removePad(store.selectedItem.index);
        else if (store.selectedItem?.type === "graphic") store.removeGraphic(store.selectedItem.index);
      }
      if (e.ctrlKey && e.key === "z") { e.preventDefault(); store.undo(); }
      if (e.ctrlKey && e.key === "y") { e.preventDefault(); store.redo(); }
      if (e.key === "Escape") { store.setEditMode("select"); store.setSelectedItem(null); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setCtxMenu({ x: e.clientX, y: e.clientY });
  }, []);

  return (
    <div ref={containerRef} className="w-full h-full relative">
      <canvas ref={canvasRef}
        onMouseDown={handleMouseDown} onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp} onMouseLeave={handleMouseUp}
        onContextMenu={handleContextMenu}
        className="absolute inset-0"
        style={{ cursor: editMode === "select" ? "default" : "crosshair" }}
      />
      <FpActiveBar editMode={editMode} />
      {ctxMenu && <FpCanvasContextMenu x={ctxMenu.x} y={ctxMenu.y} onClose={() => setCtxMenu(null)} />}
    </div>
  );
}

// ═══════════════════════════════════════════════════════════════
// CANVAS RIGHT-CLICK CONTEXT MENU (Altium-style)
// ═══════════════════════════════════════════════════════════════

function FpCanvasContextMenu({ x, y, onClose }: { x: number; y: number; onClose: () => void }) {
  const setEditMode = useFootprintEditorStore(s => s.setEditMode);
  const [sub, setSub] = useState<string | null>(null);

  const act = (fn?: () => void) => { fn?.(); onClose(); };

  return (
    <>
      <div className="fixed inset-0 z-[80]" onClick={onClose} />
      <div className="fixed z-[85] bg-bg-secondary border border-border-subtle rounded shadow-xl py-1 min-w-[200px] text-[11px]"
        style={{ left: Math.min(x, window.innerWidth - 220), top: Math.min(y, window.innerHeight - 400) }}>
        <FpMenuItem label="Find Similar Objects..." onClick={() => act()} />
        <FpMenuItem label="Clear Filter" shortcut="Shift+C" onClick={() => act()} />
        <FpMenuSep />

        {/* Place submenu */}
        <div className="relative" onMouseEnter={() => setSub("place")} onMouseLeave={() => setSub(null)}>
          <FpMenuItem label="Place" hasSubmenu />
          {sub === "place" && (
            <div className="absolute left-full top-0 bg-bg-secondary border border-border-subtle rounded shadow-xl py-1 min-w-[180px]">
              <FpMenuItem label="SMD Pad" icon={<Square size={12} />} onClick={() => act(() => setEditMode("addPadSmd"))} />
              <FpMenuItem label="Through-Hole Pad" icon={<CircleDot size={12} />} onClick={() => act(() => setEditMode("addPadTh"))} />
              <FpMenuSep />
              <FpMenuItem label="Line" icon={<Minus size={12} />} onClick={() => act(() => setEditMode("addLine"))} />
              <FpMenuItem label="Arc" icon={<Spline size={12} />} onClick={() => act(() => setEditMode("addArc"))} />
              <FpMenuItem label="Circle" icon={<Circle size={12} />} onClick={() => act(() => setEditMode("addCircle"))} />
              <FpMenuItem label="Rectangle" icon={<Square size={12} />} onClick={() => act(() => setEditMode("addRect"))} />
              <FpMenuSep />
              <FpMenuItem label="Text" icon={<Type size={12} />} onClick={() => act(() => setEditMode("addText"))} />
            </div>
          )}
        </div>

        {/* Tools submenu */}
        <div className="relative" onMouseEnter={() => setSub("tools")} onMouseLeave={() => setSub(null)}>
          <FpMenuItem label="Tools" hasSubmenu />
          {sub === "tools" && (
            <div className="absolute left-full top-0 bg-bg-secondary border border-border-subtle rounded shadow-xl py-1 min-w-[180px]">
              <FpMenuItem label="New Footprint" onClick={() => act()} />
              <FpMenuItem label="Remove Footprint" onClick={() => act()} />
              <FpMenuSep />
              <FpMenuItem label="Renumber Pads" onClick={() => act()} />
            </div>
          )}
        </div>

        {/* View submenu */}
        <div className="relative" onMouseEnter={() => setSub("view")} onMouseLeave={() => setSub(null)}>
          <FpMenuItem label="View" hasSubmenu />
          {sub === "view" && (
            <div className="absolute left-full top-0 bg-bg-secondary border border-border-subtle rounded shadow-xl py-1 min-w-[180px]">
              <FpMenuItem label="Fit All Objects" shortcut="Ctrl+PgDn" onClick={() => act()} />
              <FpMenuItem label="Fit Document" onClick={() => act()} />
              <FpMenuSep />
              <FpMenuItem label="Zoom In" shortcut="PgUp" onClick={() => act()} />
              <FpMenuItem label="Zoom Out" shortcut="PgDn" onClick={() => act()} />
            </div>
          )}
        </div>

        <FpMenuSep />
        <FpMenuItem label="Cut" shortcut="Ctrl+X" onClick={() => act()} />
        <FpMenuItem label="Copy" shortcut="Ctrl+C" onClick={() => act()} />
        <FpMenuItem label="Paste" shortcut="Ctrl+V" onClick={() => act()} />
        <FpMenuSep />
        <FpMenuItem label="Preferences..." onClick={() => act()} />
      </div>
    </>
  );
}

// ═══════════════════════════════════════════════════════════════
// ACTIVE BAR — floating toolbar on canvas (Altium-style)
// ═══════════════════════════════════════════════════════════════

function FpActiveBar({ editMode }: { editMode: FpEditMode }) {
  const setEditMode = useFootprintEditorStore(s => s.setEditMode);
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
            <FpMenuItem label="Move" onClick={close} />
            <FpMenuItem label="Rotate Selection" onClick={close} />
            <FpMenuItem label="Bring To Front" onClick={close} />
            <FpMenuItem label="Send To Back" onClick={close} />
          </>} />

        {/* Selection — with dropdown */}
        <ABBtn icon={<MousePointer2 size={14} />} title="Select"
          active={editMode === "select"}
          onClick={() => setEditMode("select")}
          hasMenu menuOpen={openMenu === "sel"} onMenuToggle={() => setOpenMenu(openMenu === "sel" ? null : "sel")}
          menu={<>
            <FpMenuItem label="Lasso Select" onClick={close} />
            <FpMenuItem label="Inside Area" onClick={close} />
            <FpMenuItem label="Touching Rectangle" onClick={close} />
            <FpMenuItem label="All" onClick={close} />
            <FpMenuItem label="Toggle Selection" onClick={close} />
          </>} />

        {/* Pad placement — with dropdown */}
        <ABBtn icon={<Square size={14} />} title="Place Pad"
          active={editMode === "addPadSmd" || editMode === "addPadTh"}
          onClick={() => setEditMode("addPadSmd")}
          hasMenu menuOpen={openMenu === "pad"} onMenuToggle={() => setOpenMenu(openMenu === "pad" ? null : "pad")}
          menu={<>
            <FpMenuItem label="SMD Pad" icon={<Square size={12} />} onClick={() => { setEditMode("addPadSmd"); close(); }} />
            <FpMenuItem label="Through-Hole Pad" icon={<CircleDot size={12} />} onClick={() => { setEditMode("addPadTh"); close(); }} />
          </>} />

        {/* Align — with dropdown */}
        <ABBtn icon={<AlignLeft size={14} />} title="Align" hasMenu
          menuOpen={openMenu === "align"} onMenuToggle={() => setOpenMenu(openMenu === "align" ? null : "align")}
          menu={<>
            <FpMenuItem label="Align Left" onClick={close} />
            <FpMenuItem label="Align Right" onClick={close} />
            <FpMenuItem label="Align Horizontal Centers" onClick={close} />
            <FpMenuItem label="Distribute Horizontally" onClick={close} />
            <FpMenuSep />
            <FpMenuItem label="Align Top" onClick={close} />
            <FpMenuItem label="Align Bottom" onClick={close} />
            <FpMenuItem label="Align Vertical Centers" onClick={close} />
            <FpMenuItem label="Distribute Vertically" onClick={close} />
            <FpMenuSep />
            <FpMenuItem label="Align To Grid" onClick={close} />
          </>} />

        {/* Draw — with dropdown */}
        <ABBtn icon={<Minus size={14} />} title="Draw"
          active={editMode === "addLine" || editMode === "addArc" || editMode === "addCircle" || editMode === "addRect"}
          onClick={() => setEditMode("addLine")}
          hasMenu menuOpen={openMenu === "draw"} onMenuToggle={() => setOpenMenu(openMenu === "draw" ? null : "draw")}
          menu={<>
            <FpMenuItem label="Line" icon={<Minus size={12} />} onClick={() => { setEditMode("addLine"); close(); }} />
            <FpMenuItem label="Arc" icon={<Spline size={12} />} onClick={() => { setEditMode("addArc"); close(); }} />
            <FpMenuItem label="Circle" icon={<Circle size={12} />} onClick={() => { setEditMode("addCircle"); close(); }} />
            <FpMenuItem label="Rectangle" icon={<Square size={12} />} onClick={() => { setEditMode("addRect"); close(); }} />
          </>} />

        {/* Text — with dropdown */}
        <ABBtn icon={<Type size={14} />} title="Text"
          active={editMode === "addText"}
          onClick={() => setEditMode("addText")}
          hasMenu menuOpen={openMenu === "text"} onMenuToggle={() => setOpenMenu(openMenu === "text" ? null : "text")}
          menu={<>
            <FpMenuItem label="Text String" icon={<Type size={12} />} onClick={() => { setEditMode("addText"); close(); }} />
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

function FpMenuItem({ label, icon, onClick, shortcut, hasSubmenu }: { label: string; icon?: React.ReactNode; onClick?: () => void; shortcut?: string; hasSubmenu?: boolean }) {
  return (
    <button onClick={onClick}
      className="flex items-center gap-2 w-full px-3 py-1 text-[11px] text-text-secondary hover:bg-bg-hover hover:text-text-primary text-left">
      {icon && <span className="w-4 shrink-0">{icon}</span>}
      <span className="flex-1">{label}</span>
      {shortcut && <span className="text-text-muted/40 text-[10px]">{shortcut}</span>}
      {hasSubmenu && <ChevronRight size={10} className="text-text-muted/40" />}
    </button>
  );
}

function FpMenuSep() {
  return <div className="my-1 border-t border-border-subtle" />;
}

// ═══════════════════════════════════════════════════════════════
// RENDERING
// ═══════════════════════════════════════════════════════════════

function renderFootprint(
  ctx: CanvasRenderingContext2D,
  fp: FootprintData,
  sel: ReturnType<typeof useFootprintEditorStore.getState>["selectedItem"],
  zoom: number,
) {
  // Graphics (silkscreen, fab, courtyard)
  for (let i = 0; i < fp.graphics.length; i++) {
    const g = fp.graphics[i];
    const isSelected = sel?.type === "graphic" && sel.index === i;
    const color = isSelected ? COLORS.selected : layerColor(g.layer);
    ctx.strokeStyle = color;
    ctx.fillStyle = color;
    ctx.lineWidth = Math.max("width" in g ? g.width : 0.1, 0.02);

    if (g.type === "line") {
      ctx.beginPath();
      ctx.moveTo(g.start.x, g.start.y); ctx.lineTo(g.end.x, g.end.y);
      ctx.stroke();
    } else if (g.type === "rect") {
      const rx = Math.min(g.start.x, g.end.x), ry = Math.min(g.start.y, g.end.y);
      const rw = Math.abs(g.end.x - g.start.x), rh = Math.abs(g.end.y - g.start.y);
      if (g.fill) { ctx.globalAlpha = 0.3; ctx.fillRect(rx, ry, rw, rh); ctx.globalAlpha = 1; }
      ctx.strokeRect(rx, ry, rw, rh);
    } else if (g.type === "circle") {
      ctx.beginPath();
      ctx.arc(g.center.x, g.center.y, g.radius, 0, Math.PI * 2);
      if (g.fill) { ctx.globalAlpha = 0.3; ctx.fill(); ctx.globalAlpha = 1; }
      ctx.stroke();
    } else if (g.type === "arc") {
      ctx.beginPath();
      ctx.moveTo(g.start.x, g.start.y);
      ctx.quadraticCurveTo(g.mid.x, g.mid.y, g.end.x, g.end.y);
      ctx.stroke();
    } else if (g.type === "poly" && g.points.length >= 2) {
      ctx.beginPath();
      ctx.moveTo(g.points[0].x, g.points[0].y);
      for (let j = 1; j < g.points.length; j++) ctx.lineTo(g.points[j].x, g.points[j].y);
      ctx.closePath();
      if (g.fill) { ctx.globalAlpha = 0.3; ctx.fill(); ctx.globalAlpha = 1; }
      ctx.stroke();
    } else if (g.type === "text") {
      const fs = g.fontSize * 0.8;
      ctx.font = `${fs}px sans-serif`;
      ctx.textBaseline = "middle";
      ctx.textAlign = "center";
      ctx.fillText(g.text, g.position.x, g.position.y);
    }
  }

  // Pads
  for (let i = 0; i < fp.pads.length; i++) {
    const p = fp.pads[i];
    const isSelected = sel?.type === "pad" && sel.index === i;
    const px = p.position.x, py = p.position.y;
    const sw = p.size[0], sh = p.size[1];

    ctx.fillStyle = isSelected ? COLORS.selected : (p.type === "smd" ? COLORS.padSmd : COLORS.padTh);

    if (p.shape === "circle") {
      ctx.beginPath(); ctx.arc(px, py, sw / 2, 0, Math.PI * 2); ctx.fill();
    } else if (p.shape === "roundrect") {
      const r = (p.roundrectRatio ?? 0.25) * Math.min(sw, sh) / 2;
      drawRoundRect(ctx, px - sw / 2, py - sh / 2, sw, sh, r);
      ctx.fill();
    } else if (p.shape === "oval") {
      ctx.beginPath(); ctx.ellipse(px, py, sw / 2, sh / 2, 0, 0, Math.PI * 2); ctx.fill();
    } else {
      ctx.fillRect(px - sw / 2, py - sh / 2, sw, sh);
    }

    // Drill hole
    if (p.type === "thru_hole" && p.drill) {
      ctx.fillStyle = COLORS.drill;
      ctx.beginPath(); ctx.arc(px, py, p.drill.diameter / 2, 0, Math.PI * 2); ctx.fill();
    }

    // Pad number
    if (p.number) {
      const fs = Math.min(sw, sh) * 0.45;
      if (fs * zoom > 4) {
        ctx.fillStyle = isSelected ? "#000" : "#fff";
        ctx.font = `${fs}px sans-serif`;
        ctx.textAlign = "center"; ctx.textBaseline = "middle";
        ctx.fillText(p.number, px, py);
      }
    }

    // Selection outline
    if (isSelected) {
      ctx.strokeStyle = COLORS.selected;
      ctx.lineWidth = 0.05;
      ctx.setLineDash([0.1, 0.1]);
      ctx.strokeRect(px - sw / 2 - 0.1, py - sh / 2 - 0.1, sw + 0.2, sh + 0.2);
      ctx.setLineDash([]);
    }
  }
}

function drawRoundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  r = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + w - r, y); ctx.arcTo(x + w, y, x + w, y + r, r);
  ctx.lineTo(x + w, y + h - r); ctx.arcTo(x + w, y + h, x + w - r, y + h, r);
  ctx.lineTo(x + r, y + h); ctx.arcTo(x, y + h, x, y + h - r, r);
  ctx.lineTo(x, y + r); ctx.arcTo(x, y, x + r, y, r);
  ctx.closePath();
}

function hitTestGraphic(point: { x: number; y: number }, g: PcbGraphic): boolean {
  const tol = 0.3;
  if (g.type === "line") {
    return pointToSegDist(point, g.start, g.end) < tol;
  }
  if (g.type === "rect") {
    const rx = Math.min(g.start.x, g.end.x) - tol, ry = Math.min(g.start.y, g.end.y) - tol;
    return point.x >= rx && point.x <= Math.max(g.start.x, g.end.x) + tol &&
           point.y >= ry && point.y <= Math.max(g.start.y, g.end.y) + tol;
  }
  if (g.type === "circle") {
    const d = Math.hypot(point.x - g.center.x, point.y - g.center.y);
    return g.fill ? d <= g.radius + tol : Math.abs(d - g.radius) < tol;
  }
  if (g.type === "text") {
    return Math.hypot(point.x - g.position.x, point.y - g.position.y) < 1;
  }
  return false;
}

function pointToSegDist(p: { x: number; y: number }, a: { x: number; y: number }, b: { x: number; y: number }): number {
  const dx = b.x - a.x, dy = b.y - a.y;
  const lenSq = dx * dx + dy * dy;
  if (lenSq === 0) return Math.hypot(p.x - a.x, p.y - a.y);
  let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}
