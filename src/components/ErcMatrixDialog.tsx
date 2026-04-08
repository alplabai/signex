import { X } from "lucide-react";
import { checkPinConnection } from "@/lib/ercMatrix";
import type { PinType, ErcSeverity } from "@/lib/ercMatrix";
import { cn } from "@/lib/utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

const PIN_TYPES: PinType[] = [
  "input", "output", "bidirectional", "passive", "tri_state",
  "open_collector", "open_emitter", "power_in", "power_out",
  "unconnected", "free", "unspecified",
];

const PIN_LABELS: Record<string, string> = {
  input: "Input",
  output: "Output",
  bidirectional: "Bidir",
  passive: "Passive",
  tri_state: "Tri-St",
  open_collector: "Open C",
  open_emitter: "Open E",
  power_in: "Pwr In",
  power_out: "Pwr Out",
  unconnected: "Uncon",
  free: "Free",
  unspecified: "Unspec",
};

const SEVERITY_COLORS: Record<ErcSeverity, string> = {
  ok: "bg-success/20 text-success",
  warning: "bg-warning/20 text-warning",
  error: "bg-error/20 text-error",
};

const SEVERITY_LABELS: Record<ErcSeverity, string> = {
  ok: "",
  warning: "W",
  error: "E",
};

export function ErcMatrixDialog({ open, onClose }: Props) {
  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50">
      <div className="bg-bg-secondary border border-border-subtle rounded-lg shadow-2xl max-w-[680px] max-h-[80vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-border-subtle">
          <span className="text-xs font-semibold">ERC Pin Connection Matrix</span>
          <button onClick={onClose} className="p-0.5 hover:bg-bg-hover rounded"><X size={14} /></button>
        </div>

        {/* Matrix */}
        <div className="flex-1 overflow-auto p-3">
          <div className="text-[10px] text-text-muted mb-2">
            Each cell shows the ERC severity when two pin types connect on the same net.
          </div>
          <table className="text-[9px] border-collapse">
            <thead>
              <tr>
                <th className="p-1 text-left text-text-muted border-b border-border-subtle sticky left-0 bg-bg-secondary z-10"></th>
                {PIN_TYPES.map((t) => (
                  <th key={t} className="p-1 text-center text-text-muted border-b border-border-subtle min-w-[38px] font-medium">
                    <span className="writing-mode-vertical" style={{ writingMode: "vertical-rl", transform: "rotate(180deg)" }}>
                      {PIN_LABELS[t]}
                    </span>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {PIN_TYPES.map((row) => (
                <tr key={row}>
                  <td className="p-1 text-right text-text-muted font-medium pr-2 border-r border-border-subtle sticky left-0 bg-bg-secondary z-10 whitespace-nowrap">
                    {PIN_LABELS[row]}
                  </td>
                  {PIN_TYPES.map((col) => {
                    const severity = checkPinConnection(row, col);
                    return (
                      <td key={col}
                        className={cn("p-1 text-center border border-border-subtle/20 min-w-[38px]", SEVERITY_COLORS[severity])}
                        title={`${PIN_LABELS[row]} + ${PIN_LABELS[col]}: ${severity}`}>
                        {SEVERITY_LABELS[severity]}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>

          {/* Legend */}
          <div className="flex items-center gap-4 mt-3 text-[9px] text-text-muted">
            <div className="flex items-center gap-1">
              <div className="w-3 h-3 rounded bg-success/20 border border-success/30" />
              <span>OK</span>
            </div>
            <div className="flex items-center gap-1">
              <div className="w-3 h-3 rounded bg-warning/20 border border-warning/30 text-center text-[8px] text-warning font-bold leading-[12px]">W</div>
              <span>Warning</span>
            </div>
            <div className="flex items-center gap-1">
              <div className="w-3 h-3 rounded bg-error/20 border border-error/30 text-center text-[8px] text-error font-bold leading-[12px]">E</div>
              <span>Error</span>
            </div>
          </div>
        </div>

        {/* Footer */}
        <div className="flex justify-end px-4 py-2.5 border-t border-border-subtle">
          <button onClick={onClose}
            className="px-3 py-1 text-[10px] text-text-secondary hover:bg-bg-hover rounded">
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
