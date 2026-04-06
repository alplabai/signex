import type { SchematicData, SchPoint, SchSymbol } from "@/types";

export interface HitResult {
  type: "symbol" | "wire" | "wireEndpoint" | "junction" | "label" | "noConnect" | "textNote" | "bus" | "busEntry" | "childSheet" | "drawing";
  uuid: string;
  endpoint?: "start" | "end"; // Only for wireEndpoint
}

interface Box {
  minX: number; minY: number;
  maxX: number; maxY: number;
}

function symToSch(lx: number, ly: number, sym: SchSymbol): SchPoint {
  const y = -ly; // Flip Y
  const rad = -(sym.rotation * Math.PI) / 180;
  const cos = Math.cos(rad), sin = Math.sin(rad);
  let rx = lx * cos - y * sin;
  let ry = lx * sin + y * cos;
  if (sym.mirror_x) ry = -ry;
  if (sym.mirror_y) rx = -rx;
  return { x: sym.position.x + rx, y: sym.position.y + ry };
}

function dist(a: SchPoint, b: SchPoint): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

function distToSegment(p: SchPoint, a: SchPoint, b: SchPoint): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const lenSq = dx * dx + dy * dy;
  if (lenSq === 0) return dist(p, a);

  let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / lenSq;
  t = Math.max(0, Math.min(1, t));

  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/**
 * Hit test at world coordinates. Returns the topmost element under the cursor.
 * Priority: junctions → noConnects → symbols → wires → buses → labels → textNotes → busEntries
 */
