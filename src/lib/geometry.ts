import type { SchPoint, SchSymbol, SchematicData } from "@/types";

/** Transform a point from symbol-local (Y-up) to schematic (Y-down) space */
export function symToSchPoint(lx: number, ly: number, sym: SchSymbol): SchPoint {
  const y = -ly;
  const rad = -(sym.rotation * Math.PI) / 180;
  const cos = Math.cos(rad), sin = Math.sin(rad);
  let rx = lx * cos - y * sin;
  let ry = lx * sin + y * cos;
  if (sym.mirror_x) ry = -ry;
  if (sym.mirror_y) rx = -rx;
  return { x: sym.position.x + rx, y: sym.position.y + ry };
}

/** Get all pin world positions for a symbol */
export function getSymbolPinPositions(sym: SchSymbol, data: SchematicData): { pinNumber: string; pinName: string; pinType: string; position: SchPoint }[] {
  const lib = data.lib_symbols[sym.lib_id];
  if (!lib) return [];
  return lib.pins.map(pin => ({
    pinNumber: pin.number,
    pinName: pin.name,
    pinType: pin.pin_type,
    position: symToSchPoint(pin.position.x, pin.position.y, sym),
  }));
}

/** Distance between two points */
export function dist(a: SchPoint, b: SchPoint): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

/** Check if two points are at the same position within tolerance */
export function pointsMatch(a: SchPoint, b: SchPoint, tol = 0.05): boolean {
  return Math.abs(a.x - b.x) < tol && Math.abs(a.y - b.y) < tol;
}
