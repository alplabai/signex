import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";

interface FootprintPickerProps {
  open: boolean;
  initialValue: string;
  onClose: () => void;
  onSelect: (footprintId: string) => void;
}

interface FootprintEntry {
  name: string;
  library: string;
  fullId: string; // "Library:Footprint"
}

export function FootprintPickerDialog({ open, initialValue, onClose, onSelect }: FootprintPickerProps) {
  const [modelName, setModelName] = useState(initialValue);
  const [description, setDescription] = useState("");
  const [libraryMode, setLibraryMode] = useState<"any" | "name" | "path">("any");
  const [libraryFilter, setLibraryFilter] = useState("");
  const [footprints, setFootprints] = useState<FootprintEntry[]>([]);
  const [selectedFp, setSelectedFp] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [foundIn, setFoundIn] = useState("");

  // Load available footprint libraries
  useEffect(() => {
    if (!open) return;
    setModelName(initialValue);
    setSelectedFp(null);
    setDescription(initialValue ? "" : "");
  }, [open, initialValue]);

  // Search footprints when library mode or filter changes
  useEffect(() => {
    if (!open) return;
    setLoading(true);
    loadFootprints().then(fps => {
      setFootprints(fps);
      setLoading(false);
    }).catch(() => { setFootprints([]); setLoading(false); });
  }, [open, libraryMode, libraryFilter]);

  const loadFootprints = async (): Promise<FootprintEntry[]> => {
    try {
      const libs: string[] = await invoke("list_footprint_libraries");
      const results: FootprintEntry[] = [];
      const targetLibs = libraryMode === "name" && libraryFilter
        ? libs.filter(l => l.toLowerCase().includes(libraryFilter.toLowerCase()))
        : libs;

      for (const lib of targetLibs.slice(0, 20)) {
        try {
          const fps: string[] = await invoke("list_footprints_in_library", { libraryName: lib });
          for (const fp of fps) {
            results.push({ name: fp, library: lib, fullId: `${lib}:${fp}` });
          }
        } catch { /* library may fail */ }
      }
      return results;
    } catch {
      return [];
    }
  };

  const handleBrowse = useCallback(() => {
    // Trigger search with current model name
    const match = footprints.find(fp =>
      fp.name.toLowerCase().includes(modelName.toLowerCase()) ||
      fp.fullId.toLowerCase().includes(modelName.toLowerCase())
    );
    if (match) {
      setSelectedFp(match.fullId);
      setFoundIn(match.library);
      setDescription(match.name);
    } else {
      setDescription("Footprint not found");
      setFoundIn("");
    }
  }, [modelName, footprints]);

  const handleSelect = useCallback((fp: FootprintEntry) => {
    setSelectedFp(fp.fullId);
    setModelName(fp.fullId);
    setFoundIn(fp.library);
    setDescription(fp.name);
  }, []);

  const handleOk = useCallback(() => {
    onSelect(selectedFp || modelName);
    onClose();
  }, [selectedFp, modelName, onSelect, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50">
      <div className="bg-bg-primary border border-border rounded-lg shadow-2xl w-[500px] max-h-[600px] flex flex-col">
        {/* Title bar */}
        <div className="flex items-center justify-between px-4 py-2 bg-bg-secondary border-b border-border-subtle rounded-t-lg">
          <span className="text-[12px] font-semibold text-text-primary">PCB Model</span>
          <button onClick={onClose} className="text-text-muted/50 hover:text-text-primary"><X size={14} /></button>
        </div>

        <div className="flex-1 overflow-y-auto p-4 space-y-4 text-[11px]">
          {/* Footprint Model section */}
          <fieldset className="border border-border-subtle rounded p-3">
            <legend className="text-[10px] font-semibold text-text-secondary px-1">Footprint Model</legend>
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <span className="w-[70px] text-text-muted/60 text-right shrink-0">Name</span>
                <input value={modelName} onChange={e => setModelName(e.target.value)}
                  className="flex-1 bg-bg-secondary border border-border-subtle rounded px-2 py-1 text-[11px] text-text-primary outline-none" />
                <button onClick={handleBrowse}
                  className="px-3 py-1 bg-bg-secondary border border-border-subtle rounded text-text-secondary hover:text-text-primary text-[10px]">
                  Browse...
                </button>
                <button className="px-3 py-1 bg-bg-secondary border border-border-subtle rounded text-text-secondary hover:text-text-primary text-[10px]">
                  Pin Map...
                </button>
              </div>
              <div className="flex items-center gap-2">
                <span className="w-[70px] text-text-muted/60 text-right shrink-0">Description</span>
                <div className="flex-1 bg-bg-secondary border border-border-subtle rounded px-2 py-1 text-[11px] text-text-muted/60">
                  {description || "Footprint not found"}
                </div>
              </div>
            </div>
          </fieldset>

          {/* PCB Library section */}
          <fieldset className="border border-border-subtle rounded p-3">
            <legend className="text-[10px] font-semibold text-text-secondary px-1">PCB Library</legend>
            <div className="space-y-1.5">
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="radio" name="libMode" checked={libraryMode === "any"}
                  onChange={() => setLibraryMode("any")} />
                <span className="text-text-secondary">Any</span>
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="radio" name="libMode" checked={libraryMode === "name"}
                  onChange={() => setLibraryMode("name")} />
                <span className="text-text-secondary">Library name</span>
                <input value={libraryFilter} onChange={e => setLibraryFilter(e.target.value)}
                  disabled={libraryMode !== "name"}
                  className="flex-1 bg-bg-secondary border border-border-subtle rounded px-2 py-0.5 text-[10px] text-text-primary outline-none disabled:opacity-40" />
              </label>
              <label className="flex items-center gap-2 cursor-pointer">
                <input type="radio" name="libMode" checked={libraryMode === "path"}
                  onChange={() => setLibraryMode("path")} />
                <span className="text-text-secondary">Library path</span>
                <input disabled className="flex-1 bg-bg-secondary border border-border-subtle rounded px-2 py-0.5 text-[10px] text-text-primary outline-none opacity-40" />
                <button disabled className="px-2 py-0.5 bg-bg-secondary border border-border-subtle rounded text-[10px] text-text-muted/40">
                  Choose...
                </button>
              </label>
            </div>
          </fieldset>

          {/* Selected Footprint section */}
          <fieldset className="border border-border-subtle rounded p-3 min-h-[120px]">
            <legend className="text-[10px] font-semibold text-text-secondary px-1">Selected Footprint</legend>
            {loading ? (
              <div className="text-center text-text-muted/40 py-4">Loading footprints...</div>
            ) : footprints.length === 0 ? (
              <div className="text-center text-text-muted/40 py-4">No footprints found</div>
            ) : (
              <div className="max-h-[150px] overflow-y-auto">
                {footprints.filter(fp =>
                  !modelName || fp.name.toLowerCase().includes(modelName.toLowerCase()) || fp.fullId.toLowerCase().includes(modelName.toLowerCase())
                ).slice(0, 50).map(fp => (
                  <div key={fp.fullId}
                    onClick={() => handleSelect(fp)}
                    className={cn("px-2 py-0.5 cursor-pointer text-[10px] truncate",
                      selectedFp === fp.fullId ? "bg-accent/15 text-accent" : "text-text-secondary hover:bg-bg-hover/50"
                    )}>
                    {fp.fullId}
                  </div>
                ))}
              </div>
            )}
          </fieldset>

          {/* Found in */}
          <div className="text-[10px] text-text-muted/50">
            Found in: <span className="text-text-secondary">{foundIn || "(none)"}</span>
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-border-subtle">
          <button onClick={handleOk}
            className="px-4 py-1.5 bg-accent text-bg-primary rounded text-[11px] font-medium hover:bg-accent/90">
            OK
          </button>
          <button onClick={onClose}
            className="px-4 py-1.5 bg-bg-secondary border border-border-subtle rounded text-[11px] text-text-secondary hover:text-text-primary">
            Cancel
          </button>
        </div>
      </div>
    </div>
  );
}
