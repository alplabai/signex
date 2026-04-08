import type { PcbData, PcbPoint, PcbLayerId, DrcViolation } from "@/types/pcb";
import { computeRatsnest } from "./pcbRatsnest";

/**
 * PCB Design Rule Check — comprehensive violation detection.
 *
 * Checks (matching Altium categories):
 * 1. Unrouted nets
 * 2. Minimum trace width
 * 3. Minimum via size
 * 4. Trace-to-trace clearance
 * 5. Board outline clearance
 * 6. Hole-to-hole clearance
 * 7. Annular ring minimum
 * 8. Solder mask sliver
 * 9. Trace-to-pad clearance
 * 10. Via-to-pad clearance
 * 11. Via-to-trace clearance
 * 12. Short circuit detection
 * 13. Silk-to-mask clearance
 * 14. Minimum drill size
 * 15. Pad-to-pad clearance
 */
export function runPcbDrc(data: PcbData): DrcViolation[] {
  const violations: DrcViolation[] = [];
  const rules = extractRules(data);

  // 1. Unrouted nets
  checkUnrouted(data, violations);

  // 2. Minimum trace width
  checkMinTraceWidth(data, rules.minTraceWidth, violations);

  // 3. Minimum via size
  checkMinViaSize(data, rules.minViaDiameter, violations);

  // 4. Minimum drill size
  checkMinDrillSize(data, rules.minViaDrill, violations);

  // 5. Trace-to-trace clearance
  checkTraceClearance(data, rules.clearance, violations);

  // 6. Board outline clearance
  checkBoardOutlineClearance(data, rules.clearance, violations);

  // 7. Hole-to-hole clearance
  checkHoleToHole(data, rules.holeClearance, violations);

  // 8. Annular ring minimum
  checkAnnularRing(data, rules.minAnnularRing, violations);

  // 9. Solder mask sliver
  checkSolderMaskSliver(data, rules.minMaskSliver, violations);

  // 10. Trace-to-pad clearance
  checkTraceToPad(data, rules.clearance, violations);

  // 11. Via-to-pad clearance
  checkViaToPad(data, rules.clearance, violations);

  // 12. Via-to-trace clearance
  checkViaToTrace(data, rules.clearance, violations);

  // 13. Short circuit detection
  checkShortCircuits(data, violations);

  // 14. Pad-to-pad clearance
  checkPadToPad(data, rules.clearance, violations);

  // 15. Silk-to-mask clearance
  checkSilkToMask(data, rules.silkToMaskClearance, violations);

  return violations;
}

// --- Rule extraction ---

interface DrcRules {
  clearance: number;
  minTraceWidth: number;
  minViaDiameter: number;
  minViaDrill: number;
  holeClearance: number;
  minAnnularRing: number;
  minMaskSliver: number;
  silkToMaskClearance: number;
}

function extractRules(data: PcbData): DrcRules {
  const find = (type: string) => data.designRules.find((r) => r.type === type && r.enabled);
  return {
    clearance: find("clearance")?.min || 0.2,
    minTraceWidth: find("trace_width")?.min || 0.15,
    minViaDiameter: find("via_size")?.min || 0.4,
    minViaDrill: 0.2,
    holeClearance: 0.25,
    minAnnularRing: 0.125,
    minMaskSliver: 0.1,
    silkToMaskClearance: 0.05,
  };
}

// --- Individual checks ---

function checkUnrouted(data: PcbData, violations: DrcViolation[]) {
  const ratsnest = computeRatsnest(data);
  for (const line of ratsnest) {
    const fpA = data.footprints.find((f) => f.uuid === line.padA.footprintUuid);
    const fpB = data.footprints.find((f) => f.uuid === line.padB.footprintUuid);
    const refA = fpA ? `${fpA.reference}:${line.padA.padNumber}` : line.padA.padNumber;
    const refB = fpB ? `${fpB.reference}:${line.padB.padNumber}` : line.padB.padNumber;
    const netName = data.nets.find((n) => n.number === line.net)?.name || `Net ${line.net}`;
    const pos = fpA ? { x: fpA.position.x, y: fpA.position.y } : { x: 0, y: 0 };
    violations.push({
      uuid: crypto.randomUUID(), type: "unrouted", severity: "error",
      message: `Unrouted: ${refA} to ${refB} (${netName})`, position: pos,
      objects: [line.padA.footprintUuid, line.padB.footprintUuid],
    });
  }
}

