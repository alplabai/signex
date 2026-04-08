import { useState } from "react";
import { useFootprintEditorStore } from "@/stores/footprintEditor";
import type { FootprintData } from "@/stores/footprintEditor";
import { ChevronDown, ChevronRight, Box, Play, Plus, Trash2, Pencil } from "lucide-react";
import { cn } from "@/lib/utils";

/**
 * PCB Library Panel — Altium-style footprint browser.
 * Shows all footprints in the current library with pad/primitive counts.
 * Provides Place, Add, Delete, Edit actions.
 */
export function PcbLibraryPanel() {
  const footprint = useFootprintEditorStore(s => s.footprint);
  const active = useFootprintEditorStore(s => s.active);

  if (!active || !footprint) {
    return (
      <div className="flex flex-col h-full">
        <div className="px-3 py-2 border-b border-border-subtle bg-bg-secondary/80 text-[10px] font-semibold text-text-secondary">
          PCB Library
        </div>
        <div className="flex-1 flex items-center justify-center p-4">
          <span className="text-[10px] text-text-muted/40">No library open</span>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full text-xs select-none">
      {/* Header */}
      <div className="flex items-center gap-1 px-2 py-1.5 border-b border-border-subtle bg-bg-secondary/80 shrink-0">
        <span className="text-[10px] font-semibold text-accent uppercase tracking-wider">PCB Library</span>
      </div>

      {/* Column headers */}
      <div className="flex items-center px-2 py-0.5 border-b border-border-subtle bg-bg-secondary/40 text-[9px] font-semibold text-text-muted/50">
        <span className="flex-1">Name</span>
        <span className="w-[50px] text-right">Pads</span>
        <span className="w-[70px] text-right">Primitives</span>
      </div>

      {/* Footprint list */}
      <div className="flex-1 overflow-y-auto">
        <FootprintTreeItem footprint={footprint} isActive={true} />
      </div>

      {/* Action bar */}
      <div className="flex items-center gap-1 px-2 py-1.5 border-t border-border-subtle bg-bg-secondary/80 shrink-0">
        <ActionButton icon={<Play size={12} />} label="Place" onClick={() => {}} />
        <ActionButton icon={<Plus size={12} />} label="Add" onClick={() => {
          const store = useFootprintEditorStore.getState();
          if (!store.footprint) return;
          const emptyFp: FootprintData = {
            id: "NewFootprint",
            pads: [],
            graphics: [],
            courtyard: [],
            model3d: "",
          };
          store.openFootprint(emptyFp, store.sourcePath || "user_library.snxpkg", "NewFootprint");
        }} />
        <ActionButton icon={<Trash2 size={12} />} label="Delete" disabled onClick={() => {}} />
        <ActionButton icon={<Pencil size={12} />} label="Edit" disabled onClick={() => {}} />
      </div>
    </div>
  );
}

function FootprintTreeItem({ footprint, isActive }: {
  footprint: FootprintData;
  isActive: boolean;
}) {
  const [expanded, setExpanded] = useState(true);
  const padCount = footprint.pads.length;
  const graphicCount = footprint.graphics.length;
  const hasPrimitives = graphicCount > 0;

  return (
    <div>
      <div
        className={cn(
          "flex items-center gap-1.5 px-2 py-[3px] cursor-pointer transition-colors",
          isActive ? "bg-accent/15 text-accent" : "text-text-secondary hover:bg-bg-hover/50"
        )}
        onClick={() => hasPrimitives && setExpanded(!expanded)}
      >
        {hasPrimitives ? (
          expanded ? <ChevronDown size={10} /> : <ChevronRight size={10} />
        ) : (
          <span className="w-[10px]" />
        )}
        <Box size={11} className="shrink-0 text-warning/70" />
        <span className="flex-1 text-[11px] truncate font-medium">{footprint.id}</span>
        <span className="w-[50px] text-right text-[10px] text-text-muted">{padCount}</span>
        <span className="w-[70px] text-right text-[10px] text-text-muted">{graphicCount}</span>
      </div>

      {hasPrimitives && expanded && (
        <div>
          {/* Footprint Primitives section */}
          <div className="flex items-center gap-1.5 pl-8 pr-2 py-[2px] text-[10px] text-text-muted/60 font-semibold">
            Primitives
          </div>
          {footprint.graphics.map((g, i) => (
            <div
              key={i}
              className="flex items-center gap-1.5 pl-10 pr-2 py-[2px] text-[10px] text-text-muted hover:bg-bg-hover/50 cursor-pointer"
            >
              <span className="w-[10px]" />
              <span className="capitalize">{g.type}</span>
              <span className="flex-1" />
              <span className="text-text-muted/40">{g.layer}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function ActionButton({ icon, label, onClick, disabled }: { icon: React.ReactNode; label: string; onClick: () => void; disabled?: boolean }) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "flex items-center gap-1 px-2 py-1 text-[10px] rounded transition-colors",
        disabled ? "text-text-muted/30 cursor-default" : "text-text-secondary hover:text-text-primary hover:bg-bg-hover"
      )}
    >
      {icon}
      <span>{label}</span>
    </button>
  );
}
