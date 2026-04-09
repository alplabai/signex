import { useState, useMemo } from "react";
import { X, Lock } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { cn } from "@/lib/utils";

interface Props {
  open: boolean;
  onClose: () => void;
}

type AnnotationOrder = "down-across" | "up-across" | "across-down" | "across-up";
type AnnotationScope = "all" | "selected" | "unannotated";

export function AnnotationDialog({ open, onClose }: Props) {
  const [order, setOrder] = useState<AnnotationOrder>("down-across");
  const [startIndex, setStartIndex] = useState(1);
  const [scope, setScope] = useState<AnnotationScope>("all");
  const [matchMultiPart, setMatchMultiPart] = useState<"none" | "per_sheet" | "whole">("per_sheet");

  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);

  // Build preview: compute what annotations would look like
  const preview = useMemo(() => {
    if (!data) return [];
    const symbols = data.symbols.filter((s) => !s.is_power);
    const groups = new Map<string, typeof symbols>();
    for (const sym of symbols) {
      const prefix = sym.reference.replace(/[0-9?]+$/, "");
      if (!groups.has(prefix)) groups.set(prefix, []);
      groups.get(prefix)!.push(sym);
    }

    const rows: { uuid: string; current: string; proposed: string; locked: boolean; inScope: boolean }[] = [];

    for (const [prefix, syms] of groups) {
      const usedNumbers = new Set<number>();
      for (const sym of syms) {
        const num = parseInt(sym.reference.replace(/^[A-Z]+/, ""), 10);
        if (!isNaN(num) && !sym.reference.endsWith("?")) usedNumbers.add(num);
      }

      const sorted = [...syms];
      sorted.sort((a, b) => {
        switch (order) {
          case "down-across": { const dy = a.position.y - b.position.y; return Math.abs(dy) > 2 ? dy : a.position.x - b.position.x; }
          case "up-across": { const dy = b.position.y - a.position.y; return Math.abs(dy) > 2 ? dy : a.position.x - b.position.x; }
          case "across-down": { const dx = a.position.x - b.position.x; return Math.abs(dx) > 2 ? dx : a.position.y - b.position.y; }
          case "across-up": { const dx = a.position.x - b.position.x; return Math.abs(dx) > 2 ? dx : b.position.y - a.position.y; }
        }
      });

      let nextNum = startIndex;
      for (const sym of sorted) {
        const needsAnnotation =
          scope === "all" ? true :
          scope === "selected" ? selectedIds.has(sym.uuid) :
          sym.reference.endsWith("?") || sym.reference === prefix;

        const inScope = needsAnnotation && !sym.locked;
        let proposed = sym.reference;
        if (inScope) {
          while (usedNumbers.has(nextNum)) nextNum++;
          proposed = `${prefix}${nextNum}`;
          usedNumbers.add(nextNum);
          nextNum++;
        }
        rows.push({ uuid: sym.uuid, current: sym.reference, proposed, locked: sym.locked, inScope });
      }
    }
    return rows;
  }, [data, order, startIndex, scope, selectedIds]);

  if (!open || !data) return null;

  const handleApply = () => {
    useSchematicStore.getState().annotateWithOptions({ order, startIndex, scope });
    onClose();
  };

  const changes = preview.filter((r) => r.inScope && r.current !== r.proposed);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[#1e1e2e] border border-[#45475a] rounded-lg shadow-2xl w-[500px] max-h-[80vh] flex flex-col">
        <div className="flex items-center justify-between px-4 py-3 border-b border-[#45475a]">
          <h2 className="text-sm font-semibold text-[#cdd6f4]">Annotate Schematics</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-[#313244] text-[#6c7086]"><X size={16} /></button>
        </div>

        <div className="p-4 space-y-3 text-xs">
          {/* Order */}
          <div className="space-y-1">
            <span className="text-[#a6adc8]">Processing Order</span>
            <select value={order} onChange={(e) => setOrder(e.target.value as AnnotationOrder)}
              className="w-full bg-[#313244] border border-[#45475a] rounded px-2 py-1 text-[11px] text-[#cdd6f4] outline-none focus:border-[#89b4fa]">
              <option value="down-across">Down Then Across</option>
              <option value="up-across">Up Then Across</option>
              <option value="across-down">Across Then Down</option>
              <option value="across-up">Across Then Up</option>
            </select>
          </div>

          {/* Starting index */}
          <div className="flex items-center gap-3">
            <span className="text-[#a6adc8]">Starting Index</span>
            <input type="number" value={startIndex} min={1} onChange={(e) => setStartIndex(Math.max(1, parseInt(e.target.value) || 1))}
              className="w-16 bg-[#313244] border border-[#45475a] rounded px-2 py-1 text-[11px] font-mono text-[#cdd6f4] outline-none focus:border-[#89b4fa]" />
          </div>

          {/* Scope */}
          <div className="space-y-1">
            <span className="text-[#a6adc8]">Scope</span>
            <div className="flex gap-3 ml-1">
              {([["all", "All Components"], ["selected", "Selected Only"], ["unannotated", "Unannotated Only"]] as const).map(([v, label]) => (
                <label key={v} className="flex items-center gap-1.5 cursor-pointer">
                  <input type="radio" name="scope" checked={scope === v} onChange={() => setScope(v)} className="accent-[#89b4fa]" />
                  <span className="text-[#cdd6f4] text-[11px]">{label}</span>
                </label>
              ))}
            </div>
          </div>

          {/* Multi-part matching */}
          <div className="space-y-1">
            <span className="text-[#a6adc8]">Multi-Part Components</span>
            <select value={matchMultiPart} onChange={(e) => setMatchMultiPart(e.target.value as "none" | "per_sheet" | "whole")}
              className="w-full bg-[#313244] border border-[#45475a] rounded px-2 py-1 text-[11px] text-[#cdd6f4] outline-none focus:border-[#89b4fa]">
              <option value="none">None (independent parts)</option>
              <option value="per_sheet">Complete Existing Packages (Per Sheet)</option>
              <option value="whole">Complete Existing Packages (Whole Project)</option>
            </select>
          </div>

          {/* Preview table */}
          <div className="border border-[#45475a] rounded overflow-hidden max-h-[200px] overflow-y-auto">
            <div className="flex bg-[#313244] text-[9px] text-[#a6adc8]/60 uppercase tracking-wider sticky top-0">
              <div className="w-8 px-1 py-0.5" />
              <div className="flex-1 px-2 py-0.5">Current</div>
              <div className="flex-1 px-2 py-0.5">Proposed</div>
            </div>
            {preview.map((row) => (
              <div key={row.uuid} className={cn(
                "flex border-t border-[#45475a]/50 text-[11px]",
                row.locked ? "text-[#6c7086]" : row.inScope && row.current !== row.proposed ? "text-[#a6e3a1]" : "text-[#cdd6f4]"
              )}>
                <div className="w-8 px-1 py-0.5 flex items-center justify-center">
                  {row.locked && <Lock size={10} className="text-[#6c7086]" />}
                </div>
                <div className="flex-1 px-2 py-0.5 font-mono">{row.current}</div>
                <div className="flex-1 px-2 py-0.5 font-mono">{row.proposed}</div>
              </div>
            ))}
          </div>
          <div className="text-[10px] text-[#a6adc8]/50">
            {changes.length} component{changes.length !== 1 ? "s" : ""} will be re-annotated
          </div>
        </div>

        <div className="flex justify-end gap-2 px-4 py-3 border-t border-[#45475a]">
          <button onClick={onClose}
            className="px-4 py-1.5 rounded text-xs bg-[#313244] text-[#a6adc8] hover:bg-[#45475a] transition-colors">
            Cancel
          </button>
          <button onClick={handleApply}
            className="px-4 py-1.5 rounded text-xs bg-[#89b4fa]/20 text-[#89b4fa] hover:bg-[#89b4fa]/30 transition-colors"
            disabled={changes.length === 0}>
            Apply ({changes.length})
          </button>
        </div>
      </div>
    </div>
  );
}