function checkMinTraceWidth(data: PcbData, minWidth: number, violations: DrcViolation[]) {
  for (const seg of data.segments) {
    if (seg.width < minWidth) {
      violations.push({
        uuid: crypto.randomUUID(), type: "minimum_width", severity: "error",
        message: `Trace width ${seg.width.toFixed(3)}mm < min ${minWidth}mm`,
        position: midpoint(seg.start, seg.end), layer: seg.layer, objects: [seg.uuid],
      });
    }
  }
}

function checkMinViaSize(data: PcbData, minDiameter: number, violations: DrcViolation[]) {
  for (const via of data.vias) {
    if (via.diameter < minDiameter) {
      violations.push({
        uuid: crypto.randomUUID(), type: "drill_size", severity: "error",
        message: `Via diameter ${via.diameter.toFixed(3)}mm < min ${minDiameter}mm`,
        position: via.position, objects: [via.uuid],
      });
    }
  }
}

function checkMinDrillSize(data: PcbData, minDrill: number, violations: DrcViolation[]) {
  for (const via of data.vias) {
    if (via.drill < minDrill) {
      violations.push({
        uuid: crypto.randomUUID(), type: "drill_size", severity: "error",
        message: `Via drill ${via.drill.toFixed(3)}mm < min ${minDrill}mm`,
        position: via.position, objects: [via.uuid],
      });
    }
  }
  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      if (pad.drill && pad.drill.diameter < minDrill) {
        violations.push({
          uuid: crypto.randomUUID(), type: "drill_size", severity: "error",
          message: `${fp.reference}:${pad.number} drill ${pad.drill.diameter.toFixed(3)}mm < min ${minDrill}mm`,
          position: { x: fp.position.x + pad.position.x, y: fp.position.y + pad.position.y },
          objects: [fp.uuid],
        });
      }
    }
  }
}

function checkTraceClearance(data: PcbData, minClearance: number, violations: DrcViolation[]) {
  for (let i = 0; i < data.segments.length; i++) {
    for (let j = i + 1; j < data.segments.length; j++) {
      const a = data.segments[i], b = data.segments[j];
      if (a.layer !== b.layer) continue;
      if (a.net === b.net && a.net !== 0) continue;
      const dist = segDist(a.start, a.end, b.start, b.end);
      const required = minClearance + (a.width + b.width) / 2;
      if (dist < required) {
        const actual = Math.max(0, dist - (a.width + b.width) / 2);
        violations.push({
          uuid: crypto.randomUUID(), type: "clearance", severity: "error",
          message: `Trace clearance ${actual.toFixed(3)}mm < min ${minClearance}mm`,
          position: midpoint(a.start, b.start), layer: a.layer, objects: [a.uuid, b.uuid],
        });
      }
    }
  }
}

function checkBoardOutlineClearance(data: PcbData, minClearance: number, violations: DrcViolation[]) {
  if (data.board.outline.length < 3) return;
  for (const seg of data.segments) {
    for (let i = 0; i < data.board.outline.length; i++) {
      const j = (i + 1) % data.board.outline.length;
      const dist = segDist(seg.start, seg.end, data.board.outline[i], data.board.outline[j]);
      if (dist < minClearance + seg.width / 2) {
        violations.push({
          uuid: crypto.randomUUID(), type: "board_outline", severity: "warning",
          message: `Trace ${dist.toFixed(3)}mm from board edge (min ${minClearance}mm)`,
          position: seg.start, layer: seg.layer, objects: [seg.uuid],
        });
        break;
      }
    }
  }
}

