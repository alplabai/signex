import { useRef, useEffect, useState } from "react";
import type { PcbData, PcbLayerId } from "@/types/pcb";

// ═══════════════════════════════════════════════════════════════
// PCB 3D VIEWER — Three.js (Phase 6)
// Renders board, traces, pads, vias, components, silkscreen.
// ═══════════════════════════════════════════════════════════════

interface Props { data: PcbData }

const COLORS = {
  board: 0x1a6b3c, pad: 0xc0c0c0, via: 0xc0c0c0,
  silk: 0xffffcc, body: 0x333333, bg: 0x1a1b2e,
  fCu: 0xff3333, bCu: 0x3333ff,
};

function layerZ(layer: PcbLayerId, t: number): number {
  if (layer === "F.Cu") return t / 2;
  if (layer === "B.Cu") return -t / 2;
  const m = layer.match(/^In(\d+)\.Cu$/);
  return m ? t / 2 - (parseInt(m[1], 10) / 32) * t : t / 2;
}

function layerColor(layer: PcbLayerId): number {
  return layer === "F.Cu" ? COLORS.fCu : layer === "B.Cu" ? COLORS.bCu : 0x808080;
}

export function Pcb3DViewer({ data }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cleanupRef = useRef<(() => void) | null>(null);
  const frameRef = useRef(0);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    let disposed = false;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let renderer: any, THREE: any;

    (async () => {
      try { THREE = await import("three"); }
      catch { setError("3D viewer requires three.js — install with: npm install three"); return; }
      if (disposed) return;

      const w = container.clientWidth || 800;
      const h = container.clientHeight || 600;
      const t = data.board.thickness || 1.6;
      const TRACE_T = 0.035;

      // Scene, camera, renderer, lights
      const scene = new THREE.Scene();
      scene.background = new THREE.Color(COLORS.bg);
      const camera = new THREE.PerspectiveCamera(45, w / h, 0.1, 10000);
      camera.up.set(0, 0, 1);
      renderer = new THREE.WebGLRenderer({ antialias: true });
      renderer.setSize(w, h);
      renderer.setPixelRatio(window.devicePixelRatio);
      container.appendChild(renderer.domElement);
      scene.add(new THREE.AmbientLight(0xffffff, 0.6));
      const dir = new THREE.DirectionalLight(0xffffff, 0.8);
      dir.position.set(50, -50, 100);
      scene.add(dir);

      // Board center
      const outline = data.board.outline;
      let cx = 0, cy = 0;
      if (outline.length >= 3) {
        cx = outline.reduce((s, p) => s + p.x, 0) / outline.length;
        cy = outline.reduce((s, p) => s + p.y, 0) / outline.length;
      }

      // Helpers
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const matCache = new Map<number, any>();
      const getMat = (color: number) => {
        let mat = matCache.get(color);
        if (!mat) {
          mat = new THREE.MeshPhongMaterial({ color });
          matCache.set(color, mat);
        }
        return mat;
      };
      const addMesh = (geo: unknown, color: number, pos: [number, number, number], rz = 0) => {
        const mesh = new THREE.Mesh(geo, getMat(color));
        mesh.position.set(...pos);
        if (rz) mesh.rotation.z = rz;
        scene.add(mesh);
      };

      // Board body
      if (outline.length >= 3) {
        const shape = new THREE.Shape();
        shape.moveTo(outline[0].x - cx, -(outline[0].y - cy));
        for (let i = 1; i < outline.length; i++)
          shape.lineTo(outline[i].x - cx, -(outline[i].y - cy));
        shape.closePath();
        const geo = new THREE.ExtrudeGeometry(shape, { depth: t, bevelEnabled: false });
        geo.translate(0, 0, -t / 2);
        const mat = new THREE.MeshPhongMaterial({ color: COLORS.board, side: THREE.DoubleSide });
        scene.add(new THREE.Mesh(geo, mat));
      } else {
        addMesh(new THREE.BoxGeometry(80, 60, t), COLORS.board, [0, 0, 0]);
      }

      // Copper traces
      for (const seg of data.segments) {
        const dx = seg.end.x - seg.start.x, dy = seg.end.y - seg.start.y;
        const len = Math.hypot(dx, dy);
        if (len < 0.001) continue;
        const mx = (seg.start.x + seg.end.x) / 2 - cx;
        const my = -((seg.start.y + seg.end.y) / 2 - cy);
        addMesh(
          new THREE.BoxGeometry(len, seg.width, TRACE_T),
          layerColor(seg.layer),
          [mx, my, layerZ(seg.layer, t)],
          Math.atan2(-dy, dx)
        );
      }

      // Pads
      for (const fp of data.footprints) {
        const cr = Math.cos((-fp.rotation * Math.PI) / 180);
        const sr = Math.sin((-fp.rotation * Math.PI) / 180);
        for (const pad of fp.pads) {
          const rx = pad.position.x * cr - pad.position.y * sr;
          const ry = pad.position.x * sr + pad.position.y * cr;
          const px = fp.position.x + rx - cx;
          const py = -(fp.position.y + ry - cy);
          const front = pad.layers.includes("F.Cu") || pad.layers.includes("*.Cu") || fp.layer === "F.Cu";
          const z = front ? t / 2 : -t / 2;
          const thru = pad.type === "thru_hole";
          const depth = thru ? t + 0.1 : 0.05;
          if (pad.shape === "circle" || pad.shape === "oval") {
            const geo = new THREE.CylinderGeometry(pad.size[0] / 2, pad.size[0] / 2, depth, 16);
            geo.rotateX(Math.PI / 2);
            addMesh(geo, COLORS.pad, [px, py, thru ? 0 : z]);
          } else {
            addMesh(
              new THREE.BoxGeometry(pad.size[0], pad.size[1], depth),
              COLORS.pad, [px, py, thru ? 0 : z],
              (-fp.rotation * Math.PI) / 180
            );
          }
          if (pad.drill && thru) {
            const geo = new THREE.CylinderGeometry(pad.drill.diameter / 2, pad.drill.diameter / 2, t + 0.2, 12);
            geo.rotateX(Math.PI / 2);
            addMesh(geo, COLORS.bg, [px, py, 0]);
          }
        }
      }

      // Vias
      for (const via of data.vias) {
        const px = via.position.x - cx, py = -(via.position.y - cy);
        const outerGeo = new THREE.CylinderGeometry(via.diameter / 2, via.diameter / 2, t + 0.1, 12);
        outerGeo.rotateX(Math.PI / 2);
        addMesh(outerGeo, COLORS.via, [px, py, 0]);
        const drillGeo = new THREE.CylinderGeometry(via.drill / 2, via.drill / 2, t + 0.2, 12);
        drillGeo.rotateX(Math.PI / 2);
        addMesh(drillGeo, COLORS.bg, [px, py, 0]);
      }

      // Component bodies (placeholder boxes)
      for (const fp of data.footprints) {
        if (fp.pads.length < 2) continue;
        let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
        for (const pad of fp.pads) {
          x0 = Math.min(x0, pad.position.x - pad.size[0] / 2);
          x1 = Math.max(x1, pad.position.x + pad.size[0] / 2);
          y0 = Math.min(y0, pad.position.y - pad.size[1] / 2);
          y1 = Math.max(y1, pad.position.y + pad.size[1] / 2);
        }
        const bw = x1 - x0, bh = y1 - y0;
        const bodyH = Math.max(0.5, Math.min(bw, bh) * 0.3);
        const front = fp.layer === "F.Cu";
        const zBase = front ? t / 2 : -t / 2;
        const bcx = fp.position.x + (x0 + x1) / 2 - cx;
        const bcy = -(fp.position.y + (y0 + y1) / 2 - cy);
        addMesh(
          new THREE.BoxGeometry(bw * 0.9, bh * 0.9, bodyH),
          COLORS.body,
          [bcx, bcy, zBase + (front ? 1 : -1) * bodyH / 2],
          (-fp.rotation * Math.PI) / 180
        );
      }

      // Silkscreen text (flat planes on board surface)
      for (const fp of data.footprints) {
        for (const g of fp.graphics) {
          if (g.type !== "text") continue;
          if (g.layer !== "F.SilkS" && g.layer !== "B.SilkS") continue;
          const label = g.text === "%R" ? fp.reference : g.text === "%V" ? fp.value : g.text;
          if (!label) continue;
          const front = g.layer === "F.SilkS";
          const z = front ? t / 2 + 0.04 : -t / 2 - 0.04;
          const geo = new THREE.PlaneGeometry(Math.max(1, label.length * g.fontSize * 0.5), g.fontSize * 1.2);
          const mat = new THREE.MeshBasicMaterial({ color: COLORS.silk, transparent: true, opacity: 0.7, side: THREE.DoubleSide });
          const mesh = new THREE.Mesh(geo, mat);
          mesh.position.set(fp.position.x + g.position.x - cx, -(fp.position.y + g.position.y - cy), z);
          scene.add(mesh);
        }
      }

      // Orbit controls (manual: left-drag=orbit, scroll=zoom, right-drag=pan)
      const orb = { phi: Math.PI / 4, theta: -Math.PI / 2, radius: 120 };
      const tgt = new THREE.Vector3(0, 0, 0);
      const updateCam = () => {
        camera.position.set(
          tgt.x + orb.radius * Math.sin(orb.phi) * Math.cos(orb.theta),
          tgt.y + orb.radius * Math.sin(orb.phi) * Math.sin(orb.theta),
          tgt.z + orb.radius * Math.cos(orb.phi)
        );
        camera.lookAt(tgt);
      };
      updateCam();

      let dragging = false, panning = false, lx = 0, ly = 0;
      const onDown = (e: MouseEvent) => {
        if (e.button === 0) dragging = true;
        if (e.button === 2) panning = true;
        lx = e.clientX; ly = e.clientY;
      };
      const onMove = (e: MouseEvent) => {
        const dx = e.clientX - lx, dy = e.clientY - ly;
        lx = e.clientX; ly = e.clientY;
        if (dragging) {
          orb.theta -= dx * 0.005;
          orb.phi = Math.max(0.05, Math.min(Math.PI - 0.05, orb.phi + dy * 0.005));
          updateCam();
        }
        if (panning) {
          const ps = orb.radius * 0.002;
          const rt = new THREE.Vector3(), up = new THREE.Vector3();
          camera.getWorldDirection(up);
          rt.crossVectors(camera.up, up).normalize();
          up.crossVectors(rt, up).normalize();
          tgt.add(rt.multiplyScalar(-dx * ps));
          tgt.add(up.multiplyScalar(dy * ps));
          updateCam();
        }
      };
      const onUp = () => { dragging = false; panning = false; };
      const onWhl = (e: WheelEvent) => {
        e.preventDefault();
        orb.radius = Math.max(5, Math.min(1000, orb.radius * (e.deltaY > 0 ? 1.1 : 0.9)));
        updateCam();
      };
      const noCtx = (e: MouseEvent) => e.preventDefault();

      const el = renderer.domElement as HTMLCanvasElement;
      el.addEventListener("mousedown", onDown);
      el.addEventListener("mousemove", onMove);
      el.addEventListener("mouseup", onUp);
      el.addEventListener("mouseleave", onUp);
      el.addEventListener("wheel", onWhl, { passive: false });
      el.addEventListener("contextmenu", noCtx);

      // Resize observer
      const ro = new ResizeObserver(() => {
        if (disposed) return;
        camera.aspect = container.clientWidth / container.clientHeight;
        camera.updateProjectionMatrix();
        renderer.setSize(container.clientWidth, container.clientHeight);
      });
      ro.observe(container);

      // Render loop
      const animate = () => {
        if (disposed) return;
        frameRef.current = requestAnimationFrame(animate);
        renderer.render(scene, camera);
      };
      frameRef.current = requestAnimationFrame(animate);

      // Store cleanup
      cleanupRef.current = () => {
        ro.disconnect();
        cancelAnimationFrame(frameRef.current);
        el.removeEventListener("mousedown", onDown);
        el.removeEventListener("mousemove", onMove);
        el.removeEventListener("mouseup", onUp);
        el.removeEventListener("mouseleave", onUp);
        el.removeEventListener("wheel", onWhl);
        el.removeEventListener("contextmenu", noCtx);
        renderer.dispose();
        scene.traverse((obj: { geometry?: { dispose(): void }; material?: { dispose(): void } }) => {
          obj.geometry?.dispose();
          if (obj.material) obj.material.dispose();
        });
        if (container.contains(el)) container.removeChild(el);
      };
    })();

    return () => {
      disposed = true;
      cleanupRef.current?.();
      cleanupRef.current = null;
    };
  }, [data]);

  if (error) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-[#1a1b2e] text-[#6c7086]">
        <p className="text-sm">{error}</p>
      </div>
    );
  }
  return <div ref={containerRef} className="w-full h-full bg-[#1a1b2e]" />;
}
