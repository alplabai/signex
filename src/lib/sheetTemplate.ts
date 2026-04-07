import type { SchPoint } from "@/types";

export interface TitleBlockField {
  label: string;
  key: string; // maps to title_block key or special string
  position: SchPoint;
  fontSize: number;
  bold?: boolean;
}

export interface SheetTemplate {
  name: string;
  paperSize: string;
  // Border lines as pairs of points [start, end]
  borderLines: [SchPoint, SchPoint][];
  titleBlockFields: TitleBlockField[];
}

/**
 * Built-in ISO A4 template (landscape 297x210mm)
 * Standard engineering title block in bottom-right corner
 */
function isoA4Template(): SheetTemplate {
  const pw = 297, ph = 210;
  const margin = 10;
  const tbw = 100, tbh = 30;
  const tbx = pw - margin - tbw, tby = ph - margin - tbh;

  return {
    name: "ISO A4",
    paperSize: "A4",
    borderLines: [
      // Outer border with margin
      [{ x: margin, y: margin }, { x: pw - margin, y: margin }],
      [{ x: pw - margin, y: margin }, { x: pw - margin, y: ph - margin }],
      [{ x: pw - margin, y: ph - margin }, { x: margin, y: ph - margin }],
      [{ x: margin, y: ph - margin }, { x: margin, y: margin }],
      // Title block box
      [{ x: tbx, y: tby }, { x: tbx + tbw, y: tby }],
      [{ x: tbx + tbw, y: tby }, { x: tbx + tbw, y: tby + tbh }],
      [{ x: tbx + tbw, y: tby + tbh }, { x: tbx, y: tby + tbh }],
      [{ x: tbx, y: tby + tbh }, { x: tbx, y: tby }],
      // Internal dividers
      [{ x: tbx, y: tby + 10 }, { x: tbx + tbw, y: tby + 10 }],
      [{ x: tbx, y: tby + 20 }, { x: tbx + tbw, y: tby + 20 }],
      [{ x: tbx + 50, y: tby }, { x: tbx + 50, y: tby + 10 }],
    ],
    titleBlockFields: [
      { label: "Title", key: "title", position: { x: tbx + 2, y: tby + 25 }, fontSize: 2.5, bold: true },
      { label: "Date", key: "date", position: { x: tbx + 52, y: tby + 5 }, fontSize: 1.2 },
      { label: "Rev", key: "rev", position: { x: tbx + 2, y: tby + 15 }, fontSize: 1.2 },
      { label: "Company", key: "company", position: { x: tbx + 52, y: tby + 15 }, fontSize: 1.2 },
    ],
  };
}

/**
 * ANSI A (US Letter landscape 279.4x215.9mm)
 */
function ansiATemplate(): SheetTemplate {
  const pw = 279.4, ph = 215.9;
  const margin = 6.35; // 0.25"
  const tbw = 101.6, tbh = 25.4; // 4"x1"
  const tbx = pw - margin - tbw, tby = ph - margin - tbh;

  return {
    name: "ANSI A",
    paperSize: "A",
    borderLines: [
      [{ x: margin, y: margin }, { x: pw - margin, y: margin }],
      [{ x: pw - margin, y: margin }, { x: pw - margin, y: ph - margin }],
      [{ x: pw - margin, y: ph - margin }, { x: margin, y: ph - margin }],
      [{ x: margin, y: ph - margin }, { x: margin, y: margin }],
      [{ x: tbx, y: tby }, { x: tbx + tbw, y: tby }],
      [{ x: tbx + tbw, y: tby }, { x: tbx + tbw, y: tby + tbh }],
      [{ x: tbx + tbw, y: tby + tbh }, { x: tbx, y: tby + tbh }],
      [{ x: tbx, y: tby + tbh }, { x: tbx, y: tby }],
      [{ x: tbx, y: tby + 8 }, { x: tbx + tbw, y: tby + 8 }],
      [{ x: tbx, y: tby + 16 }, { x: tbx + tbw, y: tby + 16 }],
      [{ x: tbx + 50.8, y: tby }, { x: tbx + 50.8, y: tby + 8 }],
    ],
    titleBlockFields: [
      { label: "Title", key: "title", position: { x: tbx + 2, y: tby + 20 }, fontSize: 2.0, bold: true },
      { label: "Date", key: "date", position: { x: tbx + 52.8, y: tby + 4 }, fontSize: 1.0 },
      { label: "Rev", key: "rev", position: { x: tbx + 2, y: tby + 12 }, fontSize: 1.0 },
      { label: "Company", key: "company", position: { x: tbx + 52.8, y: tby + 12 }, fontSize: 1.0 },
    ],
  };
}

/**
 * Blank — no border or title block
 */
function blankTemplate(): SheetTemplate {
  return {
    name: "Blank",
    paperSize: "",
    borderLines: [],
    titleBlockFields: [],
  };
}

export const BUILT_IN_TEMPLATES: SheetTemplate[] = [
  blankTemplate(),
  isoA4Template(),
  ansiATemplate(),
];

export function getTemplateByName(name: string): SheetTemplate | undefined {
  return BUILT_IN_TEMPLATES.find((t) => t.name === name);
}