function checkHoleToHole(data: PcbData, minClearance: number, violations: DrcViolation[]) {
  // Collect all drill holes: vias + through-hole pads
  const holes: { pos: PcbPoint; diameter: number; uuid: string; label: string }[] = [];
  for (const via of data.vias) {
    holes.push({ pos: via.position, diameter: via.drill, uuid: via.uuid, label: "Via" });
  }
  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      if (pad.drill) {
        holes.push({
          pos: { x: fp.position.x + pad.position.x, y: fp.position.y + pad.position.y },
          diameter: pad.drill.diameter, uuid: fp.uuid,
          label: `${fp.reference}:${pad.number}`,
        });
      }
    }
  }

  for (let i = 0; i < holes.length; i++) {
    for (let j = i + 1; j < holes.length; j++) {
      const a = holes[i], b = holes[j];
      const dist = Math.hypot(a.pos.x - b.pos.x, a.pos.y - b.pos.y);
      const required = (a.diameter + b.diameter) / 2 + minClearance;
      if (dist < required) {
        const actual = Math.max(0, dist - (a.diameter + b.diameter) / 2);
        violations.push({
          uuid: crypto.randomUUID(), type: "clearance", severity: "error",
          message: `Hole-to-hole ${actual.toFixed(3)}mm: ${a.label} ↔ ${b.label} (min ${minClearance}mm)`,
          position: midpoint(a.pos, b.pos), objects: [a.uuid, b.uuid],
        });
      }
    }
  }
}

function checkAnnularRing(data: PcbData, minRing: number, violations: DrcViolation[]) {
  for (const via of data.vias) {
    const ring = (via.diameter - via.drill) / 2;
    if (ring < minRing) {
      violations.push({
        uuid: crypto.randomUUID(), type: "annular_ring", severity: "error",
        message: `Via annular ring ${ring.toFixed(3)}mm < min ${minRing}mm`,
        position: via.position, objects: [via.uuid],
      });
    }
  }
  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      if (pad.drill && pad.type === "thru_hole") {
        const padR = Math.min(pad.size[0], pad.size[1]) / 2;
        const ring = padR - pad.drill.diameter / 2;
        if (ring < minRing) {
          violations.push({
            uuid: crypto.randomUUID(), type: "annular_ring", severity: "error",
            message: `${fp.reference}:${pad.number} annular ring ${ring.toFixed(3)}mm < min ${minRing}mm`,
            position: { x: fp.position.x + pad.position.x, y: fp.position.y + pad.position.y },
            objects: [fp.uuid],
          });
        }
      }
    }
  }
}

function checkSolderMaskSliver(data: PcbData, minSliver: number, violations: DrcViolation[]) {
  // Check if solder mask openings between adjacent pads are too narrow
  const padPositions: { pos: PcbPoint; size: number; uuid: string; ref: string }[] = [];
  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      padPositions.push({
        pos: { x: fp.position.x + pad.position.x, y: fp.position.y + pad.position.y },
        size: Math.max(pad.size[0], pad.size[1]),
        uuid: fp.uuid, ref: `${fp.reference}:${pad.number}`,
      });
    }
  }

  for (let i = 0; i < padPositions.length; i++) {
    for (let j = i + 1; j < padPositions.length; j++) {
      const a = padPositions[i], b = padPositions[j];
      const dist = Math.hypot(a.pos.x - b.pos.x, a.pos.y - b.pos.y);
      const gap = dist - (a.size + b.size) / 2;
      // Mask expansion typically 0.05mm per side
      const maskGap = gap - 0.1;
      if (maskGap > 0 && maskGap < minSliver) {
        violations.push({
          uuid: crypto.randomUUID(), type: "clearance", severity: "warning",
          message: `Solder mask sliver ${maskGap.toFixed(3)}mm between ${a.ref} and ${b.ref} (min ${minSliver}mm)`,
          position: midpoint(a.pos, b.pos), objects: [a.uuid, b.uuid],
        });
      }
    }
  }
}

