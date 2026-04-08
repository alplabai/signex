import { useRef, useEffect, useCallback } from "react";
import { useLibraryEditorStore } from "@/stores/libraryEditor";
import { useEditorStore } from "@/stores/editor";
import type { LibSymbol, SchPin, Graphic, SchPoint } from "@/types";

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

      if (store.editMode === "select" && symbol) {
        // Hit test pins
        for (let i = 0; i < symbol.pins.length; i++) {
          const pin = symbol.pins[i];
          const pe = pinEnd(pin);
          if (dist(world, pin.position) < PIN_HIT_RADIUS || dist(world, pe) < PIN_HIT_RADIUS) {
            store.setSelectedItem({ type: "pin", index: i });
            return;
          }
        }
        // Hit test graphics
        for (let i = 0; i < symbol.graphics.length; i++) {
          if (hitTestGraphic(world, symbol.graphics[i])) {
            store.setSelectedItem({ type: "graphic", index: i });
            return;
          }
        }
        store.setSelectedItem(null);
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
      } else if (store.editMode === "addRect") {
        store.addGraphic({
          type: "Rectangle",
          start: { x: sw.x - 2.54, y: sw.y - 2.54 },
          end: { x: sw.x + 2.54, y: sw.y + 2.54 },
          width: 0.254,
          fill: false,
        });
        store.setEditMode("select");
      } else if (store.editMode === "addCircle") {
        store.addGraphic({
          type: "Circle",
          center: sw,
          radius: 2.54,
          width: 0.254,
          fill: false,
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
          fill: false,
        });
        store.setEditMode("select");
      } else if (store.editMode === "addArc") {
        store.addGraphic({
          type: "Arc",
          start: { x: sw.x - 2.54, y: sw.y },
          mid: { x: sw.x, y: sw.y - 2.54 },
          end: { x: sw.x + 2.54, y: sw.y },
          width: 0.254,
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
        return;
      }
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const sx = e.clientX - rect.left;
      const sy = e.clientY - rect.top;
      const world = screenToWorld(sx, sy);
      cursorWorldRef.current = { x: snap(world.x), y: snap(world.y) };
    },
    [snap, screenToWorld]
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
    },
    []
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
    </div>
  );
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
    ctx.lineWidth = Math.max(g.width || 0.1, 0.1);

    switch (g.type) {
      case "Polyline": {
        if (g.points.length < 2) break;
        ctx.beginPath();
        ctx.moveTo(g.points[0].x, g.points[0].y);
        for (let j = 1; j < g.points.length; j++) ctx.lineTo(g.points[j].x, g.points[j].y);
        if (g.fill) {
          ctx.fillStyle = COLORS.bodyFill;
          ctx.globalAlpha = 0.15;
          ctx.fill();
          ctx.globalAlpha = 1;
        }
        ctx.stroke();
        break;
      }
      case "Rectangle": {
        const rx = Math.min(g.start.x, g.end.x), ry = Math.min(g.start.y, g.end.y);
        const rw = Math.abs(g.end.x - g.start.x), rh = Math.abs(g.end.y - g.start.y);
        if (g.fill) { ctx.fillStyle = COLORS.bodyFill; ctx.fillRect(rx, ry, rw, rh); }
        ctx.strokeRect(rx, ry, rw, rh);
        break;
      }
      case "Circle": {
        ctx.beginPath();
        ctx.arc(g.center.x, g.center.y, g.radius, 0, Math.PI * 2);
        if (g.fill) { ctx.fillStyle = COLORS.bodyFill; ctx.fill(); }
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
        g.fill
      );
    }
    case "Circle": {
      const d = dist(point, g.center);
      return g.fill ? d <= g.radius + tol : Math.abs(d - g.radius) < tol;
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
  }
}
