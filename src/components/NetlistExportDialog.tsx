import { useState } from "react";
import { X, Loader2 } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { invoke } from "@tauri-apps/api/core";

interface NetlistExportDialogProps {
  open: boolean;
  onClose: () => void;
}

export function NetlistExportDialog({ open, onClose }: NetlistExportDialogProps) {
  const [format, setFormat] = useState<"kicad" | "xml">("kicad");
  const [exporting, setExporting] = useState(false);
  const data = useSchematicStore((s) => s.data);

  if (!open) return null;

  const handleExport = async () => {
    if (!data) return;
    setExporting(true);
    try {
      let result: string;
      let filename: string;
      let mime: string;
      if (format === "xml") {
        result = await invoke<string>("export_netlist_xml", { data });
        filename = "netlist.xml";
        mime = "application/xml";
      } else {
        result = await invoke<string>("export_netlist", { data });
        filename = "netlist.net";
        mime = "text/plain";
      }
      const blob = new Blob([result], { type: mime });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = filename;
      a.click();
      URL.revokeObjectURL(url);
      onClose();
    } catch (e) {
      console.error("Netlist export failed:", e);
      alert("Netlist export failed: " + (e instanceof Error ? e.message : String(e)));
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[#1e1e2e] border border-[#45475a] rounded-lg shadow-2xl w-[380px] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-[#45475a]">
          <h2 className="text-sm font-semibold text-[#cdd6f4]">Export Netlist</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-[#313244] text-[#6c7086]">
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="p-4 space-y-4 text-xs">
          <div className="space-y-1.5">
            <span className="text-[#a6adc8]">Format</span>
            <div className="flex gap-3 ml-1">
              <label className="flex items-center gap-1.5 cursor-pointer">
                <input type="radio" name="netFormat" checked={format === "kicad"}
                  onChange={() => setFormat("kicad")} className="accent-[#89b4fa]" />
                <span className="text-[#cdd6f4]">KiCad S-expression (.net)</span>
              </label>
              <label className="flex items-center gap-1.5 cursor-pointer">
                <input type="radio" name="netFormat" checked={format === "xml"}
                  onChange={() => setFormat("xml")} className="accent-[#89b4fa]" />
                <span className="text-[#cdd6f4]">Generic XML (.xml)</span>
              </label>
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
            disabled={exporting || !data}>
            {exporting && <Loader2 size={12} className="animate-spin" />}
            {exporting ? "Exporting..." : "Export"}
          </button>
        </div>
      </div>
    </div>
  );
}
