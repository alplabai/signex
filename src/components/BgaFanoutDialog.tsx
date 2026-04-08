import { useState } from "react";
import { X } from "lucide-react";
import { usePcbStore } from "@/stores/pcb";
import { generateBgaFanout, applyBgaFanout } from "@/lib/pcbAdvancedRouting";
import type { PcbLayerId } from "@/types/pcb";
import { cn } from "@/lib/utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

export function BgaFanoutDialog({ open, onClose }: Props) {
  const [viaDiameter, setViaDiameter] = useState(0.4);
  const [viaDrill, setViaDrill] = useState(0.2);
  const [traceWidth, setTraceWidth] = useState(0.15);
  const [dogboneLength, setDogboneLength] = useState(0.6);
  const [escapeDir, setEscapeDir] = useState<"outward" | "inward" | "alternating">("outward");

  const pcbData = usePcbStore((s) => s.data);
  const selectedIds = usePcbStore((s) => s.selectedIds);

  if (!open) return null;

  const selectedFp = pcbData?.footprints.find((f) => selectedIds.has(f.uuid));

  const handleApply = () => {
    if (!pcbData || !selectedFp) return;
    const result = generateBgaFanout(pcbData, {
      footprintUuid: selectedFp.uuid,
      viaDiameter,
      viaDrill,
      traceWidth,
      dogboneLength,
      escapeDirection: escapeDir,
      topLayer: "F.Cu" as PcbLayerId,
      innerLayer: "In1.Cu" as PcbLayerId,
    });
    applyBgaFanout(result);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50">
      <div className="bg-bg-secondary border border-border-subtle rounded-lg shadow-2xl w-[360px]">
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-border-subtle">
          <span className="text-xs font-semibold">BGA Fanout</span>
          <button onClick={onClose} className="p-0.5 hover:bg-bg-hover rounded"><X size={14} /></button>
        </div>

        <div className="p-4 space-y-3">
          {!selectedFp ? (
            <div className="text-[10px] text-warning text-center py-4">
              Select a BGA footprint first.
            </div>
          ) : (
            <>
              <div className="text-[10px] text-text-muted mb-2">
                Footprint: <span className="text-text-primary font-mono">{selectedFp.reference}</span> ({selectedFp.pads.length} pads)
              </div>

              <Row label="Via Diameter" value={viaDiameter} onChange={setViaDiameter} step={0.05} unit="mm" />
              <Row label="Via Drill" value={viaDrill} onChange={setViaDrill} step={0.05} unit="mm" />
              <Row label="Trace Width" value={traceWidth} onChange={setTraceWidth} step={0.05} unit="mm" />
              <Row label="Dogbone Len" value={dogboneLength} onChange={setDogboneLength} step={0.1} unit="mm" />

              <div className="flex items-center gap-2">
                <span className="text-[10px] text-text-muted w-20">Direction</span>
                <select value={escapeDir} onChange={(e) => setEscapeDir(e.target.value as typeof escapeDir)}
                  className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] outline-none">
                  <option value="outward">Outward</option>
                  <option value="inward">Inward</option>
                  <option value="alternating">Alternating</option>
                </select>
              </div>
            </>
          )}
        </div>

        <div className="flex justify-end gap-2 px-4 py-2.5 border-t border-border-subtle">
          <button onClick={onClose} className="px-3 py-1 text-[10px] text-text-secondary hover:bg-bg-hover rounded">Cancel</button>
          <button onClick={handleApply} disabled={!selectedFp || !pcbData}
            className={cn("px-3 py-1 text-[10px] rounded",
              selectedFp ? "bg-accent text-white hover:bg-accent/80" : "bg-bg-hover text-text-muted cursor-default")}>
            Generate Fanout
          </button>
        </div>
      </div>
    </div>
  );
}

function Row({ label, value, onChange, step, unit }: {
  label: string; value: number; onChange: (v: number) => void; step: number; unit: string;
}) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-[10px] text-text-muted w-20">{label}</span>
      <input type="number" step={step} min={0.05} value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] outline-none" />
      <span className="text-[9px] text-text-muted">{unit}</span>
    </div>
  );
}
