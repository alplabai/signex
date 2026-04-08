import { useState } from "react";
import { useLibraryEditorStore } from "@/stores/libraryEditor";
import { ChevronDown, ChevronRight, Component, Play, Plus, Trash2, Pencil } from "lucide-react";
import { cn } from "@/lib/utils";
import type { LibSymbol } from "@/types";

/**
 * SCH Library Panel — Altium-style library symbol browser.
 * Shows all symbols in the current library file with multi-part expansion.
 * Provides Place, Add, Delete, Edit actions.
 */
export function SchLibraryPanel() {
  const symbol = useLibraryEditorStore(s => s.symbol);
  const active = useLibraryEditorStore(s => s.active);

  if (!active || !symbol) {
    return (
      <div className="flex flex-col h-full">
        <div className="px-3 py-2 border-b border-border-subtle bg-bg-secondary/80 text-[10px] font-semibold text-text-secondary">
          SCH Library
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
        <span className="text-[10px] font-semibold text-accent uppercase tracking-wider">SCH Library</span>
      </div>

      {/* Column headers */}
      <div className="flex items-center px-2 py-0.5 border-b border-border-subtle bg-bg-secondary/40 text-[9px] font-semibold text-text-muted/50">
        <span className="flex-1">Design Item ID</span>
        <span className="w-[100px] text-right">Description</span>
      </div>

      {/* Symbol list */}
      <div className="flex-1 overflow-y-auto">
        <SymbolTreeItem symbol={symbol} isActive={true} />
      </div>

      {/* Action bar */}
      <div className="flex items-center gap-1 px-2 py-1.5 border-t border-border-subtle bg-bg-secondary/80 shrink-0">
        <ActionButton icon={<Play size={12} />} label="Place" onClick={() => {}} />
        <ActionButton icon={<Plus size={12} />} label="Add" onClick={() => {
          const store = useLibraryEditorStore.getState();
          if (!store.symbol) return;
          // Create a new empty symbol
          const emptySymbol: LibSymbol = {
            id: "NewComponent",
            graphics: [],
            pins: [],
            show_pin_numbers: true,
            show_pin_names: true,
            pin_name_offset: 1.016,
          };
          store.openSymbol(emptySymbol, store.sourcePath || "user_library.snxsym", "NewComponent");
        }} />
        <ActionButton icon={<Trash2 size={12} />} label="Delete" onClick={() => {}} />
        <ActionButton icon={<Pencil size={12} />} label="Edit" onClick={() => {}} />
      </div>
    </div>
  );
}

function SymbolTreeItem({ symbol, isActive }: {
  symbol: LibSymbol;
  isActive: boolean;
}) {
  const [expanded, setExpanded] = useState(true);
  const unitCount = symbol.unit_count ?? 1;
  const hasParts = unitCount > 1;

  return (
    <div>
      <div
        className={cn(
          "flex items-center gap-1.5 px-2 py-[3px] cursor-pointer transition-colors",
          isActive ? "bg-accent/15 text-accent" : "text-text-secondary hover:bg-bg-hover/50"
        )}
        onClick={() => hasParts && setExpanded(!expanded)}
      >
        {hasParts ? (
          expanded ? <ChevronDown size={10} /> : <ChevronRight size={10} />
        ) : (
          <span className="w-[10px]" />
        )}
        <Component size={11} className="shrink-0 text-warning/70" />
        <span className="flex-1 text-[11px] truncate font-medium">{symbol.id}</span>
      </div>

      {hasParts && expanded && (
        <div>
          {Array.from({ length: unitCount }, (_, i) => (
            <div
              key={i}
              className="flex items-center gap-1.5 pl-8 pr-2 py-[2px] text-[10px] text-text-muted hover:bg-bg-hover/50 cursor-pointer"
            >
              <span className="w-[10px]" />
              <span>Part {String.fromCharCode(65 + i)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function ActionButton({ icon, label, onClick }: { icon: React.ReactNode; label: string; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="flex items-center gap-1 px-2 py-1 text-[10px] text-text-secondary hover:text-text-primary hover:bg-bg-hover rounded transition-colors"
    >
      {icon}
      <span>{label}</span>
    </button>
  );
}
