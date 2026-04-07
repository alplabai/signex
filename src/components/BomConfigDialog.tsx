import { useState } from "react";
import { X, Loader2 } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { invoke } from "@tauri-apps/api/core";
import { generateBomHtml, generateBomExcel } from "@/lib/bomFormats";

interface BomConfigDialogProps {
  open: boolean;
  onClose: () => void;
}

const ALL_COLUMNS = ["Designator", "Value", "Footprint", "Library", "Quantity"];
const GROUP_OPTIONS = ["Value", "Footprint", "Library"];

export function BomConfigDialog({ open, onClose }: BomConfigDialogProps) {
  const [columns, setColumns] = useState<string[]>([...ALL_COLUMNS]);
  const [groupBy, setGroupBy] = useState<string[]>(["Value", "Footprint"]);
  const [format, setFormat] = useState<"csv" | "tsv" | "html" | "excel">("csv");
  const [exporting, setExporting] = useState(false);
  const data = useSchematicStore((s) => s.data);

  if (!open) return null;

  const toggleColumn = (col: string) => {
    setColumns((prev) =>
      prev.includes(col) ? prev.filter((c) => c !== col) : [...prev, col]
    );
  };

  const toggleGroup = (col: string) => {
    setGroupBy((prev) =>
      prev.includes(col) ? prev.filter((c) => c !== col) : [...prev, col]
    );
  };

  const handleExport = async () => {
    if (!data) return;
    setExporting(true);
    try {
      let content: string;
      let ext: string;
      let mime: string;

      if (format === "html") {
        content = generateBomHtml(data, columns);
        ext = "html"; mime = "text/html";
      } else if (format === "excel") {
        content = generateBomExcel(data, columns);
        ext = "xls"; mime = "application/vnd.ms-excel";
      } else {
        content = await invoke<string>("generate_bom_configured", {
          data, columns, groupBy, format: format === "tsv" ? "tsv" : "csv",
        });
        ext = format === "tsv" ? "tsv" : "csv";
        mime = format === "tsv" ? "text/tab-separated-values" : "text/csv";
      }

      const blob = new Blob([content], { type: mime });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `bom.${ext}`;
      a.click();
      URL.revokeObjectURL(url);
      onClose();
    } catch (e) {
      console.error("BOM export failed:", e);
      alert("BOM export failed: " + (e instanceof Error ? e.message : String(e)));
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[#1e1e2e] border border-[#45475a] rounded-lg shadow-2xl w-[420px] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[#45475a]">
          <h2 className="text-sm font-semibold text-[#cdd6f4]">Bill of Materials</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-[#313244] text-[#6c7086]">
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 space-y-4 text-xs">
          {/* Format */}
          <div className="space-y-1.5">
            <span className="text-[#a6adc8]">Format</span>
            <div className="flex flex-wrap gap-3 ml-1">
              {([["csv", "CSV"], ["tsv", "TSV"], ["html", "HTML"], ["excel", "Excel (.xls)"]] as const).map(([v, label]) => (
                <label key={v} className="flex items-center gap-1.5 cursor-pointer">
                  <input type="radio" name="format" checked={format === v}
                    onChange={() => setFormat(v)} className="accent-[#89b4fa]" />
                  <span className="text-[#cdd6f4]">{label}</span>
                </label>
              ))}
            </div>
          </div>

          {/* Columns */}
          <div className="space-y-1.5">
            <span className="text-[#a6adc8]">Columns</span>
            <div className="flex flex-wrap gap-2 ml-1">
              {ALL_COLUMNS.map((col) => (
                <label key={col} className="flex items-center gap-1.5 cursor-pointer">
                  <input type="checkbox" checked={columns.includes(col)}
                    onChange={() => toggleColumn(col)} className="accent-[#89b4fa]" />
                  <span className="text-[#cdd6f4]">{col}</span>
                </label>
              ))}
            </div>
          </div>

          {/* Group By */}
          <div className="space-y-1.5">
            <span className="text-[#a6adc8]">Group By</span>
            <div className="flex flex-wrap gap-2 ml-1">
              {GROUP_OPTIONS.map((col) => (
                <label key={col} className="flex items-center gap-1.5 cursor-pointer">
                  <input type="checkbox" checked={groupBy.includes(col)}
                    onChange={() => toggleGroup(col)} className="accent-[#89b4fa]" />
                  <span className="text-[#cdd6f4]">{col}</span>
                </label>
              ))}
            </div>
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
            disabled={exporting || !data || columns.length === 0}>
            {exporting && <Loader2 size={12} className="animate-spin" />}
            {exporting ? "Exporting..." : "Export"}
          </button>
        </div>
      </div>
    </div>
  );
}
