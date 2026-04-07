import { useRef, useEffect, useCallback } from "react";
import { usePcbStore } from "@/stores/pcb";
import { useEditorStore } from "@/stores/editor";
import { DEFAULT_LAYER_COLORS } from "@/types/pcb";
import { computeRatsnest, getPadPosition } from "@/lib/pcbRatsnest";
import type { PcbPoint, PcbLayerId } from "@/types/pcb";

interface Camera { x: number; y: number; zoom: number }

// ═══════════════════════════════════════════════════════════════
// PCB RENDERER — Canvas2D (Phase 6 MVP, WebGL2 migration planned)
//
// Architecture follows KiCad's GAL pattern:
// - All drawing through abstracted primitives
// - Layer-based rendering order
// - Viewport culling via bounding box checks
// - Static/dynamic split for future framebuffer caching
// ═══════════════════════════════════════════════════════════════

export function PcbRenderer() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const camRef = useRef<Camera>({ x: 0, y: 0, zoom: 5 }); // PCB uses ~5 px/mm default
  const animRef = useRef(0);
  const dragging = useRef(false);
  const lastMouse = useRef({ x: 0, y: 0 });

  const data = usePcbStore((s) => s.data);
  const selectedIds = usePcbStore((s) => s.selectedIds);
  const activeLayer = usePcbStore((s) => s.activeLayer);
  const visibleLayers = usePcbStore((s) => s.visibleLayers);
  const editMode = usePcbStore((s) => s.editMode);
  const routingActive = usePcbStore((s) => s.routingActive);
  const routingPoints = usePcbStore((s) => s.routingPoints);
  const gridVisible = useEditorStore((s) => s.gridVisible);

  const s2w = useCallback((sx: number, sy: number): PcbPoint => {
    const cam = camRef.current;
    return { x: (sx - cam.x) / cam.zoom, y: (sy - cam.y) / cam.zoom };
  }, []);

  const getLayerColor = useCallback((layer: PcbLayerId, alpha = 1): string => {
    const hex = DEFAULT_LAYER_COLORS[layer] || "#808080";
    if (alpha >= 1) return hex;
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgba(${r},${g},${b},${alpha})`;
  }, []);

  // --- Render ---
  const render = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const cam = camRef.current;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.width / dpr;
    const h = canvas.height / dpr;

    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    // Background
    ctx.fillStyle = "#1a1a2e";
    ctx.fillRect(0, 0, w, h);

    if (!data) {
      ctx.fillStyle = "#4a4a6a";
      ctx.font = "14px sans-serif";
      ctx.textAlign = "center";
      ctx.fillText("No PCB loaded", w / 2, h / 2);
      return;
    }

    ctx.save();
    ctx.translate(cam.x, cam.y);
    ctx.scale(cam.zoom, cam.zoom);

    // Grid
    if (gridVisible && cam.zoom > 1) {
      const gs = data.board.setup.gridSize || 1.27;
      const tl = s2w(0, 0);
      const br = s2w(w, h);
      ctx.globalAlpha = 0.2;
      ctx.fillStyle = "#4a4a6a";
      const startX = Math.floor(tl.x / gs) * gs;
      const startY = Math.floor(tl.y / gs) * gs;
      for (let x = startX; x <= br.x; x += gs) {
        for (let y = startY; y <= br.y; y += gs) {
          ctx.fillRect(x - 0.02, y - 0.02, 0.04, 0.04);
        }
      }
      ctx.globalAlpha = 1;
    }

    // Board outline
    if (data.board.outline.length >= 3) {
      ctx.strokeStyle = getLayerColor("Edge.Cuts");
      ctx.lineWidth = 0.15;
      ctx.beginPath();
      ctx.moveTo(data.board.outline[0].x, data.board.outline[0].y);
      for (let i = 1; i < data.board.outline.length; i++) {
        ctx.lineTo(data.board.outline[i].x, data.board.outline[i].y);
      }
      ctx.closePath();
      ctx.fillStyle = "#1e1e3a";
      ctx.fill();
      ctx.stroke();
    }

    // Render layers back-to-front: B.Cu zones → B.Cu traces → ... → F.Cu traces → F.Cu zones → silk
    const layerOrder: PcbLayerId[] = [
      "B.Fab", "B.CrtYd", "B.SilkS", "B.Mask", "B.Cu",
      ...Array.from({ length: 30 }, (_, i) => `In${i + 1}.Cu` as PcbLayerId).filter((l) => visibleLayers.has(l)),
      "F.Cu", "F.Mask", "F.SilkS", "F.CrtYd", "F.Fab",
    ];

    for (const layer of layerOrder) {
      if (!visibleLayers.has(layer)) continue;
      const color = getLayerColor(layer);
      const isActive = layer === activeLayer;
      const alpha = isActive ? 1.0 : 0.5;

      // Zones on this layer
      for (const zone of data.zones) {
        if (zone.layer !== layer) continue;
        ctx.globalAlpha = alpha * 0.3;
        ctx.fillStyle = color;
        if (zone.outline.length >= 3) {
          ctx.beginPath();
          ctx.moveTo(zone.outline[0].x, zone.outline[0].y);
          for (let i = 1; i < zone.outline.length; i++) ctx.lineTo(zone.outline[i].x, zone.outline[i].y);
          ctx.closePath();
          ctx.fill();
        }
        ctx.globalAlpha = 1;
      }

      // Trace segments on this layer
      ctx.strokeStyle = color;
      ctx.lineCap = "round";
      for (const seg of data.segments) {
        if (seg.layer !== layer) continue;
        const sel = selectedIds.has(seg.uuid);
        ctx.globalAlpha = alpha;
        ctx.strokeStyle = sel ? "#00e5ff" : color;
        ctx.lineWidth = seg.width;
        ctx.beginPath();
        ctx.moveTo(seg.start.x, seg.start.y);
        ctx.lineTo(seg.end.x, seg.end.y);
        ctx.stroke();
      }
      ctx.globalAlpha = 1;

      // Footprint pads on this layer
      for (const fp of data.footprints) {
        for (const pad of fp.pads) {
          if (!pad.layers.includes(layer) && !pad.layers.includes("*.Cu")) continue;
          const px = fp.position.x + pad.position.x;
          const py = fp.position.y + pad.position.y;
          const sel = selectedIds.has(fp.uuid);

          ctx.fillStyle = sel ? "#00e5ff" : color;
          ctx.globalAlpha = alpha;

          if (pad.shape === "circle") {
            ctx.beginPath();
            ctx.arc(px, py, pad.size[0] / 2, 0, Math.PI * 2);
            ctx.fill();
          } else if (pad.shape === "rect" || pad.shape === "roundrect") {
            const hw = pad.size[0] / 2, hh = pad.size[1] / 2;
            if (pad.shape === "roundrect" && pad.roundrectRatio) {
              const r = Math.min(hw, hh) * (pad.roundrectRatio || 0.25);
              ctx.beginPath();
              ctx.moveTo(px - hw + r, py - hh);
              ctx.lineTo(px + hw - r, py - hh);
              ctx.arcTo(px + hw, py - hh, px + hw, py - hh + r, r);
              ctx.lineTo(px + hw, py + hh - r);
              ctx.arcTo(px + hw, py + hh, px + hw - r, py + hh, r);
              ctx.lineTo(px - hw + r, py + hh);
              ctx.arcTo(px - hw, py + hh, px - hw, py + hh - r, r);
              ctx.lineTo(px - hw, py - hh + r);
              ctx.arcTo(px - hw, py - hh, px - hw + r, py - hh, r);
              ctx.fill();
            } else {
              ctx.fillRect(px - hw, py - hh, pad.size[0], pad.size[1]);
            }
          } else if (pad.shape === "oval") {
            const hw = pad.size[0] / 2, hh = pad.size[1] / 2;
            ctx.beginPath();
            ctx.ellipse(px, py, hw, hh, 0, 0, Math.PI * 2);
            ctx.fill();
          }

          // Drill hole
          if (pad.drill && pad.type === "thru_hole") {
            ctx.fillStyle = "#1a1a2e";
            ctx.beginPath();
            ctx.arc(px, py, pad.drill.diameter / 2, 0, Math.PI * 2);
            ctx.fill();
          }

          ctx.globalAlpha = 1;
        }
      }

      // Footprint graphics on this layer
      for (const fp of data.footprints) {
        for (const g of fp.graphics) {
          if (g.layer !== layer) continue;
          ctx.strokeStyle = color;
          ctx.lineWidth = g.type === "text" ? 0.1 : ("width" in g ? g.width : 0.1);
          ctx.globalAlpha = alpha;

          const ox = fp.position.x, oy = fp.position.y;
          if (g.type === "line") {
            ctx.beginPath();
            ctx.moveTo(ox + g.start.x, oy + g.start.y);
            ctx.lineTo(ox + g.end.x, oy + g.end.y);
            ctx.stroke();
          } else if (g.type === "rect") {
            ctx.strokeRect(ox + g.start.x, oy + g.start.y, g.end.x - g.start.x, g.end.y - g.start.y);
          } else if (g.type === "circle") {
            ctx.beginPath();
            ctx.arc(ox + g.center.x, oy + g.center.y, g.radius, 0, Math.PI * 2);
            ctx.stroke();
          } else if (g.type === "text") {
            ctx.fillStyle = color;
            ctx.font = `${g.fontSize}px sans-serif`;
            ctx.textAlign = "center";
            ctx.textBaseline = "middle";
            ctx.fillText(g.text === "%R" ? fp.reference : g.text === "%V" ? fp.value : g.text, ox + g.position.x, oy + g.position.y);
          }
          ctx.globalAlpha = 1;
        }
      }
    }

    // Vias (render on top of all copper)
    for (const via of data.vias) {
      const sel = selectedIds.has(via.uuid);
      ctx.fillStyle = sel ? "#00e5ff" : "#c0c0c0";
      ctx.beginPath();
      ctx.arc(via.position.x, via.position.y, via.diameter / 2, 0, Math.PI * 2);
      ctx.fill();
      // Drill
      ctx.fillStyle = "#1a1a2e";
      ctx.beginPath();
      ctx.arc(via.position.x, via.position.y, via.drill / 2, 0, Math.PI * 2);
      ctx.fill();
    }

    // Ratsnest (unrouted connections)
    const ratsnest = computeRatsnest(data);
    if (ratsnest.length > 0) {
      ctx.strokeStyle = "#ffff00";
      ctx.lineWidth = 0.05;
      ctx.setLineDash([0.3, 0.2]);
      ctx.globalAlpha = 0.6;
      for (const line of ratsnest) {
        const posA = getPadPosition(data, line.padA);
        const posB = getPadPosition(data, line.padB);
        if (posA && posB) {
          ctx.beginPath();
          ctx.moveTo(posA.x, posA.y);
          ctx.lineTo(posB.x, posB.y);
          ctx.stroke();
        }
      }
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
    }

    // Routing preview
    if (routingActive && routingPoints.length > 0) {
      ctx.strokeStyle = getLayerColor(usePcbStore.getState().routingLayer);
      ctx.lineWidth = usePcbStore.getState().routingWidth;
      ctx.lineCap = "round";
      ctx.setLineDash([0.5, 0.3]);
      ctx.globalAlpha = 0.7;
      ctx.beginPath();
      ctx.moveTo(routingPoints[0].x, routingPoints[0].y);
      for (let i = 1; i < routingPoints.length; i++) {
        ctx.lineTo(routingPoints[i].x, routingPoints[i].y);
      }
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.globalAlpha = 1;
    }

    // Selection highlight
    for (const fp of data.footprints) {
      if (!selectedIds.has(fp.uuid)) continue;
      ctx.strokeStyle = "#00e5ff";
      ctx.lineWidth = 0.15;
      ctx.setLineDash([0.3, 0.2]);
      // Simple bbox
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const pad of fp.pads) {
        const px = fp.position.x + pad.position.x;
        const py = fp.position.y + pad.position.y;
        minX = Math.min(minX, px - pad.size[0]); maxX = Math.max(maxX, px + pad.size[0]);
        minY = Math.min(minY, py - pad.size[1]); maxY = Math.max(maxY, py + pad.size[1]);
      }
      if (isFinite(minX)) {
        ctx.strokeRect(minX - 0.5, minY - 0.5, maxX - minX + 1, maxY - minY + 1);
      }
      ctx.setLineDash([]);
    }

    ctx.restore();

    // Status overlay
    ctx.fillStyle = "#cdd6f4";
    ctx.font = "11px sans-serif";
    ctx.textAlign = "left";
    const unrouted = ratsnest.length;
    ctx.fillText(
      `Layer: ${activeLayer} | Mode: ${editMode} | Zoom: ${(cam.zoom * 100).toFixed(0)}% | ` +
      `Nets: ${data.nets.length} | Unrouted: ${unrouted} | Segments: ${data.segments.length} | Vias: ${data.vias.length}`,
      10, h - 10
    );
  }, [data, selectedIds, activeLayer, visibleLayers, editMode, routingActive, routingPoints, gridVisible, s2w, getLayerColor]);

  // --- Resize ---
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
      if (!data) {
        camRef.current.x = rect.width / 2;
        camRef.current.y = rect.height / 2;
      }
      render();
    };

    const ro = new ResizeObserver(resize);
    ro.observe(container);
    resize();
    return () => ro.disconnect();
  }, [render, data]);

  // Animation loop
  useEffect(() => {
    const loop = () => {
      render();
      animRef.current = requestAnimationFrame(loop);
    };
    animRef.current = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(animRef.current);
  }, [render]);

  // --- Mouse handlers ---
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (e.button === 1 || (e.button === 2)) {
      dragging.current = true;
      lastMouse.current = { x: e.clientX, y: e.clientY };
      e.preventDefault();
      return;
    }

    if (e.button === 0 && data) {
      const r = canvasRef.current?.getBoundingClientRect();
      if (!r) return;
      const world = s2w(e.clientX - r.left, e.clientY - r.top);
      const store = usePcbStore.getState();

      if (store.editMode === "routeTrack") {
        if (store.routingActive) {
          store.addRoutePoint(world);
        } else {
          // Find net from nearest pad
          let nearestNet = 0;
          for (const fp of data.footprints) {
            for (const pad of fp.pads) {
              if (!pad.net) continue;
              const px = fp.position.x + pad.position.x;
              const py = fp.position.y + pad.position.y;
              const d = Math.hypot(world.x - px, world.y - py);
              if (d < Math.max(pad.size[0], pad.size[1])) {
                nearestNet = pad.net.number;
              }
            }
          }
          store.startRoute(world, nearestNet);
        }
        return;
      }

      // Board outline drawing
      if (store.editMode === "drawBoardOutline") {
        const outline = [...(store.data?.board.outline || []), world];
        store.setBoardOutline(outline);
        return;
      }

      // Simple footprint hit test
      for (const fp of data.footprints) {
        let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
        for (const pad of fp.pads) {
          const px = fp.position.x + pad.position.x;
          const py = fp.position.y + pad.position.y;
          minX = Math.min(minX, px - pad.size[0] / 2); maxX = Math.max(maxX, px + pad.size[0] / 2);
          minY = Math.min(minY, py - pad.size[1] / 2); maxY = Math.max(maxY, py + pad.size[1] / 2);
        }
        if (world.x >= minX && world.x <= maxX && world.y >= minY && world.y <= maxY) {
          if (e.shiftKey) store.toggleSelect(fp.uuid);
          else store.select(fp.uuid);
          return;
        }
      }

      // Segment hit test
      const hitTol = 1.0 / camRef.current.zoom;
      for (const seg of data.segments) {
        const dx = seg.end.x - seg.start.x, dy = seg.end.y - seg.start.y;
        const lenSq = dx * dx + dy * dy;
        if (lenSq === 0) continue;
        let t = ((world.x - seg.start.x) * dx + (world.y - seg.start.y) * dy) / lenSq;
        t = Math.max(0, Math.min(1, t));
        const nearest = { x: seg.start.x + t * dx, y: seg.start.y + t * dy };
        if (Math.hypot(world.x - nearest.x, world.y - nearest.y) < seg.width / 2 + hitTol) {
          if (e.shiftKey) store.toggleSelect(seg.uuid);
          else store.select(seg.uuid);
          return;
        }
      }

      // Via hit test
      for (const via of data.vias) {
        if (Math.hypot(world.x - via.position.x, world.y - via.position.y) < via.diameter / 2 + hitTol) {
          if (e.shiftKey) store.toggleSelect(via.uuid);
          else store.select(via.uuid);
          return;
        }
      }

      store.deselectAll();
    }
  }, [data, s2w]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (dragging.current) {
      camRef.current.x += e.clientX - lastMouse.current.x;
      camRef.current.y += e.clientY - lastMouse.current.y;
      lastMouse.current = { x: e.clientX, y: e.clientY };
    }
  }, []);

  const handleMouseUp = useCallback(() => {
    dragging.current = false;
  }, []);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    const r = canvasRef.current?.getBoundingClientRect();
    if (!r) return;
    const sx = e.clientX - r.left, sy = e.clientY - r.top;
    const cam = camRef.current;
    const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
    const newZoom = Math.max(0.1, Math.min(500, cam.zoom * factor));
    cam.x = sx - (sx - cam.x) * (newZoom / cam.zoom);
    cam.y = sy - (sy - cam.y) * (newZoom / cam.zoom);
    cam.zoom = newZoom;
  }, []);

  // Keyboard
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      const store = usePcbStore.getState();

      switch (e.key) {
        case "Escape":
          if (store.routingActive) store.cancelRoute();
          else store.setEditMode("select");
          break;
        case "Delete":
          store.deleteSelected();
          break;
        case " ":
          if (store.selectedIds.size > 0) {
            e.preventDefault();
            for (const id of store.selectedIds) store.rotateFootprint(id, 90);
          }
          break;
        case "f":
        case "F":
          if (!e.ctrlKey && store.selectedIds.size > 0) {
            for (const id of store.selectedIds) store.flipFootprint(id);
          }
          break;
        case "x":
        case "X":
          if (!e.ctrlKey) store.setEditMode("routeTrack");
          break;
        case "+":
        case "=":
          store.setActiveLayer(store.activeLayer === "F.Cu" ? "B.Cu" : "F.Cu");
          break;
        case "z":
          if (e.ctrlKey) { e.preventDefault(); store.undo(); }
          break;
        case "y":
          if (e.ctrlKey) { e.preventDefault(); store.redo(); }
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  return (
    <div ref={containerRef} className="w-full h-full relative bg-[#1a1a2e]">
      <canvas
        ref={canvasRef}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onWheel={handleWheel}
        onContextMenu={(e) => e.preventDefault()}
        className="absolute inset-0"
        style={{ cursor: editMode !== "select" ? "crosshair" : "default" }}
      />
    </div>
  );
}
