import type { SchematicData, SchPoint, SchSymbol } from "@/types";
import type { SelectionFilter } from "@/stores/editor";

export interface HitResult {
  type: "symbol" | "wire" | "wireEndpoint" | "junction" | "label" | "noConnect" | "textNote" | "bus" | "busEntry" | "childSheet" | "drawing";
  uuid: string;
  endpoint?: "start" | "end"; // Only for wireEndpoint
}

// Map hit result types to filter keys
const FILTER_MAP: Record<string, string> = {
  symbol: "components",
  wire: "wires",
  wireEndpoint: "wires",
  junction: "junctions",
  label: "labels",
  noConnect: "noConnects",
  textNote: "textNotes",
  bus: "buses",
  busEntry: "buses",
  childSheet: "sheetSymbols",
  drawing: "drawings",
};

function isSelectable(type: string, filter?: SelectionFilter, isPower?: boolean): boolean {
  if (!filter) return true;
  if (isPower) return filter.powerPorts?.selectable !== false;
  const key = FILTER_MAP[type];
  if (!key) return true;
  return filter[key]?.selectable !== false;
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
  tolerance: number = 2.0,
  filter?: SelectionFilter,
): HitResult | null {
  const p: SchPoint = { x: worldX, y: worldY };

  // Junctions first (small targets)
  if (isSelectable("junction", filter)) {
    for (const j of data.junctions) {
      if (dist(p, j.position) < tolerance * 0.5) {
        return { type: "junction", uuid: j.uuid };
      }
    }
  }

  // No-connect markers (X shape, ~1.4 unit span)
  if (isSelectable("noConnect", filter)) {
    for (const nc of data.no_connects) {
      if (dist(p, nc.position) < tolerance * 0.6) {
        return { type: "noConnect", uuid: nc.uuid };
      }
    }
  }

  // Symbols — check tight transformed bounding box (including power symbols)
  for (const sym of data.symbols) {
    if (!isSelectable("symbol", filter, sym.is_power)) continue;
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
  if (isSelectable("wire", filter)) {
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
  }

  // Buses (thicker hit zone)
  if (isSelectable("bus", filter)) {
    for (const bus of data.buses) {
      if (distToSegment(p, bus.start, bus.end) < tolerance * 0.7) {
        return { type: "bus", uuid: bus.uuid };
      }
    }
  }

  // Labels (respect power port filter separately)
  for (const label of data.labels) {
    const filterKey = label.label_type === "Power" ? "powerPorts" : "labels";
    if (filter && filter[filterKey]?.selectable === false) continue;

    if (label.label_type === "Power") {
      // Power port: hit test covers full symbol (stem + head + text)
      const stemLen = 2.0, symSize = 1.2;
      const fs = label.font_size || 1.27;
      const style = label.shape || "bar";
      const isGnd = style.includes("ground") || style === "earth_ground";
      const textW = label.text.length * fs * 0.65;
      const halfW = Math.max(symSize, textW / 2) + 0.5;
      const lx = label.position.x, ly = label.position.y;

      let minX: number, minY: number, maxX: number, maxY: number;
      if (isGnd) {
        // Connection at top, symbol + text below
        minX = lx - halfW; maxX = lx + halfW;
        minY = ly - 0.5; maxY = ly + stemLen + 1.5 + fs;
      } else {
        // Connection at bottom, symbol + text above
        minX = lx - halfW; maxX = lx + halfW;
        minY = ly - stemLen - 0.5 - fs; maxY = ly + 0.5;
      }

      if (p.x >= minX && p.x <= maxX && p.y >= minY && p.y <= maxY) {
        return { type: "label", uuid: label.uuid };
      }
    } else if (label.label_type === "Global" || label.label_type === "Hierarchical") {
      // Flag/arrow shape bounding box hit test
      const fs = label.font_size || 1.27;
      const h = fs * 1.4;
      const arrowW = h * 0.5;
      const tw = label.text.length * fs * 0.65;
      const pad = fs * 0.3;
      const totalBody = arrowW + tw + pad * 2;
      const r = label.rotation;
      const lx = label.position.x, ly = label.position.y;
      const isHoriz = r === 0 || r === 180;

      let minX: number, minY: number, maxX: number, maxY: number;
      if (isHoriz) {
        const dir = r === 0 ? 1 : -1; // 0° extends right, 180° extends left
        if (dir > 0) {
          minX = lx; maxX = lx + totalBody + arrowW; // extra arrowW for output tip
          minY = ly - h / 2; maxY = ly + h / 2;
        } else {
          minX = lx - totalBody - arrowW; maxX = lx;
          minY = ly - h / 2; maxY = ly + h / 2;
        }
      } else {
        // Vertical (90°, 270°) — rotated shape
        minX = lx - h / 2; maxX = lx + h / 2;
        minY = ly - totalBody; maxY = ly + totalBody;
      }

      if (p.x >= minX && p.x <= maxX && p.y >= minY && p.y <= maxY) {
        return { type: "label", uuid: label.uuid };
      }
    } else if (dist(p, label.position) < tolerance) {
      return { type: "label", uuid: label.uuid };
    }
  }

  // Text notes (hit by proximity to position)
  if (isSelectable("textNote", filter)) {
    for (const note of data.text_notes) {
      if (dist(p, note.position) < tolerance * 1.5) {
        return { type: "textNote", uuid: note.uuid };
      }
    }
  }

  // Bus entries
  if (isSelectable("busEntry", filter)) {
    for (const be of data.bus_entries) {
      if (dist(p, be.position) < tolerance) {
        return { type: "busEntry", uuid: be.uuid };
      }
    }
  }

  // Drawing objects
  if (!isSelectable("drawing", filter)) { /* skip */ }
  else for (const d of data.drawings) {
    if (d.type === "Line") {
      if (distToSegment(p, d.start, d.end) < tolerance * 0.5) return { type: "drawing", uuid: d.uuid };
    } else if (d.type === "Rect") {
      const rx = Math.min(d.start.x, d.end.x), ry = Math.min(d.start.y, d.end.y);
      const rw = Math.abs(d.end.x - d.start.x), rh = Math.abs(d.end.y - d.start.y);
      if (d.fill === false) {
        // Unfilled rect: check proximity to edges
        const tl: SchPoint = { x: rx, y: ry };
        const tr: SchPoint = { x: rx + rw, y: ry };
        const br: SchPoint = { x: rx + rw, y: ry + rh };
        const bl: SchPoint = { x: rx, y: ry + rh };
        if (distToSegment(p, tl, tr) < tolerance || distToSegment(p, tr, br) < tolerance ||
            distToSegment(p, br, bl) < tolerance || distToSegment(p, bl, tl) < tolerance) {
          return { type: "drawing", uuid: d.uuid };
        }
      } else if (p.x >= rx - tolerance && p.x <= rx + rw + tolerance && p.y >= ry - tolerance && p.y <= ry + rh + tolerance) {
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
    } else if (d.type === "Ellipse") {
      const nx = (p.x - d.center.x) / d.radiusX, ny = (p.y - d.center.y) / d.radiusY;
      const nd = Math.sqrt(nx * nx + ny * ny);
      if (d.fill ? nd <= 1.1 : Math.abs(nd - 1) < tolerance / Math.max(d.radiusX, d.radiusY)) return { type: "drawing", uuid: d.uuid };
    } else if (d.type === "Polygon") {
      // Point-in-polygon (ray casting) + edge distance
      if (d.points.length >= 3) {
        let inside = false;
        for (let i = 0, j = d.points.length - 1; i < d.points.length; j = i++) {
          const xi = d.points[i].x, yi = d.points[i].y;
          const xj = d.points[j].x, yj = d.points[j].y;
          if (((yi > p.y) !== (yj > p.y)) && (p.x < (xj - xi) * (p.y - yi) / (yj - yi) + xi)) inside = !inside;
        }
        if (inside) return { type: "drawing", uuid: d.uuid };
        for (let i = 0; i < d.points.length; i++) {
          const j = (i + 1) % d.points.length;
          if (distToSegment(p, d.points[i], d.points[j]) < tolerance * 0.5) return { type: "drawing", uuid: d.uuid };
        }
      }
    } else if (d.type === "Image") {
      const rx = Math.min(d.start.x, d.end.x), ry = Math.min(d.start.y, d.end.y);
      const rw = Math.abs(d.end.x - d.start.x), rh = Math.abs(d.end.y - d.start.y);
      if (p.x >= rx && p.x <= rx + rw && p.y >= ry && p.y <= ry + rh) return { type: "drawing", uuid: d.uuid };
    } else if (d.type === "RoundRect" || d.type === "TextFrame") {
      const rx = Math.min(d.start.x, d.end.x), ry = Math.min(d.start.y, d.end.y);
      const rw = Math.abs(d.end.x - d.start.x), rh = Math.abs(d.end.y - d.start.y);
      if (p.x >= rx - tolerance && p.x <= rx + rw + tolerance && p.y >= ry - tolerance && p.y <= ry + rh + tolerance) {
        return { type: "drawing", uuid: d.uuid };
      }
    }
  }

  // Child sheets (rectangle hit test)
  if (!isSelectable("childSheet", filter)) { /* skip */ }
  else for (const sheet of data.child_sheets) {
    const sx = sheet.position.x, sy = sheet.position.y;
    const sw = sheet.size[0], sh = sheet.size[1];
    if (p.x >= sx - tolerance && p.x <= sx + sw + tolerance &&
        p.y >= sy - tolerance && p.y <= sy + sh + tolerance) {
      return { type: "childSheet", uuid: sheet.uuid };
    }
  }

  // Parameter sets (icon ~1 unit radius)
  if (data.parameter_sets) {
    for (const ps of data.parameter_sets) {
      if (dist(p, ps.position) < tolerance * 1.2) return { type: "drawing", uuid: ps.uuid };
    }
  }

  // Differential pair directives
  if (data.diff_pair_directives) {
    for (const dp of data.diff_pair_directives) {
      if (dist(p, dp.position) < tolerance * 1.2) return { type: "drawing", uuid: dp.uuid };
    }
  }

  // Blankets (point-in-polygon)
  if (data.blankets) {
    for (const bl of data.blankets) {
      if (bl.points.length >= 3) {
        let inside = false;
        for (let i = 0, j = bl.points.length - 1; i < bl.points.length; j = i++) {
          const xi = bl.points[i].x, yi = bl.points[i].y;
          const xj = bl.points[j].x, yj = bl.points[j].y;
          if (((yi > p.y) !== (yj > p.y)) && (p.x < (xj - xi) * (p.y - yi) / (yj - yi) + xi)) inside = !inside;
        }
        if (inside) return { type: "drawing", uuid: bl.uuid };
        for (let i = 0; i < bl.points.length; i++) {
          const j2 = (i + 1) % bl.points.length;
          if (distToSegment(p, bl.points[i], bl.points[j2]) < tolerance * 0.5) return { type: "drawing", uuid: bl.uuid };
        }
      }
    }
  }

  // Compile masks (rectangle)
  if (data.compile_masks) {
    for (const cm of data.compile_masks) {
      const cx = cm.position.x, cy = cm.position.y, cw = cm.size[0], ch = cm.size[1];
      if (p.x >= cx - tolerance && p.x <= cx + cw + tolerance && p.y >= cy - tolerance && p.y <= cy + ch + tolerance) {
        return { type: "drawing", uuid: cm.uuid };
      }
    }
  }

  // Notes (rectangle)
  if (data.notes) {
    for (const n of data.notes) {
      const nx = n.position.x, ny = n.position.y, nw = n.size[0], nh = n.size[1];
      if (p.x >= nx - tolerance && p.x <= nx + nw + tolerance && p.y >= ny - tolerance && p.y <= ny + nh + tolerance) {
        return { type: "drawing", uuid: n.uuid };
      }
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
  filter?: SelectionFilter,
): string[] {
  const crossing = endX < startX;
  const box: Box = {
    minX: Math.min(startX, endX), minY: Math.min(startY, endY),
    maxX: Math.max(startX, endX), maxY: Math.max(startY, endY),
  };

  const selected: string[] = [];

  for (const sym of data.symbols) {
    if (!isSelectable("symbol", filter, sym.is_power)) continue;
    if (crossing) {
      // Crossing: select if symbol bounding box overlaps selection box
      const lib = data.lib_symbols[sym.lib_id];
      if (lib) {
        let lMinX = -2, lMaxX = 2, lMinY = -2, lMaxY = 2;
        for (const g of lib.graphics) {
          if (g.type === "Rectangle") { lMinX = Math.min(lMinX, g.start.x, g.end.x); lMaxX = Math.max(lMaxX, g.start.x, g.end.x); lMinY = Math.min(lMinY, g.start.y, g.end.y); lMaxY = Math.max(lMaxY, g.start.y, g.end.y); }
          else if (g.type === "Polyline") { for (const p of g.points) { lMinX = Math.min(lMinX, p.x); lMaxX = Math.max(lMaxX, p.x); lMinY = Math.min(lMinY, p.y); lMaxY = Math.max(lMaxY, p.y); } }
          else if (g.type === "Circle") { lMinX = Math.min(lMinX, g.center.x - g.radius); lMaxX = Math.max(lMaxX, g.center.x + g.radius); lMinY = Math.min(lMinY, g.center.y - g.radius); lMaxY = Math.max(lMaxY, g.center.y + g.radius); }
        }
        for (const pin of lib.pins) { lMinX = Math.min(lMinX, pin.position.x); lMaxX = Math.max(lMaxX, pin.position.x); lMinY = Math.min(lMinY, pin.position.y); lMaxY = Math.max(lMaxY, pin.position.y); }
        const corners = [symToSch(lMinX, lMinY, sym), symToSch(lMaxX, lMinY, sym), symToSch(lMaxX, lMaxY, sym), symToSch(lMinX, lMaxY, sym)];
        const symBox = {
          minX: Math.min(...corners.map(c => c.x)), minY: Math.min(...corners.map(c => c.y)),
          maxX: Math.max(...corners.map(c => c.x)), maxY: Math.max(...corners.map(c => c.y)),
        };
        // Check if symbol bbox overlaps selection box
        if (symBox.maxX >= box.minX && symBox.minX <= box.maxX && symBox.maxY >= box.minY && symBox.minY <= box.maxY) {
          selected.push(sym.uuid);
        }
      } else if (pointInBox(sym.position, box)) {
        selected.push(sym.uuid);
      }
    } else {
      // Inside: symbol center must be in box
      if (pointInBox(sym.position, box)) selected.push(sym.uuid);
    }
  }

  for (const wire of data.wires) {
    if (!isSelectable("wire", filter)) continue;
    if (crossing) {
      if (segmentIntersectsBox(wire.start, wire.end, box)) selected.push(wire.uuid);
    } else {
      if (pointInBox(wire.start, box) && pointInBox(wire.end, box)) selected.push(wire.uuid);
    }
  }

  for (const label of data.labels) {
    const filterKey = label.label_type === "Power" ? "powerPorts" : "labels";
    if (filter && filter[filterKey]?.selectable === false) continue;

    if ((label.label_type === "Global" || label.label_type === "Hierarchical") && crossing) {
      // Use flag shape bounding box for crossing selection
      const fs = label.font_size || 1.27;
      const h = fs * 1.4;
      const arrowW = h * 0.5;
      const tw = label.text.length * fs * 0.65;
      const pad = fs * 0.3;
      const totalBody = arrowW + tw + pad * 2;
      const r = label.rotation;
      const lx = label.position.x, ly = label.position.y;
      const isHoriz = r === 0 || r === 180;

      let lMinX: number, lMinY: number, lMaxX: number, lMaxY: number;
      if (isHoriz) {
        const dir = r === 0 ? 1 : -1;
        if (dir > 0) {
          lMinX = lx; lMaxX = lx + totalBody + arrowW;
          lMinY = ly - h / 2; lMaxY = ly + h / 2;
        } else {
          lMinX = lx - totalBody - arrowW; lMaxX = lx;
          lMinY = ly - h / 2; lMaxY = ly + h / 2;
        }
      } else {
        lMinX = lx - h / 2; lMaxX = lx + h / 2;
        lMinY = ly - totalBody; lMaxY = ly + totalBody;
      }

      // Check if label bbox overlaps selection box
      if (lMaxX >= box.minX && lMinX <= box.maxX && lMaxY >= box.minY && lMinY <= box.maxY) {
        selected.push(label.uuid);
      }
    } else {
      if (pointInBox(label.position, box)) selected.push(label.uuid);
    }
  }

  for (const j of data.junctions) {
    if (!isSelectable("junction", filter)) continue;
    if (pointInBox(j.position, box)) selected.push(j.uuid);
  }

  for (const nc of data.no_connects) {
    if (!isSelectable("noConnect", filter)) continue;
    if (pointInBox(nc.position, box)) selected.push(nc.uuid);
  }

  for (const note of data.text_notes) {
    if (!isSelectable("textNote", filter)) continue;
    if (pointInBox(note.position, box)) selected.push(note.uuid);
  }

  for (const bus of data.buses) {
    if (!isSelectable("bus", filter)) continue;
    if (crossing) {
      if (segmentIntersectsBox(bus.start, bus.end, box)) selected.push(bus.uuid);
    } else {
      if (pointInBox(bus.start, box) && pointInBox(bus.end, box)) selected.push(bus.uuid);
    }
  }

  for (const be of data.bus_entries) {
    if (!isSelectable("busEntry", filter)) continue;
    if (pointInBox(be.position, box)) selected.push(be.uuid);
  }

  for (const sheet of data.child_sheets) {
    if (!isSelectable("childSheet", filter)) continue;
    if (pointInBox(sheet.position, box)) selected.push(sheet.uuid);
  }

  if (!isSelectable("drawing", filter)) { /* skip drawings */ }
  else for (const d of data.drawings) {
    if (d.type === "Line" && pointInBox(d.start, box) && pointInBox(d.end, box)) selected.push(d.uuid);
    else if (d.type === "Rect" && pointInBox(d.start, box) && pointInBox(d.end, box)) selected.push(d.uuid);
    else if (d.type === "Circle" && pointInBox(d.center, box)) selected.push(d.uuid);
    else if (d.type === "Arc" && pointInBox(d.start, box) && pointInBox(d.mid, box) && pointInBox(d.end, box)) selected.push(d.uuid);
    else if (d.type === "Polyline" && d.points.every(p => pointInBox(p, box))) selected.push(d.uuid);
  }

  // New directive/annotation types
  if (data.parameter_sets) for (const ps of data.parameter_sets) { if (pointInBox(ps.position, box)) selected.push(ps.uuid); }
  if (data.diff_pair_directives) for (const dp of data.diff_pair_directives) { if (pointInBox(dp.position, box)) selected.push(dp.uuid); }
  if (data.blankets) for (const bl of data.blankets) { if (bl.points.every(p => pointInBox(p, box))) selected.push(bl.uuid); }
  if (data.compile_masks) for (const cm of data.compile_masks) { if (pointInBox(cm.position, box)) selected.push(cm.uuid); }
  if (data.notes) for (const n of data.notes) { if (pointInBox(n.position, box)) selected.push(n.uuid); }

  return selected;
}

// --- Advanced selection helpers ---

function pointInPolygon(p: SchPoint, polygon: SchPoint[]): boolean {
  let inside = false;
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i++) {
    const xi = polygon[i].x, yi = polygon[i].y;
    const xj = polygon[j].x, yj = polygon[j].y;
    if (((yi > p.y) !== (yj > p.y)) && (p.x < (xj - xi) * (p.y - yi) / (yj - yi) + xi)) {
      inside = !inside;
    }
  }
  return inside;
}

/**
 * Select all objects whose position falls inside a lasso polygon.
 * Uses ray-casting point-in-polygon test for each object.
 */
export function lassoSelect(
  data: SchematicData,
  points: SchPoint[],
  filter?: SelectionFilter,
): string[] {
  if (points.length < 3) return [];

  const selected: string[] = [];

  for (const sym of data.symbols) {
    if (!isSelectable("symbol", filter, sym.is_power)) continue;
    if (pointInPolygon(sym.position, points)) selected.push(sym.uuid);
  }

  for (const wire of data.wires) {
    if (!isSelectable("wire", filter)) continue;
    // Wire is selected if both endpoints are inside the lasso
    if (pointInPolygon(wire.start, points) && pointInPolygon(wire.end, points)) {
      selected.push(wire.uuid);
    }
  }

  for (const label of data.labels) {
    const filterKey = label.label_type === "Power" ? "powerPorts" : "labels";
    if (filter && filter[filterKey]?.selectable === false) continue;
    if (pointInPolygon(label.position, points)) selected.push(label.uuid);
  }

  for (const j of data.junctions) {
    if (!isSelectable("junction", filter)) continue;
    if (pointInPolygon(j.position, points)) selected.push(j.uuid);
  }

  for (const nc of data.no_connects) {
    if (!isSelectable("noConnect", filter)) continue;
    if (pointInPolygon(nc.position, points)) selected.push(nc.uuid);
  }

  for (const note of data.text_notes) {
    if (!isSelectable("textNote", filter)) continue;
    if (pointInPolygon(note.position, points)) selected.push(note.uuid);
  }

  for (const bus of data.buses) {
    if (!isSelectable("bus", filter)) continue;
    if (pointInPolygon(bus.start, points) && pointInPolygon(bus.end, points)) {
      selected.push(bus.uuid);
    }
  }

  for (const be of data.bus_entries) {
    if (!isSelectable("busEntry", filter)) continue;
    if (pointInPolygon(be.position, points)) selected.push(be.uuid);
  }

  for (const sheet of data.child_sheets) {
    if (!isSelectable("childSheet", filter)) continue;
    if (pointInPolygon(sheet.position, points)) selected.push(sheet.uuid);
  }

  if (!isSelectable("drawing", filter)) { /* skip drawings */ }
  else for (const d of data.drawings) {
    if (d.type === "Line" && pointInPolygon(d.start, points) && pointInPolygon(d.end, points)) selected.push(d.uuid);
    else if (d.type === "Rect" && pointInPolygon(d.start, points) && pointInPolygon(d.end, points)) selected.push(d.uuid);
    else if (d.type === "Circle" && pointInPolygon(d.center, points)) selected.push(d.uuid);
    else if (d.type === "Arc" && pointInPolygon(d.start, points) && pointInPolygon(d.mid, points) && pointInPolygon(d.end, points)) selected.push(d.uuid);
    else if (d.type === "Polyline" && d.points.every(p => pointInPolygon(p, points))) selected.push(d.uuid);
  }

  return selected;
}

/**
 * Select all objects OUTSIDE a drag box (inverse of inside-mode boxSelect).
 * Returns UUIDs of objects not captured by a left-to-right box selection.
 */
export function outsideBoxSelect(
  data: SchematicData,
  startX: number, startY: number,
  endX: number, endY: number,
  filter?: SelectionFilter,
): string[] {
  // Get all selectable UUIDs
  const allUuids: string[] = [];

  for (const sym of data.symbols) {
    if (isSelectable("symbol", filter, sym.is_power)) allUuids.push(sym.uuid);
  }
  for (const wire of data.wires) {
    if (isSelectable("wire", filter)) allUuids.push(wire.uuid);
  }
  for (const label of data.labels) {
    const filterKey = label.label_type === "Power" ? "powerPorts" : "labels";
    if (filter && filter[filterKey]?.selectable === false) continue;
    allUuids.push(label.uuid);
  }
  for (const j of data.junctions) {
    if (isSelectable("junction", filter)) allUuids.push(j.uuid);
  }
  for (const nc of data.no_connects) {
    if (isSelectable("noConnect", filter)) allUuids.push(nc.uuid);
  }
  for (const note of data.text_notes) {
    if (isSelectable("textNote", filter)) allUuids.push(note.uuid);
  }
  for (const bus of data.buses) {
    if (isSelectable("bus", filter)) allUuids.push(bus.uuid);
  }
  for (const be of data.bus_entries) {
    if (isSelectable("busEntry", filter)) allUuids.push(be.uuid);
  }
  for (const sheet of data.child_sheets) {
    if (isSelectable("childSheet", filter)) allUuids.push(sheet.uuid);
  }
  if (isSelectable("drawing", filter)) {
    for (const d of data.drawings) allUuids.push(d.uuid);
  }

  // Use inside-mode boxSelect (L→R) to find objects inside the box
  const minX = Math.min(startX, endX), minY = Math.min(startY, endY);
  const maxX = Math.max(startX, endX), maxY = Math.max(startY, endY);
  const insideUuids = new Set(boxSelect(data, minX, minY, maxX, maxY, filter));

  // Return everything NOT inside the box
  return allUuids.filter(uuid => !insideUuids.has(uuid));
}

/**
 * Select objects whose position is within `tolerance` distance of a line segment.
 * For wires/buses, checks if any of their segments intersect the selection line.
 */
export function lineSelect(
  data: SchematicData,
  lineStart: SchPoint,
  lineEnd: SchPoint,
  tolerance: number,
  filter?: SelectionFilter,
): string[] {
  const selected: string[] = [];

  for (const sym of data.symbols) {
    if (!isSelectable("symbol", filter, sym.is_power)) continue;
    if (distToSegment(sym.position, lineStart, lineEnd) <= tolerance) {
      selected.push(sym.uuid);
    }
  }

  for (const wire of data.wires) {
    if (!isSelectable("wire", filter)) continue;
    // Check if wire midpoint or endpoints are near the line, or if segments intersect
    if (distToSegment(wire.start, lineStart, lineEnd) <= tolerance ||
        distToSegment(wire.end, lineStart, lineEnd) <= tolerance ||
        segmentsIntersect(wire.start, wire.end, lineStart, lineEnd)) {
      selected.push(wire.uuid);
    }
  }

  for (const label of data.labels) {
    const filterKey = label.label_type === "Power" ? "powerPorts" : "labels";
    if (filter && filter[filterKey]?.selectable === false) continue;
    if (distToSegment(label.position, lineStart, lineEnd) <= tolerance) {
      selected.push(label.uuid);
    }
  }

  for (const j of data.junctions) {
    if (!isSelectable("junction", filter)) continue;
    if (distToSegment(j.position, lineStart, lineEnd) <= tolerance) {
      selected.push(j.uuid);
    }
  }

  for (const nc of data.no_connects) {
    if (!isSelectable("noConnect", filter)) continue;
    if (distToSegment(nc.position, lineStart, lineEnd) <= tolerance) {
      selected.push(nc.uuid);
    }
  }

  for (const note of data.text_notes) {
    if (!isSelectable("textNote", filter)) continue;
    if (distToSegment(note.position, lineStart, lineEnd) <= tolerance) {
      selected.push(note.uuid);
    }
  }

  for (const bus of data.buses) {
    if (!isSelectable("bus", filter)) continue;
    if (distToSegment(bus.start, lineStart, lineEnd) <= tolerance ||
        distToSegment(bus.end, lineStart, lineEnd) <= tolerance ||
        segmentsIntersect(bus.start, bus.end, lineStart, lineEnd)) {
      selected.push(bus.uuid);
    }
  }

  for (const be of data.bus_entries) {
    if (!isSelectable("busEntry", filter)) continue;
    if (distToSegment(be.position, lineStart, lineEnd) <= tolerance) {
      selected.push(be.uuid);
    }
  }

  for (const sheet of data.child_sheets) {
    if (!isSelectable("childSheet", filter)) continue;
    if (distToSegment(sheet.position, lineStart, lineEnd) <= tolerance) {
      selected.push(sheet.uuid);
    }
  }

  if (!isSelectable("drawing", filter)) { /* skip drawings */ }
  else for (const d of data.drawings) {
    if (d.type === "Circle" && distToSegment(d.center, lineStart, lineEnd) <= tolerance) selected.push(d.uuid);
    else if ((d.type === "Line" || d.type === "Rect") && (distToSegment(d.start, lineStart, lineEnd) <= tolerance || distToSegment(d.end, lineStart, lineEnd) <= tolerance)) selected.push(d.uuid);
    else if (d.type === "Arc" && (distToSegment(d.start, lineStart, lineEnd) <= tolerance || distToSegment(d.mid, lineStart, lineEnd) <= tolerance || distToSegment(d.end, lineStart, lineEnd) <= tolerance)) selected.push(d.uuid);
    else if (d.type === "Polyline" && d.points.some(p => distToSegment(p, lineStart, lineEnd) <= tolerance)) selected.push(d.uuid);
  }

  return selected;
}

/**
 * Select all objects on the same net as the clicked object.
 * Traces connectivity through wires, labels, and symbol pins.
 */
export function connectionSelect(
  data: SchematicData,
  startUuid: string,
  filter?: SelectionFilter,
): string[] {
  const EPSILON = 0.5; // Tolerance for endpoint matching

  // Helper: check if two points are coincident
  const ptEq = (a: SchPoint, b: SchPoint) =>
    Math.abs(a.x - b.x) < EPSILON && Math.abs(a.y - b.y) < EPSILON;

  // Helper: get symbol pin positions in schematic space
  const getSymbolPinPositions = (sym: SchSymbol): SchPoint[] => {
    const lib = data.lib_symbols[sym.lib_id];
    if (!lib) return [];
    return lib.pins.map(pin => symToSch(pin.position.x, pin.position.y, sym));
  };

  const wireByUuid = new Map(data.wires.map(w => [w.uuid, w]));

  // Find the clicked object
  const clickedWire = wireByUuid.get(startUuid);
  const clickedLabel = data.labels.find(l => l.uuid === startUuid);
  const clickedSymbol = data.symbols.find(s => s.uuid === startUuid);
  const clickedJunction = data.junctions.find(j => j.uuid === startUuid);

  // Strategy 1: If clicked a label, find all objects with the same net name
  if (clickedLabel) {
    const netName = clickedLabel.text;
    const result: string[] = [];

    // All labels with the same text
    for (const label of data.labels) {
      if (!isSelectable("label", filter)) continue;
      if (label.text === netName) result.push(label.uuid);
    }

    // Find wires connected to those labels
    const labelPositions = data.labels.filter(l => l.text === netName).map(l => l.position);
    const connectedWireUuids = new Set<string>();
    floodFillWires(data, labelPositions, connectedWireUuids, EPSILON);

    for (const uuid of connectedWireUuids) {
      if (isSelectable("wire", filter)) result.push(uuid);
    }

    // Find junctions on connected wires
    for (const j of data.junctions) {
      if (!isSelectable("junction", filter)) continue;
      for (const wUuid of connectedWireUuids) {
        const w = wireByUuid.get(wUuid);
        if (w && (ptEq(j.position, w.start) || ptEq(j.position, w.end))) {
          result.push(j.uuid);
          break;
        }
      }
    }

    // Find symbols with pins touching connected wires
    for (const sym of data.symbols) {
      if (!isSelectable("symbol", filter, sym.is_power)) continue;
      const pinPositions = getSymbolPinPositions(sym);
      for (const pinPos of pinPositions) {
        let connected = false;
        for (const wUuid of connectedWireUuids) {
          const w = wireByUuid.get(wUuid);
          if (w && (ptEq(pinPos, w.start) || ptEq(pinPos, w.end))) {
            connected = true;
            break;
          }
        }
        if (connected) { result.push(sym.uuid); break; }
      }
    }

    return [...new Set(result)];
  }

  // Strategy 2: If clicked a wire, flood-fill connected wires and find labels/symbols
  if (clickedWire) {
    const result: string[] = [];
    const connectedWireUuids = new Set<string>();
    floodFillWires(data, [clickedWire.start, clickedWire.end], connectedWireUuids, EPSILON);

    for (const uuid of connectedWireUuids) {
      if (isSelectable("wire", filter)) result.push(uuid);
    }

    // Find labels at wire endpoints
    for (const label of data.labels) {
      if (!isSelectable("label", filter)) continue;
      for (const wUuid of connectedWireUuids) {
        const w = wireByUuid.get(wUuid);
        if (w && (ptEq(label.position, w.start) || ptEq(label.position, w.end))) {
          result.push(label.uuid);
          break;
        }
      }
    }

    // Find junctions on connected wires
    for (const j of data.junctions) {
      if (!isSelectable("junction", filter)) continue;
      for (const wUuid of connectedWireUuids) {
        const w = wireByUuid.get(wUuid);
        if (w && (ptEq(j.position, w.start) || ptEq(j.position, w.end))) {
          result.push(j.uuid);
          break;
        }
      }
    }

    // Find symbols with pins touching connected wires
    for (const sym of data.symbols) {
      if (!isSelectable("symbol", filter, sym.is_power)) continue;
      const pinPositions = getSymbolPinPositions(sym);
      for (const pinPos of pinPositions) {
        let connected = false;
        for (const wUuid of connectedWireUuids) {
          const w = wireByUuid.get(wUuid);
          if (w && (ptEq(pinPos, w.start) || ptEq(pinPos, w.end))) {
            connected = true;
            break;
          }
        }
        if (connected) { result.push(sym.uuid); break; }
      }
    }

    // Also include labels with same net name (for global connectivity)
    const netLabels = data.labels.filter(l => {
      for (const wUuid of connectedWireUuids) {
        const w = wireByUuid.get(wUuid);
        if (w && (ptEq(l.position, w.start) || ptEq(l.position, w.end))) return true;
      }
      return false;
    });
    if (netLabels.length > 0) {
      const netName = netLabels[0].text;
      for (const label of data.labels) {
        if (label.text === netName && !result.includes(label.uuid)) {
          if (isSelectable("label", filter)) result.push(label.uuid);
        }
      }
    }

    return [...new Set(result)];
  }

  // Strategy 3: If clicked a symbol, find wires/labels connected to its pins
  if (clickedSymbol) {
    const result: string[] = [];
    if (isSelectable("symbol", filter, clickedSymbol.is_power)) {
      result.push(clickedSymbol.uuid);
    }

    const pinPositions = getSymbolPinPositions(clickedSymbol);
    for (const pinPos of pinPositions) {
      // Find wires touching this pin
      const connectedWireUuids = new Set<string>();
      floodFillWires(data, [pinPos], connectedWireUuids, EPSILON);

      for (const uuid of connectedWireUuids) {
        if (isSelectable("wire", filter)) result.push(uuid);
      }

      // Find labels at wire endpoints
      for (const label of data.labels) {
        if (!isSelectable("label", filter)) continue;
        for (const wUuid of connectedWireUuids) {
          const w = wireByUuid.get(wUuid);
          if (w && (ptEq(label.position, w.start) || ptEq(label.position, w.end))) {
            result.push(label.uuid);
            break;
          }
        }
      }

      // Find junctions on connected wires
      for (const j of data.junctions) {
        if (!isSelectable("junction", filter)) continue;
        for (const wUuid of connectedWireUuids) {
          const w = wireByUuid.get(wUuid);
          if (w && (ptEq(j.position, w.start) || ptEq(j.position, w.end))) {
            result.push(j.uuid);
            break;
          }
        }
      }
    }

    return [...new Set(result)];
  }

  // Strategy 4: If clicked a junction, treat like a wire endpoint
  if (clickedJunction) {
    const result: string[] = [];
    if (isSelectable("junction", filter)) result.push(clickedJunction.uuid);

    const connectedWireUuids = new Set<string>();
    floodFillWires(data, [clickedJunction.position], connectedWireUuids, EPSILON);

    for (const uuid of connectedWireUuids) {
      if (isSelectable("wire", filter)) result.push(uuid);
    }

    for (const label of data.labels) {
      if (!isSelectable("label", filter)) continue;
      for (const wUuid of connectedWireUuids) {
        const w = wireByUuid.get(wUuid);
        if (w && (ptEq(label.position, w.start) || ptEq(label.position, w.end))) {
          result.push(label.uuid);
          break;
        }
      }
    }

    for (const sym of data.symbols) {
      if (!isSelectable("symbol", filter, sym.is_power)) continue;
      const pinPositions = getSymbolPinPositions(sym);
      for (const pinPos of pinPositions) {
        let connected = false;
        for (const wUuid of connectedWireUuids) {
          const w = wireByUuid.get(wUuid);
          if (w && (ptEq(pinPos, w.start) || ptEq(pinPos, w.end))) {
            connected = true;
            break;
          }
        }
        if (connected) { result.push(sym.uuid); break; }
      }
    }

    return [...new Set(result)];
  }

  // Fallback: just return the clicked object if it exists
  return [startUuid];
}

function wirePointKey(p: SchPoint, epsilon: number): string {
  const bucket = Math.round(1 / epsilon);
  return `${Math.round(p.x * bucket)},${Math.round(p.y * bucket)}`;
}

/**
 * Flood-fill connected wires starting from seed points.
 * Uses a spatial index (Map<pointKey, wireUuids[]>) for O(n) instead of O(n^2).
 */
function floodFillWires(
  data: SchematicData,
  seedPoints: SchPoint[],
  visited: Set<string>,
  epsilon: number,
): void {
  // Build spatial index: pointKey → [wireUuid, ...]
  const index = new Map<string, string[]>();
  for (const wire of data.wires) {
    const ks = wirePointKey(wire.start, epsilon);
    const ke = wirePointKey(wire.end, epsilon);
    if (!index.has(ks)) index.set(ks, []);
    if (!index.has(ke)) index.set(ke, []);
    index.get(ks)!.push(wire.uuid);
    index.get(ke)!.push(wire.uuid);
  }

  const wireByUuid = new Map(data.wires.map(w => [w.uuid, w]));

  const frontier: SchPoint[] = [...seedPoints];
  while (frontier.length > 0) {
    const point = frontier.pop()!;
    const key = wirePointKey(point, epsilon);
    const candidates = index.get(key) || [];
    for (const uuid of candidates) {
      if (visited.has(uuid)) continue;
      visited.add(uuid);
      const wire = wireByUuid.get(uuid)!;
      frontier.push(wire.start, wire.end);
    }
  }
}
