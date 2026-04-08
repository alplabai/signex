/**
 * Advanced PCB Routing — Multi-track, BGA fanout, push placement.
 */

import type { PcbData, PcbPoint, PcbSegment, PcbVia, PcbLayerId, PcbFootprint } from "@/types/pcb";
import { usePcbStore } from "@/stores/pcb";

// ═══════════════════════════════════════════════════════════════
// MULTI-TRACK (BUS) ROUTING
// ═══════════════════════════════════════════════════════════════

export interface MultiTrackOptions {
  width: number;
  spacing: number; // Center-to-center spacing between tracks
  layer: PcbLayerId;
  nets: number[]; // Array of net numbers to route in parallel
  cornerStyle: "45" | "90";
}

/**
 * Route multiple parallel tracks simultaneously (bus routing).
 * Takes a path defined by waypoints and fans out N parallel traces.
 */
export function routeMultiTrack(
  waypoints: PcbPoint[],
  options: MultiTrackOptions,
): PcbSegment[] {
  if (waypoints.length < 2 || options.nets.length === 0) return [];

  const segments: PcbSegment[] = [];
  const n = options.nets.length;
  const totalWidth = (n - 1) * options.spacing;

  for (let trackIdx = 0; trackIdx < n; trackIdx++) {
    const offset = -totalWidth / 2 + trackIdx * options.spacing;
    const net = options.nets[trackIdx];

    for (let i = 0; i < waypoints.length - 1; i++) {
      const start = waypoints[i];
      const end = waypoints[i + 1];

      // Offset perpendicular to the track direction
      const dx = end.x - start.x;
      const dy = end.y - start.y;
      const len = Math.hypot(dx, dy) || 1;
      const nx = -dy / len * offset;
      const ny = dx / len * offset;

      const seg: PcbSegment = {
        uuid: crypto.randomUUID(),
        start: { x: start.x + nx, y: start.y + ny },
        end: { x: end.x + nx, y: end.y + ny },
        width: options.width,
        layer: options.layer,
        net,
      };

      segments.push(seg);
    }
  }

  return segments;
}

// ═══════════════════════════════════════════════════════════════
// BGA FANOUT
// ═══════════════════════════════════════════════════════════════

export interface BgaFanoutOptions {
  footprintUuid: string;
  viaDiameter: number;
  viaDrill: number;
  traceWidth: number;
  dogboneLength: number; // Length of escape trace from pad to via
  escapeDirection: "outward" | "inward" | "alternating";
  topLayer: PcbLayerId;
  innerLayer: PcbLayerId;
}

/**
 * Generate BGA fanout (dog-bone escape routing).
 * For each BGA pad, creates a short trace + via for layer transition.
 */
export function generateBgaFanout(
  data: PcbData,
  options: BgaFanoutOptions,
): { segments: PcbSegment[]; vias: PcbVia[] } {
  const segments: PcbSegment[] = [];
  const vias: PcbVia[] = [];

  const fp = data.footprints.find((f) => f.uuid === options.footprintUuid);
  if (!fp) return { segments, vias };

  // Use footprint position as centroid
  const cx = fp.position.x;
  const cy = fp.position.y;

  for (let i = 0; i < fp.pads.length; i++) {
    const pad = fp.pads[i];
    if (!pad.net || pad.net.number === 0) continue;

    const px = fp.position.x + pad.position.x;
    const py = fp.position.y + pad.position.y;

    // Determine escape direction
    let angle: number;
    if (options.escapeDirection === "outward") {
      angle = Math.atan2(py - cy, px - cx);
    } else if (options.escapeDirection === "inward") {
      angle = Math.atan2(cy - py, cx - px);
    } else {
      // Alternating: even pads go outward, odd go at 45°
      angle = i % 2 === 0
        ? Math.atan2(py - cy, px - cx)
        : Math.atan2(py - cy, px - cx) + Math.PI / 4;
    }

    const viaX = px + Math.cos(angle) * options.dogboneLength;
    const viaY = py + Math.sin(angle) * options.dogboneLength;

    // Trace from pad to via
    segments.push({
      uuid: crypto.randomUUID(),
      start: { x: px, y: py },
      end: { x: viaX, y: viaY },
      width: options.traceWidth,
      layer: options.topLayer,
      net: pad.net.number,
    });

    // Via at the end of the dog-bone
    vias.push({
      uuid: crypto.randomUUID(),
      position: { x: viaX, y: viaY },
      diameter: options.viaDiameter,
      drill: options.viaDrill,
      layers: [options.topLayer, options.innerLayer],
      net: pad.net.number,
      type: "through",
    });
  }

  return { segments, vias };
}

