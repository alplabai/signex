import { useState } from "react";
import { Plus, Trash2, Settings2 } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { cn } from "@/lib/utils";

export function VariantPanel() {
  const data = useSchematicStore((s) => s.data);
  const [selectedVariant, setSelectedVariant] = useState<string | null>(null);
  const [newName, setNewName] = useState("");
  const [showAdd, setShowAdd] = useState(false);

  if (!data) return <div className="p-4 text-xs text-text-muted/50">No schematic loaded</div>;

  const variants = data.variants || [];

  const handleAdd = () => {
    const name = newName.trim();
    if (!name) return;
    useSchematicStore.getState().addVariant(name);
    setNewName("");
    setShowAdd(false);
    setSelectedVariant(name);
  };

  const handleRemove = (name: string) => {
    useSchematicStore.getState().removeVariant(name);
    if (selectedVariant === name) setSelectedVariant(null);
  };

  const selectedVarData = variants.find((v) => v.name === selectedVariant);

  return (
    <div className="text-xs h-full flex flex-col">
      <div className="flex items-center justify-between px-3 py-2 border-b border-border-subtle shrink-0">
        <span className="text-[11px] font-semibold text-text-secondary">Design Variants</span>
        <button onClick={() => setShowAdd(!showAdd)}
          className="text-accent hover:text-accent/80 transition-colors">
          <Plus size={14} />
        </button>
      </div>

      {showAdd && (
        <div className="flex items-center gap-1 px-3 py-2 border-b border-border-subtle">
          <input value={newName} onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => { e.stopPropagation(); if (e.key === "Enter") handleAdd(); }}
            placeholder="Variant name..."
            className="flex-1 bg-bg-surface border border-border-subtle rounded px-2 py-0.5 text-[10px] outline-none focus:border-accent" />
          <button onClick={handleAdd} className="text-[10px] text-accent px-2 py-0.5 rounded bg-accent/20 hover:bg-accent/30">Add</button>
        </div>
      )}

      <div className="flex-1 overflow-y-auto">
        {variants.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-text-muted/30 gap-2 p-4">
            <Settings2 size={20} />
            <span className="text-[11px]">No variants defined</span>
            <span className="text-[10px]">Create variants for different assembly configurations</span>
          </div>
        ) : (
          variants.map((variant) => (
            <div key={variant.name}
              className={cn("border-b border-border-subtle/30",
                selectedVariant === variant.name && "bg-accent/5")}>
              <div className="flex items-center gap-2 px-3 py-1.5 cursor-pointer hover:bg-bg-hover/50"
                onClick={() => setSelectedVariant(selectedVariant === variant.name ? null : variant.name)}>
                <span className="flex-1 text-[11px] text-text-primary">{variant.name}</span>
                <span className="text-[9px] text-text-muted/40">
                  {Object.keys(variant.components).length} overrides
                </span>
                <button onClick={(e) => { e.stopPropagation(); handleRemove(variant.name); }}
                  className="p-0.5 rounded text-text-muted/30 hover:text-error transition-colors">
                  <Trash2 size={11} />
                </button>
              </div>

              {selectedVariant === variant.name && selectedVarData && (
                <div className="px-3 pb-2 space-y-1">
                  {data.symbols.filter((s) => !s.is_power).map((sym) => {
                    const override = selectedVarData.components[sym.uuid];
                    const fitted = override?.fitted ?? true;
                    return (
                      <div key={sym.uuid} className="flex items-center gap-2 text-[10px]">
                        <input type="checkbox" checked={fitted}
                          onChange={(e) => {
                            useSchematicStore.getState().setVariantComponent(
                              variant.name, sym.uuid, e.target.checked,
                              override?.altValue, override?.altFootprint
                            );
                          }}
                          className="accent-[#89b4fa]" />
                        <span className={cn("font-mono", !fitted && "text-text-muted/30 line-through")}>
                          {sym.reference}
                        </span>
                        <span className={cn("text-text-muted/60", !fitted && "text-text-muted/20")}>
                          {override?.altValue || sym.value}
                        </span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          ))
        )}
      </div>
    </div>
  );
}
