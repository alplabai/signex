import { useState } from "react";
import { Plus, Trash2, X } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { cn } from "@/lib/utils";

export function NetClassPanel() {
  const data = useSchematicStore((s) => s.data);
  const addNetClass = useSchematicStore((s) => s.addNetClass);
  const removeNetClass = useSchematicStore((s) => s.removeNetClass);
  const assignNetToClass = useSchematicStore((s) => s.assignNetToClass);
  const removeNetFromClass = useSchematicStore((s) => s.removeNetFromClass);
  const [newClassName, setNewClassName] = useState("");
  const [selectedClass, setSelectedClass] = useState<string | null>(null);
  const [assignNet, setAssignNet] = useState("");

  const netClasses = data?.net_classes ?? [];

  const handleAdd = () => {
    const name = newClassName.trim();
    if (!name) return;
    addNetClass(name);
    setNewClassName("");
    setSelectedClass(name);
  };

  const handleAssignNet = () => {
    const net = assignNet.trim();
    if (!net || !selectedClass) return;
    assignNetToClass(net, selectedClass);
    setAssignNet("");
  };

  return (
    <div className="flex flex-col h-full text-xs select-none">
      <div className="flex items-center gap-1 px-2 py-1.5 border-b border-border-subtle bg-bg-secondary/80 shrink-0">
        <span className="text-[10px] font-semibold text-accent uppercase tracking-wider">Net Classes</span>
      </div>

      {/* Add class */}
      <div className="flex items-center gap-1 px-2 py-1 border-b border-border-subtle shrink-0">
        <input
          value={newClassName}
          onChange={(e) => setNewClassName(e.target.value)}
          onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Enter") handleAdd(); }}
          placeholder="New class name..."
          className="flex-1 bg-bg-surface border border-border-subtle rounded px-1.5 py-0.5 text-[10px] outline-none focus:border-accent"
        />
        <button onClick={handleAdd} disabled={!newClassName.trim()}
          className="p-0.5 text-text-muted hover:text-accent disabled:opacity-30">
          <Plus size={12} />
        </button>
      </div>

      {/* Class list */}
      <div className="flex-1 overflow-y-auto">
        {netClasses.length === 0 ? (
          <div className="flex items-center justify-center h-full text-[10px] text-text-muted/40">
            No net classes defined
          </div>
        ) : (
          netClasses.map((nc) => (
            <div key={nc.name}>
              <div
                className={cn(
                  "flex items-center gap-1.5 px-2 py-[3px] cursor-pointer transition-colors",
                  selectedClass === nc.name ? "bg-accent/15 text-accent" : "text-text-secondary hover:bg-bg-hover/50"
                )}
                onClick={() => setSelectedClass(selectedClass === nc.name ? null : nc.name)}
              >
                {nc.color && <div className="w-2.5 h-2.5 rounded-sm" style={{ backgroundColor: nc.color }} />}
                <span className="flex-1 text-[11px] font-medium truncate">{nc.name}</span>
                <span className="text-[9px] text-text-muted">{nc.nets.length} nets</span>
                <button onClick={(e) => { e.stopPropagation(); removeNetClass(nc.name); }}
                  className="p-0.5 text-text-muted/30 hover:text-error">
                  <Trash2 size={10} />
                </button>
              </div>

              {selectedClass === nc.name && (
                <div className="pl-6 pr-2 py-1 bg-bg-surface/30">
                  {nc.nets.length > 0 ? (
                    nc.nets.map((net) => (
                      <div key={net} className="flex items-center gap-1 py-[1px] text-[10px] text-text-muted">
                        <span className="flex-1 font-mono truncate">{net}</span>
                        <button onClick={() => removeNetFromClass(net, nc.name)}
                          className="p-0.5 text-text-muted/20 hover:text-error">
                          <X size={8} />
                        </button>
                      </div>
                    ))
                  ) : (
                    <span className="text-[9px] text-text-muted/30 italic">No nets assigned</span>
                  )}
                  <div className="flex items-center gap-1 mt-1">
                    <input
                      value={assignNet}
                      onChange={(e) => setAssignNet(e.target.value)}
                      onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Enter") handleAssignNet(); }}
                      placeholder="Add net..."
                      className="flex-1 bg-bg-surface border border-border-subtle rounded px-1 py-0.5 text-[9px] outline-none focus:border-accent"
                    />
                    <button onClick={handleAssignNet} disabled={!assignNet.trim()}
                      className="p-0.5 text-text-muted hover:text-accent disabled:opacity-30">
                      <Plus size={10} />
                    </button>
                  </div>
                </div>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
