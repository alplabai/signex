/**
 * Gerber X2 Extended Attributes — adds metadata to standard RS-274X Gerber files.
 *
 * X2 extends Gerber with file attributes (TF), aperture attributes (TA),
 * and object attributes (TO) for automated PCB fabrication.
 */

import type { PcbData, PcbLayerId } from "@/types/pcb";

/**
 * File function attribute mapping (TF.FileFunction).
 * Maps layer IDs to Gerber X2 file function descriptions.
 */
const FILE_FUNCTIONS: Record<string, string> = {
  "F.Cu": "Copper,L1,Top",
  "In1.Cu": "Copper,L2,Inr",
  "In2.Cu": "Copper,L3,Inr",
  "In3.Cu": "Copper,L4,Inr",
  "In4.Cu": "Copper,L5,Inr",
  "B.Cu": "Copper,L2,Bot", // L2 for 2-layer, adjusted dynamically
  "F.SilkS": "Legend,Top",
  "B.SilkS": "Legend,Bot",
  "F.Mask": "Soldermask,Top",
  "B.Mask": "Soldermask,Bot",
  "F.Paste": "Paste,Top",
  "B.Paste": "Paste,Bot",
  "Edge.Cuts": "Profile,NP",
};

/**
 * Generate Gerber X2 header attributes for a given layer.
 * These are prepended to the standard RS-274X Gerber content.
 */
export function generateX2Header(
  data: PcbData,
  layer: PcbLayerId,
  jobName: string = "board",
): string {
  const lines: string[] = [];
  const now = new Date();
  const dateStr = now.toISOString().replace(/[-:]/g, "").split(".")[0];

  // File attributes
  lines.push(`%TF.GenerationSoftware,Signex,1.0*%`);
  lines.push(`%TF.CreationDate,${dateStr}*%`);
  lines.push(`%TF.ProjectId,${jobName},${data.board.uuid},1*%`);

  // File function
  const fileFunc = getFileFunction(layer, data.board.layers.copperCount);
  if (fileFunc) {
    lines.push(`%TF.FileFunction,${fileFunc}*%`);
  }

  // File polarity
  if (layer === "F.Mask" || layer === "B.Mask") {
    lines.push(`%TF.FilePolarity,Negative*%`);
  } else {
    lines.push(`%TF.FilePolarity,Positive*%`);
  }

  // Part — single PCB
  lines.push(`%TF.Part,Single*%`);

  // Same units as main Gerber
  lines.push(`%TF.SameCoordinates,Original*%`);

  return lines.join("\n") + "\n";
}

/**
 * Generate aperture attributes for a pad.
 * These describe what the aperture represents (component pad, via, etc.)
 */
export function generatePadApertureAttributes(
  componentRef: string,
  padNumber: string,
  padFunction: "ComponentPad" | "ViaPad" | "FiducialPad" | "TestPad" | "CastellatedPad",
): string {
  const lines: string[] = [];

  // Object attribute: associate with component
  if (componentRef) {
    lines.push(`%TO.C,${componentRef}*%`);
  }

  // Pad function
  lines.push(`%TO.P,${componentRef},${padNumber},${padFunction}*%`);

  return lines.join("\n") + "\n";
}

/**
 * Generate net attribute for a trace or pad.
 */
export function generateNetAttribute(netName: string): string {
  if (!netName) return "";
  return `%TO.N,${escapeGerberString(netName)}*%\n`;
}

/**
 * Clear all object attributes (between different objects).
 */
export function clearAttributes(): string {
  return "%TD*%\n";
}

/**
 * Enhance a standard Gerber file with X2 attributes.
 * Takes the plain RS-274X content and prepends X2 header.
 */
export function enhanceWithX2(
  gerberContent: string,
  data: PcbData,
  layer: PcbLayerId,
): string {
  const header = generateX2Header(data, layer);

  // Insert X2 header after the format specification line
  const lines = gerberContent.split("\n");
  const insertIdx = lines.findIndex((l) => l.startsWith("%FSLAX") || l.startsWith("G04"));

  if (insertIdx >= 0) {
    lines.splice(insertIdx, 0, ...header.split("\n").filter(Boolean));
  } else {
    // Prepend if no format line found
    return header + gerberContent;
  }

  return lines.join("\n");
}

// --- Helpers ---

function getFileFunction(layer: PcbLayerId, copperCount: number): string | null {
  if (layer === "B.Cu") {
    return `Copper,L${copperCount},Bot`;
  }
  const match = layer.match(/^In(\d+)\.Cu$/);
  if (match) {
    const num = parseInt(match[1], 10);
    return `Copper,L${num + 1},Inr`;
  }
  return FILE_FUNCTIONS[layer] || null;
}

function escapeGerberString(s: string): string {
  return s.replace(/[%*\\]/g, "");
}