function checkTraceToPad(data: PcbData, minClearance: number, violations: DrcViolation[]) {
  for (const seg of data.segments) {
    for (const fp of data.footprints) {
      for (const pad of fp.pads) {
        if (!pad.layers.includes(seg.layer as PcbLayerId) && !pad.layers.includes("*.Cu" as PcbLayerId)) continue;
        const padNet = pad.net?.number || 0;
        if (padNet === seg.net && padNet !== 0) continue;

        const px = fp.position.x + pad.position.x;
        const py = fp.position.y + pad.position.y;
        const padR = Math.max(pad.size[0], pad.size[1]) / 2;
        const dist = ptSegDist({ x: px, y: py }, seg.start, seg.end);
        const required = padR + seg.width / 2 + minClearance;

        if (dist < required) {
          const actual = Math.max(0, dist - padR - seg.width / 2);
          violations.push({
            uuid: crypto.randomUUID(), type: "clearance", severity: "error",
            message: `Trace-to-pad ${actual.toFixed(3)}mm: ${fp.reference}:${pad.number} (min ${minClearance}mm)`,
            position: { x: px, y: py }, layer: seg.layer, objects: [seg.uuid, fp.uuid],
          });
        }
      }
    }
  }
}

function checkViaToPad(data: PcbData, minClearance: number, violations: DrcViolation[]) {
  for (const via of data.vias) {
    for (const fp of data.footprints) {
      for (const pad of fp.pads) {
        const padNet = pad.net?.number || 0;
        if (padNet === via.net && padNet !== 0) continue;

        const px = fp.position.x + pad.position.x;
        const py = fp.position.y + pad.position.y;
        const padR = Math.max(pad.size[0], pad.size[1]) / 2;
        const dist = Math.hypot(via.position.x - px, via.position.y - py);
        const required = padR + via.diameter / 2 + minClearance;

        if (dist < required) {
          violations.push({
            uuid: crypto.randomUUID(), type: "clearance", severity: "error",
            message: `Via-to-pad ${(dist - padR - via.diameter / 2).toFixed(3)}mm: ${fp.reference}:${pad.number}`,
            position: via.position, objects: [via.uuid, fp.uuid],
          });
        }
      }
    }
  }
}

function checkViaToTrace(data: PcbData, minClearance: number, violations: DrcViolation[]) {
  for (const via of data.vias) {
    for (const seg of data.segments) {
      if (seg.net === via.net && seg.net !== 0) continue;
      const dist = ptSegDist(via.position, seg.start, seg.end);
      const required = via.diameter / 2 + seg.width / 2 + minClearance;
      if (dist < required) {
        violations.push({
          uuid: crypto.randomUUID(), type: "clearance", severity: "error",
          message: `Via-to-trace ${(dist - via.diameter / 2 - seg.width / 2).toFixed(3)}mm`,
          position: via.position, layer: seg.layer, objects: [via.uuid, seg.uuid],
        });
      }
    }
  }
}

function checkShortCircuits(data: PcbData, violations: DrcViolation[]) {
  // Check if traces from different nets touch
  for (let i = 0; i < data.segments.length; i++) {
    for (let j = i + 1; j < data.segments.length; j++) {
      const a = data.segments[i], b = data.segments[j];
      if (a.layer !== b.layer) continue;
      if (a.net === b.net) continue;
      if (a.net === 0 || b.net === 0) continue;

      const dist = segDist(a.start, a.end, b.start, b.end);
      if (dist < (a.width + b.width) / 2) {
        const netA = data.nets.find((n) => n.number === a.net)?.name || `Net ${a.net}`;
        const netB = data.nets.find((n) => n.number === b.net)?.name || `Net ${b.net}`;
        violations.push({
          uuid: crypto.randomUUID(), type: "short_circuit", severity: "error",
          message: `Short circuit: ${netA} ↔ ${netB}`,
          position: midpoint(a.start, b.start), layer: a.layer, objects: [a.uuid, b.uuid],
        });
      }
    }
  }
}

