import type { SchematicData } from "@/types";

/**
 * Substitute Altium-style special strings in text.
 * Supports: =Title, =Date, =Rev, =Company, =Comment1..=Comment9,
 *           =CurrentDate, =CurrentTime, =SheetNumber, =SheetTotal
 */
export function substituteSpecialStrings(
  text: string,
  data: SchematicData,
  sheetNumber?: number,
  sheetTotal?: number,
): string {
  if (!text.includes("=")) return text;

  const tb = data.title_block || {};
  const now = new Date();

  return text
    .replace(/=Title/gi, tb.title || "")
    .replace(/=Date/gi, tb.date || "")
    .replace(/=Rev/gi, tb.rev || "")
    .replace(/=Company/gi, tb.company || "")
    .replace(/=Comment(\d)/gi, (_, n) => tb[`comment_${n}`] || "")
    .replace(/=CurrentDate/gi, now.toLocaleDateString())
    .replace(/=CurrentTime/gi, now.toLocaleTimeString())
    .replace(/=SheetNumber/gi, String(sheetNumber ?? 1))
    .replace(/=SheetTotal/gi, String(sheetTotal ?? 1))
    .replace(/=DocumentName/gi, tb.title || "Untitled")
    .replace(/=Author/gi, tb.company || "");
}
