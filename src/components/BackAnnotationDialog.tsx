import { useState, useMemo } from "react";
import { X, ArrowRight, AlertTriangle, CheckCircle2 } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { usePcbStore } from "@/stores/pcb";
import { detectEcoChanges, applyEcoChanges } from "@/lib/pcbBackAnnotation";
import type { EcoChange } from "@/lib/pcbBackAnnotation";
import { cn } from "@/lib/utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

const TYPE_LABELS: Record<string, string> = {
  reference_changed: "Reference Changed",
  value_changed: "Value Changed",
  footprint_changed: "Footprint Changed",
  component_added: "Added in PCB",
  component_removed: "Removed from PCB",
};

const TYPE_COLORS: Record<string, string> = {
  reference_changed: "text-accent",
  value_changed: "text-warning",
  footprint_changed: "text-info",
  component_added: "text-success",
  component_removed: "text-error",
};

export function BackAnnotationDialog({ open, onClose }: Props) {
  const schData = useSchematicStore((s) => s.data);
  const pcbData = usePcbStore((s) => s.data);
  const [applied, setApplied] = useState(false);

  const changes = useMemo<EcoChange[]>(() => {
    if (!schData || !pcbData) return [];
    return detectEcoChanges(schData, pcbData);
  }, [schData, pcbData]);

  if (!open) return null;

  const handleApply = () => {
    if (!schData || changes.length === 0) return;
    // Clone data to avoid mutating the live store object
    const store = useSchematicStore.getState();
    store.pushUndo();
    const cloned = structuredClone(schData);
    const count = applyEcoChanges(cloned, changes);
    if (count > 0) {
      useSchematicStore.setState({ data: cloned, dirty: true });
    }
    setApplied(true);
  };

  const handleClose = () => {
    setApplied(false);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50">
      <div className="bg-bg-secondary border border-border-subtle rounded-lg shadow-2xl w-[560px] max-h-[80vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-border-subtle">
          <span className="text-xs font-semibold">Back Annotate from PCB</span>
          <button onClick={handleClose} className="p-0.5 hover:bg-bg-hover rounded"><X size={14} /></button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {!schData || !pcbData ? (
            <div className="text-[11px] text-text-muted text-center py-8">
              Both schematic and PCB must be loaded to detect changes.
            </div>
          ) : changes.length === 0 ? (
            <div className="flex flex-col items-center gap-2 py-8">
              <CheckCircle2 size={24} className="text-success" />
              <span className="text-[11px] text-text-muted">Schematic and PCB are in sync. No changes needed.</span>
            </div>
          ) : (
            <>
              <div className="text-[11px] text-text-secondary mb-3">
                {changes.length} change{changes.length !== 1 ? "s" : ""} detected between PCB and schematic:
              </div>
              <table className="w-full text-[10px]">
                <thead>
                  <tr className="text-text-muted border-b border-border-subtle">
                    <th className="text-left py-1 px-2">Reference</th>
                    <th className="text-left py-1 px-2">Change</th>
                    <th className="text-left py-1 px-2">Schematic</th>
                    <th className="text-center py-1 px-2"></th>
                    <th className="text-left py-1 px-2">PCB</th>
                  </tr>
                </thead>
                <tbody>
                  {changes.map((c, i) => (
                    <tr key={i} className="border-b border-border-subtle/30 hover:bg-bg-hover/30">
                      <td className="py-1.5 px-2 font-mono font-medium">{c.reference}</td>
                      <td className={cn("py-1.5 px-2", TYPE_COLORS[c.type])}>
                        {TYPE_LABELS[c.type] || c.type}
                      </td>
                      <td className="py-1.5 px-2 text-text-muted font-mono">{c.oldValue || "-"}</td>
                      <td className="py-1.5 px-2 text-center"><ArrowRight size={10} className="text-text-muted/40" /></td>
                      <td className="py-1.5 px-2 text-text-primary font-mono">{c.newValue || "-"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          )}

          {applied && (
            <div className="flex items-center gap-2 mt-3 p-2 rounded bg-success/10 text-success text-[10px]">
              <CheckCircle2 size={12} />
              Changes applied to schematic.
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-4 py-2.5 border-t border-border-subtle">
          <div className="flex items-center gap-1 text-[10px] text-text-muted">
            <AlertTriangle size={10} />
            Only value and footprint changes are applied automatically.
          </div>
          <div className="flex items-center gap-2">
            <button onClick={handleClose}
              className="px-3 py-1 text-[10px] text-text-secondary hover:bg-bg-hover rounded">
              Close
            </button>
            <button onClick={handleApply}
              disabled={changes.length === 0 || applied || !schData}
              className={cn("px-3 py-1 text-[10px] rounded",
                changes.length > 0 && !applied ? "bg-accent text-white hover:bg-accent/80" : "bg-bg-hover text-text-muted cursor-default")}>
              Apply Changes
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