export function hitTest(
  data: SchematicData,
  worldX: number,
  worldY: number,
  tolerance: number = 2.0
): HitResult | null {
  const p: SchPoint = { x: worldX, y: worldY };

  // Junctions first (small targets)
  for (const j of data.junctions) {
    if (dist(p, j.position) < tolerance * 0.5) {
      return { type: "junction", uuid: j.uuid };
    }
  }

  // No-connect markers (X shape, ~1.4 unit span)
  for (const nc of data.no_connects) {
    if (dist(p, nc.position) < tolerance * 0.6) {
      return { type: "noConnect", uuid: nc.uuid };
    }
  }

  // Symbols — check tight transformed bounding box (including power symbols)
  for (const sym of data.symbols) {
    const lib = data.lib_symbols[sym.lib_id];
    if (!lib) continue;

    // Calculate local-space bounds from body graphics only (not default +-4)
    let lMinX = Infinity, lMaxX = -Infinity, lMinY = Infinity, lMaxY = -Infinity;
    for (const g of lib.graphics) {
      if (g.type === "Rectangle") {
        lMinX = Math.min(lMinX, g.start.x, g.end.x);
        lMaxX = Math.max(lMaxX, g.start.x, g.end.x);
        lMinY = Math.min(lMinY, g.start.y, g.end.y);
        lMaxY = Math.max(lMaxY, g.start.y, g.end.y);
      } else if (g.type === "Polyline") {
        for (const pt of g.points) {
          lMinX = Math.min(lMinX, pt.x); lMaxX = Math.max(lMaxX, pt.x);
          lMinY = Math.min(lMinY, pt.y); lMaxY = Math.max(lMaxY, pt.y);
        }
      } else if (g.type === "Circle") {
        lMinX = Math.min(lMinX, g.center.x - g.radius);
        lMaxX = Math.max(lMaxX, g.center.x + g.radius);
        lMinY = Math.min(lMinY, g.center.y - g.radius);
        lMaxY = Math.max(lMaxY, g.center.y + g.radius);
      }
    }
    // Include pins
    for (const pin of lib.pins) {
      lMinX = Math.min(lMinX, pin.position.x); lMaxX = Math.max(lMaxX, pin.position.x);
      lMinY = Math.min(lMinY, pin.position.y); lMaxY = Math.max(lMaxY, pin.position.y);
    }
    if (!isFinite(lMinX)) { lMinX = -2; lMaxX = 2; lMinY = -2; lMaxY = 2; }

    // Transform corners to schematic space
    const pad = tolerance * 0.3;
    const corners = [
      symToSch(lMinX - pad, lMinY - pad, sym),
      symToSch(lMaxX + pad, lMinY - pad, sym),
      symToSch(lMaxX + pad, lMaxY + pad, sym),
      symToSch(lMinX - pad, lMaxY + pad, sym),
    ];
    const bx0 = Math.min(...corners.map(c => c.x));
    const by0 = Math.min(...corners.map(c => c.y));
    const bx1 = Math.max(...corners.map(c => c.x));
    const by1 = Math.max(...corners.map(c => c.y));

    if (p.x >= bx0 && p.x <= bx1 && p.y >= by0 && p.y <= by1) {
      return { type: "symbol", uuid: sym.uuid };
    }
  }

  // Wire endpoints (tight tolerance for dragging)
  const epTol = tolerance * 0.35;
  for (const wire of data.wires) {
    if (dist(p, wire.start) < epTol) return { type: "wireEndpoint", uuid: wire.uuid, endpoint: "start" };
    if (dist(p, wire.end) < epTol) return { type: "wireEndpoint", uuid: wire.uuid, endpoint: "end" };
  }

  // Wires (segment body)
  for (const wire of data.wires) {
    if (distToSegment(p, wire.start, wire.end) < tolerance * 0.5) {
      return { type: "wire", uuid: wire.uuid };
    }
  }

  // Buses (thicker hit zone)
  for (const bus of data.buses) {
    if (distToSegment(p, bus.start, bus.end) < tolerance * 0.7) {
      return { type: "bus", uuid: bus.uuid };
    }
  }

  // Labels
  for (const label of data.labels) {
    if (dist(p, label.position) < tolerance) {
      return { type: "label", uuid: label.uuid };
    }
  }

  // Text notes (hit by proximity to position)
  for (const note of data.text_notes) {
    if (dist(p, note.position) < tolerance * 1.5) {
      return { type: "textNote", uuid: note.uuid };
    }
  }

  // Bus entries
  for (const be of data.bus_entries) {
    if (dist(p, be.position) < tolerance) {
      return { type: "busEntry", uuid: be.uuid };
    }
  }

  // Drawing objects
  for (const d of data.drawings) {
    if (d.type === "Line") {
      if (distToSegment(p, d.start, d.end) < tolerance * 0.5) return { type: "drawing", uuid: d.uuid };
    } else if (d.type === "Rect") {
      const rx = Math.min(d.start.x, d.end.x), ry = Math.min(d.start.y, d.end.y);
      const rw = Math.abs(d.end.x - d.start.x), rh = Math.abs(d.end.y - d.start.y);
      if (p.x >= rx - tolerance && p.x <= rx + rw + tolerance && p.y >= ry - tolerance && p.y <= ry + rh + tolerance) {
        return { type: "drawing", uuid: d.uuid };
      }
    } else if (d.type === "Circle") {
      const dr = Math.abs(dist(p, d.center) - d.radius);
      if (dr < tolerance || (d.fill && dist(p, d.center) < d.radius)) return { type: "drawing", uuid: d.uuid };
    } else if (d.type === "Arc") {
      if (dist(p, d.start) < tolerance || dist(p, d.mid) < tolerance || dist(p, d.end) < tolerance) {
        return { type: "drawing", uuid: d.uuid };
      }
    } else if (d.type === "Polyline") {
      for (let i = 0; i < d.points.length - 1; i++) {
        if (distToSegment(p, d.points[i], d.points[i + 1]) < tolerance * 0.5) return { type: "drawing", uuid: d.uuid };
      }
    }
  }

  // Child sheets (rectangle hit test)
  for (const sheet of data.child_sheets) {
    const sx = sheet.position.x, sy = sheet.position.y;
    const sw = sheet.size[0], sh = sheet.size[1];
    if (p.x >= sx - tolerance && p.x <= sx + sw + tolerance &&
        p.y >= sy - tolerance && p.y <= sy + sh + tolerance) {
      return { type: "childSheet", uuid: sheet.uuid };
    }
  }

  return null;
}

function pointInBox(p: SchPoint, box: Box): boolean {
  return p.x >= box.minX && p.x <= box.maxX && p.y >= box.minY && p.y <= box.maxY;
}