function checkPadToPad(data: PcbData, minClearance: number, violations: DrcViolation[]) {
  const pads: { pos: PcbPoint; size: number; net: number; uuid: string; ref: string; layers: string[] }[] = [];
  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      pads.push({
        pos: { x: fp.position.x + pad.position.x, y: fp.position.y + pad.position.y },
        size: Math.max(pad.size[0], pad.size[1]),
        net: pad.net?.number || 0,
        uuid: fp.uuid,
        ref: `${fp.reference}:${pad.number}`,
        layers: pad.layers,
      });
    }
  }

  for (let i = 0; i < pads.length; i++) {
    for (let j = i + 1; j < pads.length; j++) {
      const a = pads[i], b = pads[j];
      if (a.net === b.net && a.net !== 0) continue;
      // Check if they share any layer
      const sharedLayer = a.layers.some((l) => b.layers.includes(l) || l === "*.Cu" || b.layers.includes("*.Cu"));
      if (!sharedLayer) continue;

      const dist = Math.hypot(a.pos.x - b.pos.x, a.pos.y - b.pos.y);
      const required = (a.size + b.size) / 2 + minClearance;
      if (dist < required) {
        const actual = Math.max(0, dist - (a.size + b.size) / 2);
        violations.push({
          uuid: crypto.randomUUID(), type: "clearance", severity: "error",
          message: `Pad-to-pad ${actual.toFixed(3)}mm: ${a.ref} ↔ ${b.ref} (min ${minClearance}mm)`,
          position: midpoint(a.pos, b.pos), objects: [a.uuid, b.uuid],
        });
      }
    }
  }
}

function checkSilkToMask(data: PcbData, minClearance: number, violations: DrcViolation[]) {
  // Check if silkscreen overlaps solder mask openings (pad locations)
  for (const fp of data.footprints) {
    for (const g of fp.graphics) {
      if (g.type !== "text" && g.type !== "line") continue;
      const isFrontSilk = g.layer === "F.SilkS";
      const isBackSilk = g.layer === "B.SilkS";
      if (!isFrontSilk && !isBackSilk) continue;

      const maskLayer = isFrontSilk ? "F.Mask" : "B.Mask";

      for (const pad of fp.pads) {
        if (!pad.layers.includes(maskLayer as PcbLayerId)) continue;
        const px = fp.position.x + pad.position.x;
        const py = fp.position.y + pad.position.y;
        const padR = Math.max(pad.size[0], pad.size[1]) / 2 + 0.05; // mask expansion

        if (g.type === "line" && g.start && g.end) {
          const gx1 = fp.position.x + g.start.x;
          const gy1 = fp.position.y + g.start.y;
          const gx2 = fp.position.x + g.end.x;
          const gy2 = fp.position.y + g.end.y;
          const dist = ptSegDist({ x: px, y: py }, { x: gx1, y: gy1 }, { x: gx2, y: gy2 });
          if (dist < padR + minClearance) {
            violations.push({
              uuid: crypto.randomUUID(), type: "clearance", severity: "warning",
              message: `Silk-to-mask ${(dist - padR).toFixed(3)}mm at ${fp.reference}`,
              position: { x: px, y: py }, layer: g.layer, objects: [fp.uuid],
            });
          }
        }
      }
    }
  }
}

// --- Geometry helpers ---

function midpoint(a: PcbPoint, b: PcbPoint): PcbPoint {
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
}

function ptSegDist(p: PcbPoint, a: PcbPoint, b: PcbPoint): number {
  const dx = b.x - a.x, dy = b.y - a.y;
  const lenSq = dx * dx + dy * dy;
  if (lenSq < 1e-12) return Math.hypot(p.x - a.x, p.y - a.y);
  let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

function segsCross(a1: PcbPoint, a2: PcbPoint, b1: PcbPoint, b2: PcbPoint): boolean {
  const cross = (o: PcbPoint, p: PcbPoint, q: PcbPoint) =>
    (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
  const d1 = cross(b1, b2, a1), d2 = cross(b1, b2, a2);
  const d3 = cross(a1, a2, b1), d4 = cross(a1, a2, b2);
  return ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) &&
         ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0));
}

function segDist(a1: PcbPoint, a2: PcbPoint, b1: PcbPoint, b2: PcbPoint): number {
  if (segsCross(a1, a2, b1, b2)) return 0;
  return Math.min(
    ptSegDist(a1, b1, b2), ptSegDist(a2, b1, b2),
    ptSegDist(b1, a1, a2), ptSegDist(b2, a1, a2),
  );
}
