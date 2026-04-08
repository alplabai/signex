import type { SchematicData } from "@/types";
import type { ErcMarker } from "@/stores/editor";
import { resolveNets } from "./netResolver";
import { renderSchematicToCanvas } from "./pdfExport";

const MAX_COMPONENTS = 50;
const MAX_NETS = 30;
const MAX_CHARS = 8000;

/**
 * Build a rich text context string with component list, net connectivity, and ERC details.
 * This gets sent to Claude as part of the system prompt for better analysis.
 */
export function buildRichContext(
  data: SchematicData,
  selectedIds: Set<string>,
  ercMarkers: ErcMarker[],
): string {
  const parts: string[] = [];

  // Component list
  const components = data.symbols.filter((s) => !s.is_power);
  if (components.length > 0) {
    parts.push("Components:");
    const sorted = [...components].sort((a, b) => a.reference.localeCompare(b.reference, undefined, { numeric: true }));
    const shown = sorted.slice(0, MAX_COMPONENTS);
    for (const c of shown) {
      const sel = selectedIds.has(c.uuid) ? " [SELECTED]" : "";
      parts.push(`  ${c.reference} = ${c.value} (${c.footprint || "no footprint"})${sel}`);
    }
    if (components.length > MAX_COMPONENTS) {
      parts.push(`  ... and ${components.length - MAX_COMPONENTS} more`);
    }
  }

  // Net connectivity
  try {
    const nets = resolveNets(data);
    if (nets.length > 0) {
      parts.push("\nNets:");
      const named = nets.filter((n) => n.name);
      const shown = named.slice(0, MAX_NETS);
      for (const net of shown) {
        const pins = net.pins.map((p) => `${p.symbolRef}:${p.pinName || p.pinNumber}`).join(", ");
        parts.push(`  ${net.name}: ${pins} (${net.pins.length} pins, ${net.wireUuids.length} wires)`);
      }
      const unnamed = nets.filter((n) => !n.name);
      if (unnamed.length > 0) {
        parts.push(`  + ${unnamed.length} unnamed nets`);
      }
    }
  } catch (e) {
    console.warn("Net resolution failed:", e);
  }

  // Net classes
  if (data.net_classes && data.net_classes.length > 0) {
    parts.push("\nNet Classes:");
    for (const nc of data.net_classes) {
      parts.push(`  ${nc.name}: ${nc.nets.join(", ")}`);
    }
  }

  // ERC violations
  if (ercMarkers.length > 0) {
    parts.push("\nERC Violations:");
    for (const m of ercMarkers.slice(0, 20)) {
      parts.push(`  [${m.severity}] ${m.message}`);
    }
    if (ercMarkers.length > 20) {
      parts.push(`  ... and ${ercMarkers.length - 20} more`);
    }
  }

  // Truncate to max chars
  let result = parts.join("\n");
  if (result.length > MAX_CHARS) {
    result = result.slice(0, MAX_CHARS) + "\n... (context truncated)";
  }
  return result;
}

/**
 * Capture the current schematic as a base64 PNG for visual context.
 * Returns the raw base64 string (no data URL prefix).
 */
export function captureSchematicScreenshot(data: SchematicData): string | null {
  try {
    const canvas = document.createElement("canvas");
    // Use a reasonable resolution for Claude vision
    const width = 1200;
    const height = 800;
    canvas.width = width;
    canvas.height = height;
    renderSchematicToCanvas(data, canvas, { showGrid: false, colorMode: "color" });
    const dataUrl = canvas.toDataURL("image/png");
    // Strip the data URL prefix to get raw base64
    return dataUrl.replace(/^data:image\/png;base64,/, "");
  } catch {
    return null;
  }
}

/**
 * Estimate token count from text (rough: ~4 chars per token).
 */
export function estimateTokens(text: string): number {
  return Math.ceil(text.length / 4);
}

/**
 * Estimate cost in USD for a given token count and model.
 */
export function estimateCost(
  inputTokens: number,
  outputTokens: number,
  model: string,
): number {
  if (model.includes("opus")) {
    return (inputTokens * 15 + outputTokens * 75) / 1_000_000;
  }
  // Sonnet pricing
  return (inputTokens * 3 + outputTokens * 15) / 1_000_000;
}
