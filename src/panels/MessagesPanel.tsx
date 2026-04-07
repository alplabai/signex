import { useState } from "react";
import { CheckCircle2, AlertTriangle, XCircle, Play, Trash2, Download } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { useEditorStore, type ErcMarker } from "@/stores/editor";
import { runErc, type ErcViolation } from "@/lib/erc";
import { generateErcHtmlReport } from "@/lib/ercReport";
import { cn } from "@/lib/utils";

export function MessagesPanel() {
  const data = useSchematicStore((s) => s.data);
  const [violations, setViolations] = useState<ErcViolation[]>([]);
  const [lastRun, setLastRun] = useState<string | null>(null);

  const handleRunErc = () => {
    if (!data) return;
    const result = runErc(data);
    setViolations(result.violations);
    setLastRun(new Date().toLocaleTimeString());

    // Build ERC markers for canvas display
    const markers: ErcMarker[] = [];
    for (const v of result.violations) {
      if (v.position) {
        markers.push({ position: v.position, severity: v.severity, message: v.message, uuids: v.uuids });
      } else if (v.uuids.length > 0) {
        // Try to find position from the first involved element
        const sym = data.symbols.find(s => s.uuid === v.uuids[0]);
        if (sym) {
          markers.push({ position: sym.position, severity: v.severity, message: v.message, uuids: v.uuids });
        }
      }
    }
    useEditorStore.getState().setErcMarkers(markers);
  };

  const errors = violations.filter(v => v.severity === "error");
  const warnings = violations.filter(v => v.severity === "warning");

  if (!data) {
    return (
      <div className="flex items-center gap-2.5 px-4 py-3 text-xs">
        <CheckCircle2 size={13} className="text-success/60" />
        <span className="text-text-muted">Ready</span>
        <span className="text-text-muted/40">|</span>
        <span className="text-text-muted/40">Open a project to begin designing</span>
      </div>
    );
  }

  return (
    <div className="text-xs h-full flex flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border-subtle shrink-0">
        <button onClick={handleRunErc}
          className="flex items-center gap-1.5 px-2 py-0.5 rounded bg-accent/15 text-accent hover:bg-accent/25 transition-colors text-[11px]">
          <Play size={11} /> Run ERC
        </button>
        <button onClick={() => { setViolations([]); setLastRun(null); useEditorStore.getState().setErcMarkers([]); }}
          className="p-1 rounded text-text-muted/50 hover:text-text-primary hover:bg-bg-hover transition-colors">
          <Trash2 size={12} />
        </button>
        {violations.length > 0 && (
          <button onClick={() => {
            const d = useSchematicStore.getState().data;
            const projectName = d?.title_block?.["Title"] || d?.title_block?.["title"] || "Untitled";
            const html = generateErcHtmlReport(violations, projectName);
            const blob = new Blob([html], { type: "text/html" });
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = `erc-report-${projectName.replace(/\s+/g, "-").toLowerCase()}.html`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);
          }}
            className="flex items-center gap-1.5 px-2 py-0.5 rounded bg-accent/10 text-accent/80 hover:bg-accent/20 hover:text-accent transition-colors text-[11px]">
            <Download size={11} /> Export Report
          </button>
        )}
        <div className="flex-1" />
        {lastRun && (
          <span className="text-text-muted/40 text-[10px]">
            Last run: {lastRun}
          </span>
        )}
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
            <span className="text-text-muted">{lastRun ? "No violations found" : "Click Run ERC to check design"}</span>
          </div>
        ) : (
          violations.map((v, i) => (
            <button key={i}
              className="w-full flex items-start gap-2 px-3 py-1.5 text-left hover:bg-bg-hover/50 transition-colors border-b border-border-subtle/50"
              onClick={() => {
                // Select the involved elements
                if (v.uuids.length > 0) {
                  useSchematicStore.getState().selectMultiple(v.uuids);
                  useEditorStore.getState().setAutoFocus(v.uuids);
                }
              }}>
              {v.severity === "error" ? (
                <XCircle size={12} className="text-error shrink-0 mt-0.5" />
              ) : (
                <AlertTriangle size={12} className="text-warning shrink-0 mt-0.5" />
              )}
              <div className="min-w-0">
                <div className={cn("text-[11px]",
                  v.severity === "error" ? "text-error" : "text-warning"
                )}>
                  {v.message}
                </div>
                <div className="text-[9px] text-text-muted/40 mt-0.5">
                  {v.type.replace(/_/g, " ")}
                </div>
              </div>
            </button>
          ))
        )}
      </div>
    </div>
  );
}
