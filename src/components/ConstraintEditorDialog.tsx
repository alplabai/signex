import { useState } from "react";
import { X, Plus, Trash2 } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { cn } from "@/lib/utils";
import type { DesignConstraint } from "@/types";

interface Props {
  open: boolean;
  onClose: () => void;
}

const CONSTRAINT_TYPES: DesignConstraint["type"][] = [
  "clearance", "trace_width", "via_size", "diff_pair_gap", "length_match", "custom",
];

const TYPE_LABELS: Record<string, string> = {
  clearance: "Clearance",
  trace_width: "Trace Width",
  via_size: "Via Size",
  diff_pair_gap: "Diff Pair Gap",
  length_match: "Length Match",
  custom: "Custom",
};

export function ConstraintEditorDialog({ open, onClose }: Props) {
  const data = useSchematicStore((s) => s.data);
  const addConstraint = useSchematicStore((s) => s.addConstraint);
  const removeConstraint = useSchematicStore((s) => s.removeConstraint);
  const updateConstraintEnabled = useSchematicStore((s) => s.updateConstraintEnabled);
  const [newType, setNewType] = useState<DesignConstraint["type"]>("clearance");

  if (!open) return null;

  const constraints = data?.constraints ?? [];

  const handleAdd = () => {
    const value = newType === "clearance" ? 0.2 : newType === "trace_width" ? 0.25 : 0.4;
    addConstraint(`${TYPE_LABELS[newType]} Rule`, newType, "all", value, "mm");
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50" role="dialog" aria-modal="true" aria-label="Design Constraints">
      <div className="bg-bg-secondary border border-border-subtle rounded-lg shadow-2xl w-[600px] max-h-[80vh] flex flex-col">
        <div className="flex items-center justify-between px-4 py-2.5 border-b border-border-subtle">
          <span className="text-xs font-semibold">Design Constraints</span>
          <button onClick={onClose} className="p-0.5 hover:bg-bg-hover rounded"><X size={14} /></button>
        </div>

        {/* Add bar */}
        <div className="flex items-center gap-2 px-4 py-2 border-b border-border-subtle">
          <select value={newType} onChange={(e) => setNewType(e.target.value as DesignConstraint["type"])}
            className="bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] outline-none">
            {CONSTRAINT_TYPES.map((t) => <option key={t} value={t}>{TYPE_LABELS[t]}</option>)}
          </select>
          <button onClick={handleAdd}
            className="flex items-center gap-1 px-2 py-0.5 rounded bg-accent/15 text-accent hover:bg-accent/25 text-[10px]">
            <Plus size={10} /> Add Rule
          </button>
          <div className="flex-1" />
          <span className="text-[9px] text-text-muted">{constraints.length} rules</span>
        </div>

        {/* Table */}
        <div className="flex-1 overflow-y-auto">
          {constraints.length === 0 ? (
            <div className="flex items-center justify-center py-12 text-[10px] text-text-muted/40">
              No design constraints defined. Add a rule above.
            </div>
          ) : (
            <table className="w-full text-[10px]">
              <thead>
                <tr className="text-text-muted border-b border-border-subtle bg-bg-secondary/40">
                  <th className="text-left py-1 px-3 w-8"></th>
                  <th className="text-left py-1 px-2">Name</th>
                  <th className="text-left py-1 px-2">Type</th>
                  <th className="text-left py-1 px-2">Scope</th>
                  <th className="text-right py-1 px-2">Value</th>
                  <th className="text-right py-1 px-2">Unit</th>
                  <th className="text-right py-1 px-2">Priority</th>
                  <th className="text-center py-1 px-2 w-8"></th>
                </tr>
              </thead>
              <tbody>
                {constraints.map((c) => (
                  <tr key={c.uuid} className={cn("border-b border-border-subtle/30 hover:bg-bg-hover/30", !c.enabled && "opacity-40")}>
                    <td className="py-1 px-3">
                      <input type="checkbox" checked={c.enabled}
                        onChange={() => updateConstraintEnabled(c.uuid, !c.enabled)}
                        className="accent-[#89b4fa]" />
                    </td>
                    <td className="py-1 px-2 font-medium">{c.name}</td>
                    <td className="py-1 px-2 text-text-muted">{TYPE_LABELS[c.type] || c.type}</td>
                    <td className="py-1 px-2 text-text-muted capitalize">{c.scope.kind}</td>
                    <td className="py-1 px-2 text-right font-mono text-accent">{c.value.toFixed(2)}</td>
                    <td className="py-1 px-2 text-right font-mono text-text-muted">{c.unit}</td>
                    <td className="py-1 px-2 text-right font-mono text-text-muted">{c.priority}</td>
                    <td className="py-1 px-2 text-center">
                      <button onClick={() => removeConstraint(c.uuid)}
                        className="p-0.5 text-text-muted/30 hover:text-error">
                        <Trash2 size={10} />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        <div className="flex justify-end px-4 py-2.5 border-t border-border-subtle">
          <button onClick={onClose}
            className="px-3 py-1 text-[10px] bg-accent text-white rounded hover:bg-accent/80">
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
