import { useState } from "react";
import { X, Loader2 } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { useProjectStore } from "@/stores/project";
import { exportSchematicPdf, exportMultiSheetPdf, getPaperSize } from "@/lib/pdfExport";
import type { PdfExportOptions } from "@/lib/pdfExport";
import { invoke } from "@tauri-apps/api/core";
import type { SchematicData } from "@/types";

interface ExportPdfDialogProps {
  open: boolean;
  onClose: () => void;
}

export function ExportPdfDialog({ open, onClose }: ExportPdfDialogProps) {
  const [dpi, setDpi] = useState<150 | 300>(300);
  const [colorMode, setColorMode] = useState<"color" | "monochrome">("color");
  const [showGrid, setShowGrid] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [scope, setScope] = useState<"current" | "all">("current");
  const data = useSchematicStore((s) => s.data);
  const project = useProjectStore((s) => s.project);

  if (!open) return null;

  const paperSize = data?.paper_size || "A4";
  const [pw, ph] = getPaperSize(paperSize);

  const handleExport = async () => {
    if (!data) return;
    setExporting(true);
    try {
      const opts: PdfExportOptions = { dpi, showGrid, colorMode };

      let blob: Blob;
      let filename: string;

      if (scope === "all" && project) {
        const sheets: SchematicData[] = [];
        for (const sheet of project.sheets) {
          sheets.push(await invoke<SchematicData>("get_schematic", { projectDir: project.dir, filename: sheet.filename }));
        }
        blob = await exportMultiSheetPdf(sheets, opts);
        filename = "schematic_all.pdf";
      } else {
        blob = await exportSchematicPdf(data, opts);
        filename = "schematic.pdf";
      }

      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = filename;
      a.click();
      URL.revokeObjectURL(url);
      onClose();
    } catch (e) {
      console.error("PDF export failed:", e);
      alert("PDF export failed: " + (e instanceof Error ? e.message : String(e)));
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[#1e1e2e] border border-[#45475a] rounded-lg shadow-2xl w-[400px] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[#45475a]">
          <h2 className="text-sm font-semibold text-[#cdd6f4]">Export as PDF</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-[#313244] text-[#6c7086]">
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 space-y-4 text-xs">
          {/* Paper size (read-only) */}
          <div className="flex items-center justify-between">
            <span className="text-[#a6adc8]">Paper Size</span>
            <span className="text-[#cdd6f4] font-mono text-[11px]">
              {paperSize} ({pw} x {ph} mm)
            </span>
          </div>

          {/* Scope */}
          <div className="space-y-1.5">
            <span className="text-[#a6adc8]">Scope</span>
            <div className="flex gap-3 ml-1">
              <label className="flex items-center gap-1.5 cursor-pointer">
                <input type="radio" name="scope" checked={scope === "current"}
                  onChange={() => setScope("current")} className="accent-[#89b4fa]" />
                <span className="text-[#cdd6f4]">Current Sheet</span>
              </label>
              <label className="flex items-center gap-1.5 cursor-pointer">
                <input type="radio" name="scope" checked={scope === "all"}
                  onChange={() => setScope("all")} className="accent-[#89b4fa]" />
                <span className="text-[#cdd6f4]">All Sheets</span>
              </label>
            </div>
          </div>

          {/* DPI */}
          <div className="space-y-1.5">
            <span className="text-[#a6adc8]">Resolution (DPI)</span>
            <div className="flex gap-3 ml-1">
              <label className="flex items-center gap-1.5 cursor-pointer">
                <input type="radio" name="dpi" checked={dpi === 150}
                  onChange={() => setDpi(150)}
                  className="accent-[#89b4fa]" />
                <span className="text-[#cdd6f4]">150 (Draft)</span>
              </label>
              <label className="flex items-center gap-1.5 cursor-pointer">
                <input type="radio" name="dpi" checked={dpi === 300}
                  onChange={() => setDpi(300)}
                  className="accent-[#89b4fa]" />
                <span className="text-[#cdd6f4]">300 (High Quality)</span>
              </label>
            </div>
          </div>

          {/* Color mode */}
          <div className="space-y-1.5">
            <span className="text-[#a6adc8]">Color Mode</span>
            <div className="flex gap-3 ml-1">
              <label className="flex items-center gap-1.5 cursor-pointer">
                <input type="radio" name="colorMode" checked={colorMode === "color"}
                  onChange={() => setColorMode("color")}
                  className="accent-[#89b4fa]" />
                <span className="text-[#cdd6f4]">Color (Dark Theme)</span>
              </label>
              <label className="flex items-center gap-1.5 cursor-pointer">
                <input type="radio" name="colorMode" checked={colorMode === "monochrome"}
                  onChange={() => setColorMode("monochrome")}
                  className="accent-[#89b4fa]" />
                <span className="text-[#cdd6f4]">Print (Light)</span>
              </label>
            </div>
          </div>

          {/* Show grid */}
          <div className="flex items-center justify-between">
            <span className="text-[#a6adc8]">Show Grid</span>
            <input type="checkbox" checked={showGrid}
              onChange={(e) => setShowGrid(e.target.checked)}
              className="accent-[#89b4fa]" />
          </div>
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 px-4 py-3 border-t border-[#45475a]">
          <button onClick={onClose}
            className="px-4 py-1.5 rounded text-xs bg-[#313244] text-[#a6adc8] hover:bg-[#45475a] transition-colors"
            disabled={exporting}>
            Cancel
          </button>
          <button onClick={handleExport}
            className="px-4 py-1.5 rounded text-xs bg-[#89b4fa]/20 text-[#89b4fa] hover:bg-[#89b4fa]/30 transition-colors flex items-center gap-1.5"
            disabled={exporting || !data}>
            {exporting && <Loader2 size={12} className="animate-spin" />}
            {exporting ? "Exporting..." : "Export"}
          </button>
        </div>
      </div>
    </div>
  );
}
