import { useState } from "react";
import { Play, Trash2, XCircle, AlertTriangle, CheckCircle2 } from "lucide-react";
import { usePcbStore } from "@/stores/pcb";
import { runPcbDrc } from "@/lib/pcbDrc";
import { cn } from "@/lib/utils";
import type { DrcViolation } from "@/types/pcb";

export function DrcPanel() {
  const data = usePcbStore((s) => s.data);
  const [violations, setViolations] = useState<DrcViolation[]>([]);
  const [lastRun, setLastRun] = useState<string | null>(null);

  const handleRunDrc = () => {
    if (!data) return;
    const result = runPcbDrc(data);
    setViolations(result);
    setLastRun(new Date().toLocaleTimeString());
  };

  const errors = violations.filter((v) => v.severity === "error");
  const warnings = violations.filter((v) => v.severity === "warning");

  if (!data) {
    return (
      <div className="flex items-center gap-2.5 px-4 py-3 text-xs">
        <CheckCircle2 size={13} className="text-success/60" />
        <span className="text-text-muted">No PCB loaded</span>
      </div>
    );
  }

  return (
    <div className="text-xs h-full flex flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border-subtle shrink-0">
        <button
          onClick={handleRunDrc}
          className="flex items-center gap-1.5 px-2 py-0.5 rounded bg-accent/15 text-accent hover:bg-accent/25 transition-colors text-[11px]"
        >
          <Play size={11} /> Run DRC
        </button>
        <button
          onClick={() => { setViolations([]); setLastRun(null); }}
          className="p-1 rounded text-text-muted/50 hover:text-text-primary hover:bg-bg-hover transition-colors"
        >
          <Trash2 size={12} />
        </button>
        <div className="flex-1" />
        {lastRun && <span className="text-text-muted/40 text-[10px]">Last: {lastRun}</span>}
        {violations.length > 0 && (
          <div className="flex items-center gap-2">
            {errors.length > 0 && (
              <span className="flex items-center gap-1 text-error text-[10px]">
                <XCircle size={10} /> {errors.length}
              </span>
            )}
            {warnings.length > 0 && (
              <span className="flex items-center gap-1 text-warning text-[10px]">
                <AlertTriangle size={10} /> {warnings.length}
              </span>
            )}
          </div>
        )}
      </div>

      {/* Violations list */}
      <div className="flex-1 overflow-y-auto">
        {violations.length === 0 ? (
          <div className="flex items-center gap-2.5 px-4 py-3">
            <CheckCircle2 size={13} className="text-success/60" />
            <span className="text-text-muted">{lastRun ? "No violations" : "Click Run DRC to check"}</span>
          </div>
        ) : (
          violations.map((v) => (
            <button
              key={v.uuid}
              className="w-full flex items-start gap-2 px-3 py-1.5 text-left hover:bg-bg-hover/50 transition-colors border-b border-border-subtle/50"
              onClick={() => {
                if (v.objects.length > 0) {
                  usePcbStore.getState().selectMultiple(v.objects);
                }
              }}
            >
              {v.severity === "error" ? (
                <XCircle size={12} className="text-error shrink-0 mt-0.5" />
              ) : (
                <AlertTriangle size={12} className="text-warning shrink-0 mt-0.5" />
              )}
              <div className="min-w-0">
                <div className={cn("text-[11px]", v.severity === "error" ? "text-error" : "text-warning")}>
                  {v.message}
                </div>
                <div className="flex items-center gap-2 text-[9px] text-text-muted/40 mt-0.5">
                  <span>{v.type.replace(/_/g, " ")}</span>
                  {v.layer && <span>| {v.layer}</span>}
                  <span>| ({v.position.x.toFixed(1)}, {v.position.y.toFixed(1)})</span>
                </div>
              </div>
            </button>
          ))
        )}
      </div>
    </div>
  );
}