/**
 * Apply BGA fanout results to the PCB.
 */
export function applyBgaFanout(result: { segments: PcbSegment[]; vias: PcbVia[] }) {
  const store = usePcbStore.getState();
  if (!store.data) return;

  store.pushUndo();
  const nd = structuredClone(store.data);
  nd.segments.push(...result.segments);
  nd.vias.push(...result.vias);
  usePcbStore.setState({ data: nd, dirty: true });
}

// ═══════════════════════════════════════════════════════════════
// PUSH/SHOVE PLACEMENT
// ═══════════════════════════════════════════════════════════════

/**
 * Move a footprint while pushing overlapping footprints out of the way.
 * Returns the list of footprints that were moved and their new positions.
 */
export function pushPlaceFootprint(
  data: PcbData,
  movingUuid: string,
  newPosition: PcbPoint,
  clearance: number = 1.0, // mm clearance between footprints
): { uuid: string; position: PcbPoint }[] {
  const moved: { uuid: string; position: PcbPoint }[] = [];
  const moving = data.footprints.find((f) => f.uuid === movingUuid);
  if (!moving) return moved;

  // Get bounding box of the moving footprint at new position
  const movingBbox = getFootprintBbox(moving, newPosition);

  // Check each other footprint for overlap
  for (const fp of data.footprints) {
    if (fp.uuid === movingUuid) continue;
    if (fp.locked) continue;

    const fpBbox = getFootprintBbox(fp, fp.position);

    // Check overlap with clearance
    if (bboxOverlap(movingBbox, fpBbox, clearance)) {
      // Push the footprint away from the moving one
      const dx = fp.position.x - newPosition.x;
      const dy = fp.position.y - newPosition.y;
      const dist = Math.hypot(dx, dy) || 1;
      const pushDist = getOverlapAmount(movingBbox, fpBbox) + clearance;

      moved.push({
        uuid: fp.uuid,
        position: {
          x: fp.position.x + (dx / dist) * pushDist,
          y: fp.position.y + (dy / dist) * pushDist,
        },
      });
    }
  }

  return moved;
}

/**
 * Apply push placement — move the target footprint and all pushed footprints.
 */
export function applyPushPlacement(
  movingUuid: string,
  newPosition: PcbPoint,
  pushedFootprints: { uuid: string; position: PcbPoint }[],
) {
  const store = usePcbStore.getState();
  if (!store.data) return;

  store.pushUndo();
  const nd = structuredClone(store.data);

  // Move the target
  const target = nd.footprints.find((f) => f.uuid === movingUuid);
  if (target) target.position = { ...newPosition };

  // Move pushed footprints
  for (const pushed of pushedFootprints) {
    const fp = nd.footprints.find((f) => f.uuid === pushed.uuid);
    if (fp) fp.position = { ...pushed.position };
  }

  usePcbStore.setState({ data: nd, dirty: true });
}

// ═══════════════════════════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════════════════════════

interface Bbox { minX: number; minY: number; maxX: number; maxY: number }

function getFootprintBbox(fp: PcbFootprint, pos: PcbPoint): Bbox {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const pad of fp.pads) {
    const px = pos.x + pad.position.x;
    const py = pos.y + pad.position.y;
    const hw = pad.size[0] / 2, hh = pad.size[1] / 2;
    minX = Math.min(minX, px - hw); maxX = Math.max(maxX, px + hw);
    minY = Math.min(minY, py - hh); maxY = Math.max(maxY, py + hh);
  }
  if (!isFinite(minX)) {
    return { minX: pos.x - 2, minY: pos.y - 2, maxX: pos.x + 2, maxY: pos.y + 2 };
  }
  return { minX, minY, maxX, maxY };
}

function bboxOverlap(a: Bbox, b: Bbox, margin: number): boolean {
  return !(a.maxX + margin < b.minX || a.minX - margin > b.maxX ||
           a.maxY + margin < b.minY || a.minY - margin > b.maxY);
}

function getOverlapAmount(a: Bbox, b: Bbox): number {
  const overlapX = Math.min(a.maxX, b.maxX) - Math.max(a.minX, b.minX);
  const overlapY = Math.min(a.maxY, b.maxY) - Math.max(a.minY, b.minY);
  return Math.max(0, Math.min(overlapX, overlapY));
}
