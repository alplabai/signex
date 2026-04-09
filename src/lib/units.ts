/** Convert mm to display value based on selected units */
export function mmToDisplay(mm: number, units: "mm" | "mil" | "inch"): string {
  if (units === "mil") return (mm / 0.0254).toFixed(1);
  if (units === "inch") return (mm / 25.4).toFixed(4);
  return mm.toFixed(2);
}

/** Convert display value back to mm */
export function displayToMm(val: number, units: "mm" | "mil" | "inch"): number {
  if (units === "mil") return val * 0.0254;
  if (units === "inch") return val * 25.4;
  return val;
}
