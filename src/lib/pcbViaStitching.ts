/**
 * Via Stitching — place arrays of ground/power vias for thermal and EMI management.
 */

import type { PcbPoint, PcbVia, PcbLayerId } from "@/types/pcb";
import { usePcbStore } from "@/stores/pcb";

export interface ViaStitchOptions {
  net: number;
  diameter: number;
  drill: number;
  spacing: number; // Center-to-center spacing
  layers: [PcbLayerId, PcbLayerId];
}

/**
 * Place a rectangular grid of vias within a bounding box.
 */
export function placeViaGrid(
  topLeft: PcbPoint,
  bottomRight: PcbPoint,
  options: ViaStitchOptions,
): PcbVia[] {
  const vias: PcbVia[] = [];
  const { spacing, diameter, drill, net, layers } = options;

  const startX = topLeft.x + spacing / 2;
  const startY = topLeft.y + spacing / 2;
  const endX = bottomRight.x;
  const endY = bottomRight.y;

  for (let x = startX; x <= endX; x += spacing) {
    for (let y = startY; y <= endY; y += spacing) {
      vias.push({
        uuid: crypto.randomUUID(),
        position: { x, y },
        diameter,
        drill,
        layers,
        net,
        type: "through",
      });
    }
  }

  return vias;
}

/**
 * Place vias along a zone outline perimeter (fence stitching).
 */
export function placeViaFence(
  outline: PcbPoint[],
  options: ViaStitchOptions,
): PcbVia[] {
  const vias: PcbVia[] = [];
  const { spacing, diameter, drill, net, layers } = options;

  for (let i = 0; i < outline.length; i++) {
    const start = outline[i];
    const end = outline[(i + 1) % outline.length];
    const dx = end.x - start.x;
    const dy = end.y - start.y;
    const edgeLen = Math.hypot(dx, dy);
    if (edgeLen < spacing) continue;

    const count = Math.floor(edgeLen / spacing);
    for (let j = 0; j <= count; j++) {
      const t = j / count;
      vias.push({
        uuid: crypto.randomUUID(),
        position: { x: start.x + dx * t, y: start.y + dy * t },
        diameter, drill, layers, net, type: "through",
      });
    }
  }

  return vias;
}

/**
 * Apply via stitching to the current PCB data.
 */
export function applyViaStitching(vias: PcbVia[]) {
  const store = usePcbStore.getState();
  if (!store.data || vias.length === 0) return;

  store.pushUndo();
  const nd = structuredClone(store.data);
  nd.vias.push(...vias);
  usePcbStore.setState({ data: nd, dirty: true });
}
