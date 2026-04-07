import type { PcbData, PcbPoint, RatsnestLine } from "@/types/pcb";

/**
 * Compute ratsnest lines for all unrouted connections.
 * For each net, finds pads that share the net and determines which
 * pad-to-pad connections are still unrouted (no trace path between them).
 *
 * Uses a simple minimum spanning tree approach: for each net,
 * connect all pads via the shortest unrouted edges.
 */
export function computeRatsnest(data: PcbData): RatsnestLine[] {
  const ratsnest: RatsnestLine[] = [];

  // Group pads by net number
  const netPads = new Map<number, { footprintUuid: string; padNumber: string; position: PcbPoint }[]>();

  for (const fp of data.footprints) {
    for (const pad of fp.pads) {
      if (!pad.net || pad.net.number === 0) continue;
      const netNum = pad.net.number;
      if (!netPads.has(netNum)) netPads.set(netNum, []);
      // Compute absolute pad position
      const absPos: PcbPoint = {
        x: fp.position.x + pad.position.x,
        y: fp.position.y + pad.position.y,
      };
      netPads.get(netNum)!.push({
        footprintUuid: fp.uuid,
        padNumber: pad.number,
        position: absPos,
      });
    }
  }

  // Build connectivity from existing traces + vias
  // For simplicity, use a union-find to group pads connected by trace segments
  const parent = new Map<string, string>();
  const padKey = (fpUuid: string, padNum: string) => `${fpUuid}:${padNum}`;

  function find(key: string): string {
    if (!parent.has(key)) parent.set(key, key);
    let k = key;
    while (parent.get(k) !== k) { k = parent.get(k)!; }
    // Path compression
    let c = key;
    while (c !== k) { const next = parent.get(c)!; parent.set(c, k); c = next; }
    return k;
  }

  function union(a: string, b: string) {
    const ra = find(a), rb = find(b);
    if (ra !== rb) parent.set(ra, rb);
  }

  // For each net, check which pads are connected via trace segments
  // A segment connects two points; if those points are close to pad positions, the pads are connected
  const TOLERANCE = 0.1; // mm

  for (const [netNum, pads] of netPads) {
    if (pads.length < 2) continue;

    // Find segments on this net
    const netSegments = data.segments.filter((s) => s.net === netNum);

    // Build a point-to-pad mapping for this net
    for (const seg of netSegments) {
      // Find pads near segment endpoints
      const startPads = pads.filter((p) =>
        Math.abs(p.position.x - seg.start.x) < TOLERANCE &&
        Math.abs(p.position.y - seg.start.y) < TOLERANCE
      );
      const endPads = pads.filter((p) =>
        Math.abs(p.position.x - seg.end.x) < TOLERANCE &&
        Math.abs(p.position.y - seg.end.y) < TOLERANCE
      );

      // Connect start pad(s) to end pad(s)
      for (const sp of startPads) {
        for (const ep of endPads) {
          union(padKey(sp.footprintUuid, sp.padNumber), padKey(ep.footprintUuid, ep.padNumber));
        }
      }

      // Also connect consecutive segments (endpoint of one to start of another)
      for (const other of netSegments) {
        if (other === seg) continue;
        if (Math.abs(seg.end.x - other.start.x) < TOLERANCE && Math.abs(seg.end.y - other.start.y) < TOLERANCE) {
          // These segments are connected at seg.end/other.start
          const segEndPads = pads.filter((p) =>
            Math.abs(p.position.x - seg.end.x) < TOLERANCE &&
            Math.abs(p.position.y - seg.end.y) < TOLERANCE
          );
          const otherStartPads = pads.filter((p) =>
            Math.abs(p.position.x - other.start.x) < TOLERANCE &&
            Math.abs(p.position.y - other.start.y) < TOLERANCE
          );
          for (const a of segEndPads) {
            for (const b of otherStartPads) {
              union(padKey(a.footprintUuid, a.padNumber), padKey(b.footprintUuid, b.padNumber));
            }
          }
        }
      }
    }

    // Vias connect pads across layers at the same position
    const netVias = data.vias.filter((v) => v.net === netNum);
    for (const via of netVias) {
      const viaPads = pads.filter((p) =>
        Math.abs(p.position.x - via.position.x) < TOLERANCE &&
        Math.abs(p.position.y - via.position.y) < TOLERANCE
      );
      for (let i = 0; i < viaPads.length; i++) {
        for (let j = i + 1; j < viaPads.length; j++) {
          union(padKey(viaPads[i].footprintUuid, viaPads[i].padNumber),
                padKey(viaPads[j].footprintUuid, viaPads[j].padNumber));
        }
      }
    }

    // Find unconnected pad groups — generate ratsnest between groups
    const groups = new Map<string, typeof pads>();
    for (const pad of pads) {
      const root = find(padKey(pad.footprintUuid, pad.padNumber));
      if (!groups.has(root)) groups.set(root, []);
      groups.get(root)!.push(pad);
    }

    if (groups.size <= 1) continue; // All connected

    // Connect groups via shortest edges (simple MST: Prim's on group centroids)
    const groupArr = Array.from(groups.values());
    const connected = new Set<number>([0]);
    const remaining = new Set<number>();
    for (let i = 1; i < groupArr.length; i++) remaining.add(i);

    while (remaining.size > 0) {
      let bestDist = Infinity;
      let bestB = 0;
      let bestPadA = groupArr[0][0], bestPadB = groupArr[0][0];

      for (const ci of connected) {
        for (const ri of remaining) {
          for (const pa of groupArr[ci]) {
            for (const pb of groupArr[ri]) {
              const dx = pa.position.x - pb.position.x;
              const dy = pa.position.y - pb.position.y;
              const dist = dx * dx + dy * dy;
              if (dist < bestDist) {
                bestDist = dist;
                bestB = ri;
                bestPadA = pa; bestPadB = pb;
              }
            }
          }
        }
      }

      ratsnest.push({
        padA: { footprintUuid: bestPadA.footprintUuid, padNumber: bestPadA.padNumber },
        padB: { footprintUuid: bestPadB.footprintUuid, padNumber: bestPadB.padNumber },
        net: netNum,
      });

      connected.add(bestB);
      remaining.delete(bestB);
    }
  }

  return ratsnest;
}

/**
 * Get the absolute position of a pad given a ratsnest line endpoint.
 */
export function getPadPosition(
  data: PcbData,
  ref: { footprintUuid: string; padNumber: string },
): PcbPoint | null {
  const fp = data.footprints.find((f) => f.uuid === ref.footprintUuid);
  if (!fp) return null;
  const pad = fp.pads.find((p) => p.number === ref.padNumber);
  if (!pad) return null;
  return {
    x: fp.position.x + pad.position.x,
    y: fp.position.y + pad.position.y,
  };
}
