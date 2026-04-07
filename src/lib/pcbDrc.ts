import type { PcbData, PcbPoint, DrcViolation } from "@/types/pcb";
import { computeRatsnest } from "./pcbRatsnest";

/**
 * Run Design Rule Check on a PCB.
 * Returns a list of violations.
 */
export function runPcbDrc(data: PcbData): DrcViolation[] {
  const violations: DrcViolation[] = [];

  // Get default rules
  const clearanceRule = data.designRules.find((r) => r.type === "clearance" && r.enabled);
  const minClearance = clearanceRule?.min || 0.2;

  const widthRule = data.designRules.find((r) => r.type === "trace_width" && r.enabled);
  const minWidth = widthRule?.min || 0.15;

  const viaRule = data.designRules.find((r) => r.type === "via_size" && r.enabled);
  const minViaDiameter = viaRule?.min || 0.4;

  // 1. Unrouted nets
  const ratsnest = computeRatsnest(data);
  if (ratsnest.length > 0) {
    for (const line of ratsnest) {
      const fpA = data.footprints.find((f) => f.uuid === line.padA.footprintUuid);
      const fpB = data.footprints.find((f) => f.uuid === line.padB.footprintUuid);
      const refA = fpA ? `${fpA.reference}:${line.padA.padNumber}` : line.padA.padNumber;
      const refB = fpB ? `${fpB.reference}:${line.padB.padNumber}` : line.padB.padNumber;
      const netName = data.nets.find((n) => n.number === line.net)?.name || `Net ${line.net}`;
      const pos = fpA ? { x: fpA.position.x, y: fpA.position.y } : { x: 0, y: 0 };
      violations.push({
        uuid: crypto.randomUUID(),
        type: "unrouted",
        severity: "error",
        message: `Unrouted: ${refA} to ${refB} (${netName})`,
        position: pos,
        objects: [line.padA.footprintUuid, line.padB.footprintUuid],
      });
    }
  }

  // 2. Minimum trace width
  for (const seg of data.segments) {
    if (seg.width < minWidth) {
      violations.push({
        uuid: crypto.randomUUID(),
        type: "minimum_width",
        severity: "error",
        message: `Trace width ${seg.width.toFixed(3)}mm below minimum ${minWidth}mm`,
        position: { x: (seg.start.x + seg.end.x) / 2, y: (seg.start.y + seg.end.y) / 2 },
        layer: seg.layer,
        objects: [seg.uuid],
      });
    }
  }

  // 3. Minimum via size
  for (const via of data.vias) {
    if (via.diameter < minViaDiameter) {
      violations.push({
        uuid: crypto.randomUUID(),
        type: "drill_size",
        severity: "error",
        message: `Via diameter ${via.diameter.toFixed(3)}mm below minimum ${minViaDiameter}mm`,
        position: via.position,
        objects: [via.uuid],
      });
    }
  }

  // 4. Trace-to-trace clearance (same layer, different nets)
  for (let i = 0; i < data.segments.length; i++) {
    for (let j = i + 1; j < data.segments.length; j++) {
      const a = data.segments[i];
      const b = data.segments[j];
      if (a.layer !== b.layer) continue;
      if (a.net === b.net && a.net !== 0) continue;

      const dist = segmentToSegmentDist(a.start, a.end, b.start, b.end);
      const required = minClearance + (a.width + b.width) / 2;
      if (dist < required) {
        violations.push({
          uuid: crypto.randomUUID(),
          type: "clearance",
          severity: "error",
          message: `Clearance violation: ${(dist - (a.width + b.width) / 2).toFixed(3)}mm (min ${minClearance}mm)`,
          position: { x: (a.start.x + b.start.x) / 2, y: (a.start.y + b.start.y) / 2 },
          layer: a.layer,
          objects: [a.uuid, b.uuid],
        });
      }
    }
  }

  // 5. Board outline clearance
  if (data.board.outline.length >= 3) {
    for (const seg of data.segments) {
      for (let i = 0; i < data.board.outline.length; i++) {
        const j = (i + 1) % data.board.outline.length;
        const edgeStart = data.board.outline[i];
        const edgeEnd = data.board.outline[j];
        const dist = segmentToSegmentDist(seg.start, seg.end, edgeStart, edgeEnd);
        if (dist < minClearance + seg.width / 2) {
          violations.push({
            uuid: crypto.randomUUID(),
            type: "board_outline",
            severity: "warning",
            message: `Trace too close to board edge: ${dist.toFixed(3)}mm`,
            position: seg.start,
            layer: seg.layer,
            objects: [seg.uuid],
          });
          break;
        }
      }
    }
  }

  return violations;
}

// --- Geometry helpers ---

function pointToSegmentDist(p: PcbPoint, a: PcbPoint, b: PcbPoint): number {
  const dx = b.x - a.x, dy = b.y - a.y;
  const lenSq = dx * dx + dy * dy;
  if (lenSq === 0) return Math.hypot(p.x - a.x, p.y - a.y);
  let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

function segmentToSegmentDist(a1: PcbPoint, a2: PcbPoint, b1: PcbPoint, b2: PcbPoint): number {
  // Approximate: check endpoints of each segment against the other
  return Math.min(
    pointToSegmentDist(a1, b1, b2),
    pointToSegmentDist(a2, b1, b2),
    pointToSegmentDist(b1, a1, a2),
    pointToSegmentDist(b2, a1, a2),
  );
}
