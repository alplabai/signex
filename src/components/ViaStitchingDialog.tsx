import { useState } from "react";
import { X } from "lucide-react";
import { usePcbStore } from "@/stores/pcb";
import { placeViaGrid, placeViaFence, applyViaStitching } from "@/lib/pcbViaStitching";
import type { PcbLayerId } from "@/types/pcb";
import { cn } from "@/lib/utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

export function ViaStitchingDialog({ open, onClose }: Props) {
  const [mode, setMode] = useState<"grid" | "fence">("grid");
  const [spacing, setSpacing] = useState(2.0);
  const [diameter, setDiameter] = useState(0.6);
  const [drill, setDrill] = useState(0.3);
  const [netNumber, setNetNumber] = useState(0);

  const pcbData = usePcbStore((s) => s.data);
  const selectedIds = usePcbStore((s) => s.selectedIds);

  if (!open) return null;

  const handleApply = () => {
    if (!pcbData) return;

    if (spacing <= 0 || diameter <= 0 || drill <= 0) return;
    if (drill >= diameter) { alert("Drill must be smaller than diameter"); return; }

    const options = {
      net: netNumber,
      diameter,
      drill,
      spacing,
      layers: ["F.Cu", "B.Cu"] as [PcbLayerId, PcbLayerId],
    };

    if (mode === "grid") {
      // Use board outline bounding box or selected zone
      const zone = pcbData.zones.find((z) => selectedIds.has(z.uuid));
      let tl = { x: -10, y: -10 }, br = { x: 10, y: 10 };
      if (zone && zone.outline.length > 0) {
        const xs = zone.outline.map((p) => p.x);
        const ys = zone.outline.map((p) => p.y);
        tl = { x: Math.min(...xs), y: Math.min(...ys) };
        br = { x: Math.max(...xs), y: Math.max(...ys) };
      } else if (pcbData.board.outline.length > 0) {
        const xs = pcbData.board.outline.map((p: { x: number; y: number }) => p.x);
        const ys = pcbData.board.outline.map((p: { x: number; y: number }) => p.y);
        tl = { x: Math.min(...xs), y: Math.min(...ys) };
        br = { x: Math.max(...xs), y: Math.max(...ys) };
      }
      const vias = placeViaGrid(tl, br, options);
      applyViaStitching(vias);
    } else {
      const zone = pcbData.zones.find((z) => selectedIds.has(z.uuid));
      if (!zone || zone.outline.length < 3) {
        alert("Select a zone for fence stitching");
        return;
      }
      const vias = placeViaFence(zone.outline, options);
      applyViaStitching(vias);
    }

    onClose();
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50">
      <div className="bg-bg-secondary border border-border-subtle rounded-lg shadow-2xl w-[360px]">
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-border-subtle">
          <span className="text-xs font-semibold">Via Stitching</span>
          <button onClick={onClose} className="p-0.5 hover:bg-bg-hover rounded"><X size={14} /></button>
        </div>

        <div className="p-4 space-y-3">
          <div className="flex items-center gap-2">
            <span className="text-[10px] text-text-muted w-16">Mode</span>
            <select value={mode} onChange={(e) => setMode(e.target.value as "grid" | "fence")}
              className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] outline-none">
              <option value="grid">Grid (fill area)</option>
              <option value="fence">Fence (perimeter)</option>
            </select>
          </div>

          <div className="flex items-center gap-2">
            <span className="text-[10px] text-text-muted w-16">Spacing</span>
            <input type="number" step="0.1" min="0.5" value={spacing}
              onChange={(e) => setSpacing(Number(e.target.value))}
              className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] outline-none" />
            <span className="text-[9px] text-text-muted">mm</span>
          </div>

          <div className="flex items-center gap-2">
            <span className="text-[10px] text-text-muted w-16">Diameter</span>
            <input type="number" step="0.05" min="0.2" value={diameter}
              onChange={(e) => setDiameter(Number(e.target.value))}
              className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] outline-none" />
            <span className="text-[9px] text-text-muted">mm</span>
          </div>

          <div className="flex items-center gap-2">
            <span className="text-[10px] text-text-muted w-16">Drill</span>
            <input type="number" step="0.05" min="0.1" value={drill}
              onChange={(e) => setDrill(Number(e.target.value))}
              className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] outline-none" />
            <span className="text-[9px] text-text-muted">mm</span>
          </div>

          <div className="flex items-center gap-2">
            <span className="text-[10px] text-text-muted w-16">Net</span>
            <input type="number" min="0" value={netNumber}
              onChange={(e) => setNetNumber(Number(e.target.value))}
              className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] outline-none" />
            <span className="text-[9px] text-text-muted">(0 = GND)</span>
          </div>
        </div>

        <div className="flex justify-end gap-2 px-4 py-2.5 border-t border-border-subtle">
          <button onClick={onClose}
            className="px-3 py-1 text-[10px] text-text-secondary hover:bg-bg-hover rounded">
            Cancel
          </button>
          <button onClick={handleApply}
            disabled={!pcbData}
            className={cn("px-3 py-1 text-[10px] rounded",
              pcbData ? "bg-accent text-white hover:bg-accent/80" : "bg-bg-hover text-text-muted cursor-default")}>
            Place Vias
          </button>
        </div>
      </div>
    </div>
  );
}
