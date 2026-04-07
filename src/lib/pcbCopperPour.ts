import type { PcbData, PcbPoint, PcbZone } from "@/types/pcb";

/**
 * Copper pour fill computation.
 *
 * For each zone, computes the filled polygon by subtracting clearance
 * areas around pads, traces, vias, and other zones with higher priority.
 * Generates thermal relief spokes for pads on the same net.
 *
 * This is a simplified implementation — production EDA tools use
 * Clipper library for polygon boolean operations.
 */
export function fillZones(data: PcbData): PcbZone[] {
  const filledZones: PcbZone[] = [];

  // Sort by priority (lower number = lower priority = filled first)
  const sorted = [...data.zones].sort((a, b) => a.priority - b.priority);

  for (const zone of sorted) {
    if (zone.outline.length < 3) {
      filledZones.push({ ...zone, filled: [] });
      continue;
    }

    // Start with the zone outline as the fill
    const obstacles = collectObstacles(data, zone);

    // For simplicity, use the outline directly and mark obstacles
    // A real implementation would use polygon clipping (Clipper/Martinez)
    const filled = computeSimpleFill(zone, obstacles);
    filledZones.push({ ...zone, filled });
  }

  return filledZones;
}

interface Obstacle {
  type: "pad" | "trace" | "via" | "zone" | "keepout";
  polygon: PcbPoint[];
  net: number;
  sameNet: boolean;
}

function collectObstacles(data: PcbData, zone: PcbZone): Obstacle[] {
  const obstacles: Obstacle[] = [];
  const clearance = zone.clearance;

  // Pads
  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      if (!pad.layers.includes(zone.layer) && !pad.layers.includes("*.Cu")) continue;

      const px = fp.position.x + pad.position.x;
      const py = fp.position.y + pad.position.y;
      const padNet = pad.net?.number || 0;
      const sameNet = padNet === zone.net && padNet !== 0;
      const gap = sameNet ? 0 : clearance;
      const hw = pad.size[0] / 2 + gap;
      const hh = pad.size[1] / 2 + gap;

      obstacles.push({
        type: "pad",
        polygon: [
          { x: px - hw, y: py - hh },
          { x: px + hw, y: py - hh },
          { x: px + hw, y: py + hh },
          { x: px - hw, y: py + hh },
        ],
        net: padNet,
        sameNet,
      });
    }
  }

  // Trace segments
  for (const seg of data.segments) {
    if (seg.layer !== zone.layer) continue;
    const sameNet = seg.net === zone.net && seg.net !== 0;
    if (sameNet) continue; // Same-net traces connect to pour

    const hw = seg.width / 2 + clearance;
    const dx = seg.end.x - seg.start.x;
    const dy = seg.end.y - seg.start.y;
    const len = Math.hypot(dx, dy) || 1;
    const nx = -dy / len * hw;
    const ny = dx / len * hw;

    obstacles.push({
      type: "trace",
      polygon: [
        { x: seg.start.x + nx, y: seg.start.y + ny },
        { x: seg.end.x + nx, y: seg.end.y + ny },
        { x: seg.end.x - nx, y: seg.end.y - ny },
        { x: seg.start.x - nx, y: seg.start.y - ny },
      ],
      net: seg.net,
      sameNet: false,
    });
  }

  // Vias
  for (const via of data.vias) {
    const sameNet = via.net === zone.net && via.net !== 0;
    const gap = sameNet ? 0 : clearance;
    const r = via.diameter / 2 + gap;

    // Approximate circle as octagon
    const points: PcbPoint[] = [];
    for (let i = 0; i < 8; i++) {
      const angle = (i * Math.PI * 2) / 8;
      points.push({
        x: via.position.x + Math.cos(angle) * r,
        y: via.position.y + Math.sin(angle) * r,
      });
    }

    obstacles.push({ type: "via", polygon: points, net: via.net, sameNet });
  }

  // Keepout zones
  for (const other of data.zones) {
    if (other.uuid === zone.uuid) continue;
    if (other.layer !== zone.layer) continue;
    if (other.keepout?.copperpour) {
      obstacles.push({
        type: "keepout",
        polygon: other.outline,
        net: 0,
        sameNet: false,
      });
    }
  }

  return obstacles;
}

function computeSimpleFill(zone: PcbZone, obstacles: Obstacle[]): PcbPoint[][] {
  // Simplified: return the zone outline as the fill polygon
  // In production, we'd subtract obstacle polygons from the zone polygon
  // using Clipper library (polygon boolean difference)
  //
  // For now, return the outline itself — the renderer will draw the
  // filled zone, and obstacles will be visually on top

  if (obstacles.length === 0) {
    return [zone.outline];
  }

  // Return the outline as a single filled polygon
  // The renderer handles obstacle overlap by drawing pads/traces on top
  return [zone.outline];
}

/**
 * Generate thermal relief spokes for a pad connected to a zone.
 * Returns 4 spoke line segments (cross pattern).
 */
export function generateThermalRelief(
  padX: number, padY: number,
  padW: number, padH: number,
  spokeWidth: number,
  gap: number,
): { start: PcbPoint; end: PcbPoint }[] {
  const outerR = Math.max(padW, padH) / 2 + gap + 1.0; // Extend spokes into pour
  const hw = spokeWidth / 2;

  return [
    // Top spoke
    { start: { x: padX - hw, y: padY - padH / 2 - gap }, end: { x: padX + hw, y: padY - outerR } },
    // Bottom spoke
    { start: { x: padX - hw, y: padY + padH / 2 + gap }, end: { x: padX + hw, y: padY + outerR } },
    // Left spoke
    { start: { x: padX - padW / 2 - gap, y: padY - hw }, end: { x: padX - outerR, y: padY + hw } },
    // Right spoke
    { start: { x: padX + padW / 2 + gap, y: padY - hw }, end: { x: padX + outerR, y: padY + hw } },
  ];
}
