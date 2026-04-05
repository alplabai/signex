import { MousePointer2 } from "lucide-react";

export function PropertiesPanel() {
  return (
    <div className="flex flex-col items-center justify-center h-full text-text-muted text-xs gap-3 p-6">
      <MousePointer2 size={28} className="text-text-muted/20" />
      <span className="text-text-muted/50">No selection</span>
      <span className="text-text-muted/30 text-[11px]">
        Click an object to inspect
      </span>
    </div>
  );
}