function segmentIntersectsBox(a: SchPoint, b: SchPoint, box: Box): boolean {
  // Check if either endpoint is inside
  if (pointInBox(a, box) || pointInBox(b, box)) return true;
  // Check segment-edge intersections (simplified: check if segment crosses any box edge)
  const edges: [SchPoint, SchPoint][] = [
    [{ x: box.minX, y: box.minY }, { x: box.maxX, y: box.minY }],
    [{ x: box.maxX, y: box.minY }, { x: box.maxX, y: box.maxY }],
    [{ x: box.maxX, y: box.maxY }, { x: box.minX, y: box.maxY }],
    [{ x: box.minX, y: box.maxY }, { x: box.minX, y: box.minY }],
  ];
  for (const [c, d] of edges) {
    if (segmentsIntersect(a, b, c, d)) return true;
  }
  return false;
}

function segmentsIntersect(a: SchPoint, b: SchPoint, c: SchPoint, d: SchPoint): boolean {
  const cross = (o: SchPoint, p: SchPoint, q: SchPoint) =>
    (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
  const d1 = cross(c, d, a), d2 = cross(c, d, b);
  const d3 = cross(a, b, c), d4 = cross(a, b, d);
  if (((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) &&
      ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0))) return true;
  return false;
}

/**
 * Select all objects within a drag box.
 * crossing=true: select if object intersects box (right-to-left drag)
 * crossing=false: select only if object is fully inside box (left-to-right drag)
 */
export function boxSelect(
  data: SchematicData,
  startX: number, startY: number,
  endX: number, endY: number,
): string[] {
  const crossing = endX < startX;
  const box: Box = {
    minX: Math.min(startX, endX), minY: Math.min(startY, endY),
    maxX: Math.max(startX, endX), maxY: Math.max(startY, endY),
  };

  const selected: string[] = [];

  for (const sym of data.symbols) {
    if (crossing) {
      // Crossing: any part of symbol in box
      if (pointInBox(sym.position, box)) selected.push(sym.uuid);
    } else {
      // Inside: symbol center must be in box
      if (pointInBox(sym.position, box)) selected.push(sym.uuid);
    }
  }

  for (const wire of data.wires) {
    if (crossing) {
      if (segmentIntersectsBox(wire.start, wire.end, box)) selected.push(wire.uuid);
    } else {
      if (pointInBox(wire.start, box) && pointInBox(wire.end, box)) selected.push(wire.uuid);
    }
  }

  for (const label of data.labels) {
    if (pointInBox(label.position, box)) selected.push(label.uuid);
  }

  for (const j of data.junctions) {
    if (pointInBox(j.position, box)) selected.push(j.uuid);
  }

  for (const nc of data.no_connects) {
    if (pointInBox(nc.position, box)) selected.push(nc.uuid);
  }

  for (const note of data.text_notes) {
    if (pointInBox(note.position, box)) selected.push(note.uuid);
  }

  for (const bus of data.buses) {
    if (crossing) {
      if (segmentIntersectsBox(bus.start, bus.end, box)) selected.push(bus.uuid);
    } else {
      if (pointInBox(bus.start, box) && pointInBox(bus.end, box)) selected.push(bus.uuid);
    }
  }

  for (const be of data.bus_entries) {
    if (pointInBox(be.position, box)) selected.push(be.uuid);
  }

  for (const sheet of data.child_sheets) {
    if (pointInBox(sheet.position, box)) selected.push(sheet.uuid);
  }

  for (const d of data.drawings) {
    if (d.type === "Line" && pointInBox(d.start, box) && pointInBox(d.end, box)) selected.push(d.uuid);
    else if (d.type === "Rect" && pointInBox(d.start, box) && pointInBox(d.end, box)) selected.push(d.uuid);
    else if (d.type === "Circle" && pointInBox(d.center, box)) selected.push(d.uuid);
    else if (d.type === "Arc" && pointInBox(d.start, box)) selected.push(d.uuid);
    else if (d.type === "Polyline" && d.points.every(p => pointInBox(p, box))) selected.push(d.uuid);
  }

  return selected;
}
