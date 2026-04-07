import { useState, useMemo } from "react";
import { X, Search } from "lucide-react";
import { useSchematicStore } from "@/stores/schematic";
import { cn } from "@/lib/utils";
import type { SchSymbol, SchLabel, SchWire } from "@/types";

interface Props {
  open: boolean;
  onClose: () => void;
}

type ObjectType = "symbol" | "label" | "wire";

interface MatchCriteria {
  sameValue: boolean;
  sameFootprint: boolean;
  sameLibId: boolean;
  sameRotation: boolean;
  sameLabelType: boolean;
  sameLabelText: boolean;
}

export function FindSimilarDialog({ open, onClose }: Props) {
  const [criteria, setCriteria] = useState<MatchCriteria>({
    sameValue: true,
    sameFootprint: false,
    sameLibId: false,
    sameRotation: false,
    sameLabelType: true,
    sameLabelText: false,
  });

  const data = useSchematicStore((s) => s.data);
  const selectedIds = useSchematicStore((s) => s.selectedIds);

  // Determine selected object type
  const selectedObject = useMemo(() => {
    if (!data || selectedIds.size !== 1) return null;
    const uuid = [...selectedIds][0];
    const sym = data.symbols.find((s) => s.uuid === uuid);
    if (sym) return { type: "symbol" as ObjectType, obj: sym };
    const label = data.labels.find((l) => l.uuid === uuid);
    if (label) return { type: "label" as ObjectType, obj: label };
    const wire = data.wires.find((w) => w.uuid === uuid);
    if (wire) return { type: "wire" as ObjectType, obj: wire };
    return null;
  }, [data, selectedIds]);

  // Find matching objects
  const matches = useMemo(() => {
    if (!data || !selectedObject) return [];
    const uuids: string[] = [];

    if (selectedObject.type === "symbol") {
      const ref = selectedObject.obj as SchSymbol;
      for (const sym of data.symbols) {
        if (sym.uuid === ref.uuid) continue;
        if (criteria.sameValue && sym.value !== ref.value) continue;
        if (criteria.sameFootprint && sym.footprint !== ref.footprint) continue;
        if (criteria.sameLibId && sym.lib_id !== ref.lib_id) continue;
        if (criteria.sameRotation && sym.rotation !== ref.rotation) continue;
        uuids.push(sym.uuid);
      }
    } else if (selectedObject.type === "label") {
      const ref = selectedObject.obj as SchLabel;
      for (const label of data.labels) {
        if (label.uuid === ref.uuid) continue;
        if (criteria.sameLabelType && label.label_type !== ref.label_type) continue;
        if (criteria.sameLabelText && label.text !== ref.text) continue;
        uuids.push(label.uuid);
      }
    } else if (selectedObject.type === "wire") {
      // All wires match
      for (const w of data.wires) {
        if (w.uuid !== (selectedObject.obj as SchWire).uuid) uuids.push(w.uuid);
      }
    }

    return uuids;
  }, [data, selectedObject, criteria]);

  if (!open) return null;

  const toggle = (key: keyof MatchCriteria) => {
    setCriteria((c) => ({ ...c, [key]: !c[key] }));
  };

  const handleSelect = () => {
    if (matches.length > 0) {
      const store = useSchematicStore.getState();
      store.selectMultiple([...selectedIds, ...matches]);
    }
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[#1e1e2e] border border-[#45475a] rounded-lg shadow-2xl w-[400px] flex flex-col">
        <div className="flex items-center justify-between px-4 py-3 border-b border-[#45475a]">
          <h2 className="text-sm font-semibold text-[#cdd6f4]">Find Similar Objects</h2>
          <button onClick={onClose} className="p-1 rounded hover:bg-[#313244] text-[#6c7086]"><X size={16} /></button>
        </div>

        <div className="p-4 space-y-3 text-xs">
          {!selectedObject ? (
            <div className="text-center py-4 text-[#6c7086]">
              <Search size={20} className="mx-auto mb-2 opacity-40" />
              <div>Select a single object first</div>
            </div>
          ) : (
            <>
              <div className="text-[#a6adc8] text-[11px]">
                Selected: <span className="text-[#cdd6f4] font-semibold capitalize">{selectedObject.type}</span>
                {selectedObject.type === "symbol" && ` (${(selectedObject.obj as SchSymbol).reference})`}
                {selectedObject.type === "label" && ` "${(selectedObject.obj as SchLabel).text}"`}
              </div>

              <div className="space-y-1.5">
                <span className="text-[#a6adc8]">Match Criteria</span>
                {selectedObject.type === "symbol" && (
                  <div className="space-y-1 ml-1">
                    <Checkbox label={`Same Value (${(selectedObject.obj as SchSymbol).value})`} checked={criteria.sameValue} onChange={() => toggle("sameValue")} />
                    <Checkbox label={`Same Footprint (${(selectedObject.obj as SchSymbol).footprint || "none"})`} checked={criteria.sameFootprint} onChange={() => toggle("sameFootprint")} />
                    <Checkbox label="Same Library ID" checked={criteria.sameLibId} onChange={() => toggle("sameLibId")} />
                    <Checkbox label={`Same Rotation (${(selectedObject.obj as SchSymbol).rotation}°)`} checked={criteria.sameRotation} onChange={() => toggle("sameRotation")} />
                  </div>
                )}
                {selectedObject.type === "label" && (
                  <div className="space-y-1 ml-1">
                    <Checkbox label={`Same Type (${(selectedObject.obj as SchLabel).label_type})`} checked={criteria.sameLabelType} onChange={() => toggle("sameLabelType")} />
                    <Checkbox label={`Same Text ("${(selectedObject.obj as SchLabel).text}")`} checked={criteria.sameLabelText} onChange={() => toggle("sameLabelText")} />
                  </div>
                )}
                {selectedObject.type === "wire" && (
                  <div className="text-[10px] text-[#6c7086] ml-1 italic">All wires will match</div>
                )}
              </div>

              <div className="text-[11px] text-[#a6adc8]">
                <span className={cn("font-mono", matches.length > 0 ? "text-[#a6e3a1]" : "text-[#6c7086]")}>
                  {matches.length}
                </span> matching object{matches.length !== 1 ? "s" : ""} found
              </div>
            </>
          )}
        </div>

        <div className="flex justify-end gap-2 px-4 py-3 border-t border-[#45475a]">
          <button onClick={onClose}
            className="px-4 py-1.5 rounded text-xs bg-[#313244] text-[#a6adc8] hover:bg-[#45475a] transition-colors">
            Cancel
          </button>
          <button onClick={handleSelect}
            className="px-4 py-1.5 rounded text-xs bg-[#89b4fa]/20 text-[#89b4fa] hover:bg-[#89b4fa]/30 transition-colors"
            disabled={!selectedObject || matches.length === 0}>
            Select Matching ({matches.length})
          </button>
        </div>
      </div>
    </div>
  );
}

function Checkbox({ label, checked, onChange }: { label: string; checked: boolean; onChange: () => void }) {
  return (
    <label className="flex items-center gap-1.5 cursor-pointer">
      <input type="checkbox" checked={checked} onChange={onChange} className="accent-[#89b4fa]" />
      <span className="text-[#cdd6f4] text-[11px]">{label}</span>
    </label>
  );
}
