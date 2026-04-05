import { Settings } from "lucide-react";

export function PropertiesPanel() {
  return (
    <div className="flex flex-col items-center justify-center h-full text-text-muted text-xs gap-3 p-4">
      <Settings size={32} className="opacity-30" />
      <span>No selection</span>
      <span className="text-text-muted/60">
        Select an object to view properties
      </span>
    </div>
  );
}
