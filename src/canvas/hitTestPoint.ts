import type { SchematicData } from "@/types";
import type { SelectionFilter } from "@/stores/editor";
import { isSelectable, symToSch, dist, distToSegment } from "./hitTestUtils";
export type { HitResult } from "./hitTestUtils";

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
): import("./hitTestUtils").HitResult | null {
  const p = { x: worldX, y: worldY };

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
        const tl = { x: rx, y: ry };
        const tr = { x: rx + rw, y: ry };
        const br = { x: rx + rw, y: ry + rh };
        const bl = { x: rx, y: ry + rh };
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
